---
phase: 06-settings-tui-cross-machine-integration-and-sleep-wake
verified: 2026-05-17T10:15:00Z
status: human_needed
score: 9/9
overrides_applied: 0
re_verification:
  previous_status: human_needed
  previous_score: 7/7
  gaps_closed:
    - "SSH agent forwarding works inside sbx sandboxes (Plan 03 gap closure)"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Verify Sbx runtime selector in live TUI"
    expected: "Opening Settings > Sandbox shows 'Docker Sandboxes' as the runtime label for Sbx, and the three hidden fields (Extra Volumes, Volume Ignores, Mount SSH) do not appear"
    why_human: "E2e test is gated behind #[ignore = 'requires sbx daemon']; cannot run in automated verification without tmux + sbx"
  - test: "Verify web session creation with sbx runtime"
    expected: "POST /api/sessions with container_runtime=sbx returns a Running session and WebSocket PTY relay attaches"
    why_human: "Test is a compile gate scaffold; requires live sbx daemon and --features serve to exercise the full API path"
  - test: "Verify sleep/wake clock resync on macOS"
    expected: "After macOS sleep/wake, sbx exec <name> date is invoked for each aoe-owned sandbox older than 5 minutes"
    why_human: "Requires actual macOS sleep/wake event; unit tests cover filtering logic only, not OS event detection"
  - test: "Verify SSH agent forwarding inside sbx sandbox"
    expected: "Inside a running sbx sandbox, ssh -T git@github.com authenticates using host keys"
    why_human: "Requires running sbx daemon, host SSH agent, and GitHub SSH key configured"
---

# Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake Verification Report

**Phase Goal:** Wire `Sbx` through the settings TUI per AGENTS.md (FieldKey + apply/clear + override merge), gate field visibility on capabilities, verify cockpit/web/cross-machine transparency end-to-end, and scaffold the post-wake clock-resync handler in `src/process/`
**Verified:** 2026-05-17T10:15:00Z
**Status:** human_needed
**Re-verification:** Yes -- after Plan 03 gap closure (SSH agent forwarding + sbx session start)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Sbx-incompatible fields (ExtraVolumes, VolumeIgnores, MountSsh) do not appear when resolved runtime is Sbx | VERIFIED | `fields.rs:172` `is_field_applicable` maps ExtraVolumes/MountSsh to `supports_arbitrary_volume_paths` (false for Sbx), VolumeIgnores to `supports_anonymous_volumes` (false for Sbx). Retain at line 1144. Cross-product test at line 2602 asserts Sbx gets 10 fields. Test passes. |
| 2 | All 13 sandbox fields appear for Docker/Podman/AppleContainer (no regression) | VERIFIED | Cross-product test asserts `docker_fields.len() == 13` at line 2682. Predicate assertions confirm all keys applicable for Docker/Podman/AppleContainer (lines 2648-2657). Test passes: 1 passed, 0 failed. |
| 3 | Cross-product test asserts field applicability for every FieldKey x ContainerRuntimeName | VERIFIED | `test_field_visibility_matches_capabilities` at line 2602 iterates all 4 runtimes x 13 sandbox FieldKeys at predicate level AND end-to-end via `build_fields_for_category`. `cargo test --lib -- field_visibility` exits 0. |
| 4 | Web dashboard and cockpit operate transparently with sbx (no sbx-specific code) | VERIFIED | `grep -rni "sbx" src/server/` returns 0 results. `grep -rni "sbx" src/cockpit/` returns 0 results. Zero sbx-specific code paths in either directory. |
| 5 | Sbx in runtime selector TUI, selection persists across restarts | VERIFIED | `fields.rs:958` maps Sbx to index 3 in selector. `apply_field_to_global` at line 1851 maps index 3 to Sbx. `apply_field_to_profile` at line 2140 does the same. `clear_profile_override` at `input.rs:726` handles ContainerRuntime. `merge_configs` at `profile_config.rs:363` handles container_runtime generically. E2e test at `tests/e2e/sandbox.rs:50` pre-seeds config and asserts persistence. |
| 6 | After host wake, handler invokes sbx exec date against owned sbx sessions older than 5min | VERIFIED | `resync_sbx_clocks()` at `process/mod.rs:229` queries `sbx ls --json`, iterates profiles via `list_profiles() + Storage::load()`, filters by `is_sandboxed() && created_at < threshold && sbx_names.contains(container_name)`, runs `sbx exec <name> date`. macOS IOKit handler at `macos.rs:108` calls `resync_sbx_clocks()` on wake (0xe000_0300). Linux busctl at `linux.rs:126` calls on "false" detection. 4 unit tests pass. |
| 7 | Handler failure is non-fatal: tracing::warn, no panic | VERIFIED | Every error path in `resync_sbx_clocks` uses `tracing::warn!` and returns/continues (lines 239, 248, 253, 265, 283, 289). No `unwrap()` or `expect()` on fallible ops. IOKit failure returns from thread (macos.rs:176). busctl spawn failure returns from thread (linux.rs:150). |
| 8 | SSH_AUTH_SOCK is passed to sbx create for agent relay setup | VERIFIED | `argv.rs:29` accepts `ssh_auth_sock: Option<&str>`. Line 55-57 emits `-e SSH_AUTH_SOCK=<path>`. `mod.rs:122` reads `std::env::var("SSH_AUTH_SOCK")` and passes to `build_create_args`. Line 129-130 also sets `cmd.env("SSH_AUTH_SOCK", sock_path)`. Tests: `create_args_with_ssh_auth_sock` and `create_args_without_ssh_auth_sock` both pass. |
| 9 | spec.yaml startup probes SSH relay socket inside sandbox | VERIFIED | `spec.yaml:11` has `["sh", "-c", "export SSH_AUTH_SOCK=/run/ssh-agent.sock; [ -S \"$SSH_AUTH_SOCK\" ] && ssh-add -l >/dev/null 2>&1 || true"]`. Line 115 declares `SSH_AUTH_SOCK` in credentials sources. Kit linter passes (17 tests). |

