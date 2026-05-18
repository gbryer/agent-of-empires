<!-- refreshed: 2026-05-15 -->
# Architecture

**Analysis Date:** 2026-05-15

## System Overview

```text
┌────────────────────────────────────────────────────────────────────┐
│                        CLI Entry Point                             │
│                       `src/main.rs`                                │
│   clap parse → migrations → dispatch to TUI or CLI subcommand     │
└───────────┬──────────────────┬─────────────────┬──────────────────┘
            │                  │                  │
            ▼                  ▼                  ▼
┌───────────────┐  ┌─────────────────┐  ┌──────────────────────────┐
│  TUI Mode     │  │  CLI Subcommands│  │  `aoe serve` (serve feat) │
│ `src/tui/`    │  │  `src/cli/`     │  │  `src/server/`            │
│  ratatui +    │  │  add, list,     │  │  axum REST + WS + PTY     │
│  crossterm    │  │  remove, send,  │  │  relay; web/dist/ embed   │
│  HomeView,    │  │  status, group, │  │  `web/` React frontend    │
│  CockpitView  │  │  worktree, ...  │  │                           │
└───────┬───────┘  └───────┬─────────┘  └────────────┬─────────────┘
        │                  │                           │
        └──────────────────┴───────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────────────────┐
│                     Session Layer                                  │
│                    `src/session/`                                  │
│  Storage (sessions.json) · Config · Instance · ProfileConfig      │
│  builder · poller · groups · projects · repo_config               │
└───────────┬──────────────────────┬────────────────────────────────┘
            │                      │
            ▼                      ▼
┌───────────────────┐  ┌──────────────────────────────────────────┐
│   tmux Layer      │  │  Cockpit / ACP Layer  (`src/cockpit/`)   │
│  `src/tmux/`      │  │  AcpClient · Supervisor · Runner shim    │
│  Session · tmux   │  │  state (single-writer actor) · protocol  │
│  cmd wrappers     │  │  fs_handler · terminal_handler           │
└──────────┬────────┘  └──────────────────┬───────────────────────┘
           │                               │
           ▼                               ▼
┌───────────────────┐  ┌──────────────────────────────────────────┐
│  OS Process Layer │  │  Container Layer  (`src/containers/`)    │
│  `src/process/`   │  │  Docker / Podman / apple-container       │
│  macos.rs         │  │  ContainerRuntime · ContainerInterface   │
│  linux.rs         │  └──────────────────────────────────────────┘
└───────────────────┘
```

## Component Responsibilities

| Component | Responsibility | Key File(s) |
|-----------|----------------|-------------|
| `src/main.rs` | Binary entrypoint: parse CLI, init logging, run migrations, dispatch | `src/main.rs` |
| `src/cli/` | One file per subcommand; clap `Args` structs and `run()` functions | `src/cli/definition.rs`, `src/cli/add.rs` |
| `src/tui/` | ratatui TUI: `App` event loop, `HomeView`, dialogs, settings, cockpit native view | `src/tui/app.rs`, `src/tui/home/mod.rs` |
| `src/session/` | Session persistence, config resolution, instance lifecycle, builder | `src/session/storage.rs`, `src/session/instance.rs`, `src/session/config.rs` |
| `src/tmux/` | tmux subprocess wrappers, session/pane management, status detection | `src/tmux/mod.rs`, `src/tmux/status_detection.rs` |
| `src/cockpit/` | ACP-based native agent rendering: client, supervisor, runner shim, state | `src/cockpit/supervisor.rs`, `src/cockpit/acp_client.rs`, `src/cockpit/runner.rs` |
| `src/server/` | axum HTTP server (serve feature): REST API, WS PTY relay, auth, push | `src/server/mod.rs`, `src/server/api/sessions.rs`, `src/server/ws.rs` |
| `src/containers/` | Docker/Podman/apple-container abstraction for sandboxed sessions | `src/containers/runtime.rs` |
| `src/git/` | git worktree lifecycle, branch operations, template resolution | `src/git/worktree.rs` |
| `src/process/` | OS-specific process tree kill/stop/continue via nix signals | `src/process/macos.rs`, `src/process/linux.rs` |
| `src/hooks/` | Agent hook installation into settings.json; sidecar status-file writes | `src/hooks/mod.rs` |
| `src/migrations/` | Versioned one-time data migrations gated by `.schema_version` | `src/migrations/mod.rs` |
| `src/update/` | GitHub release version check | `src/update/` |
| `web/` | React 19 + TypeScript frontend (Vite + Tailwind v4 + xterm.js v6) | `web/src/App.tsx`, `web/src/components/`, `web/src/hooks/` |
| `xtask/` | Cargo workspace build tasks: `gen-docs`, `check-skill` | `xtask/src/main.rs` |

