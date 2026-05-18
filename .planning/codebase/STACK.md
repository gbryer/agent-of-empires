# Technology Stack

**Analysis Date:** 2026-05-15

## Languages

**Primary:**
- Rust (edition 2021, `rust-version = "1.85"`) - all binary/library code under `src/`
- TypeScript ~6.0.2 - web dashboard under `web/src/`, cockpit worker under `cockpit-worker/aoe-agent/`

**Secondary:**
- Nix - reproducible build system (`flake.nix`, `flake.lock`)

## Runtime

**Environment:**
- Rust stable toolchain (pinned via `rust-toolchain.toml`: `channel = "stable"`)
- Node.js >= 20 required for cockpit-worker ACP agent; >= 22 for `aoe-agent` (`cockpit-worker/aoe-agent/package.json`)
- Tokio 1.50 async runtime (`tokio = { version = "1.50", features = ["full"] }`) — all async I/O

**Package Manager:**
- Cargo (Rust) — lockfile `Cargo.lock` present and committed
- npm (web frontend and cockpit-worker) — lockfile `web/package-lock.json` present

## Frameworks

**Core (Rust):**
- `ratatui 0.30` with `crossterm` backend — TUI rendering (`src/tui/`)
- `crossterm 0.29` with `event-stream` — terminal input/output
- `clap 4.6` with `derive` + `env` — CLI argument parsing (`src/cli/`)
- `axum 0.8` with `ws` — web server + WebSocket PTY relay (`src/server/`); behind `serve` feature flag

**Core (TypeScript/Web):**
- React 19.2.4 — web dashboard UI (`web/src/`)
- React Router 7.14.2 — client-side routing
- Tailwind CSS v4.2.2 — styling
- xterm.js (via `@wterm/react 0.1.8`, `@wterm/core`, `@wterm/dom`) — PTY terminal in browser
- Vite 8.0.3 — frontend bundler/dev server

**Cockpit Worker (TypeScript Node.js):**
- Vercel AI SDK 6 (`ai ^6.0.0`) — multi-provider LLM abstraction
- `@agentclientprotocol/sdk ^0.20.0` — ACP server implementation for `aoe-agent`

**Testing:**
- Rust: `#[test]` in-module + `tests/*.rs` integration tests; `serial_test 3.4` for tmux isolation
- Web: Playwright 1.59.1 for e2e tests (`web/tests/`), Vitest 4.1.5 for unit tests (`web/test:unit`)
- E2E harness: `tests/e2e/harness.rs` (`TuiTestHarness`) wrapping `tmux` + the `aoe` binary

**Build/Dev:**
- `build.rs` — invokes `npm ci` + `npm run build` in `web/` when `--features serve` is set
- `xtask/` workspace crate — build automation (`cargo xtask gen-docs`, `cargo xtask check-skill`)
- `cargo-husky 1` — pre-commit hooks running `cargo fmt` + `cargo clippy`

## Key Dependencies

**Critical:**
- `tokio 1.50` — async runtime powering all I/O, including axum server and PTY streams
- `ratatui 0.30` — TUI rendering engine; all UI layout/widgets
- `git2 0.20` with `vendored-openssl` — git worktree creation/deletion/branch operations (`src/git/`)
- `axum 0.8` (optional, `serve`) — REST API, WebSocket PTY relay, cockpit WebSocket fanout
- `agent-client-protocol 0.11` (optional, `serve`) — ACP client for cockpit structured agent rendering
- `rusqlite 0.39` with `bundled` — reads opencode's SQLite session store without spawning subprocess
- `portable-pty 0.9` — cross-platform PTY allocation for terminal sessions

**Infrastructure:**
- `serde 1.0` + `serde_json 1.0` + `serde_yaml 0.9` + `toml 1.1` — serialization across config/session storage
- `reqwest 0.13` with `rustls` + `json` + `stream`, no default features — HTTP client (GitHub API for updates, web push delivery)
- `nix 0.31` with `signal`, `process`, `net`, `fs`, `resource` — Unix process control and signals
- `notify 8.2` — filesystem watching for session state changes
- `tracing 0.1` + `tracing-subscriber 0.3` — structured logging with `env-filter`
- `argon2 0.5` (optional, `serve`) — password hashing for web auth
- `p256 0.13` + `hkdf 0.13` + `aes-gcm 0.10` + `jsonwebtoken 10.3` (optional, `serve`) — VAPID web push encryption/signing
- `tokio-tungstenite 0.29` (optional, `serve`) — WebSocket client for cockpit-in-TUI view connecting to daemon
- `tar 0.4` + `xz2 0.1` (optional, `serve`) — download and extract Node.js tarball for bundled-Node fallback
- `nucleo-matcher 0.3` — fuzzy matching for session search
- `ansi-to-tui 8.0` — renders ANSI escape codes inside ratatui widgets
- `sha2 0.11` — hashing (auth tokens, cache invalidation)
- `uuid 1.23` v4 — session ID generation

