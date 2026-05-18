# Testing Patterns

**Analysis Date:** 2026-05-15

## Test Frameworks

**Rust:**
- Runner: `cargo test` (rustc built-in)
- No separate assertion library — standard `assert!`, `assert_eq!`, `panic!`
- `serial_test` crate for `#[serial]` isolation of tests that mutate `HOME`/environment
- `tempfile` crate for isolated temp directories
- `anyhow::Result` used as test return type for `?` propagation

**TypeScript (unit):**
- Runner: Vitest (`npx vitest run` or `npm run test:unit`)
- Config: `web/vite.config.ts` (`test.include: ["src/**/*.{test,spec}.{ts,tsx}"]`)
- Assertion: `expect()` from Vitest; fake timers via `vi.useFakeTimers()` / `vi.setSystemTime()`

**TypeScript (integration/e2e):**
- Runner: Playwright (`npx playwright test` or `npm test`)
- Config: `web/playwright.config.ts`
- Browser: Chromium headless (single project in config)
- Base URL: `http://localhost:4173` (Vite preview server)
- Retries: 1 on CI, 0 locally
- Screenshots: on failure only

**Run Commands:**
```bash
cargo test                          # all Rust tests (unit + integration)
cargo test --test e2e               # full-binary e2e tests only
cargo test --test e2e -- --nocapture  # e2e with screen dumps on failure
cargo test -- --nocapture           # all tests with stdout visible
RECORD_E2E=1 cargo test --test e2e -- --nocapture  # record TUI sessions

cd web && npm run test:unit         # Vitest unit tests only
cd web && npm test                  # Playwright e2e suite
cd web && npm run test:ui           # Playwright interactive UI mode
```

## Rust Test File Organization

