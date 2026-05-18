# Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire `Sbx` through the settings TUI with capability-driven field filtering, verify transparent operation across all surfaces (cockpit, web dashboard, cross-machine TUI), and implement the post-wake clock-resync handler for aoe-managed sbx sessions. The runtime dropdown already includes `Sbx` at index 3 with basic apply/clear wiring from Phase 2; Phase 6 adds the capability-gating layer and the cross-product correctness test.

Out of scope: real `sbx` subprocess wiring (Phase 5, must land first); user-facing docs (Phase 7); new UI fields for sandbox config (PROJECT.md scope exclusion); any sbx-specific code paths in `src/server/` or `src/cockpit/` (INT-01 constraint).

</domain>

<decisions>
## Implementation Decisions

### Field Visibility Gating (SET-02)
- **D-01:** Filter-at-build-time approach. `build_sandbox_fields()` excludes fields irrelevant to the resolved runtime by checking the capability matrix. Fields unsupported by the current runtime simply don't appear in the settings list. No new rendering states (no greyed-out, no disabled input handling, no footer notes). Rationale: minimal diff, follows existing dynamic field-list construction pattern, clean for public PR.
- **D-02:** Cross-product unit test (every `FieldKey` x every `ContainerRuntimeName`) asserts that fields excluded by the filter match the capability declaration. Test lives in-module in `src/tui/settings/fields.rs` per existing test pattern. The test is the enforcement mechanism that catches future field/capability mismatches.

### Sleep/Wake Handler (INT-02)
- **D-03:** Register for host wake notifications: IOKit `kIOMessageSystemHasPoweredOn` on macOS, `org.freedesktop.login1.Manager` `PrepareForSleep(false)` D-Bus signal on Linux. Handler lives in `src/process/macos.rs` / `src/process/linux.rs` per AGENTS.md (no inline `#[cfg(target_os)]` elsewhere).
- **D-04:** On wake, query aoe's `sessions.json` for running sbx-backed sessions with `created_at` older than 5 minutes. Only resync sandboxes aoe owns (tracked in storage), not all sbx sandboxes on the system.
- **D-05:** Resync command: `sbx exec <sandbox_name> date` to force clock interaction with the host. Fire-and-forget background task with `tracing::warn!` on failure (sandbox may be gone post-wake). No rollback, no user-facing error surface.
- **D-06:** The 5-minute threshold avoids resyncing freshly-created sandboxes whose clocks haven't drifted meaningfully.

### Cross-Machine / Web Transparency (INT-01)
- **D-07:** INT-01 is a verification + fix-if-needed concern. The PTY relay (`src/server/ws.rs`) attaches to tmux panes which are runtime-agnostic. Cockpit goes through ACP which is also agnostic. No preemptive code changes in `src/server/` or `src/cockpit/`. Researcher traces the code paths during planning; planner adds fixes only if something concretely breaks.
- **D-08:** Integration test (`cargo test --features serve --test web_session_create -- --ignored`) creates an sbx session via the API and confirms PTY relay attaches. Gated behind `#[ignore]` like Docker tests.

### E2E Test Strategy (TEST-04)
- **D-09:** Follow existing `tests/e2e/` `TuiTestHarness` pattern with `#[ignore]` gating (same as Docker tests). TUI test navigates to Settings, confirms `Sbx` appears in the runtime dropdown, selects it, saves, and confirms persistence after restart. Uses unique `aoe_e2e_*` tmux names.
- **D-10:** Integration tests requiring a real `sbx` binary are `#[ignore]`-gated and skip when unavailable. No new test infrastructure; just new test cases in the existing harness.