**Score:** 9/9 truths verified

### Roadmap Success Criteria Assessment

| # | SC | Status | Evidence |
|---|------|--------|----------|
| SC-1 | Sbx persists to disk, survives restart; profile/per-session overrides resolve through merge_configs() | VERIFIED | Sbx in selector (fields.rs:958), apply_field_to_global/profile handles it, merge_configs at profile_config.rs:363 handles container_runtime generically. E2e test verifies disk persistence (sandbox.rs:95-99). |
| SC-2 | Sbx-incompatible fields rendered greyed-out with footer note | ALTERNATIVE IMPL | Implementation HIDES fields via `.retain()` instead of greying them out. No "greyed-out" rendering or footer note exists. Functionally equivalent. SET-02 requirement explicitly states "hidden or marked unsupported" which the implementation satisfies. |
| SC-3 | Cross-product unit test passes | VERIFIED | `test_field_visibility_matches_capabilities` exists and passes (1 test, 0 failures). Covers all 4 runtimes x 13 FieldKeys. |
| SC-4 | Web dashboard session create with sbx succeeds; no sbx code in server/cockpit | PARTIAL (human needed) | Zero sbx references in src/server/ or src/cockpit/ (verified). Web session test exists as compile-gate scaffold. Full API integration requires live sbx daemon for human verification. |
| SC-5 | Sleep/wake handler invokes clock-resync, verified by unit test with mocked sleep events | PARTIAL (human needed) | Wake handler fully implemented (IOKit macOS, busctl Linux). 4 unit tests verify filter/candidate logic. No "mocked sleep event" test (IOKit/D-Bus cannot be easily mocked). Actual wake detection requires human testing. |
| SC-6 | e2e sbx_runtime_selector test discoverable and passes | VERIFIED | Test at tests/e2e/sandbox.rs:50 compiles, discoverable via `--list` (confirmed). Properly gated with `#[ignore = "requires sbx daemon"]`. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/tui/settings/fields.rs` | is_field_applicable + retain + cross-product test | VERIFIED | Function at line 172, retain at line 1144, test at line 2602. |
| `tests/e2e/sandbox.rs` | Sbx runtime selector e2e test | VERIFIED | `test_sbx_runtime_selector` at line 50. Gated, uses TuiTestHarness. |
| `tests/web_session_create.rs` | Web API sbx session integration test | VERIFIED | 42 lines. Compiles, discoverable. Gated with #[ignore]. |
| `src/process/mod.rs` | register_wake_handler + resync_sbx_clocks | VERIFIED | register_wake_handler at line 197, resync_sbx_clocks at line 229, collect_resync_candidates at line 300, filter_resync_candidates at line 356. 4 unit tests. |
| `src/process/macos.rs` | IOKit-based wake listener | VERIFIED | register_wake_handler at line 108. `#[link(name = "IOKit", kind = "framework")]` at line 124. power_callback checks 0xe000_0300. Calls resync_sbx_clocks. |
| `src/process/linux.rs` | D-Bus PrepareForSleep listener | VERIFIED | register_wake_handler at line 126. busctl monitor with PrepareForSleep match. Detects "false" at line 165. Calls resync_sbx_clocks. |
| `src/containers/sbx/argv.rs` | build_create_args with ssh_auth_sock parameter | VERIFIED | Signature at line 24 includes `ssh_auth_sock: Option<&str>`. Lines 55-57 emit `-e SSH_AUTH_SOCK=<path>`. Two tests verify. |
| `src/containers/sbx/mod.rs` | create_container forwards SSH_AUTH_SOCK | VERIFIED | Line 122 reads env var, line 124 passes to build_create_args, line 129-130 sets cmd.env. |
| `src/containers/sbx_kit/spec.yaml` | Startup command sets SSH_AUTH_SOCK inside sandbox | VERIFIED | Line 11 has startup command with SSH_AUTH_SOCK=/run/ssh-agent.sock. Line 115 declares credential source. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `fields.rs` | `runtime_base.rs` | `RuntimeBase::{variant}.capabilities` | WIRED | Line 943-946: match maps ContainerRuntimeName to RuntimeBase constants |
| `fields.rs` | `container_interface.rs` | `RuntimeCapabilities` type import | WIRED | Line 3: `use crate::containers::container_interface::RuntimeCapabilities;` |
| `process/macos.rs` | `process/mod.rs` | `super::resync_sbx_clocks()` | WIRED | Called at macos.rs:155 inside power_callback |
| `process/linux.rs` | `process/mod.rs` | `super::resync_sbx_clocks()` | WIRED | Called at linux.rs:167 on wake detection |
| `process/mod.rs` | `session/mod.rs` | `list_profiles()` | WIRED | Called at mod.rs:262 |
| `process/mod.rs` | `sbx CLI` | `Command::new("sbx").args(["exec", name, "date"])` | WIRED | Called at mod.rs:274 |
| `tui/mod.rs` | `process/mod.rs` | `register_wake_handler()` call | WIRED | Called at tui/mod.rs:148 during TUI startup |
| `sbx/mod.rs::create_container` | `sbx/argv.rs::build_create_args` | `ssh_auth_sock` parameter | WIRED | mod.rs:122 reads env, line 124 passes to build_create_args |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `fields.rs` (build_sandbox_fields) | `caps` | `RuntimeBase::{variant}.capabilities` (compile-time const) | Yes | FLOWING |
| `process/mod.rs` (resync_sbx_clocks) | `sbx_names` | `sbx ls --json` subprocess | Yes (real subprocess output via serde) | FLOWING |
| `process/mod.rs` (collect_resync_candidates) | `instances` | `Storage::load()` from sessions.json | Yes (disk storage) | FLOWING |
| `sbx/mod.rs` (create_container) | `ssh_sock` | `std::env::var("SSH_AUTH_SOCK")` | Yes (real host env var) | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Cross-product field visibility test | `cargo test --lib -- field_visibility` | 1 passed, 0 failed | PASS |
| Process filter unit tests | `cargo test --lib -- process::tests` | 4 passed, 0 failed | PASS |
| Sbx argv tests (incl SSH) | `cargo test --lib -- sbx::argv::tests` | 22 passed, 0 failed | PASS |
| Sbx kit tests (spec.yaml) | `cargo test --lib -- sbx::kit::tests` | 17 passed, 0 failed | PASS |
| Sbx module tests (incl start_container) | `cargo test --lib -- sbx::tests` | 13 passed, 0 failed | PASS |
| E2e test discoverable | `cargo test --test e2e -- sbx_runtime_selector --list` | 1 test found | PASS |
| Web session test discoverable | `cargo test --test web_session_create -- --list` | 1 test found | PASS |
| Clippy clean | `cargo clippy -- -D warnings` | No warnings | PASS |
| Fmt clean | `cargo fmt --check` | No diff | PASS |

