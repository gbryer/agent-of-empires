# Coding Conventions

**Analysis Date:** 2026-05-15

## Naming Patterns

**Files:**
- Rust modules: `snake_case.rs` (e.g., `storage.rs`, `group_persistence.rs`)
- Rust module directories with submodules: `snake_case/mod.rs` (e.g., `src/tui/settings/mod.rs`)
- Migration files: `vNNN_description.rs` — zero-padded three-digit version prefix (e.g., `v001_xdg_linux.rs`, `v006_unlimited_cockpit_history.rs`)
- TypeScript/TSX: `camelCase.ts` or `PascalCase.tsx` for React components (e.g., `session.ts`, `CockpitRuntime.tsx`)
- Test files (Vitest): co-located `*.test.ts` alongside source (e.g., `src/lib/session.test.ts`)
- Test files (Playwright): `kebab-case.spec.ts` in `web/tests/` (e.g., `mobile-keyboard.spec.ts`)

**Functions:**
- Rust: `snake_case` for all functions and methods (e.g., `get_pane_pid`, `run_migrations`, `apply_field_to_global`)
- TypeScript: `camelCase` (e.g., `mockTerminalApis`, `simulateKeyboardOpen`, `clickSidebarSession`)

**Variables:**
- Rust: `snake_case` locals; `SCREAMING_SNAKE_CASE` for `const` and `static` items (e.g., `CURRENT_VERSION`, `DEFAULT_TARGET_ROOTS`, `MIGRATIONS`)
- TypeScript: `camelCase` for locals and module exports; `UPPER_SNAKE` for module-level constants (e.g., `IDLE_DECAY_WINDOW_MS`)

**Types:**
- Rust: `CamelCase` for all structs, enums, traits (e.g., `TuiTestHarness`, `SettingsCategory`, `FieldKey`, `GroupTree`)
- TypeScript: `PascalCase` for interfaces/types; exported components are `PascalCase`

## Code Style

