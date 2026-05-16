---
phase: 06-settings-tui-cross-machine-integration-and-sleep-wake
plan: 02
subsystem: process/wake-handler
tags: [sleep-wake, sbx, clock-resync, iokit, dbus]
dependency_graph:
  requires: []
  provides: [register_wake_handler, resync_sbx_clocks]
  affects: [src/process/mod.rs, src/process/macos.rs, src/process/linux.rs]
tech_stack:
  added: [IOKit FFI (macOS), busctl/D-Bus (Linux)]
  patterns: [platform-conditional dispatch, fire-and-forget resync, double-source verification]
key_files:
  created: []
  modified:
    - src/process/mod.rs
    - src/process/macos.rs
    - src/process/linux.rs
decisions:
  - Resync logic (resync_sbx_clocks) lives in mod.rs as platform-agnostic code; only wake detection is per-platform
  - IOKit FFI types declared inline in the spawned thread closure to avoid polluting module scope
  - busctl monitor used on Linux instead of libdbus crate to avoid adding a native dependency
  - Double-source verification: container must appear in both sessions.json AND sbx ls output
metrics:
  duration: 6m 42s
  completed: 2026-05-16T12:56:59Z
---

# Phase 06 Plan 02: Post-Wake Clock-Resync Handler Summary

Platform-specific wake detection (IOKit on macOS, D-Bus busctl on Linux) triggers sbx exec date on all aoe-owned sandboxes older than 5 minutes, preventing HTTPS handshake failures from stale guest clocks after host sleep.

## Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement resync_sbx_clocks and register_wake_handler | e5bd451 | src/process/mod.rs, src/process/macos.rs, src/process/linux.rs |

## Implementation Details

### src/process/mod.rs
- `pub fn register_wake_handler()`: three-branch `#[cfg]` dispatch following existing pattern (macOS/Linux/unsupported no-op)
- `pub(crate) fn resync_sbx_clocks()`: queries `sbx ls --json` via serde, iterates all profiles via `list_profiles() + Storage::load()`, filters to sandboxed instances older than 5 minutes whose container_name appears in the sbx name set, then runs `sbx exec <name> date` for each
- `SbxLsOutput` / `SbxSandboxEntry`: local deserialization structs with `#[serde(default)]`
- `collect_resync_candidates()`: extracted helper for profile iteration and filtering
- `filter_resync_candidates()`: `#[cfg(test)]`-only pure function for unit testing the filter logic

### src/process/macos.rs
- `pub(super) fn register_wake_handler()`: spawns a thread with IOKit FFI
- `#[link(name = "IOKit", kind = "framework")]` extern block with `IORegisterForSystemPower`, `IONotificationPortGetRunLoopSource`
- `#[link(name = "CoreFoundation", kind = "framework")]` extern block with `CFRunLoopGetCurrent`, `CFRunLoopAddSource`, `CFRunLoopRun`, `kCFRunLoopDefaultMode`
- `power_callback` extern "C" function checks for `0xe000_0300` (kIOMessageSystemHasPoweredOn)
- Logs warning and returns on registration failure (root_port == 0)

### src/process/linux.rs
- `pub(super) fn register_wake_handler()`: spawns a thread running `busctl monitor --system --match type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'`
- Reads stdout line-by-line; when a line contains "false" (PrepareForSleep(false) = waking), calls `resync_sbx_clocks()`
- Logs warning on spawn failure; thread exits silently if busctl exits

### Unit Tests (4 tests)
- `filter_skips_non_sandboxed_instances`: verifies non-sandboxed instances are excluded
- `filter_skips_recent_instances`: verifies instances created within 5 minutes are excluded
- `filter_skips_instances_not_in_sbx_ls`: verifies instances not in sbx ls output are excluded
- `filter_includes_matching_candidates`: verifies correct candidates are included (sandboxed, old enough, in sbx ls)

## Verification Results

- `cargo build`: PASSED
- `cargo test --lib -- process::tests`: 4 tests PASSED
- `cargo clippy -- -D warnings`: PASSED (no warnings)
- `cargo fmt --check`: PASSED (no diff)
- All tracing uses `target: "process.wake"`: confirmed via grep
- No `#[cfg(target_os)]` outside `src/process/`: confirmed

## Deviations from Plan

None; plan executed exactly as written.

## Threat Surface Compliance

Threat mitigations from the plan are implemented:
- T-06-04: `SbxLsOutput` and `SbxSandboxEntry` use `#[serde(default)]` for defensive parsing
- T-06-05: Double-source verification; container names must appear in both sessions.json AND sbx ls output
- T-06-06: busctl runs in a dedicated thread, isolated from the main application
- T-06-07: IOKit power notifications use standard unprivileged API

No new threat surface was introduced beyond what the plan's threat model covers.

## Self-Check: PASSED
