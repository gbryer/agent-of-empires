---
phase: 03-container-config-capability-gating-and-workspace-path-confor
plan: 02
subsystem: containers
tags: [rust, sbx, capabilities, container-runtime, workdir, image-pull]

requires:
  - phase: 03-container-config-capability-gating-and-workspace-path-confor
    provides: "build_container_config(caps, runtime_name) + ContainerConfigError + conform_workspace_paths + conform_for_capabilities + 14 in-module tests (from plan 03-01)"
  - phase: 01-capability-surface-and-trait-foundation
    provides: "RuntimeCapabilities matrix with supports_arbitrary_volume_paths + supports_image_pull"
provides:
  - "RT-06 ensure_image capability guard at instance.rs get_container_for_instance"
  - "ContainerRuntime::name() -> ContainerRuntimeName accessor (eliminates Config::load() round-trip in build_container_config wrapper)"
  - "Instance::container_workdir(caps) canonical pure form returning conformed workdir; mockable for unit tests"
  - "Instance::container_workdir_now() convenience shim for 17 production exec-time call sites"
  - "Finalized Instance::build_container_config wrapper threading capabilities + runtime_name"
  - "SC-4 source-substring + capability-matrix tests for ensure_image gating"
  - "SC-4 companion test verifying container_workdir threads caps correctly under sbx vs Docker"
affects: [sbx-runtime-implementation, instance-lifecycle, status-pollers, session-hooks]

tech-stack:
  added: []
  patterns: ["pure-form + convenience-shim split for capability-threaded methods (pure(caps) + now() shim that fetches live caps)"]

key-files:
  created: []
  modified:
    - src/containers/runtime.rs
    - src/session/instance.rs
    - src/session/container_config.rs
    - src/cli/remove.rs
    - src/session/deletion.rs
    - src/tui/creation_poller.rs

key-decisions:
  - "Introduced ContainerRuntime::name() accessor (over Config::load round-trip) to refine Plan 03-01's temporary build_container_config wrapper"
  - "Split container_workdir into pure container_workdir(caps) form + container_workdir_now() shim; keeps 17 production call sites at a uniform `_now` append and preserves pure-function mockability"
  - "SC-4 tests live in container_config::tests (not a new tests mod in the 3164-line instance.rs), per RESEARCH Open Question #2"
  - "Source-substring + capability-matrix assertion pair chosen over mock-runtime test for ensure_image gating (no precedent for mock runtimes; assertion cost is proportional to the two-line guard)"

patterns-established:
  - "Capability-threaded methods get split into a pure(caps) form for tests and a _now() shim for production exec-time call sites that fetches caps off the live runtime"
  - "Source-substring tests via include_str! catch guard removal/movement when behavior is structural rather than observable in unit tests"

requirements-completed:
  - RT-04
  - RT-06

duration: ~45min (across two dispatches + orchestrator inline finish)
completed: 2026-05-16
---

# Phase 03 Plan 02: ensure_image guard + container_workdir capability threading

**RT-06 ensure_image now skips under sbx (no image-pull verb); container_workdir threads RuntimeCapabilities through 17 exec-time call sites via a pure(caps)+now() split so sbx exec -w / pollers see the same conformed workdir that mount-time produced.**

## Performance

- **Duration:** ~45min (two executor dispatches, then orchestrator inline finish)
- **Started:** 2026-05-15T13:25:00Z
- **Completed:** 2026-05-16T00:46:00Z
- **Tasks:** 2/2
- **Files modified:** 6

## Accomplishments

- RT-06 (D-12): `runtime.ensure_image(image)` is now gated on `runtime.capabilities().supports_image_pull` at `instance.rs:1253`. sbx silently skips the pull; Docker/Podman/AppleContainer paths unchanged. `RuntimeBase::ensure_image`'s defensive `Err(NotSupported)` remains as a safety net per D-13.
- `ContainerRuntime::name() -> ContainerRuntimeName` accessor (1:1 mapping from internal `RuntimeKind` discriminant) eliminates the `Config::load()` round-trip that Plan 03-01 left as temporary state in the `build_container_config` wrapper.
- RT-04 (D-10): `Instance::container_workdir(caps)` becomes the canonical pure form, applying `conform_workspace_paths` so exec-time consumers see the same conformed workdir that mount-time produced. `container_workdir_now()` is a convenience shim that fetches caps off the live runtime, keeping the 17 production call sites at a uniform `_now` append.
- All 17 production call sites of `container_workdir` updated across 4 files (instance.rs: 13, cli/remove.rs: 1, session/deletion.rs: 1, tui/creation_poller.rs: 2).
- SC-4 tests (3 total) in `container_config::tests`: source-substring assertion (guard placement), capability-matrix assertion (Docker/Podman/Apple = true, sbx = false), and `test_instance_container_workdir_threads_capabilities` (conformance round-trip).