**Formatting:**
- Rust: `cargo fmt` — mandatory, enforced in CI (`cargo fmt --all -- --check`)
- TypeScript/TSX: ESLint with `typescript-eslint` recommended rules, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh` (`web/eslint.config.js`)
- TypeScript compile check: `tsc -b` (strict mode via `web/tsconfig.app.json`)

**Linting:**
- Rust: `cargo clippy -- -D warnings` — CI fails on any warning; no suppressions without strong justification
- TypeScript: `npm run lint` in `web/` invokes ESLint against all `**/*.{ts,tsx}` files; `ecmaVersion: 2020`, browser globals

**No-dead-code rule (Rust):**
- Never add `#[allow(dead_code)]` to production source code (`src/`). Remove unused fields and functions rather than suppressing the warning.
- The only legitimate uses are in test helpers (`tests/common/mod.rs`, `tests/e2e/harness.rs`) where the Rust compiler flags pub items not exercised by every test binary.

**No-emdash rule:**
- No emdashes (`—`) and no `--` used as prose separators in human-authored documentation, comments, or commit messages. Use commas, semicolons, or rephrase. Auto-generated files (e.g., `docs/cli/reference.md` from `cargo xtask gen-docs`) are exempt.

## Import Organization

**Rust order (by convention):**
1. Standard library (`std::`, `core::`)
2. External crates (`anyhow`, `serde`, `tokio`, `ratatui`, etc.)
3. Internal crate modules (`crate::`, `super::`, relative)

**TypeScript order:**
1. External packages (`react`, `@playwright/test`, `vitest`)
2. Relative project imports (`./helpers/sidebar`, `./session`)
3. Type-only imports use `import type { ... }` syntax

**Path Aliases:**
- None configured in `web/tsconfig.json`; all imports are relative paths or bare package names

## Error Handling

**Rust patterns:**
- All fallible operations return `Result<T>` using `anyhow::Result` (not `std::result::Result` with a custom error type in most cases)
- Use `anyhow::bail!("message")` for early returns with custom messages
- Use `.context("action description")` / `anyhow::Context` to annotate errors with operation context (e.g., `current_exe().context("locating current executable")`)
- `thiserror` is available for typed error enums in library boundaries
- CLI handler functions uniformly return `Result<()>` (e.g., `pub async fn run(...) -> Result<()>`)
- Never silently discard errors; propagate with `?` or handle with explicit logging

## Logging

**Framework:** `tracing` crate + `tracing-subscriber` with reloadable `EnvFilter`

**Environment variables (in priority order):**
- `AOE_LOG_LEVEL=trace|debug|info|warn|error` — preferred; applies to all `aoe` processes
- `AGENT_OF_EMPIRES_DEBUG=1` — legacy alias for `debug` level
- `AOE_ACP_TRACE=1` — raw JSON-RPC cockpit firehose
- `AOE_TERMINAL_TRACE=1` — per-byte WebSocket PTY firehose

**Log sink routing (configured in `src/main.rs`):**
- Env var set, any `aoe` invocation: writes to `<app-dir>/debug.log` (file)
- `aoe serve` with no env var: stdout (captured into `serve.log` by daemon)
- TUI mode with no env var: `<app-dir>/debug.log` (stderr garbles ratatui alt-screen)
- One-shot CLI with no env var: no subscriber (opt in via `AOE_LOG_LEVEL`)

**Patterns:**
- Use structured fields with `tracing::info!(target: "process.signal", pid = p, signal = "SIGTERM", "sending signal")` — dot-namespaced targets for filtering
- Use `tracing::debug!` for verbose internal state; `tracing::info!` for significant lifecycle events; `tracing::warn!` for recoverable unexpected conditions; `tracing::error!` for failures requiring attention
- Migrations always use `tracing::info!(target: "migrations", ...)` with `version`, `name`, and `duration_ms` fields

## Settings/Config Field Protocol

When adding a configurable field to `SandboxConfig`, `WorktreeConfig`, or any other config struct, all five wiring steps are required (per `CLAUDE.md`):

1. Add a `FieldKey` variant in `src/tui/settings/fields.rs` under `pub enum FieldKey`
2. Add a `SettingField` entry in the matching `build_*_fields()` function in `src/tui/settings/fields.rs`
3. Wire `apply_field_to_global()` and `apply_field_to_profile()` in `src/tui/settings/fields.rs`
4. Add a `clear_profile_override()` case in `src/tui/settings/input.rs`
5. Include the field in the `*ConfigOverride` struct in `src/session/profile_config.rs` with merge logic in `merge_configs()`

## OS-Specific Module Pattern

OS-specific logic lives exclusively in `src/process/macos.rs` and `src/process/linux.rs`. The public interface is exposed through `src/process/mod.rs` using `cfg` blocks:

```rust
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

pub fn get_foreground_pid(shell_pid: u32) -> Option<u32> {
    #[cfg(target_os = "linux")]
    { linux::get_foreground_pid(shell_pid) }
    #[cfg(target_os = "macos")]
    { macos::get_foreground_pid(shell_pid) }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { let _ = shell_pid; None }
}
```

Never add `#[cfg(target_os = "...")]` blocks directly inside `src/cli/`, `src/tui/`, or `src/session/` code.

## Migration Protocol

Breaking changes to stored data go through `src/migrations/`:

1. Create `src/migrations/vNNN_description.rs` with `pub fn run() -> anyhow::Result<()>`
2. In `src/migrations/mod.rs`: add `mod vNNN_description;`, increment `CURRENT_VERSION`, append a `Migration { version: NNN, name: "description", run: vNNN_description::run }` entry to `MIGRATIONS`
3. Migrations must be idempotent, use `tracing::info!(target: "migrations", ...)`, and use `#[cfg(target_os = "...")]` for platform-specific logic

Schema version is tracked in `<app-dir>/.schema_version`; `migrations::run_migrations()` is called on startup from `src/main.rs`.

## CLI Auto-Generated Docs

`docs/cli/reference.md` is generated by `cargo xtask gen-docs`. Never edit it directly. Edit clap help text in `src/cli/` and run `cargo xtask gen-docs` to regenerate. CI enforces this via `git diff --exit-code docs/cli/reference.md`.

## Commit and Branch Conventions

**Branch names:** `feature/...`, `fix/...`, `docs/...`, `refactor/...`

**Commit message prefixes (conventional commits):** `feat:`, `fix:`, `docs:`, `refactor:`

**PR bodies:** Use `.github/pull_request_template.md` as the structure. Include what/why, how tested (`cargo test` plus manual tmux/TUI checks), and screenshots/recordings for any UI change.

## Comments

**When to comment:**
- Explain non-obvious "why", not "what" — code should be self-documenting for the "what"
- No section header comments (e.g., `// --- Helpers ---`); no comments that restate the code
- Module-level doc comments (`//!`) describe the module's purpose and key design choices

**Module-level doc comments are standard:**
```rust
//! Session storage - JSON file persistence
```

---

*Convention analysis: 2026-05-15*
