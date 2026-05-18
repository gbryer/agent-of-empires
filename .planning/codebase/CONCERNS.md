# Codebase Concerns

**Analysis Date:** 2026-05-15

## Tech Debt

**Legacy `.aoe/` repo config path is an inline compat shim (not a migration):**
- Issue: `src/session/repo_config.rs:104-127` implements a runtime fallback to read `.aoe/config.toml` (pre-1.1 path) with a `tracing::warn!` rather than a one-time data migration via `src/migrations/`. This runs on every `load_repo_config` call, indefinitely.
- Files: `src/session/repo_config.rs:104-170`
- Impact: Perpetual runtime overhead; the migration pattern from `src/migrations/` is bypassed. Per AGENTS.md, breaking changes to file locations should go through migrations, not inline compat shims.
- Fix approach: Add a `v007_rename_aoe_repo_config.rs` migration that renames `.aoe/config.toml` to `.agent-of-empires/config.toml` for any repo that has it. Remove the fallback from `load_repo_config`.

**`src/session/capture.rs` is the largest file at 3726 lines:**
- Issue: This single file handles session ID capture logic for all 14+ agent types. Each agent type embeds agent-specific heuristics (file paths, directory naming conventions, subprocess discovery) in one monolithic file.
- Files: `src/session/capture.rs`
- Impact: Hard to add a new agent, high merge conflict risk, test coverage is harder to reason about.
- Fix approach: Split per-agent capture logic into `src/session/capture/` subdirectory, one file per agent type, mirroring the pattern used in `src/agents.rs`.

**`#[allow(clippy::too_many_arguments)]` used in 11 locations:**
- Issue: 11 functions suppress the too-many-arguments lint rather than extracting parameter structs.
- Files: `src/tui/components/text_input.rs:142`, `src/tui/components/preview.rs:15,141`, `src/tui/dialogs/delete_options.rs:357`, `src/tui/dialogs/serve.rs:1370,1737,1915`, `src/cockpit/worker_registry.rs:61`, `src/cockpit/acp_client.rs:315,389,1752`
- Impact: Functions are hard to call correctly; callers must track argument order precisely.
- Fix approach: Introduce builder or config structs for heavily-parameterized functions. Start with `src/tui/dialogs/serve.rs` which suppresses the lint three times.

**`src/server/api/sessions.rs` at 2576 lines:**
- Issue: A single API file handles session create, delete, start, stop, PTY respawn, environment sync, container management, worktree operations, and status updates.
- Files: `src/server/api/sessions.rs`
- Impact: Difficult to navigate, high cognitive load for reviewers, forces broad write-lock acquisitions on `state.instances`.
- Fix approach: Split by domain: `sessions/lifecycle.rs`, `sessions/terminal.rs`, `sessions/container.rs`.

## Known Bugs

**Composer prompt loss on WebSocket disconnect:**
- Symptoms: When the cockpit WebSocket is closed (network flap, server restart), a user who types and submits a prompt sees it silently discarded. The `POST /cockpit/prompt` request either never fires or returns an error with no retry path.
- Files: `web/src/components/cockpit/Composer.tsx:50,718`
- Trigger: Submit a prompt immediately after a brief server restart or mobile network switch.
- Workaround: Two `TODO(post-disconnect)` comments identify this as a known gap. The `disabled` prop blocks the send button when `connected === false`, but brief disconnect windows between connected=true detection and the POST are not covered.

**`SESSION_CACHE` RwLock silently returns stale data on poisoning:**
- Symptoms: `SESSION_CACHE.read().ok()?` in `src/tmux/mod.rs:165` returns `None` if the lock is poisoned, which is treated as "cache expired" — causing a fresh tmux subprocess invocation. In theory correct; in practice if a panic inside `refresh_session_cache` (line 74) poisons the write lock, all future reads silently fall back to spawning tmux with every poll tick.
- Files: `src/tmux/mod.rs:56-177`
- Trigger: Panic inside `refresh_session_cache` (unlikely, but no panic boundary exists).
- Workaround: None visible; the `ok()?` pattern masks poisoning.

**PTY orphan on `clone_reader` failure path:**
- Symptoms: If `master.try_clone_reader()` fails (line 398), the code kills and waits the child. If `master.take_writer()` then fails (line 413), the code also kills and waits — but `pair.slave` was already dropped at line 391. On some platforms the slave drop does not guarantee the child's stdin is EOF; the SIGKILL path at line 407 is the only backstop.
- Files: `src/server/ws.rs:390-426`
- Impact: Edge case only; no known user reports. Worst case: `tmux attach-session` leaks until SIGKILL.
- Workaround: The explicit `let _ = child.kill(); let _ = child.wait();` guards this well in practice.

