---
phase: "05"
plan: "01"
subsystem: containers/sbx
tags: [sbx, runtime, subprocess, parse, ports, lifecycle]
dependency_graph:
  requires: [phase-02-sbx-skeleton, phase-04-kit-materializer]
  provides: [sbx-action-verbs, sbx-parse-module, sbx-ports-module, sbx-dispatch-wiring]
  affects: [containers/runtime.rs, containers/sbx/mod.rs]
tech_stack:
  added: []
  patterns: [mock-binary-injection, exponential-backoff, first-exec-retry, per-port-error-capture]
key_files:
  created:
    - src/containers/sbx/parse.rs
    - src/containers/sbx/ports.rs
    - tests/fixtures/sbx_ls.json
  modified:
    - src/containers/sbx/mod.rs
    - src/containers/runtime.rs
decisions:
  - "Committed all three tasks as single atomic commit due to clippy dead_code enforcement requiring dispatch wiring for all new pub methods"
  - "Wired Sbx dispatch arms in runtime.rs (originally planned for 05-02) to satisfy clippy; this reduces 05-02 scope"
  - "is_daemon_running delegates to daemon_health() internally, keeping both methods reachable from the dispatch layer"
  - "Agent name for create_container extracted from aoe container naming convention (aoe_<agent>_<id>) in dispatch layer"
metrics:
  duration: "13m 26s"
  completed: "2026-05-16T10:45:47Z"
  tasks_completed: 3
  tasks_total: 3
  tests_added: 18
  files_changed: 5
---

# Phase 05 Plan 01: SbxRuntime Action Verbs, Parse Module, and Ports Module Summary

SbxRuntime fully implements all action verbs (create/exec/stop/rm/queries/daemon-health) with exponential backoff readiness probe, first-exec retry on not-ready stderr, and per-port error capture via the new parse and ports modules.

## What Was Done

### Task 1: parse.rs and JSON fixture

Created `src/containers/sbx/parse.rs` with:
- `SbxListOutput` struct with `#[serde(default)]` on the `sandboxes` Vec field
- `SbxSandboxInfo` struct with `#[serde(default)]` on all six fields (id, name, agent, status, socket_path, workspaces)
- `SbxDaemonHealth` enum with five variants: Running, NotLoggedIn, PolicyNotConfigured, NotInstalled, Unknown(String)
- `find_by_name` helper for case-sensitive sandbox lookup

Created `tests/fixtures/sbx_ls.json` with real `sbx ls --json` output shape.

### Task 2: ports.rs with PortPublishResult

Created `src/containers/sbx/ports.rs` with:
- `PortPublishResult` enum: `Ok(String)` for success, `Failed { port, stderr }` for per-port failure
- `publish_ports(&self, name, port_mappings)` method iterating each port spec individually

### Task 3: mod.rs action verbs

Expanded `src/containers/sbx/mod.rs` with:
- `run()` private helper wrapping Command::new with stderr capture
- `sandbox_status()` parsing sbx ls --json for a named sandbox's state
- `wait_for_ready()` with 7-step exponential backoff (200ms to 12.8s, ~25s total)
- `create_container(name, image, config, agent_name)` with kit materialization and readiness probe
- `start_container()` as no-op (sbx auto-starts on first exec)
- `stop_container()` and `remove()` with force flag support
- `exec()` with 3-retry loop on "not ready"/"not running" in stderr
- `does_container_exist()` and `is_container_running()` via sbx ls --json parsing
- `is_daemon_running()` delegating to daemon_health() for Running check
- `daemon_health()` with stderr inspection for typed failure reasons
- `batch_running_states()` with client-side prefix filtering

### Dispatch Wiring (Deviation)

Updated `src/containers/runtime.rs` to delegate all Sbx dispatch arms to SbxRuntime methods instead of Phase 1 stubs. This was required to pass clippy's dead_code check.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Wired Sbx dispatch arms in runtime.rs**
- **Found during:** Task 1 commit attempt
- **Issue:** clippy `-D warnings` (enforced by pre-commit hook) rejects dead code; all new pub methods on SbxRuntime triggered dead_code errors since no dispatch code called them
- **Fix:** Replaced Phase 1 stubs in runtime.rs with delegation to SbxRuntime methods. This is work originally planned for 05-02 but required now for compilation.
- **Files modified:** src/containers/runtime.rs
- **Commit:** f3378ca

**2. [Rule 3 - Blocking] Single atomic commit instead of per-task commits**
- **Found during:** Task 1 commit attempt
- **Issue:** Each task's files depend on the others (ports.rs calls run() from mod.rs; mod.rs uses parse.rs; clippy requires runtime.rs to call all methods). Independent commits would each fail clippy.
- **Fix:** Combined all three tasks into one commit that passes all checks atomically.
- **Commit:** f3378ca

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1+2+3 | f3378ca | feat(05-01): implement SbxRuntime action verbs, parse module, and ports module |

## Test Results

- 18 new test functions across 3 files (3 parse + 2 ports + 13 mod)
- Total sbx module tests: 55 passing
- All mock-script-based (no real sbx binary required)
- Pre-existing failures in unrelated modules (tmux, container_config) confirmed on base commit

## Self-Check: PASSED