## Pattern Overview

**Overall:** Layered CLI/TUI application with an optional feature-gated web server. Two distinct session backends: tmux-backed PTY sessions and ACP cockpit sessions (agent subprocess over stdio). All state is owned by the session layer; TUI and server layers are consumers.

**Key Characteristics:**
- Feature gate `serve` controls compilation of `src/server/`, `src/cockpit/`, and the web frontend embed; TUI-only builds require no Node.js
- Debug builds use isolated namespaces (`aoe_dev_` tmux prefix, `agent-of-empires-dev` app dir) so `cargo run` never collides with an installed release `aoe`
- Migrations run at startup via `migrations::run_migrations()` gated by `.schema_version` in the app dir
- Settings are two-layer: global `config.toml` (`Config`) + per-profile overrides (`ProfileConfig`); `merge_configs()` resolves the final effective config
- Cockpit sessions have a separate process lifetime: the `aoe __cockpit-runner` shim process outlives `aoe serve`, bridging a stdio ACP agent over a Unix socket

## Layers

**CLI/Dispatch Layer:**
- Purpose: Parse user input via clap, init tracing, dispatch to TUI or subcommand handler
- Location: `src/main.rs`, `src/cli/`
- Contains: `Cli`, `Commands` enum, one `run()` per subcommand
- Depends on: session, tmux, git, containers, cockpit (serve feature)
- Used by: End user; nothing calls into `src/main.rs`

**TUI Layer:**
- Purpose: Interactive ratatui terminal dashboard. Two home views: local `HomeView` and remote `remote_home` (when `AOE_DAEMON_URL` is set). CockpitView for ACP sessions.
- Location: `src/tui/`
- Contains: `App` (event loop), `HomeView` (session list, input, render), dialogs, settings, `cockpit_view/`, `remote_home/`
- Depends on: session, tmux, cockpit client (serve feature)
- Used by: `src/main.rs` when no subcommand is given

**Session Layer:**
- Purpose: Single source of truth for session state. Persistence to JSON. Config resolution.
- Location: `src/session/`
- Contains: `Storage` (reads/writes `sessions.json`, `groups.json`), `Instance`, `Config`, `ProfileConfig`, `builder`, `poller`
- Depends on: tmux, containers, git, hooks, process
- Used by: cli, tui, server

**tmux Layer:**
- Purpose: Manage tmux sessions/panes for each agent session
- Location: `src/tmux/`
- Contains: `Session` (start/stop/attach tmux session), `TerminalSession`, status detection, env helpers
- Depends on: OS `tmux` binary (must be installed)
- Used by: session/instance, server/ws

**Cockpit / ACP Layer:**
- Purpose: Native ACP agent rendering independent of tmux. `aoe` is the ACP **client**; agent subprocesses are ACP **servers**.
- Location: `src/cockpit/`
- Contains: `AcpClient` (stdio transport), `Supervisor` (per-session watchdog + respawn), `runner` (shim process that outlives daemon), `CockpitState` (single-writer actor), `fs_handler`, `terminal_handler`, `agent_registry`, `approvals`, `protocol`
- Depends on: `agent-client-protocol` crate, tokio, Unix sockets
- Used by: server/cockpit_reconciler, server/cockpit_ws, tui/cockpit_view

**Server Layer (serve feature):**
- Purpose: axum HTTP/WebSocket server for the web dashboard. REST CRUD for sessions, PTY relay over WS, push notifications, cockpit WS bridge.
- Location: `src/server/`
- Contains: `AppState` (Arc<RwLock<>> session list), REST handlers in `api/`, `ws.rs` (PTY relay via `portable-pty`), `auth.rs`, `push.rs`, `tunnel.rs`, `cockpit_reconciler.rs`, `cockpit_ws.rs`
- Depends on: session, tmux, cockpit, process
- Used by: `src/cli/serve.rs`

