# Codebase Structure

**Analysis Date:** 2026-05-15

## Directory Layout

```
agent-of-empires/
├── src/                    # Rust library + binary source
│   ├── main.rs             # Binary entrypoint (aoe)
│   ├── lib.rs              # Library root; declares all pub modules
│   ├── cli/                # One file per CLI subcommand
│   ├── tui/                # ratatui TUI: app loop, home view, dialogs, settings
│   │   ├── app.rs          # App struct + event loop
│   │   ├── home/           # HomeView (session list, input, render, tests)
│   │   ├── cockpit_view/   # Native ACP cockpit renderer (serve feature)
│   │   ├── remote_home/    # Cross-machine session picker (serve feature)
│   │   ├── dialogs/        # Modal dialogs (new session, confirm, serve, etc.)
│   │   ├── settings/       # Settings TUI (fields, input, render)
│   │   ├── diff/           # Inline diff view
│   │   ├── components/     # Shared ratatui widgets
│   │   └── styles/         # Theme + color definitions
│   ├── session/            # Session persistence, config, instance lifecycle
│   │   ├── storage.rs      # sessions.json + groups.json reader/writer
│   │   ├── instance.rs     # Instance struct + ensure_pane_ready + Status enum
│   │   ├── config.rs       # Config struct (global config.toml)
│   │   ├── profile_config.rs # ProfileConfig + *ConfigOverride + merge_configs()
│   │   ├── builder.rs      # build_instance() — creates Instance + worktree + container
│   │   ├── poller.rs       # AdaptiveInterval + SessionPoller (background status threads)
│   │   ├── capture.rs      # Agent-specific session ID capture + status polling fns
│   │   ├── groups.rs       # Group/GroupTree/Item types + flatten helpers
│   │   ├── projects.rs     # Project registry (multi-repo workspace)
│   │   ├── repo_config.rs  # Repo-local .agent-of-empires/config.toml + hooks
│   │   ├── environment.rs  # Env var resolution + user_shell detection
│   │   ├── container_config.rs # Container launch arg builders
│   │   ├── civilizations.rs    # Agent definitions (claude, codex, gemini, etc.)
│   │   └── serde_helpers.rs    # Custom deserializers (string_or_vec, etc.)
│   ├── tmux/               # tmux subprocess wrappers
│   │   ├── session.rs      # Session struct (start/stop/kill tmux session)
│   │   ├── terminal_session.rs # TerminalSession + ContainerTerminalSession
│   │   ├── status_detection.rs # detect_status_from_content() — parse pane content
│   │   ├── status_bar.rs   # tmux status-bar integration
│   │   ├── env.rs          # Hidden tmux env var helpers (AOE_INSTANCE_ID etc.)
│   │   └── utils.rs        # tmux_prefix_display()
│   ├── cockpit/            # ACP native agent rendering (serve feature)
│   │   ├── acp_client.rs   # AcpClient — stdio transport, ACP protocol impl
│   │   ├── supervisor.rs   # Supervisor — per-session watchdog, spawn/respawn
│   │   ├── runner.rs       # __cockpit-runner shim process (outlives daemon)
│   │   ├── state.rs        # CockpitState single-writer actor + Event enum
│   │   ├── protocol.rs     # Wire types shared between daemon and clients
│   │   ├── agent_registry.rs   # AgentRegistry + AgentSpec (registered ACP agents)
│   │   ├── approvals.rs    # Permission approval flow + Nonce
│   │   ├── fs_handler.rs   # ACP fs/* request handler (aoe owns disk)
│   │   ├── terminal_handler.rs # ACP terminal/* request handler
│   │   ├── context_primer.rs   # Context injection before session/new
│   │   ├── event_store.rs  # Replay buffer for cockpit events
│   │   ├── node.rs         # Bundled Node.js management for aoe-agent
│   │   ├── permissions.rs  # Permission decision builder
│   │   ├── worker_registry.rs  # On-disk registry of live runner shims
│   │   └── client/         # Client-side cockpit HTTP/WS (for TUI + CLI verbs)
│   │       ├── discovery.rs    # AOE_DAEMON_URL discovery
│   │       ├── http.rs         # HttpClient for /api/* calls
│   │       └── ws.rs           # WS connection to cockpit stream
│   ├── server/             # axum web server (serve feature)
│   │   ├── mod.rs          # AppState, TokenManager, router assembly, server start
│   │   ├── api/            # REST handlers
│   │   │   ├── sessions.rs # Session CRUD, ensure-*, diff endpoints
│   │   │   ├── system.rs   # Agents, settings, themes, profiles, filesystem, groups
│   │   │   ├── git.rs      # Repo clone, branch list
│   │   │   ├── cockpit.rs  # Cockpit enable/disable/spawn/prompt/replay (serve feature)
│   │   │   ├── projects.rs # Project registry endpoints
│   │   │   ├── log_level.rs # Runtime log filter GET/PATCH
│   │   │   └── client_log.rs # Browser error log ingestion (serve feature)
│   │   ├── ws.rs           # WebSocket PTY relay (tmux attach-session via portable-pty)
│   │   ├── cockpit_ws.rs   # WebSocket cockpit event bridge (serve feature)
│   │   ├── cockpit_reconciler.rs # Background task: attach cockpit workers to sessions
│   │   ├── auth.rs         # Token auth middleware + TokenManager
│   │   ├── push.rs         # Web push notifications (VAPID)
│   │   ├── push_send.rs    # Push delivery
│   │   ├── login.rs        # Login page HTML
│   │   ├── rate_limit.rs   # Per-IP rate limiter
│   │   └── tunnel.rs       # Cloudflare tunnel integration
│   ├── git/                # git operations
│   │   ├── worktree.rs     # GitWorktree — create/remove/list worktrees
│   │   ├── template.rs     # Template variable resolution for worktree paths
│   │   ├── command.rs      # git subprocess helpers
│   │   ├── diff.rs         # Diff computation (similar crate)
│   │   ├── remote.rs       # Remote URL helpers
│   │   ├── error.rs        # GitError type
│   │   └── cleanup.rs      # Orphaned worktree cleanup
│   ├── process/            # OS-specific process tree management
│   │   ├── mod.rs          # Public API: kill/stop/continue process tree
│   │   ├── macos.rs        # sysctl-based pid tree collection
│   │   └── linux.rs        # /proc-based pid tree collection
│   ├── containers/         # Container runtime abstraction
│   │   ├── mod.rs          # Public API + runtime selection
│   │   ├── runtime.rs      # ContainerRuntime (docker/podman/apple-container)
│   │   ├── runtime_base.rs # Shared container lifecycle logic
│   │   ├── container_interface.rs # ContainerRuntimeInterface trait
│   │   └── error.rs        # Container error types
│   ├── hooks/              # Agent hook management
│   │   └── mod.rs          # Hook install/update in agent settings.json; sidecar status files
│   ├── migrations/         # Versioned data migrations
│   │   ├── mod.rs          # Migration runner + MIGRATIONS registry + CURRENT_VERSION
│   │   ├── v001_xdg_linux.rs
│   │   ├── v002_seed_sandbox_from_volumes.rs
│   │   ├── v003_yolo_mode_config.rs
│   │   ├── v004_unified_environment.rs
│   │   ├── v005_cockpit_defaults.rs
│   │   └── v006_unlimited_cockpit_history.rs
│   ├── update/             # Version update check (GitHub releases)
│   ├── sound/              # Sound effect playback on status transitions
│   ├── terminal.rs         # Terminal capability helpers
│   ├── logging.rs          # tracing subscriber init + LogConfig
│   ├── agents.rs           # Supported agent list (public API over civilizations)
│   └── claude_settings.rs  # Claude ~/.claude/settings.json reader
├── web/                    # React + TypeScript frontend (Vite + Tailwind v4)
│   ├── src/
│   │   ├── App.tsx         # Root component + routing
│   │   ├── main.tsx        # Vite entrypoint
│   │   ├── components/     # React UI components (Dashboard, TerminalView, etc.)
│   │   ├── hooks/          # React hooks (useSessions, useCockpit, useTerminal, etc.)
│   │   └── lib/            # Shared utilities (api.ts, types.ts, session.ts, etc.)
│   ├── tests/              # Playwright browser tests
│   ├── package.json
│   └── vite.config.ts
├── tests/                  # Rust integration tests
│   ├── *.rs                # Integration test files (one concern per file)
│   ├── common/             # Shared test helpers
│   ├── fixtures/           # Test fixture data
│   └── e2e/                # Full-binary e2e tests via tmux harness
│       ├── harness.rs      # TuiTestHarness (spawn_tui, send_keys, wait_for, etc.)
│       └── *.rs            # E2e test scenarios
├── xtask/                  # Cargo workspace build automation
│   └── src/main.rs         # gen-docs + check-skill commands
├── docs/                   # User-facing documentation
│   ├── cli/reference.md    # Auto-generated by `cargo xtask gen-docs` — do not edit
│   ├── development/        # Developer guides (adding-agents.md, etc.)
│   └── guides/             # User guides
├── website/                # Astro static site (agent-of-empires.com)
│   ├── src/                # Astro components and pages
│   └── scripts/sync-docs.mjs # Syncs docs/ → website/
├── cockpit-worker/         # Node.js aoe-agent binary + test shim
│   ├── aoe-agent/          # Vercel AI SDK 6 ACP agent (the native aoe backend)
│   └── test-shim/          # Shim used in cockpit e2e tests
├── contrib/                # Community-maintained integrations
│   └── openclaw-skill/     # OpenClaw skill definition (validated by cargo xtask check-skill)
├── docker/                 # Sandbox Dockerfile and dev variant
├── bundled_sounds/         # Default sound effect assets
├── assets/                 # App icons and images
├── theme/                  # Default theme files
├── scripts/                # Install and utility shell scripts
├── .planning/              # GSD planning artifacts (not committed by default)
│   └── codebase/           # Codebase map documents
├── .claude/skills/         # Project skill definitions
├── Cargo.toml              # Workspace root (members: "." + "xtask")
├── AGENTS.md               # Repository guidelines (CLAUDE.md is a symlink to this)
└── DESIGN.md               # Visual/UI design system reference
```