## Task Commits

1. **Task 1 — RT-06 guard + ContainerRuntime::name() + SC-4 (guard) tests** — `c7e84da` (feat)
2. **Task 2 — RT-04 container_workdir capability threading across 19 call sites + SC-4 companion test** — `93fef3c` (feat)

## Files Created/Modified

- `src/containers/runtime.rs` (+14) — Added `ContainerRuntime::name() -> ContainerRuntimeName` accessor
- `src/session/instance.rs` (+38 / -16) — ensure_image guard, container_workdir pure form + _now shim, 13 call-site updates, build_container_config wrapper finalization
- `src/session/container_config.rs` (+95) — SC-4 source-substring test, capability-matrix test, container_workdir threading test
- `src/cli/remove.rs` (+1 / -1) — on-destroy hook call-site
- `src/session/deletion.rs` (+1 / -1) — run_on_destroy_hooks call-site
- `src/tui/creation_poller.rs` (+2 / -2) — on-create + on-launch hook call-sites

## Decisions Made

- **`ContainerRuntime::name()` accessor over Config::load()**: Plan 03-01 left a temporary `Config::load()` round-trip in the `build_container_config` wrapper. Adding a tiny accessor on the resolved runtime is simpler, avoids re-reading global config inside an instance method, and reads both `capabilities()` and `name()` off the same runtime resolution.
- **Pure(caps) + now() shim split for container_workdir**: Keeps the pure form mockable for unit tests while giving the 17 production exec-time call sites a uniform single-line update (`.container_workdir()` → `.container_workdir_now()`). Avoids forcing every caller to fetch `RuntimeCapabilities` locally.
- **SC-4 tests in container_config::tests (not instance.rs)**: Per RESEARCH Open Question #2, the giant 3164-line `instance.rs` already lacks a `tests` mod; adding one there adds compilation overhead with no organizational benefit. `container_config::tests` already has the conformance fixtures.

## Deviations from Plan

### Re-dispatch and inline finish

**1. [Orchestrator-level] First executor dispatch lost socket mid-execution**
- **Found during:** Wave 2 initial dispatch (agent `a08f725b7fd778f73`)
- **Issue:** Socket closed after 54 tool calls (~13 min); agent left 24 lines of uncommitted RT-06 guard work but no commits and no SUMMARY.md
- **Fix:** Discarded partial work, dropped the worktree/branch, re-dispatched fresh
- **Verification:** New worktree (`a3f60627554ea9a40`) started from same base
- **Committed in:** N/A (discarded)

**2. [Orchestrator-level] Second executor dispatch killed by network policy block**
- **Found during:** Wave 2 retry dispatch (agent `a3f60627554ea9a40`)
- **Issue:** 73 tool calls in, the agent's outbound api.anthropic.com:443 was denied by a system network policy. Task 1 had already been committed (`c7e84da`); Task 2 work (5 files, +93/-25 lines) was uncommitted but compiled cleanly.
- **Fix:** Orchestrator took ownership inline: ran `cargo fmt`, verified `cargo clippy --lib -- -D warnings` and `cargo test --lib` (1660 passed, 0 failed), committed Task 2 (`93fef3c`), wrote SUMMARY.md.
- **Verification:** Full library test suite + clippy + fmt all green before commit.
- **Committed in:** `93fef3c` (Task 2)

---

**Total deviations:** 2 orchestrator-level recovery events. No scope creep; the inline finish executed exactly the work the plan specified.
**Impact on plan:** Plan delivered all must_haves (truths + artifacts). Two infrastructure failures (one socket close, one network policy block) extended wall time but did not change scope.

## Issues Encountered

- Socket close + network policy block as documented above. Both handled per the workflow's stall/failure recovery patterns (`continue waiting`, `kill and retry`, `kill and switch to inline execution`).

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- RT-04 + RT-06 fully delivered. Phase 03 closes the gap between Phase 02's pure argv builders and the capability-driven container surface needed for sbx adoption.
- Next phase (per ROADMAP.md): Phase 04 will implement the sbx runtime itself against the now-complete capability-gated container surface. The build_container_config + container_workdir contracts are stable.
- No blockers.

---
*Phase: 03-container-config-capability-gating-and-workspace-path-confor*
*Completed: 2026-05-16*