**Container Layer:**
- Purpose: Abstraction over Docker, Podman, and apple-container for sandboxed sessions
- Location: `src/containers/`
- Contains: `ContainerRuntime`, `ContainerRuntimeInterface` trait, `DockerContainer`
- Depends on: container runtime binary on PATH
- Used by: session/instance, session/builder, cli/add

**OS Process Layer:**
- Purpose: Platform-specific process tree operations (kill, stop, continue)
- Location: `src/process/`
- Contains: `macos.rs` (sysctl-based pid tree), `linux.rs` (procfs-based pid tree), shared signal logic in `mod.rs`
- Depends on: `nix` crate, Linux/macOS only
- Used by: server/ws (SIGSTOP/SIGCONT for scrollback pause), session deletion

## Data Flow

### Primary: Session Create (CLI path)

1. User runs `aoe add .` → `src/main.rs` dispatches to `cli::add::run()` (`src/cli/add.rs`)
2. `add::run()` calls `session::builder::build_instance()` (`src/session/builder.rs`) with `InstanceParams`
3. Builder: resolves config via `merge_configs()` (`src/session/profile_config.rs`), optionally creates git worktree (`src/git/worktree.rs`), optionally starts container (`src/containers/`)
4. Returns `BuildResult { instance, created_worktree, ... }` — `instance` written to `Storage::save()` (`src/session/storage.rs` → `sessions.json`)
5. If `--launch`: calls `instance.start()` → `tmux::Session::new().start()` (`src/tmux/`) launching agent CLI in a tmux pane

### Primary: TUI Session Monitoring

1. `tui::run()` → `App::new()` → spawns `StatusPoller` (`src/tui/status_poller.rs`) and `CreationPoller`
2. `StatusPoller` reads `sessions.json` via `Storage::load()`, polls tmux pane status via `tmux::status_detection::detect_status_from_content()` and hook sidecar files in `/tmp/aoe-hooks/<id>/status`
3. Changes sent via `mpsc` channel to `HomeView` → triggers ratatui re-render

### Primary: Web Dashboard (serve feature)

1. Browser → `GET /api/sessions` → `server::api::sessions::list_sessions()` (`src/server/api/sessions.rs`) — reads `AppState.instances` (Arc<RwLock<Vec<Instance>>>)
2. Browser → `WS /sessions/{id}/ws` → `server::ws::session_ws()` (`src/server/ws.rs`) — spawns `tmux attach-session` inside a PTY via `portable-pty`, relays raw bytes bidirectionally
3. Browser → `POST /api/sessions/{id}/send` → `send_message()` → calls `instance.ensure_pane_ready()` then writes to tmux pane
4. Status changes pushed to browser via `broadcast::Sender<StatusChange>` in `server::push` (`src/server/push.rs`)

### Primary: Cockpit Session (ACP path)

1. `aoe serve` starts `Supervisor` (`src/cockpit/supervisor.rs`)
2. `POST /api/sessions/{id}/cockpit/spawn` → `Supervisor::spawn()` → `aoe __cockpit-runner` shim detached via `setsid` (`src/cockpit/runner.rs`)
3. Runner spawns ACP agent subprocess over stdio; binds Unix socket at `<app_dir>/cockpit-workers/<id>.sock`
4. Supervisor connects to socket; ACP events flow: agent → AcpClient → `state::apply_event()` → `CockpitState`
5. State changes broadcast via `cockpit_events_tx` → `cockpit_ws.rs` → browser WebSocket

### Cross-machine (remote TUI)

1. `AOE_DAEMON_URL` set or `--daemon-url` passed → `tui::run()` detects via `cockpit::client::discovery::discover_env()` (`src/cockpit/client/discovery.rs`)
2. Swaps `HomeView` for `remote_home::run_standalone()` (`src/tui/remote_home/`) — fetches session list from remote daemon via HTTP, opens `CockpitView` on selection

**State Management:**
- Session list: JSON file (`sessions.json`) per profile, loaded on each TUI tick or API call via `Storage`
- TUI state: in-memory `HomeView` struct — not persisted
- Cockpit state: `CockpitState` behind single-writer actor (`state::apply_event`) — in-memory, no persistence beyond event replay buffer in runner
- Config: `config.toml` (global) + per-profile `<profile>/config.toml`, merged at call time by `resolve_config()`