## Directory Purposes

**`src/cli/`:**
- Purpose: One Rust file per CLI subcommand. Each exports `Args` (clap derive) and `pub async fn run(...)`.
- Contains: `add.rs`, `list.rs`, `remove.rs`, `send.rs`, `status.rs`, `session.rs`, `group.rs`, `worktree.rs`, `profile.rs`, `project.rs`, `serve.rs`, `cockpit.rs`, `url.rs`, `log_level.rs`, `init.rs`, `update.rs`, `uninstall.rs`, `sounds.rs`, `theme.rs`, `tmux.rs`, `logs.rs`, `agents.rs`, `output.rs`
- Key files: `src/cli/definition.rs` (all `Commands` variants), `src/cli/add.rs` (session creation)

**`src/tui/`:**
- Purpose: Full ratatui TUI including home session list, modal dialogs, settings panel, cockpit native view, and remote cross-machine picker.
- Key files: `src/tui/app.rs` (event loop), `src/tui/home/mod.rs` (HomeView state), `src/tui/home/input.rs` (keymap), `src/tui/home/render.rs` (ratatui widgets), `src/tui/dialogs/new_session/mod.rs` (session creation wizard), `src/tui/settings/fields.rs` (all setting field definitions)

**`src/session/`:**
- Purpose: Core domain layer. Owns all session state, config, and instance lifecycle.
- Key files: `src/session/storage.rs` (JSON persistence), `src/session/instance.rs` (3164 lines — Instance methods, ensure_pane_ready, start/stop/delete), `src/session/config.rs` (1665 lines — all config structs), `src/session/capture.rs` (3726 lines — per-agent status capture functions)

