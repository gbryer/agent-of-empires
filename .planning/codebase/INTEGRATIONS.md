# External Integrations

**Analysis Date:** 2026-05-15

## Agent CLIs (Shelled Out)

`aoe` spawns agent CLIs as tmux pane subprocesses. All agent definitions live in `src/agents.rs` as static `AgentDef` entries in the `AGENTS` constant. Detection uses `which <binary>` or `<binary> --version`.

| Agent | Binary | YOLO Flag | Install Hint |
|-------|--------|-----------|--------------|
| Claude Code | `claude` | `--dangerously-skip-permissions` | `npm install -g @anthropic-ai/claude-code` |
| OpenCode | `opencode` | env `OPENCODE_PERMISSION={"*":"allow"}` | `curl -fsSL https://opencode.ai/install \| bash` |
| Codex CLI | `codex` | `--dangerously-bypass-approvals-and-sandbox` | `npm install -g @openai/codex` |
| Gemini CLI | `gemini` | `--approval-mode yolo` | `npm install -g @google/gemini-cli` |
| Cursor Agent | `agent` | `--yolo` | see docs.cursor.com/cli |
| GitHub Copilot | `copilot` | `--yolo` | GitHub docs |
| Mistral Vibe | `vibe` | `--agent auto-approve` | `pip install mistral-vibe` |
| Pi Coding Agent | `pi` | AlwaysYolo | `npm install -g @mariozechner/pi-coding-agent` |
| Factory Droid | `droid` | `--skip-permissions-unsafe` | `npm install -g droid` |
| Kiro | `kiro` | (env-based) | `curl -fsSL https://cli.kiro.dev/install \| bash` |
| Hermes Agent | `hermes` | (env `HERMES_ACCEPT_HOOKS=1`) | curl install script |
| Settl | `settl` | (binary-specific) | `brew install --cask mozilla-ai/tap/settl` |
| Qwen Code | `qwen-coder` | (hook-based) | - |

**Interaction mechanism:** `aoe` calls `tmux send-keys` to inject text/commands into agent panes. Status detection reads pane content via `tmux capture-pane` — see `src/tmux/status_detection.rs`.

**Session resume:** Per-agent `ResumeStrategy` in `src/agents.rs` — e.g., `Flag("--resume")`, `Subcommand("resume")` for Codex, `FlagPair { existing: "--resume", new_session: "--session-id" }` for Claude Code.

**Hook-based status:** Claude Code, Cursor, Gemini, and Qwen Code support file-based hooks. `aoe` installs entries into agent settings files (e.g., `.claude/settings.json`) so agents write `running`/`idle`/`waiting` status to a file on events like `PreToolUse`, `Stop`, `Notification`. See `src/hooks/` and `AgentHookConfig` in `src/agents.rs`.

## tmux

**Role:** Core substrate. All agent sessions run as tmux panes/windows.

**Interaction:** `aoe` shells out to the `tmux` binary for all session operations:
- `tmux new-session`, `tmux new-window`, `tmux split-window` — session creation
- `tmux send-keys` — sending prompts and commands to agents
- `tmux capture-pane` — reading agent output for status detection
- `tmux kill-session` / `tmux kill-window` — session teardown
- `tmux setenv` / `tmux showenv` — hidden env var storage for session metadata

**Environment:** tmux env vars prefixed with `_AOE_` are used to persist session metadata (session IDs, tool names, status) across tmux restarts. See `src/tmux/env.rs`.

**Session naming:** `aoe_<id>` prefix in release builds; `aoe_dev_<id>` in dev/dev-release builds. Tests use `aoe_test_*` or `aoe_e2e_*`.

**Status bar integration:** `src/tmux/status_bar.rs` — formats pane status for tmux status bar display.

**Detection:** `src/tmux/status_detection.rs` — per-agent regex patterns matching pane content to detect `running`, `idle`, `waiting` states. Also reads file-based hook status for agents that support it.

## Docker / Container Runtimes

**Role:** Sandbox isolation for agent sessions. Mounts project directories into containers.