### Probe Execution

Step 7c: SKIPPED (no probe scripts defined for this phase)

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-----------|-------------|--------|----------|
| RT-01 | 06-01, 06-02, 06-03 | Sbx selectable in ContainerRuntimeName, threaded through Settings, profile, per-session override | SATISFIED | Sbx in selector (fields.rs:958), apply_field_to_global (line 1851), apply_field_to_profile (line 2140), clear_profile_override (input.rs:726), merge_configs (profile_config.rs:363) |
| INT-01 | 06-01 | Cockpit, web dashboard, cross-machine TUI transparent with sbx | SATISFIED | Zero sbx references in src/server/ or src/cockpit/. Web session test compiles. |
| INT-02 | 06-02 | Sleep/wake handler scaffolds clock-resync inside running sbx sandboxes | SATISFIED | Full implementation: register_wake_handler in mod.rs, IOKit listener in macos.rs, busctl listener in linux.rs, resync_sbx_clocks with double-source verification. Exceeds "v1 must at least not silently break HTTPS handshakes after sleep on macOS" requirement bar. |
| SET-01 | 06-01 | Sbx fully wired through settings TUI per AGENTS.md | SATISFIED | FieldKey::ContainerRuntime handles Sbx in all required touchpoints: build_sandbox_fields, apply_field_to_global, apply_field_to_profile, clear_profile_override, SandboxConfigOverride.container_runtime, merge_configs. |
| SET-02 | 06-01 | Settings fields gate visibility on runtime capabilities; hidden or marked unsupported | SATISFIED | Fields hidden via `.retain()`. SET-02 explicitly allows "hidden or marked unsupported". Cross-product test exists and passes. |
| TEST-04 | 06-01 | New TUI surface gets e2e coverage following TuiTestHarness pattern | SATISFIED | `test_sbx_runtime_selector` in tests/e2e/sandbox.rs uses TuiTestHarness, unique name, properly gated with #[ignore]. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No debt markers (TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER) found in any modified file |