**`src/cockpit/`:**
- Purpose: ACP (Agent Client Protocol) native rendering. Only compiled under the `serve` feature.
- Key files: `src/cockpit/acp_client.rs` (3319 lines — full ACP client), `src/cockpit/supervisor.rs` (2432 lines — watchdog + per-session state), `src/cockpit/runner.rs` (runner shim process)

**`src/server/`:**
- Purpose: axum web dashboard backend. Only compiled under the `serve` feature.
- Key files: `src/server/mod.rs` (2014 lines — AppState, router, server startup), `src/server/api/sessions.rs` (2576 lines — all session REST handlers), `src/server/ws.rs` (PTY relay)

**`tests/`:**
- Purpose: Integration tests (no tmux required for most; a few skip when tmux unavailable).
- Key files: `tests/session_lifecycle.rs`, `tests/status_detection.rs`, `tests/config_merge.rs`

**`tests/e2e/`:**
- Purpose: Full-binary e2e tests via `TuiTestHarness`. All use `#[serial]` for tmux isolation.
- Key files: `tests/e2e/harness.rs`, `tests/e2e/new_session.rs`, `tests/e2e/tui_launch.rs`

**`web/src/`:**
- Purpose: React 19 + TypeScript web dashboard frontend built with Vite.
- Key files: `web/src/App.tsx`, `web/src/components/Dashboard.tsx`, `web/src/components/TerminalView.tsx`, `web/src/hooks/useSessions.ts`, `web/src/hooks/useCockpit.ts`, `web/src/lib/api.ts`, `web/src/lib/types.ts`

