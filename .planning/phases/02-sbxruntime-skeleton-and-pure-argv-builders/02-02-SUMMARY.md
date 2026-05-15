---
phase: 02-sbxruntime-skeleton-and-pure-argv-builders
plan: 02
subsystem: container-runtime
tags: [sbx, rust, container-runtime, dispatch-tests, sc-4]

# Dependency graph
requires:
  - phase: 02-sbxruntime-skeleton-and-pure-argv-builders
    plan: 01
    provides: "Storage field sbx: Option<SbxRuntime>; four wired RuntimeKind::Sbx dispatch arms (is_available, capabilities, exec_command, build_create_args); pure argv builders"
provides:
  - "Strengthened test_sbx_dispatch_arms_return_safe_stubs (5 discriminating assertions against Phase 1 sentinel)"
  - "New test_sbx_build_create_args_emits_sbx_shape_not_docker_shape (proves build_create_args emits sbx CLI shape, not Docker shape)"
  - "Refreshed Phase 1 stub comments on action-verb arms to point at correct deferral target (Phase 5, not Phase 2 RT-04)"
affects: [phase-03, phase-04, phase-05]

tech-stack:
  added: []
  patterns:
    - "Discriminating test assertions: prove the rewire actually happened, not just that a substring appears (assert against the prior sentinel string + absence of sentinel comment)"
    - "Stub-comment hygiene: when a stale invariant is replaced, update the comments that reference it in the same commit class"

key-files:
  created: []
  modified:
    - "src/containers/runtime.rs (test strengthening + new test + stub-comment refresh)"

key-decisions:
  - "Scope reduction: the original Plan 02-02 dispatch arm wiring (storage field + 3 dispatch arms) was pulled forward into Plan 02-01 by that executor, because the cargo-husky pre-commit hook runs `cargo clippy -- -D warnings` and AGENTS.md forbids `#[allow(dead_code)]`; the new sbx module could not commit in 02-01 without immediately being wired. This plan delivers only the test-quality work that remained."
  - "Stub-comment refresh: kept the action-verb Phase 1 stubs (does_container_exist, is_container_running, batch_running_states) per D-05/D-06 but rewrote their comments. The old text claimed 'no sbx container can exist while is_available returns false' which 02-01's pull-forward invalidated (is_available now does a real PATH probe), and pointed at 'Phase 2 (RT-04)' for replacement when RT-04 actually lives in Phase 5 per CONTEXT.md."

requirements-completed:
  - PHASE-2-SCAFFOLDING
  - SC-4

# Metrics
duration: ~30min
completed: 2026-05-15
---

# Phase 02 Plan 02: Wire SbxRuntime into ContainerRuntime Dispatch — Summary

**Reduced-scope follow-up to 02-01: the dispatch arm wiring (D-03 Option A storage field + 3 dispatch arms) shipped in 02-01 to clear the clippy -D warnings gate, leaving only the test-quality work (strengthened sbx dispatch test + new build_create_args shape test + stub-comment refresh) for 02-02. 56 containers tests pass; cargo fmt and clippy clean.**

## Performance

- **Duration:** ~30 minutes
- **Started:** 2026-05-15
- **Completed:** 2026-05-15
- **Tasks:** 1 logical task split into 2 commits (test + refactor)
- **Files modified:** 1 (`src/containers/runtime.rs`, +60 -16 lines)

## Accomplishments