## Security Considerations

**Token embedded in `serve.url` file:**
- Risk: `serve.url` contains the full URL including `?token=<256-bit-hex>`, written with 0600 permissions via `write_secret_file`. However, the file is read by the TUI (`src/tui/dialogs/serve.rs:123`, `src/cli/serve.rs:136`) and the token is surfaced in the TUI for copy-paste. If the file is logged or included in a bug report, the token is exposed.
- Files: `src/server/mod.rs:625-662`, `src/server/mod.rs:1236-1254`
- Current mitigation: 0600 permissions on Unix. Windows fallback (`#[cfg(not(unix))]`) at `src/server/mod.rs:1252-1255` uses `tokio::fs::write` with no permission restriction — the file is world-readable on Windows.
- Recommendations: Consider storing token and URL separately; never bundle them in a user-visible file. Fix Windows path to apply ACL restrictions.

**`style-src 'unsafe-inline'` in CSP:**
- Risk: The Content Security Policy at `src/server/mod.rs:1080` permits `unsafe-inline` styles. If an XSS injection lands in a style attribute, it can leak information even though script injection is blocked.
- Files: `src/server/mod.rs:1064-1087`
- Current mitigation: `script-src` is restricted to `'self' 'wasm-unsafe-eval'`, blocking inline script. Style-only XSS is lower severity.
- Recommendations: Migrate to CSS-in-JS without inline styles, or use nonces. Tailwind v4 and xterm.js write inline styles — this may require upstream changes first.

**`--no-auth` mode with network binding checked at CLI, not server layer:**
- Risk: The guard at `src/cli/serve.rs:317-325` blocks `--no-auth --remote` combinations, but if the `AppState::no_auth` flag is set and the server is reached via a misconfigured reverse proxy, all auth is bypassed. The `is_no_auth()` check in the middleware at `src/server/auth.rs:238` passes every request through with no token check.
- Files: `src/server/auth.rs:238-246`, `src/cli/serve.rs:317-325`
- Current mitigation: CLI-level guard. Documentation warning.
- Recommendations: Add a runtime assertion in the server startup that refuses `no_auth=true` when the bind address is not `127.0.0.1`.

**`aoe_token` cookie set without `Secure` flag on non-tunnel connections:**
- Risk: The `build_cookie` function at `src/server/auth.rs:65-70` conditionally sets `Secure` based on `state.behind_tunnel`. For plain HTTP local connections (`aoe serve --host 0.0.0.0` on LAN), the cookie is sent over cleartext, leaking the auth token to any network observer.
- Files: `src/server/auth.rs:65-92`
- Current mitigation: LAN use is documented as local-only; most deployments use tunnel or Tailscale.
- Recommendations: Document clearly; consider warning at startup when binding non-loopback without TLS.

**`unsafe { std::env::set_var(...) }` in production code path:**
- Risk: `src/main.rs:38-41` mutates `AOE_DAEMON_URL` with `unsafe { env::set_var }` in the main process before the tokio runtime is fully multi-threaded. The SAFETY comment notes this is single-threaded at that point. Rust 2024 edition (stabilized `unsafe_op_in_unsafe_fn`) will flag this pattern more visibly.
- Files: `src/main.rs:33-41`, `src/cockpit/node.rs:373`
- Current mitigation: SAFETY comments. The timing claim is correct at present.
- Recommendations: Use a dedicated config struct or OnceLock instead of mutating the process environment.

## Performance Bottlenecks

**`state.instances` is a single `RwLock<Vec<Instance>>` under all API handlers:**
- Problem: Every session list, create, update, delete, and terminal respawn acquires a write lock on the same `Vec`. Under 50+ concurrent sessions with active polling, this creates a write-lock bottleneck — read-heavy paths starve behind write operations.
- Files: `src/server/mod.rs:200`, `src/server/api/sessions.rs` (23 lock acquisitions)
- Cause: The `Vec<Instance>` design favors simplicity over concurrency. Each mutation also requires a linear scan (`.iter_mut().find(...)`) rather than O(1) lookup.
- Improvement path: Replace with `DashMap<String, Instance>` (concurrent hashmap) keyed by session ID, eliminating the global lock contention for independent sessions.