**Supported runtimes** (see `src/containers/runtime.rs`):
- `Docker` — default, uses `docker` binary
- `Podman` — uses `podman` binary
- `AppleContainer` — uses `container` binary (macOS-only, Apple's container tool)

**Interaction:** All container operations shell out to the respective CLI binary — `docker run`, `docker exec`, `docker rm`, `docker ps`, `docker pull`, `docker inspect`. No Docker daemon SDK used. See `src/containers/runtime_base.rs`.

**Configuration:** `SandboxConfig` in `src/session/container_config.rs`. Settings include:
- Container image selection (configurable per agent/profile)
- Mount points (project dir, `~/.ssh` opt-in via `mount_ssh`, excludes like `target/`, `node_modules`)
- Custom environment variable injection
- Per-agent container env vars (e.g., `CLAUDE_CONFIG_DIR=/root/.claude`)

**Agent container env** (from `src/agents.rs`):
- Claude: `CLAUDE_CONFIG_DIR=/root/.claude`
- Cursor: `CURSOR_CONFIG_DIR=/root/.cursor`
- Copilot: `COPILOT_CONFIG_DIR=/root/.copilot`
- Pi: `PI_CODING_AGENT_DIR=/root/.pi/agent`

## Git (libgit2 via git2 crate)

**Role:** git worktree creation/deletion and branch management for multi-session workflows.

**Library:** `git2 0.20` with `vendored-openssl` — all git operations use libgit2 bindings, not `git` CLI. See `src/git/`.

**Operations:**
- Create/delete git worktrees for isolated per-session branches (`src/git/worktree.rs`)
- Branch name generation from session titles (with unicode NFKD normalization, `unicode-normalization 0.1`)
- Remote branch resolution and tracking (`src/git/remote.rs`)
- Template file resolution for new worktrees (`src/git/template.rs`)
- Diff computation between branches (`src/git/diff.rs`, uses `similar 3.1` crate)

**Web API:** `src/server/api/git.rs` — REST endpoints for git operations accessible from the web dashboard.

## GitHub API (Release Updates)

**Role:** Version checking and release notes display.

**Endpoint:** `https://api.github.com/repos/njbrake/agent-of-empires/releases` (up to 20 releases per fetch). See `src/update/mod.rs`.

**Client:** `reqwest 0.13` with `rustls`, no default features.

**Auth:** No auth token — public API, unauthenticated reads only.

**Env override:** `AGENT_OF_EMPIRES_GITHUB_API_BASE` overrides the base URL (used by e2e tests to serve a fake release fixture).

**Release hosting:** `https://github.com/njbrake/agent-of-empires/releases/tag/vX.Y.Z` — links shown in TUI update banner.

## Agent Client Protocol (ACP)

**Role:** Structured agent rendering in the "Cockpit" view. `aoe` acts as an ACP client; agent processes act as ACP servers.

**Crates:** `agent-client-protocol 0.11` + `agent-client-protocol-tokio 0.11` (both optional, behind `serve` feature).

**Cockpit worker processes** (see `src/cockpit/agent_registry.rs`):
- `claude` → `claude-agent-acp` (npm: `@agentclientprotocol/claude-agent-acp`)
- `opencode` → `opencode acp` (native)
- `gemini` → `gemini --acp` (native)
- `codex` → `codex-acp` (npm: `@agentclientprotocol/codex-acp`)
- `vibe` → `vibe-acp` (native)
- `pi` → `pi-acp` (adapter)
- Generic fallback → `aoe-agent` (`cockpit-worker/aoe-agent/`, Vercel AI SDK 6)

**ACP operations delegated to aoe:** `fs/read_text_file`, `fs/write_text_file` (`src/cockpit/fs_handler.rs`); terminal execution (`src/cockpit/terminal_handler.rs`).

**Cockpit WebSocket:** `/sessions/{id}/cockpit/ws` — fanout of ACP state events to web dashboard clients. See `src/server/cockpit_ws.rs`.

**Node.js requirement:** ACP workers require Node.js >= 20 (min validated at runtime). Resolution order: `AOE_COCKPIT_NODE` env → settings `cockpit.node_path` → `node` on `PATH` → previously-downloaded Node at `$AOE_DATA_DIR/cockpit/node-v22.21.0/`. See `src/cockpit/node.rs`.

## Cloudflare Tunnel

**Role:** HTTPS tunnel to expose local `aoe serve` to public internet (e.g., phone access).

**Binary:** `cloudflared` — shelled out; not a library. See `src/server/tunnel.rs`.

**Modes:**
- Quick tunnel (`cloudflared tunnel --url http://localhost:<port>`) — zero-config, URL rotates on restart. Breaks installed PWAs.
- Named tunnel (`cloudflared tunnel run --url http://localhost:<port> <tunnel-name>`) — stable hostname on user's own domain; requires prior `cloudflared tunnel create`.

**URL extraction:** Parses `cloudflared` stderr output to extract the assigned `https://` URL.

**IP forwarding:** Auth middleware trusts `cf-connecting-ip` header for real IP when request originates from loopback (cloudflared proxy). See `src/server/auth.rs`.

## Tailscale Funnel

**Role:** Alternative HTTPS tunnel using Tailscale's Funnel feature. Preferred over Cloudflare when available.

**Binary:** `tailscale` — shelled out. See `src/server/tunnel.rs`.

**Commands used:**
- `tailscale funnel <port>` — exposes port via Funnel
- `tailscale funnel reset` — removes existing funnel configuration
- `tailscale status --json` — checks availability and login state

**Discovery:** `tailscale_available_sync()` checks `tailscale` binary presence + logged-in state; `tailscale_funnel_cap_ready_sync()` checks nodeAttr ACL. Probed in `src/tui/dialogs/serve.rs`.

**URL format:** Stable `https://<machine>.<tailnet>.ts.net` — survives server restarts; PWAs remain functional.

## Web Push Notifications (Browser Push API)

**Role:** Notifies connected browser clients (PWA) when agent status changes.

**Protocol:** Web Push (RFC 8291) + VAPID (RFC 8292). No third-party push service — directly POSTs to browser push endpoints. See `src/server/push_send.rs`.

**Crypto stack** (all behind `serve` feature):
- `p256 0.13` — ECDH key agreement (server ephemeral + subscription `p256dh`)
- `hkdf 0.13` — key derivation for `aes128gcm` content encryption
- `aes-gcm 0.10` — payload encryption
- `jsonwebtoken 10.3` — VAPID JWT signing (ES256)
- `base64 0.22` — URL-safe base64 encoding

**Delivery:** `reqwest` POSTs to the browser-provided push endpoint URL. 10-second timeout. Handles 410 Gone (subscription expired).

**Kill switch:** Global setting `push_notifications_enabled` in config. See `src/tui/settings/fields.rs`.

## Web Dashboard Auth

**Role:** Token-based authentication for the `aoe serve` web dashboard.

**Implementation:** Custom, no external auth provider. See `src/server/auth.rs`.

**Token delivery methods:**
- Cookie: `aoe_token=<token>` (HttpOnly, SameSite=Strict)
- Query parameter: `?token=<token>` (sets cookie)
- WebSocket: `Sec-WebSocket-Protocol: <token>`
- Authorization header: `Bearer <token>` (for PWA localStorage flow)

**Password auth:** Argon2 (`argon2 0.5`) for passphrase hashing. Used when `--passphrase` is set on `aoe serve`.

**Rate limiting:** 5 failed attempts → 15-minute lockout. Implemented in `src/server/rate_limit.rs`.

**Token rotation:** Server rotates token on suspicious activity; `X-Aoe-Token` response header lets PWA update cached token.

## SQLite (opencode session store)

**Role:** Read opencode session list without spawning opencode subprocess.

**Crate:** `rusqlite 0.39` with `bundled` feature (statically links libsqlite3).

**Usage:** Reads opencode's SQLite session store directly to populate session resume UI. Avoids a known opencode bug that leaks 4.5MB `/tmp/.<hash>.so` per invocation. See `Cargo.toml` comment referencing `anomalyco/opencode#6523`.

**File location:** Detected from opencode's known data directory conventions.

## Sound Playback (System Audio)

**Role:** Audio notifications on agent state changes (e.g., agent goes idle).

**Mechanism:** Shells out to system audio tools — no audio library crate. See `src/sound/playback.rs`.

- macOS: `afplay -v <volume> <path>`
- Linux OGG: `paplay --volume=<vol> <path>` (falls back to `aplay`)
- Linux WAV: `aplay <path>` (falls back to `paplay`)

**Bundled sounds:** `bundled_sounds/` directory — shipped with the binary.

**Settings:** `SoundConfig` in `src/session/config.rs`; toggled per-profile or globally. Disabled automatically when SSH session detected (`$SSH_CLIENT` env var check in `src/tui/settings/render.rs`).

## Log Viewers (`aoe logs`)

**Role:** Display `aoe serve` logs via a preferred external viewer. See `src/cli/logs.rs`.

**Discovery order** (using `which` crate, `which 7.0`):
1. `lnav` — preferred; color, level filters, search (`https://lnav.org`)
2. `bat` — syntax highlighting pager (`bat --paging=always -l log`)
3. `less` — fallback pager
4. Plain stdout — last resort

## Homebrew (Install/Update)

**Role:** Update detection and self-update path for Homebrew-managed installs.

**Detection:** `brew list aoe` — checks if current binary path matches Homebrew-managed path. See `src/update/install.rs` (`classify_with_brew`).

**Update instruction:** When Homebrew-managed, directs user to `brew upgrade aoe` rather than in-app update.

**Install methods tracked:** `Homebrew`, `Script` (install.sh), `Binary` (direct download). Shown in TUI update banner.

## asciinema + agg (E2E Test Recording)

**Role:** Optional recording of e2e TUI tests to `.cast` files + GIF conversion for PR reviews.

**Usage:** `RECORD_E2E=1 cargo test --test e2e -- --nocapture`. Both tools must be on `$PATH`. See `tests/e2e/harness.rs`.

**Commands:**
- `asciinema rec --overwrite --cols 100 --rows 30 -c '<cmd>' <output.cast>` — records tmux session
- `agg <input.cast> <output.gif>` — converts to GIF

**CI trigger:** `needs-recording` PR label triggers `e2e-recording.yml` workflow.

**Output location:** `target/e2e-recordings/`

## Nix Build System

**Role:** Reproducible builds and Nix flake packaging. See `flake.nix`.

**Tools:** `crane` (Cargo build for Nix), `flake-parts`, `nixpkgs-unstable`.

**Packages exposed:**
- `aoe` — TUI-only binary
- `aoe-with-web` — binary with embedded web dashboard

**Build cache:** Cachix (`cachix-push.yml`) — pushes Nix build artifacts to the team's Cachix cache after flake changes.

**CI:** `nix-npm-hash.yml` workflow auto-updates the `npmDepsHash` in `flake.nix` when `web/package-lock.json` changes.

**Build environment variable:** `AOE_WEB_DIST` — when set, `build.rs` copies pre-built frontend from that path instead of running npm (used by `aoe-with-web` Nix package).

## CI/CD (GitHub Actions)

**Platform:** GitHub Actions. Workflows in `.github/workflows/`.

**Key workflows:**
- `ci.yml` — fmt, clippy, tests, cargo-deny license/advisory check, Nix flake eval, web lint + build
- `release.yml` — cross-compilation to 4 targets (macOS amd64/arm64, Linux amd64/arm64) with `--features serve`; creates GitHub Release with tarballs + SHA256
- `cachix-push.yml` — Nix build artifact caching via Cachix
- `docs.yml` — syncs `docs/` to Astro website in `website/`
- `e2e-recording.yml` — records TUI e2e tests as GIFs for PRs with `needs-recording` label

**CI toolchain pinning:** All action versions are pinned to exact commit hashes (e.g., `actions/checkout@de0fac2e...`).

**Rust cache:** `Swatinem/rust-cache` used in CI for faster builds.

## OpenClaw Integration (Community)

**Role:** Community-maintained integration for using `aoe` CLI from OpenClaw workflows.

**Location:** `contrib/openclaw-skill/` — skill definition for AI coding agents.

**Validation:** `cargo xtask check-skill` validates that CLI commands referenced in skill files exist in the `aoe` CLI. Run in CI (`ci.yml`).

---

*Integration audit: 2026-05-15*