## Configuration

**Environment:**
- `AGENT_OF_EMPIRES_DEBUG=1` — enables file logging to `debug.log` in app data dir
- `AOE_WEB_DIST` — bypass npm build; supply pre-built frontend directory (used by Nix builds)
- `AOE_COCKPIT_NODE` — override Node.js binary path for cockpit-worker
- Config file: `config.toml` in app data dir (Linux: `~/.config/agent-of-empires/`, macOS/Windows: `~/.agent-of-empires/`)
- Dev namespace: `~/.agent-of-empires-dev` / `~/.config/agent-of-empires-dev`, tmux prefix `aoe_dev_`, port `8081`
- Release namespace: `agent-of-empires`, tmux prefix `aoe_`, port `8080`

**Build:**
- `Cargo.toml` — workspace root, single package `agent-of-empires`, binary `aoe` at `src/main.rs`
- `.cargo/config.toml` — `jobs = 8`, alias `xtask = "run -p xtask --"`
- `build.rs` — Cargo.lock hash check; `build_frontend()` gated on `#[cfg(feature = "serve")]`
- `deny.toml` — `cargo-deny` advisory and license policy (MIT, Apache-2.0, BSD, etc.)
- `flake.nix` — Nix flake using `crane` for reproducible builds; exposes `aoe` (TUI-only) and `aoe-with-web` packages

## Feature Flags

**`serve` (optional):** Enables the full web dashboard stack. Pulls in:
- `axum`, `tower-http`, `rust-embed`, `mime_guess`, `qrcode` — HTTP server + static asset embedding
- `argon2`, `p256`, `base64`, `hkdf`, `aes-gcm`, `jsonwebtoken`, `getrandom` — auth + web push crypto
- `webbrowser` — cross-platform browser launcher for `aoe serve --open`
- `agent-client-protocol`, `agent-client-protocol-tokio` — ACP cockpit surface
- `tar`, `xz2` — Node.js tarball extraction
- `tokio-tungstenite` — cockpit-in-TUI WebSocket client

**`test-support` (dev only):** Re-exports private tmux env helpers via `crate::tmux::test_support` for integration tests.

## Build Profiles

| Profile | LTO | Codegen Units | Strip | Debug Assertions | Notes |
|---------|-----|--------------|-------|-----------------|-------|
| `dev` (default) | off | default | off | on | Standard debug build |
| `dev-release` | off | 16 | no | on | Fast optimized local build; uses dev namespace |
| `release` | full | 1 | yes | off | CI/distribution; used with `--features serve` for release binaries |

Release builds for 4 targets: `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`.

## Workspace Structure

```
agent-of-empires/       # workspace root
├── Cargo.toml          # [workspace] members = [".", "xtask"]
├── src/                # main binary/library crate
├── xtask/              # build automation crate (cargo xtask ...)
│   └── Cargo.toml      # depends on agent-of-empires with serve feature
├── web/                # React+TypeScript frontend (npm)
│   ├── package.json    # React 19, Vite 8, Tailwind v4, xterm.js (@wterm)
│   └── src/            # frontend source
├── cockpit-worker/     # Node.js ACP agent worker
│   └── aoe-agent/      # Vercel AI SDK 6 + @agentclientprotocol/sdk
│       └── package.json
├── contrib/            # community integrations (e.g., openclaw-skill)
├── tests/              # Rust integration tests
│   └── e2e/            # full-binary e2e harness (TuiTestHarness)
├── build.rs            # frontend build trigger
├── rust-toolchain.toml # channel = "stable"
├── deny.toml           # cargo-deny advisory + license policy
├── flake.nix           # Nix flake (crane, nixpkgs-unstable)
└── .cargo/config.toml  # jobs=8, xtask alias
```

## Platform Requirements

**Development:**
- Rust stable toolchain
- `tmux` installed (required to run; tests skip if absent)
- Node.js >= 22 + npm (only for `--features serve` builds)
- Linux or macOS (Windows not supported; process module has `macos.rs` + `linux.rs`)

**Production:**
- Deployment as a single self-contained binary (`aoe`)
- Web dashboard compiled into binary via `rust-embed` when built with `--features serve`
- Distribution: GitHub Releases tarballs, Homebrew tap, Nix flake, install script

---

*Stack analysis: 2026-05-15*