**`capture.rs` scans the filesystem on every status poll cycle:**
- Problem: Session ID capture for some agents (opencode, Claude Code) walks the filesystem (`~/.config/opencode/`, `~/.claude/projects/`) on every poll to find the agent's session file. At 1s poll frequency for running sessions this is 1+ directory walk per second per running session.
- Files: `src/session/capture.rs` (entire file)
- Cause: No inotify/FSEvents watching; polling is the only mechanism.
- Improvement path: Cache the resolved session ID once found; only re-scan after a session restart event.

**Cockpit broadcast channel too small for high-frequency events:**
- Problem: `COCKPIT_CHANNEL_CAPACITY = 256` at `src/server/mod.rs:33`. A cockpit session under active agent use emits tool call events, progress updates, and approval requests at high frequency. Lagged receivers trigger a `"kind":"lagged"` message (`src/server/cockpit_ws.rs:131-142`) forcing client re-fetch from SQLite, adding latency spikes.
- Files: `src/server/mod.rs:33`, `src/server/cockpit_ws.rs:131-142`
- Cause: Static capacity chosen conservatively.
- Improvement path: Increase to 1024-4096, or make capacity configurable via env var for high-throughput deployments.

## Fragile Areas

**tmux status detection via pane content scraping:**
- Files: `src/tmux/status_detection.rs`
- Why fragile: Detection relies on matching agent-specific text patterns (spinner characters, verb+ellipsis shape, approval prompt text) in `tmux capture-pane` output. Any UI change to a supported agent immediately breaks detection for that agent. The file is 1574 lines and growing as new agents are added. The codex detector is specifically noted in commit history as having had multiple regressions.
- Safe modification: Always add a `detect_status_from_content(content, "agent_name")` unit test with a real pane capture fixture before changing any agent's detection logic. Fixtures live in `tests/fixtures/`.
- Test coverage: Reasonable at the unit level; no integration test that runs the actual agent binary and verifies the detected status.

**`src/tui/settings/render.rs` uses `.unwrap()` inside rendering hot path:**
- Files: `src/tui/settings/render.rs:479,573,651`
- Why fragile: Three `.unwrap()` calls dereference `self.editing_input.as_ref()` and `self.list_edit_state.as_ref()` inside render functions. If called when the field is `None` (possible during state transitions), the TUI panics and crashes.
- Safe modification: Add `if let Some(input) = self.editing_input.as_ref()` guards, returning early without rendering the editing overlay when the state is unexpectedly absent.
- Test coverage: No TUI render tests for the settings panel.

**`src/tui/deletion_poller.rs:93` — `.unwrap()` on background thread result:**
- Files: `src/tui/deletion_poller.rs:93`
- Why fragile: `let result = result.unwrap();` unwraps a `Result` from a background task. If the task panics or the `oneshot` receiver is dropped, this panics on the main render thread.
- Safe modification: Replace with `let Ok(result) = result else { return; }`.
- Test coverage: Not covered by unit tests.

**`src/tui/creation_poller.rs:152,195` — `.unwrap()` on `hooks.as_ref()`:**
- Files: `src/tui/creation_poller.rs:152,195`
- Why fragile: Two calls to `hooks.as_ref().unwrap()` inside the session creation background path. If `hooks` is `None` (no hook config), these panic.
- Safe modification: Guard with `if let Some(hooks) = hooks.as_ref()`.
- Test coverage: Not covered by unit tests.

**Cookie header parsing with `.expect()` at request time:**
- Files: `src/server/auth.rs:89`, `src/server/auth.rs:413`
- Why fragile: `cookie.parse().expect("cookie format must be valid")` panics if the generated cookie string somehow fails header value parsing. The test at line 1928 validates the static CSP string, but not the runtime cookie string which includes the dynamic token value.
- Safe modification: Replace with `if let Ok(v) = cookie.parse() { ... }` and log an error if parsing fails, returning a response without the Set-Cookie header rather than panicking.

**Docker integration tests are fully `#[ignore]`:**
- Files: `tests/e2e/sandbox.rs:8`, `tests/parallel_capture.rs:105,175,241`
- Why fragile: All Docker-dependent tests are skipped in CI. Container sandbox boundary checks, volume mount logic, and container config sync (`src/session/container_config.rs`, 3014 lines) have no automated test coverage in CI.
- Safe modification: Any change to `src/session/container_config.rs` or `src/containers/` must be manually tested with a live Docker daemon.
- Test coverage: Large gap — the entirety of Docker sandbox behavior.

## Scaling Limits