## Key Abstractions

**`Instance`:**
- Purpose: One agent session (id, title, path, tool, status, sandbox/worktree metadata)
- Examples: `src/session/instance.rs`
- Pattern: Plain struct, serialized to JSON by `Storage`

**`Storage`:**
- Purpose: Profile-scoped reader/writer for `sessions.json` and `groups.json`
- Examples: `src/session/storage.rs`
- Pattern: Struct with a `profile` field; `load()` / `save()` / `load_with_groups()`; writes backup `.json.bak` before each save

**`Config` / `ProfileConfig` / `merge_configs()`:**
- Purpose: Two-layer config — global `Config` in `config.toml` + per-profile `ProfileConfig` with `Option<T>` overrides
- Examples: `src/session/config.rs`, `src/session/profile_config.rs`
- Pattern: Every new config field needs entries in both `Config` and `*ConfigOverride`, plus `FieldKey` in `src/tui/settings/fields.rs`, wired through `apply_field_to_global()` / `apply_field_to_profile()` / `merge_configs()`

**`SessionPoller` / `AdaptiveInterval`:**
- Purpose: Background thread per session; polls tmux pane status with exponential backoff
- Examples: `src/session/poller.rs`
- Pattern: Cap of 20 concurrent pollers via `ACTIVE_POLLER_COUNT` atomic; backs off from 2s to 60s when status is stable

**`AcpClient` / `Supervisor`:**
- Purpose: ACP protocol client wrapping an agent subprocess; supervisor adds watchdog respawn logic
- Examples: `src/cockpit/acp_client.rs`, `src/cockpit/supervisor.rs`
- Pattern: mpsc command channel → ACP requests; `session/update` notifications → `Event` → `state::apply_event()`; max 3 respawns per 60s window before parking

**`AppState` (server):**
- Purpose: Shared server state behind `Arc<RwLock<>>`
- Examples: `src/server/mod.rs`
- Pattern: `Arc<AppState>` injected via axum `State` extractor; contains session list, token manager, broadcast channels, supervisor handle

## Entry Points

**TUI (default):**
- Location: `src/main.rs` → `tui::run()` → `tui::app::App`
- Triggers: `aoe` with no subcommand
- Responsibilities: Run ratatui event loop; dispatch to `HomeView` or `remote_home` or `cockpit_view`

**CLI subcommands:**
- Location: `src/main.rs` → `src/cli/<verb>.rs::run()`
- Triggers: `aoe add`, `aoe list`, `aoe send`, `aoe remove`, etc.
- Responsibilities: One-shot operations on sessions; return exit code

**`aoe serve` (serve feature):**
- Location: `src/cli/serve.rs` → `src/server/mod.rs`
- Triggers: `aoe serve [--host] [--port] [--daemon]`
- Responsibilities: Start axum server; serve embedded `web/dist/`; manage auth tokens; start `Supervisor`

**`aoe __cockpit-runner` (internal, serve feature):**
- Location: `src/cockpit/runner.rs`
- Triggers: Spawned by `Supervisor::spawn()` via `setsid`; not intended for direct user invocation
- Responsibilities: Own ACP agent subprocess; bind Unix socket; bridge daemon ↔ agent; survive daemon restarts

**xtask:**
- Location: `xtask/src/main.rs`
- Triggers: `cargo xtask gen-docs`, `cargo xtask check-skill`
- Responsibilities: Generate `docs/cli/reference.md` from clap definitions; validate `contrib/openclaw-skill/SKILL.md`

## Architectural Constraints