### Claude's Discretion
- Exact IOKit/D-Bus registration boilerplate (planner picks the minimal working approach)
- Whether the wake handler spawns a `std::thread` or `tokio::spawn_blocking` (both work; pick whichever matches existing `src/process/` patterns)
- Which sandbox fields are currently capability-gated vs not (planner audits the full FieldKey list against capabilities)
- Whether the web integration test lives in a new `tests/web_session_create.rs` or extends an existing test file

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 6 — phase goal, dependencies (Phase 5), six success criteria
- `.planning/REQUIREMENTS.md` — RT-01 (Sbx selectable + threaded through settings/profile/session), INT-01 (all surfaces transparent), INT-02 (sleep/wake clock-resync scaffold), SET-01 (full settings TUI wiring per AGENTS.md), SET-02 (field visibility gated on capabilities + cross-product test), TEST-04 (e2e TUI + CLI coverage)
- `.planning/PROJECT.md` § Key Decisions — sbx as 4th ContainerRuntime peer; capability flags over `if runtime == Sbx` branches; no new UI fields this milestone

### Prior phase context (carry-forward)
- `.planning/phases/05-full-sbxruntime-integration-subprocess-lifecycle-port-publis/05-CONTEXT.md` — D-03 (publish_ports is sbx-specific, not a trait method), D-05 (SbxDaemonHealth enum for future diagnostics), D-13 (runtime.rs dispatch arms delegate to SbxRuntime)
- `.planning/phases/04-embedded-sbx-kit-and-materialization/04-CONTEXT.md` — D-02 (sbx_hook_status_dir for workspace-relative hooks), D-04 (no 6th capability flag; !supports_arbitrary_volume_paths covers the constraint)
- `.planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-CONTEXT.md` — D-09 (capabilities parameter threading pattern), D-12 (ensure_image skip already wired)

### Codebase (the surface being modified)
- `src/tui/settings/fields.rs` — `FieldKey` enum, `build_sandbox_fields()`, `apply_field_to_global()`, `apply_field_to_profile()`. Phase 6 adds capability filter predicate in `build_sandbox_fields()` and cross-product test in-module.
- `src/tui/settings/input.rs` — `clear_profile_override()` cases. Sbx already handled via `ContainerRuntimeName::Sbx` at index 3.
- `src/session/profile_config.rs` — `SandboxConfigOverride`, `merge_configs()`. Sbx already supported as a `ContainerRuntimeName` variant.
- `src/session/config.rs` — `ContainerRuntimeName` enum (Sbx variant already exists), `SandboxConfig`
- `src/containers/container_interface.rs` — `RuntimeCapabilities` struct (Phase 1). Used for field gating predicate.
- `src/containers/runtime_base.rs` — `RuntimeBase::SBX.capabilities` (locked matrix). Used in cross-product test.
- `src/process/macos.rs` — currently pid-tree only. Phase 6 adds wake notification registration + resync handler.
- `src/process/linux.rs` — currently pid-tree only. Phase 6 adds D-Bus PrepareForSleep listener + resync handler.
- `src/process/mod.rs` — public API surface for process utilities. Phase 6 adds a `pub fn register_wake_handler(...)` or similar.
- `src/session/storage.rs` — `Storage::load()` for querying running sbx sessions on wake.
- `src/server/ws.rs` — PTY relay (verification target, no changes expected)
- `src/cockpit/` — ACP layer (verification target, no changes expected)
- `tests/e2e/` — `TuiTestHarness`, existing test patterns. Phase 6 adds sbx runtime selector test.