- Strengthened `test_sbx_dispatch_arms_return_safe_stubs` with five discriminating assertions that prove `exec_command` returns the new build_exec_args output (`sbx exec -i -t foo bar`) rather than the Phase 1 sentinel (`sbx exec foo /* Phase 1 stub: cmd and options dropped */`). Previous assertions `cmd.contains("sbx")` and `cmd.contains("foo")` held against both strings, so they did not actually exercise the rewire.
- Added `test_sbx_build_create_args_emits_sbx_shape_not_docker_shape` so the new SC-4 dispatch arm has a discriminating in-runtime.rs test (the in-sbx/argv tests cover the builder; this test covers the runtime-level routing).
- Refreshed the comment block above the dispatch test (it referenced the removed `test_sbx_is_available_returns_false_unconditionally` test and the dead "is_available short-circuits to false" invariant) to accurately describe the post-Phase-2 routing.
- Refreshed three stale stub-arm comments on `does_container_exist`, `is_container_running`, and `batch_running_states`. The old text claimed "no sbx container can exist while is_available returns false" (invalidated by 02-01's real PATH probe) and pointed at "Phase 2 (RT-04)" for replacement when RT-04 is a Phase 5 requirement per D-05/D-06.

## Task Commits

1. **Strengthen sbx dispatch tests + add build_create_args shape test** — `d10caab` (test)
2. **Refresh stale Phase 1 stub comments on action-verb arms** — `8aaeb96` (refactor)

## Files Created/Modified

- **`src/containers/runtime.rs`** (modified, +60 -16) —
  - `test_sbx_dispatch_arms_return_safe_stubs` now asserts the 5 discriminating conditions (`cmd.starts_with("sbx exec ")`, `cmd.contains("bar")`, `!cmd.contains("/* Phase 1")`, `!cmd.contains("dropped")`, plus the existing `cmd.contains("foo")`).
  - New `test_sbx_build_create_args_emits_sbx_shape_not_docker_shape` constructs an empty-ish `ContainerConfig` and asserts the argv shape (`create shell --name ... --template alpine:latest` etc.) and the absence of the Docker shape (`run`, `-d`).
  - Comment block above the dispatch test rewritten to reflect the post-Phase-2 reality: D-05/D-06 keeps three action-verb arms as Phase 1 stubs; the four routed arms (is_available, capabilities, exec_command, build_create_args) are covered by their own tests.
  - Three stub-arm comments rewritten to point at the correct deferral target (Phase 5 per D-05/D-06) and acknowledge that the arms are now reachable on hosts with an `sbx` binary.

## Test Results

```
cargo test -p agent-of-empires --lib containers::    PASS  (56/56)
  prev: 55 tests; +1 from test_sbx_build_create_args_emits_sbx_shape_not_docker_shape

cargo fmt --check       PASS
cargo clippy -- -D warnings   PASS
cargo-husky pre-commit hook   PASS (both commits)
```

## Success Criteria Coverage

| ID | Description | Status | Evidence |
|----|-------------|--------|----------|
| SC-4 | RuntimeKind::Sbx dispatch routes correctly without Docker/Podman/AppleContainer regression | PASS | All 56 containers tests pass; the two new/strengthened tests exercise the routing without weak `cmd.contains("sbx")` assertions that the Phase 1 sentinel also satisfied. |

SC-1, SC-2, SC-3 were delivered by 02-01.

## Deviations from Plan

### Scope reduction (carried over from 02-01)

The original Plan 02-02 specified wiring three `RuntimeKind::Sbx` dispatch arms in `src/containers/runtime.rs` (Action steps 1-7), plus the test work (steps 8-9). Plan 02-01's executor pulled steps 1-7 forward because:

- Plan 02-01's deliverable (the new `src/containers/sbx/` module with `SbxRuntime`, `build_create_args`, `build_exec_args`, `build_ports_args`) cannot pass `cargo clippy -- -D warnings` in isolation; nothing at the lib level reads any of those items until the runtime.rs dispatch arms are updated to call them.
- AGENTS.md forbids `#[allow(dead_code)]`. The plan acceptance criterion for 02-01 includes a clippy-clean build, so the pull-forward was a Rule 3 (blocking) fix.

What 02-02 delivered (the test-quality work) was the only scope remaining after that pull-forward.

### Auto-fixed issues

**1. [Rule 2 - Missing critical functionality] Stub-arm comments still claimed `is_available` short-circuits to false**

- **Found during:** Acceptance-criteria grep (`grep -c '/\* Phase 1 stub' src/containers/runtime.rs returns 0`)
- **Issue:** Three `RuntimeKind::Sbx` arms (`does_container_exist`, `is_container_running`, `batch_running_states`) still carried "Phase 1 stub; no sbx container can exist while is_available returns false. Phase 2 (RT-04) replaces..." comments. Both halves of that text are wrong now: is_available does a real PATH probe (02-01), and RT-04 is a Phase 5 requirement (per CONTEXT.md D-05/D-06), not Phase 2.
- **Fix:** Rewrote each of the three comments to point at D-05/D-06 + Phase 5 and acknowledge that the arms are reachable post-02-01. Committed as `refactor(02-02): refresh stale Phase 1 stub comments on action-verb arms` (`8aaeb96`).
- **Files modified:** `src/containers/runtime.rs`
- **Commit:** `8aaeb96`

The objective explicitly asked for a refresh at lines 559-564 (the comment above the dispatch test); the three action-verb stub comments are the same class of staleness and were updated under the same "correctness of code documentation" Rule 2 banner.

### Acceptance criteria drift

Two of the plan's grep-based acceptance criteria from PLAN 02-02 (which were authored before the 02-01 pull-forward) are now N/A because the artefacts they assert against were created in 02-01:

- `grep -c 'pub(crate) sbx: Option<sbx::SbxRuntime>' src/containers/runtime.rs returns 1` — still returns 1, but the line was added by 02-01's commit `77e3e21`, not by 02-02.
- `grep -c 'sbx::argv::build_create_args' src/containers/runtime.rs returns 1` — returns 2 in 02-02 (1 in the dispatch arm from 02-01's `927344b`, 1 added by the new test in `d10caab`).
- `grep -c 'sbx::argv::build_exec_args' src/containers/runtime.rs returns 1` — returns 2 (1 in 02-01 dispatch arm, 1 in the existing dispatch test).
- `grep -c 'self.sbx.as_ref().expect(' src/containers/runtime.rs returns at least 1` — returns 0 because rustfmt wraps the chain across newlines; `grep -c 'expect("ContainerRuntime::sbx() invariant' src/containers/runtime.rs` returns 2, which proves the underlying invariant gate is in place for both dispatch arms.

All other acceptance criteria from PLAN 02-02 (test names, no `#[allow(dead_code)]`, fmt + clippy + cargo test green) pass as written.

## Forward Coverage Notes

- **Phase 3 (RT-04, RT-06):** Unaffected; 02-01 already wires the build_create_args dispatch arm. Phase 3 will enforce `host_path == container_path` upstream and add capability gating on `pull_image`/`ensure_image`.
- **Phase 5 (RT-02, LIFE-01, TEST-01):** The three Phase 1 stub action verbs (`does_container_exist`, `is_container_running`, `batch_running_states`) are now correctly documented as deferred to Phase 5. Phase 5 will wire them via real subprocess calls and use `build_ports_args`, which still has no internal lib caller.

## Known Stubs

The three action-verb arms (`does_container_exist`, `is_container_running`, `batch_running_states`) remain as Phase 1 stubs (return false / empty HashMap) per D-05/D-06. This is intentional and documented in the rewritten comments; Phase 5 will resolve them. None of these stubs prevent the Plan 02 goal of "ContainerRuntime built via ::sbx() routes through SbxRuntime + sbx::argv" from being achieved.

## Self-Check: PASSED

- File `src/containers/runtime.rs` modified (FOUND: `M src/containers/runtime.rs` before this commit; `git diff --stat 2b80ec5..HEAD` shows `1 file changed, 60 insertions(+), 16 deletions(-)`)
- Commit `d10caab` exists in git log (FOUND)
- Commit `8aaeb96` exists in git log (FOUND)
- `test_sbx_dispatch_arms_return_safe_stubs` passes with new strict assertions (FOUND)
- `test_sbx_build_create_args_emits_sbx_shape_not_docker_shape` exists and passes (FOUND)
- 56 containers tests pass (FOUND)
- `cargo fmt --check` exits 0 (FOUND)
- `cargo clippy -- -D warnings` exits 0 (FOUND)
- cargo-husky pre-commit hook passed on both commits (FOUND)