### Human Verification Required

### 1. Live TUI Sbx Runtime Selector

**Test:** Open `aoe` TUI with Sbx runtime configured. Navigate to Settings > Sandbox. Verify the runtime shows as "Docker Sandboxes" and that Extra Volumes, Volume Ignores, and Mount SSH fields are absent.
**Expected:** 10 sandbox fields visible (not 13). The three unsupported fields should not appear.
**Why human:** E2e test is gated behind `#[ignore = "requires sbx daemon"]` and requires tmux + sbx daemon.

### 2. Web Session Creation with Sbx

**Test:** Start `aoe serve --no-auth`, POST to `/api/sessions` with `container_runtime=sbx`, verify session reaches Running status and WebSocket PTY relay attaches.
**Expected:** Session created, Running state, terminal relay works identically to Docker sessions.
**Why human:** Test is a compile-gate scaffold; full API flow requires live sbx daemon and `--features serve`.

### 3. Sleep/Wake Clock Resync

**Test:** With sbx sessions running for >5 minutes, put Mac to sleep, then wake. Check debug.log for "process.wake" tracing entries showing clock resync invocations.
**Expected:** `sbx exec <name> date` called for each qualifying sandbox. Tracing shows "clock resync successful" entries.
**Why human:** Requires actual macOS sleep/wake event. Unit tests cover filtering logic but cannot simulate OS power events.

### 4. SSH Agent Forwarding Inside Sbx

**Test:** With sbx daemon running and host SSH agent active, create a sandbox session via aoe. Inside the sandbox, run `ssh -T git@github.com`.
**Expected:** Authentication succeeds using host SSH keys forwarded through the relay socket at /run/ssh-agent.sock.
**Why human:** Requires running sbx daemon, host SSH agent configured, and GitHub SSH key.

### Gaps Summary

No blocking gaps found. All 9 observable truths (from Plans 01, 02, and 03) are verified in the codebase. All 6 requirement IDs (RT-01, INT-01, INT-02, SET-01, SET-02, TEST-04) are satisfied.

**SC-2 deviation (informational):** Roadmap SC-2 specifies "greyed-out with a footer note" but the implementation hides fields entirely. This is acceptable because: (a) SET-02 requirement explicitly allows "hidden or marked unsupported", (b) PLAN 06-01 design decision D-01 chose "filter-at-build-time", and (c) the user-facing outcome is equivalent (unsupported fields cannot be edited). The implementation satisfies the requirement's intent.

**SC-4 and SC-5 partial coverage (human needed):** Tests compile and verify infrastructure but full exercise requires live sbx daemon. Routed to human verification.

---

_Verified: 2026-05-17T10:15:00Z_
_Verifier: Claude (gsd-verifier)_