### Repo policy
- `AGENTS.md` § Settings & Configuration — full FieldKey/profile/merge/clear wiring checklist (SET-01 already partially done)
- `AGENTS.md` § Coding Style — no dead code; no inline `#[cfg(target_os)]` outside `src/process/`
- `AGENTS.md` § Testing Guidelines — unit tests in-module for pure logic; e2e tests in `tests/e2e/`; tests must be deterministic

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ContainerRuntimeName::Sbx` already at settings dropdown index 3 (`src/tui/settings/fields.rs:936,949`) with apply/clear wiring in both global and profile paths — SET-01 is mostly complete from Phase 2
- `RuntimeBase::SBX.capabilities` (`src/containers/runtime_base.rs`) — locked capability matrix for the cross-product test assertions
- `RuntimeCapabilities` struct (`src/containers/container_interface.rs`) — `Copy + PartialEq + Eq`; cheap to pass into field-gating predicates
- `Storage::load()` (`src/session/storage.rs`) — reads sessions.json; wake handler uses this to find running sbx sessions
- `SbxRuntime { binary: PathBuf }` (`src/containers/sbx/mod.rs`) — injectable binary for the `sbx exec <sandbox> date` resync call
- `tests/e2e/harness.rs` — `TuiTestHarness` with `spawn_tui()`, `send_keys()`, `wait_for()`, `capture_screen()` — pattern for the new runtime selector e2e test

### Established Patterns
- **Dynamic field-list construction**: `build_sandbox_fields()` already constructs fields conditionally (e.g., cockpit fields gated on `serve` feature). Adding a runtime-capability filter is the same pattern.
- **`#[ignore]`-gated integration tests**: Docker tests gate on `is_available() && is_daemon_running()`. Sbx tests follow identically.
- **OS-specific logic in `src/process/`**: all platform-specific code is confined here. Wake handler registration follows this constraint.
- **In-module `#[cfg(test)]`**: settings fields, container config, and hooks all have extensive in-module test suites. Cross-product test slots in naturally.
- **`tracing::warn!` for non-fatal failures**: used throughout for background tasks that shouldn't crash the app.

### Integration Points
- `src/tui/settings/fields.rs::build_sandbox_fields()` — the ONE site where the field filter is added. Needs access to the resolved runtime's capabilities.
- `src/process/mod.rs` — new public function for wake handler registration, called from `src/main.rs` or `src/tui/app.rs` at startup.
- `src/session/storage.rs` — queried by the wake handler to find aoe-owned sbx sessions.
- `src/containers/sbx/mod.rs::SbxRuntime` — the wake handler calls `sbx exec` through this (or directly via `Command::new("sbx")`).

</code_context>

<specifics>
## Specific Ideas

- **Field filter predicate shape:**
  ```rust
  fn is_field_applicable(key: FieldKey, caps: &RuntimeCapabilities) -> bool {
      match key {
          FieldKey::SandboxImage => true, // always relevant (template for sbx)
          // Fields that depend on specific capabilities...
          _ => true,
      }
  }
  ```
  Planner fills the full match arms by auditing every sandbox FieldKey against capability flags.

- **Wake handler registration (macOS sketch):**
  ```rust
  // src/process/macos.rs
  pub fn register_wake_handler<F: Fn() + Send + 'static>(on_wake: F) {
      std::thread::spawn(move || {
          // IOKit power notification registration
          // On kIOMessageSystemHasPoweredOn: call on_wake()
      });
  }
  ```

- **Resync caller (in session or process layer):**
  ```rust
  fn resync_sbx_clocks() {
      let storage = Storage::new(profile);
      let instances = storage.load().unwrap_or_default();
      let five_min_ago = SystemTime::now() - Duration::from_secs(300);
      for inst in instances.iter().filter(|i| i.is_sbx() && i.is_running() && i.created_before(five_min_ago)) {
          let _ = Command::new("sbx").args(["exec", &inst.container_name, "date"]).output();
      }
  }
  ```

</specifics>

<deferred>
## Deferred Ideas

- **Richer diagnostic surface for capability-gated fields** — Could show a "why is this hidden?" tooltip or `aoe doctor` listing unsupported fields per runtime. Not needed for v1; the filter is self-explanatory.
- **Visual greyed-out rendering for gated fields** — ROADMAP SC-2 mentions this; the filter-at-build-time approach is simpler. Can revisit if user confusion emerges about missing fields.
- **Wake handler for Docker/Podman sessions** — Clock drift is primarily an sbx/microVM concern. Docker containers share the host clock. Not needed.
- **Gitconfig identity / agent-config dirs** — Still deferred from Phase 4 D-07. Not Phase 6 scope.

</deferred>

---

*Phase: 06-settings-tui-cross-machine-integration-and-sleep-wake*
*Context gathered: 2026-05-16*