**Token rotation and multi-device state is in-memory only:**
- Current capacity: Works correctly for a single long-running `aoe serve` process.
- Limit: Token state (current token, legacy tokens, login sessions) lives in `TokenManager` which is heap-allocated on startup. On server restart, all active sessions must re-authenticate.
- Scaling path: Persist token state to disk (similar to how `serve.token` is written) so restarts are transparent to clients.

**PTY relay spawns one `tmux attach-session` subprocess per WebSocket connection:**
- Current capacity: Works for tens of simultaneous browser sessions.
- Limit: Each `/ws/terminal/{id}` connection spawns a dedicated `tmux attach-session` process (`src/server/ws.rs:371`). At hundreds of connections (unlikely but possible in a team setting), this exhausts process table entries and FDs.
- Scaling path: Multiplex multiple WebSocket clients through a single PTY reader using the broadcast/fan-out pattern already used by `cockpit_ws`.

## Dependencies at Risk

**`portable-pty` for PTY relay:**
- Risk: `portable-pty` is a niche crate with limited maintenance activity. PTY spawning is a core feature for the web terminal.
- Impact: If the crate goes unmaintained, PTY relay breaks on new OS versions.
- Migration plan: `tokio` + `nix` for Unix PTY, or `conpty` for Windows, would be the fallback. The abstraction in `src/server/ws.rs:317-382` makes a swap tractable.

## Missing Critical Features

**No prompt queuing on WebSocket reconnect:**
- Problem: Prompts typed and submitted during a WebSocket gap are dropped. The `Composer` blocks new sends when `connected === false`, but brief reconnect windows are not covered.
- Blocks: Reliable use on mobile connections (frequent network switches) and immediately after `aoe serve` restarts.
- Both TODO markers: `web/src/components/cockpit/Composer.tsx:50,718`

**No Playwright test for touch velocity / inertia scrolling on real-device simulation:**
- Problem: AGENTS.md explicitly documents the e2e touch velocity gotcha ("synthetic touchmove events fire back-to-back with Dt~1ms, which blows up any velocity calculation"). The `clampV` and `MAX_WHEELS_PER_FRAME` guards are in place (`web/src/hooks/useTerminal.ts:772-775`), but there is no Playwright test that exercises the inertia decay path (`onTouchEnd` → `decay()` loop at line 984).
- Blocks: Catching regressions where velocity clamping is accidentally removed or the decay constant is changed.
- Note: `web/tests/mobile-keyboard.spec.ts` exists but two key tests skip non-WebKit browsers (`test.skip(browserName !== "webkit")`), meaning Chromium CI never runs the proxy-input path.

## Test Coverage Gaps

**Docker sandbox full lifecycle (create, run, stop, destroy):**
- What's not tested: Container creation, volume mounting, agent config sync inside container, container terminal respawn, `--sandbox` flag interaction with worktrees.
- Files: `src/session/container_config.rs`, `src/containers/runtime.rs`, `src/containers/container_interface.rs`
- Risk: Container-specific bugs ship undetected. The 3014-line `container_config.rs` has unit tests but no integration tests that run Docker.
- Priority: High

**`src/server/ws.rs` PTY relay teardown:**
- What's not tested: The multiple cleanup paths when PTY reader, PTY writer, WebSocket sender, or child process fails independently.
- Files: `src/server/ws.rs:390-900`
- Risk: PTY/process leaks in error paths go undetected.
- Priority: Medium

**Settings panel rendering with `None` editing state:**
- What's not tested: The `settings/render.rs` render functions when `editing_input` and `list_edit_state` are `None`, which would panic on the `.unwrap()` calls.
- Files: `src/tui/settings/render.rs:479,573,651`
- Risk: State transition bugs cause TUI crash.
- Priority: Medium

**Migration idempotency under partial failure:**
- What's not tested: Migrations `v001` through `v006` are each tested for the happy path. No test verifies that a migration run that fails midway and is retried leaves state consistent.
- Files: `src/migrations/`
- Risk: Corrupted config after interrupted upgrade.
- Priority: Medium

**Cockpit `RecvError::Lagged` client recovery:**
- What's not tested: What happens in the browser when the cockpit WS sends `{"kind":"lagged","skipped":N}`. The client is expected to call `GET /cockpit/replay` to refill, but there is no test verifying this.
- Files: `src/server/cockpit_ws.rs:131-142`, `web/src/` (client-side lagged handler)
- Risk: Client silently diverges from server state after a high-throughput burst.
- Priority: Medium

---

*Concerns audit: 2026-05-15*
