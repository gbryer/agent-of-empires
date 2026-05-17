---
phase: "06"
plan: "03"
subsystem: containers/sbx
tags: [sbx, session-lifecycle, ssh-agent, gap-closure]
dependency_graph:
  requires: [06-01, 06-02]
  provides: [sbx-session-start, ssh-agent-relay]
  affects: [src/containers/sbx/mod.rs, src/containers/sbx/argv.rs, src/containers/sbx_kit/spec.yaml, src/containers/runtime.rs]
tech_stack:
  added: []
  patterns: [exponential-backoff-polling, child-process-spawn-and-kill]
key_files:
  created: []
  modified:
    - src/containers/sbx/mod.rs
    - src/containers/sbx/argv.rs
    - src/containers/sbx/kit.rs
    - src/containers/sbx_kit/spec.yaml
    - src/containers/runtime.rs
decisions:
  - "start_container spawns sbx run as background child then polls; child is killed once running confirmed"
  - "SSH_AUTH_SOCK passed both via -e argv flag (sbx create) and process env (sbx daemon relay setup)"
  - "spec.yaml startup probes relay socket without failing (exit 0 regardless)"
metrics:
  duration: "8m 12s"
  completed: "2026-05-17"
---

# Phase 06 Plan 03: Sbx Session Start and SSH Relay Gap Closure Summary

Implemented sbx run spawn with backoff polling for session start, and SSH_AUTH_SOCK forwarding to sbx create for agent relay setup.

## Task Summary

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement start_container to call sbx run and poll for running state | 329537f | src/containers/sbx/mod.rs |
| 2 | Pass SSH_AUTH_SOCK to sbx create and add startup relay command | ba63f1a | src/containers/sbx/argv.rs, mod.rs, kit.rs, spec.yaml, runtime.rs |

## Changes Made

### Task 1: start_container implementation

Replaced the no-op `start_container` with an implementation that:
- Spawns `sbx run <name>` as a background child process (stdin null, stderr piped)
- Polls `is_container_running(name)` with exponential backoff (200ms to 12.8s)
- On success: kills child process, returns Ok(())
- On timeout (~25s): kills child, collects stderr, returns Err with diagnostic message
- Includes tracing::info at method entry

Removed `test_start_container_is_noop`, added `test_start_container_calls_run` using a mock script that returns running JSON from ls and sleeps on run.

### Task 2: SSH_AUTH_SOCK forwarding

- Added `ssh_auth_sock: Option<&str>` parameter to `build_create_args`
- When `Some(path)`, emits `-e SSH_AUTH_SOCK=<path>` in the argv vector
- In `create_container`, reads `std::env::var("SSH_AUTH_SOCK")` and passes to both `build_create_args` and `cmd.env()`
- Updated all existing call sites (runtime.rs, kit.rs tests) to pass `None`
- Added startup command in spec.yaml that sets `SSH_AUTH_SOCK=/run/ssh-agent.sock` and probes the relay socket
- Added tests: `create_args_with_ssh_auth_sock` and `create_args_without_ssh_auth_sock`

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

- cargo test --lib -- sbx::tests: 13 passed
- cargo test --lib -- sbx::argv::tests: 22 passed
- cargo test --lib -- sbx::kit::tests: 17 passed
- cargo clippy -- -D warnings: no warnings
- cargo fmt --check: no diff

## Self-Check: PASSED