- **Threading:** tokio multi-thread runtime (`#[tokio::main]`). `StatusPoller` runs on OS threads (not tokio tasks) via `std::thread::spawn` because it makes blocking tmux subprocess calls. Cockpit uses tokio tasks throughout.
- **Global state:** `SESSION_CACHE: RwLock<SessionCache>` in `src/tmux/mod.rs` is a module-level singleton caching the result of `tmux list-sessions`. `ACTIVE_POLLER_COUNT: AtomicU32` in `src/session/poller.rs` is a global budget counter.
- **Circular imports:** None detected; the layer boundaries (main → cli/tui → session → tmux/containers) are strictly one-way.
- **Feature gating:** `#[cfg(feature = "serve")]` gates `src/server/`, `src/cockpit/`, cockpit-related CLI verbs (`serve`, `url`, `cockpit`, `log-level`), and TUI modules (`cockpit_view`, `remote_home`). TUI-only builds compile without axum or Node.js.
- **Platform:** OS-specific process code is confined to `src/process/macos.rs` and `src/process/linux.rs`; no `cfg` checks outside that module. Linux uses `$XDG_CONFIG_HOME/agent-of-empires/`; macOS/Windows uses `~/.agent-of-empires/`.
- **Debug vs release isolation:** Debug builds use `aoe_dev_` tmux prefix and `agent-of-empires-dev` app dir. Both constants are resolved at compile time via `cfg!(debug_assertions)` in `src/tmux/mod.rs` and `src/session/mod.rs`.

## Anti-Patterns

### Inline `cfg` blocks for OS-specific logic

**What happens:** Callers outside `src/process/` might be tempted to write `#[cfg(target_os = "linux")]` blocks inline when they need platform-specific behavior.
**Why it's wrong:** Scatters platform forks across the codebase; `src/process/` exists specifically to encapsulate this.
**Do this instead:** Add a public function in `src/process/mod.rs` that delegates to `linux.rs` / `macos.rs` and has a no-op fallback for unsupported platforms. See `get_foreground_pid()` as the canonical example.

### Adding config fields without the full settings wiring

**What happens:** Adding a field to `SandboxConfig` or similar without adding `FieldKey`, `SettingField`, `apply_field_to_*`, and `*ConfigOverride` entries.
**Why it's wrong:** The field becomes invisible in the TUI settings panel and won't be persisted by profile overrides.
**Do this instead:** Follow the full checklist in `CLAUDE.md`: `FieldKey` in `src/tui/settings/fields.rs`, `SettingField` in the matching `build_*_fields()`, `apply_field_to_global()` + `apply_field_to_profile()`, `clear_profile_override()` case in `src/tui/settings/input.rs`, and the `*ConfigOverride` struct in `profile_config.rs`.

### Breaking data format changes without migrations

**What happens:** Changing the shape of `sessions.json` or `config.toml` in-place without a migration.
**Why it's wrong:** Existing user data becomes unreadable; serde errors on startup with no recovery path.
**Do this instead:** Add a `src/migrations/vNNN_description.rs`, bump `CURRENT_VERSION` in `src/migrations/mod.rs`, and append a `Migration` entry to `MIGRATIONS`. Migrations must be idempotent.

## Error Handling

**Strategy:** `anyhow::Result<T>` throughout application code; `thiserror` for typed error enums at module boundaries where callers need to pattern-match (e.g., `AcpError`, `EnsureReadyError`, `SupervisorError`, `GitError`).

**Patterns:**
- CLI subcommands return `anyhow::Result<()>`; `main()` prints the error chain and exits non-zero
- Server handlers return `impl IntoResponse`; errors map to HTTP status codes (409 for transient `EnsureReadyError::Transient`, 500 for `EnsureReadyError::Tmux`)
- TUI surfaces errors in dialogs (`InfoDialog`) rather than crashing

## Cross-Cutting Concerns

**Logging:** `tracing` crate. Filter configured by `AOE_LOG_LEVEL` env var (preferred) or `AGENT_OF_EMPIRES_DEBUG=1` (legacy). Sink: file (`debug.log` in app dir) for TUI/CLI; stdout for `aoe serve` (redirected to `serve.log` in daemon mode). Runtime filter changes via `PATCH /api/log-level` when serve is running. Per-byte WS tracing behind `AOE_TERMINAL_TRACE=1`.

**Validation:** Shell metacharacter injection prevented in server handlers by `validate_no_shell_injection()` in `src/server/api/mod.rs`. Session IDs validated by `is_valid_session_id()` in `src/session/capture.rs`.

**Authentication:** Token-based auth in `src/server/auth.rs` / `TokenManager`. Tokens rotated with grace period. `--no-auth` flag for trusted local use.

---

*Architecture analysis: 2026-05-15*