**`xtask/`:**
- Purpose: Cargo workspace build tasks. Run via `cargo xtask <command>`.
- Key files: `xtask/src/main.rs` — `gen-docs` generates `docs/cli/reference.md`; `check-skill` validates `contrib/openclaw-skill/SKILL.md` against real CLI commands.

## Key File Locations

**Entry Points:**
- `src/main.rs`: Binary entrypoint — clap parse, logging init, migrations, dispatch
- `src/tui/mod.rs`: `tui::run()` — TUI mode entry
- `src/cli/serve.rs`: `serve::run()` — web server entry

**Core Domain:**
- `src/session/instance.rs`: `Instance` struct (the central data type)
- `src/session/storage.rs`: `Storage::load()` / `Storage::save()` — JSON persistence
- `src/session/config.rs`: `Config` struct — all global settings
- `src/session/profile_config.rs`: `ProfileConfig`, `merge_configs()` — profile override system

**Configuration:**
- `src/session/config.rs`: Global `Config` (TOML deserialization)
- `src/session/profile_config.rs`: Per-profile overrides + merge logic
- `src/tui/settings/fields.rs`: `FieldKey` enum + `build_*_fields()` (all settings UI definitions)
- `src/tui/settings/input.rs`: Settings keyboard handler + `apply_field_to_*` / `clear_profile_override()`

**Agent/Civilization Registry:**
- `src/session/civilizations.rs`: All supported agents (claude, codex, gemini, opencode, hermes, pi, vibe)
- `src/session/capture.rs`: Per-agent status polling functions + session ID capture

**CLI Reference (auto-generated):**
- `docs/cli/reference.md`: Generated by `cargo xtask gen-docs`. Never edit manually.

**Migrations:**
- `src/migrations/mod.rs`: `MIGRATIONS` array, `CURRENT_VERSION`, `run_migrations()`

**Web API contract:**
- `src/server/api/sessions.rs`: `SessionResponse` (the JSON shape browsers consume)
- `web/src/lib/types.ts`: TypeScript mirror of server types

**Testing:**
- `tests/e2e/harness.rs`: `TuiTestHarness` — the e2e test API
- `tests/common/`: Shared helpers across integration tests

## Naming Conventions

**Files (Rust):**
- `snake_case.rs` for modules (e.g., `status_detection.rs`, `profile_config.rs`)
- One module per file; split large modules into subdirectories with `mod.rs` (e.g., `src/tui/home/`)
- Migration files follow `vNNN_description.rs` (e.g., `v006_unlimited_cockpit_history.rs`)

**Files (TypeScript/React):**
- `PascalCase.tsx` for React components (e.g., `Dashboard.tsx`, `TerminalView.tsx`)
- `camelCase.ts` for utilities and hooks (e.g., `useSessions.ts`, `api.ts`, `types.ts`)
- Test files co-located with extension `.test.ts` (e.g., `ansi.test.ts`)

**Directories:**
- Rust: `snake_case/` (e.g., `cockpit_view/`, `new_session/`)
- Web: `camelCase/` convention for `hooks/`, `lib/`, `components/` (flat, not deeply nested)

**Rust identifiers:**
- `snake_case` for functions and modules
- `CamelCase` for types and enums
- `SCREAMING_SNAKE_CASE` for constants
- Error types use `thiserror` derive with `#[error("...")]` messages

## Where to Add New Code

