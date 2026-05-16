---
phase: "05"
plan: "02"
subsystem: containers/sbx, session
tags: [sbx, runtime, dispatch, integration-test, manual-test, agent-name]
dependency_graph:
  requires: [05-01-sbx-action-verbs]
  provides: [sbx-agent-name-threading, sbx-integration-test, sbx-manual-test-plan, sbx-public-api]
  affects: [containers/container_interface.rs, containers/runtime.rs, session/container_config.rs]
tech_stack:
  added: []
  patterns: [agent-name-threading, capability-gated-port-publish, ignore-gated-integration-test]
key_files:
  created:
    - tests/sbx_integration.rs
    - .planning/phases/05-full-sbxruntime-integration-subprocess-lifecycle-port-publis/05-MANUAL-TESTS.md
  modified:
    - src/containers/container_interface.rs
    - src/containers/runtime.rs
    - src/containers/runtime_base.rs
    - src/containers/mod.rs
    - src/containers/sbx/mod.rs
    - src/containers/sbx/parse.rs
    - src/containers/sbx/ports.rs
    - src/containers/sbx/argv.rs
    - src/containers/sbx/kit.rs
    - src/session/container_config.rs
    - tests/sandbox_integration.rs
decisions:
  - "Task 1 (dispatch arm wiring) verified as already done by Plan 01 deviation; skipped"
  - "publish_ports stays in runtime.rs create_container Sbx arm (from Plan 01) rather than duplicating in instance.rs"
  - "agent_name added to ContainerConfig with fallback to container name parsing for backward compat"
  - "SbxRuntime, parse, and ports modules made pub for integration test access"
metrics:
  duration: "11m 19s"
  completed: "2026-05-16T11:03:05Z"
  tasks_completed: 3
  tasks_total: 3
  tests_added: 3
  files_changed: 13
---

# Phase 05 Plan 02: Sbx Dispatch Integration, Session Layer Threading, and Test Plan Summary

ContainerConfig gains agent_name field threaded from session layer; SbxRuntime API made public for integration testing; 3 lifecycle tests and a manual test plan document created.

## What Was Done

### Task 1: Verify dispatch arms (skipped, already done)

All nine RuntimeKind::Sbx dispatch arms in runtime.rs were already wired by Plan 01 as a deviation (required to pass clippy dead_code). Verified:
1. does_container_exist delegates to SbxRuntime
2. is_container_running delegates to SbxRuntime
3. is_daemon_running delegates to SbxRuntime
4. create_container delegates with agent_name extraction and port publishing
5. start_container delegates (no-op)
6. stop_container delegates
7. remove delegates
8. exec delegates with (name, cmd, None, &[], false, false)
9. batch_running_states delegates

### Task 2: agent_name field and session layer integration

- Added `agent_name: Option<String>` to `ContainerConfig` in container_interface.rs
- Updated runtime.rs `create_container` Sbx arm to prefer `config.agent_name` over parsing the container naming convention
- Populated `agent_name: Some(tool.to_string())` in session/container_config.rs `build_container_config`
- Updated all ContainerConfig struct literals across 8 files to include `agent_name: None`

### Task 3: Integration test and manual test document

- Created `tests/sbx_integration.rs` with 3 `#[ignore]`-gated tests:
  - `lifecycle_create_exec_stop_rm`: full create/exec/publish_ports/stop/remove lifecycle
  - `batch_running_states_includes_created_sandbox`: verifies sandbox appears in batch query
  - `is_daemon_running_returns_true_when_sbx_configured`: daemon health check
- Made `SbxRuntime` struct `pub` (was `pub(crate)`) for integration test access
- Made `parse` and `ports` modules `pub` (were `pub(crate)`)
- Made `SbxDaemonHealth`, `SbxListOutput`, `SbxSandboxInfo`, `find_by_name`, `PortPublishResult` pub
- Added `Default` impl for `SbxRuntime` (required by clippy after pub promotion)
- Created `05-MANUAL-TESTS.md` with sections: Cockpit Session Creation, Web Dashboard Session, Cross-Machine Session, Sleep/Wake Clock Drift, Disk Usage Growth

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 1 already complete from Plan 01**
- **Found during:** Task 1 verification
- **Issue:** Plan 01 wired all dispatch arms as a deviation to satisfy clippy dead_code
- **Fix:** Skipped Task 1 execution, proceeded to Task 2
- **Impact:** None; acceptance criteria still met

**2. [Rule 2 - Missing functionality] publish_ports placement**
- **Found during:** Task 2 analysis
- **Issue:** Plan specified adding publish_ports in instance.rs, but it already exists in runtime.rs create_container Sbx arm (from Plan 01). Adding it to instance.rs would double-publish.
- **Fix:** Left publish_ports in runtime.rs where it already works correctly with capability awareness
- **Impact:** instance.rs does not contain an explicit publish_ports call; the functionality is handled transparently in the dispatch layer

**3. [Rule 3 - Blocking] SbxRuntime visibility for integration test**
- **Found during:** Task 3 implementation
- **Issue:** SbxRuntime was pub(crate), inaccessible from integration tests
- **Fix:** Made SbxRuntime pub, along with parse and ports modules and their key types
- **Files modified:** src/containers/sbx/mod.rs, src/containers/sbx/parse.rs, src/containers/sbx/ports.rs
- **Commit:** f73c2bb

**4. [Rule 3 - Blocking] clippy new_without_default on pub SbxRuntime**
- **Found during:** Task 3 clippy check
- **Issue:** Making SbxRuntime pub triggers clippy's new_without_default lint
- **Fix:** Added `impl Default for SbxRuntime` delegating to `Self::new()`
- **Files modified:** src/containers/sbx/mod.rs
- **Commit:** f73c2bb

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 2 | ccdeb6e | feat(05-02): add agent_name field to ContainerConfig and thread from session layer |
| 3 | f73c2bb | feat(05-02): add sbx integration test and manual test plan document |

## Test Results

- 3 new integration tests (all #[ignore]-gated, require sbx daemon)
- 88 container module unit tests passing
- 1694 lib tests passing (6 pre-existing failures in unrelated tmux/container_config modules)
- cargo clippy clean, cargo fmt clean

## Self-Check: PASSED

- tests/sbx_integration.rs: FOUND
- 05-MANUAL-TESTS.md: FOUND
- 05-02-SUMMARY.md: FOUND
- Commit ccdeb6e: FOUND
- Commit f73c2bb: FOUND
- agent_name field in ContainerConfig: FOUND
- publish_ports in runtime.rs: FOUND
- 3 #[ignore]-gated tests: FOUND