**Unit tests (in-module):**
- Location: `#[cfg(test)] mod tests { ... }` at the bottom of the source file being tested
- Naming: `fn test_<what_is_tested>()` with `#[test]` attribute
- Example location: `src/migrations/mod.rs` has `mod tests` verifying sequential migration ordering

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_are_sequential() {
        let mut prev = 0;
        for m in MIGRATIONS {
            assert!(m.version > prev, "Migration {} should be > {}", m.version, prev);
            prev = m.version;
        }
    }
}
```

**Integration tests:**
- Location: `tests/*.rs` — each file is a separate Cargo integration test binary
- Common helpers: `tests/common/mod.rs` — declared via `mod common;` at the top of each test file
- Key test files: `tests/session_lifecycle.rs`, `tests/migration_pipeline.rs`, `tests/config_merge.rs`, `tests/group_persistence.rs`, `tests/sandbox_integration.rs`, `tests/worktree_integration.rs`, `tests/status_detection.rs`, `tests/diff_integration.rs`

**E2E tests:**
- Location: `tests/e2e/` — single binary declared via `tests/e2e/main.rs`
- Module declarations in `tests/e2e/main.rs`: `mod harness; mod cli; mod command_palette; mod errors; mod logs; mod new_session; mod profile_picker; mod project_registry; mod sandbox; mod serve; mod tui_launch; mod unified_view; mod update_command;`

## Test Data Hygiene

**Always use isolated temp directories:**
```rust
mod common;
use common::setup_temp_home;

#[test]
#[serial]
fn test_create_session_persists() -> Result<()> {
    let _temp = setup_temp_home();  // sets HOME and XDG_CONFIG_HOME; drops to clean up
    // ... test body using agent_of_empires::session::Storage::new("default")
}
```

`tests/common/mod.rs` provides `setup_temp_home() -> TempDir` and `set_temp_home(path)`. The returned `TempDir` must be bound (not `let _`) to prevent early drop.

**Why `#[serial]`:** `setup_temp_home` calls `std::env::set_var` which is not thread-safe; `#[serial]` from `serial_test` prevents concurrent test execution within the process.

**Never read/write real user state:** Tests must not touch `~/.agent-of-empires/` or `~/.config/agent-of-empires/`. Always use temp dirs.

## E2E TUI Harness

The harness lives in `tests/e2e/harness.rs` and provides `TuiTestHarness`.

**Construction:**
```rust
let mut h = TuiTestHarness::new("test_name");
```
This creates an isolated `$HOME` (via `tempfile::TempDir`), seeds `config.toml` to skip welcome dialog and update checks, creates a fake `claude` stub on `$PATH`, and uses a unique tmux socket (`<home>/tmux.sock`) per test instance.

Session names use the pattern `aoe_e2e_{test_name}_{pid}` to avoid collisions.

**Key methods:**

| Method | Description |
|--------|-------------|
| `h.spawn_tui()` | Launch `aoe` (no args) in a detached 100x30 tmux session |
| `h.spawn(&["arg1", "arg2"])` | Launch `aoe <args>` in tmux |
| `h.send_keys("Enter")` | Send tmux key name; sleeps 50ms after |
| `h.type_text("literal text")` | Send literal text via `tmux send-keys -l`; prevents "Enter" interpretation |
| `h.capture_screen()` | Return current screen as plain text (no ANSI) |
| `h.wait_for("text")` | Poll screen until `text` appears; 10s default timeout; panics with screen dump |
| `h.wait_for_timeout("text", dur)` | Poll with custom timeout |
| `h.wait_for_absent("text", dur)` | Poll until `text` disappears |
| `h.wait_for_exit(dur)` | Wait for the tmux session process to exit |
| `h.assert_screen_contains("text")` | Assert with 5-retry tolerance for transient blank captures on macOS CI |
| `h.assert_screen_not_contains("text")` | Assert absence |
| `h.run_cli(&["add", "..."])` | Run `aoe <args>` as subprocess (no tmux); returns `std::process::Output` |
| `h.home_path()` | Path to isolated temp home |
| `h.project_path()` | Create and return `<home>/test-project/` |
| `h.session_alive()` | Check if tmux session is still running |

`TuiTestHarness::Drop` auto-kills the tmux session and converts any recording cast to GIF.

**tmux availability guard:**
```rust
use crate::harness::{require_tmux, TuiTestHarness};

#[test]
#[serial]
fn test_tui_launches() {
    require_tmux!();  // macro: skips the test if tmux is not installed

    let mut h = TuiTestHarness::new("launch");
    h.spawn_tui();
    h.wait_for(" aoe [");
    h.assert_screen_contains("No sessions yet");
}
```

All TUI tests use the `require_tmux!()` macro. All TUI e2e tests use `#[serial]` to prevent overlapping tmux session names and socket conflicts.

**Docker tests:**
```rust
#[test]
#[serial]
#[ignore = "requires Docker daemon"]
fn test_cli_add_with_sandbox() { ... }
```
Docker-dependent tests are `#[ignore]` and never run in standard `cargo test`. Run with `cargo test -- --ignored` on a machine with Docker.

## E2E Recording

```bash
RECORD_E2E=1 cargo test --test e2e -- --nocapture
```

- Requires `asciinema` and `agg` on `$PATH`
- Each TUI test produces a `.cast` file in `target/e2e-recordings/`; `Drop` converts it to a `.gif` via `agg --font-size 14`
- If `asciinema` is absent, recording is silently disabled (warning printed)
- CI: add `needs-recording` label to a PR to trigger recording workflow

## Playwright Web Tests

**Unit tests (Vitest, co-located):**
- Files: `web/src/**/*.test.ts` (e.g., `web/src/lib/session.test.ts`, `web/src/lib/diffTree.test.ts`, `web/src/hooks/useCockpit.queue.test.ts`)
- Pattern: `describe` / `it` / `expect` from Vitest; fake timers with `vi.useFakeTimers()` / `vi.setSystemTime(new Date(NOW))`; `beforeEach` / `afterEach` for setup/teardown
- Factory helpers are plain functions returning shaped objects:
```typescript
function session(status: SessionStatus, idleEnteredAt: string | null) {
  return { status, idle_entered_at: idleEnteredAt };
}
```

**Playwright integration tests (`web/tests/*.spec.ts`):**
- Config: `web/playwright.config.ts` — Chromium only, `baseURL: http://localhost:4173`, `webServer` runs `npx vite preview`
- Test helpers: `web/tests/helpers/sidebar.ts`, `web/tests/helpers/terminal-mocks.ts`

**API mocking pattern (no live server required):**
```typescript
await page.route("**/api/sessions", (r) => r.fulfill({ json: [...] }));
await page.routeWebSocket(/\/sessions\/.*\/ws$/, (ws) => { ws.send(Buffer.from("$ ")); });
```
`mockTerminalApis()` from `web/tests/helpers/terminal-mocks.ts` stubs all REST endpoints and the PTY WebSocket so tests run without a live `aoe serve`.

**Mobile device emulation:**
```typescript
import { devices } from "@playwright/test";
test.use({ ...devices["iPhone 13"] });
```
Mobile tests use the iPhone 13 profile (`pointer: coarse`, `hasTouch: true`, correct viewport, WebKit UA).

**WebSocket spy via `addInitScript`:**
```typescript
await page.addInitScript(() => {
  (window as unknown as { __PTY_SENT__: string[] }).__PTY_SENT__ = [];
  const Orig = window.WebSocket;
  window.WebSocket = class extends Orig {
    constructor(url: string | URL, protocols?: string | string[]) {
      super(url, protocols);
      const origSend = this.send.bind(this);
      this.send = (data) => {
        if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
          const bytes = new Uint8Array(data instanceof ArrayBuffer ? data : data.buffer);
          (window as unknown as { __PTY_SENT__: string[] }).__PTY_SENT__.push(
            new TextDecoder().decode(bytes)
          );
        }
        return origSend(data);
      };
    }
  } as typeof WebSocket;
});
```
Install via `addInitScript` BEFORE `page.goto()` so the spy is active from the first navigation. Read results via `page.evaluate(() => window.__PTY_SENT__)`.

**Multi-touch synthesis (pinch-zoom and swipe):**
Playwright's `page.touchscreen` is single-finger only. Synthesize multi-touch events via `page.evaluate`:
```typescript
await page.evaluate((touches) => {
  const el = document.querySelector(".wterm");
  const touchList = touches.map(t =>
    new Touch({ identifier: t.id, target: el!, clientX: t.x, clientY: t.y })
  );
  el!.dispatchEvent(new TouchEvent("touchstart", { touches: touchList, changedTouches: touchList, bubbles: true }));
}, touchPoints);
```
See `web/tests/helpers/terminal-mocks.ts` `fireTouches()` for the shared helper.

**Velocity-cap gotcha:** Synthetic `touchmove` events fire back-to-back with approximately 1ms between them. A naive `delta-px / delta-time` velocity calculation will produce runaway momentum in tests while real devices look sane. Always cap per-frame velocity and emit counts.

**Mobile keyboard simulation:**
Override `visualViewport.height` and optionally `window.innerHeight` via `page.evaluate` + `Object.defineProperty`. See `web/tests/mobile-keyboard.spec.ts` `simulateKeyboardOpen()` for the reference implementation.

**Test isolation pattern for Playwright:**
- Route mocks must be installed BEFORE `page.goto()`; `addInitScript` is always before navigation
- `seedSettings(page, {...})` writes to `localStorage` — call after `page.goto()`, then `page.reload()` so the app picks up the seeded values with mocks still active

## Coverage

**Requirements:** No coverage threshold is enforced in CI.

**View coverage (Rust):**
```bash
cargo test       # run tests; use cargo-llvm-cov or cargo-tarpaulin for coverage reports
```

**View coverage (Vitest):**
```bash
cd web && npx vitest run --coverage
```

## Test Types

**Unit tests (Rust, in-module):**
- Scope: pure logic, no I/O — e.g., migration ordering, config merge logic, status detection parsing
- 141 `#[cfg(test)]` blocks across `src/`; approximately 192 `#[test]` functions

**Integration tests (`tests/*.rs`):**
- Scope: multi-module flows with real file I/O in temp dirs — session lifecycle, profile management, migration pipeline, sandbox configuration, worktree integration
- Use `setup_temp_home()` + `#[serial]`; test against the library crate directly

**E2E tests (`tests/e2e/*.rs`):**
- Scope: full `aoe` binary via subprocess or tmux — TUI rendering, CLI subcommands, startup flows, profile picker, command palette
- TUI tests auto-skip without tmux; Docker tests are `#[ignore]`

**Playwright e2e (`web/tests/*.spec.ts`):**
- Scope: frontend behavior against mocked API — touch gestures, keyboard handling, WebSocket PTY relay, mobile layout, routing, sidebar behavior
- All run against Vite preview server; no live `aoe serve` needed

**Vitest unit (`web/src/**/*.test.ts`):**
- Scope: pure TypeScript logic — diff tree building, session status helpers, ANSI parsing, cockpit queue behavior, MCP classification
- Co-located with source files; use Vitest fake timers for time-dependent tests

---

*Testing analysis: 2026-05-15*