**New CLI subcommand:**
1. Create `src/cli/<verb>.rs` with `pub struct <Verb>Args` (clap `Args` derive) and `pub async fn run(...) -> Result<()>`
2. Add `pub mod <verb>;` in `src/cli/mod.rs`
3. Add variant to `Commands` enum in `src/cli/definition.rs`
4. Add dispatch arm in `src/main.rs`
5. Run `cargo xtask gen-docs` to regenerate `docs/cli/reference.md`

**New session config field:**
1. Add field to the relevant `*Config` struct in `src/session/config.rs`
2. Add `Option<T>` field to the matching `*ConfigOverride` in `src/session/profile_config.rs` with merge logic in `merge_configs()`
3. Add `FieldKey` variant in `src/tui/settings/fields.rs`
4. Add `SettingField` entry in the matching `build_*_fields()`
5. Wire `apply_field_to_global()` + `apply_field_to_profile()` in `src/tui/settings/input.rs`
6. Add `clear_profile_override()` case in `src/tui/settings/input.rs`

**New agent (civilizations):**
- See `docs/development/adding-agents.md`
- Primary changes in `src/session/civilizations.rs` and `src/session/capture.rs`

**New REST endpoint:**
1. Add handler function in the appropriate `src/server/api/<module>.rs`
2. Export it in `src/server/api/mod.rs`
3. Register route in the router in `src/server/mod.rs`
4. Add TypeScript API call in `web/src/lib/api.ts` and type in `web/src/lib/types.ts`

**New TUI dialog:**
1. Create `src/tui/dialogs/<name>.rs`
2. Add `pub mod <name>;` and re-export in `src/tui/dialogs/mod.rs`
3. Add enum variant to the dialog enum in `src/tui/home/mod.rs`
4. Handle open/close in `src/tui/home/input.rs`
5. Render in `src/tui/home/render.rs`

**New data migration:**
1. Create `src/migrations/vNNN_description.rs` with `pub fn run() -> anyhow::Result<()>`
2. In `src/migrations/mod.rs`: add `mod vNNN_description;`, bump `CURRENT_VERSION`, append `Migration { version: NNN, name: "description", run: vNNN_description::run }` to `MIGRATIONS`
3. Migrations must be idempotent

**New integration test:**
- Create `tests/<concern>.rs` (access via `mod common;` for shared helpers in `tests/common/`)
- Unit tests go in `#[cfg(test)]` blocks inside the source file

**New e2e test:**
- Create `tests/e2e/<scenario>.rs`
- Use `TuiTestHarness` from `tests/e2e/harness.rs`
- Add `#[serial]` from the `serial_test` crate for tmux isolation
- Add the test name to `tests/e2e/main.rs`

**New web component:**
- Implementation: `web/src/components/<PascalCase>.tsx`
- Shared logic / React hooks: `web/src/hooks/use<Name>.ts`
- Utility functions: `web/src/lib/<camelCase>.ts`
- Playwright tests: `web/tests/`

## Special Directories

**`.planning/codebase/`:**
- Purpose: GSD codebase map documents (this file is one of them)
- Generated: Yes (by `/gsd:map-codebase`)
- Committed: No (in `.gitignore` via the `?? .planning/` status)

**`web/dist/`:**
- Purpose: Compiled frontend assets embedded into the `serve` binary via `rust-embed`
- Generated: Yes (by `npm run build` in `web/`, triggered by `build.rs` when `serve` feature is active)
- Committed: No

**`docs/cli/reference.md`:**
- Purpose: Auto-generated CLI reference from clap definitions
- Generated: Yes (by `cargo xtask gen-docs`)
- Committed: Yes (CI enforces it matches the binary)

**`bundled_sounds/`:**
- Purpose: Default sound effect audio files compiled into the binary
- Generated: No
- Committed: Yes

**`contrib/`:**
- Purpose: Community-maintained integration files. `cargo xtask check-skill` validates them in CI.
- Generated: No
- Committed: Yes

**`cockpit-worker/aoe-agent/`:**
- Purpose: The `aoe-agent` Node.js binary (Vercel AI SDK 6) — the native ACP backend agent
- Generated: No (authored)
- Committed: Yes

---

*Structure analysis: 2026-05-15*
