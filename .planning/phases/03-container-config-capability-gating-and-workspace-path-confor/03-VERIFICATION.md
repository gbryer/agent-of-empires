---
phase: 03-container-config-capability-gating-and-workspace-path-confor
verified: 2026-05-16T01:30:00Z
status: passed
score: 12/12 must-haves verified
overrides_applied: 0
---

# Phase 3: container_config Capability Gating and Workspace Path Conformance — Verification Report

**Phase Goal (ROADMAP):** `build_container_config` and `compute_volume_paths` are rewritten to consult `runtime.capabilities()` so that for `!supports_arbitrary_volume_paths` runtimes (sbx) BOTH `working_dir` AND every `VolumeMount.container_path` are rewritten consistently to the host path before `ContainerConfig` leaves the function. Inconformable configs surface a clear validation error. `ensure_image` is skipped silently when `!supports_image_pull`. Existing Docker/Podman/AppleContainer behavior is unchanged.

**Verified:** 2026-05-16T01:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

Merged from ROADMAP Phase 3 Success Criteria (4) and PLAN frontmatter must_haves (Plan 03-01: 6, Plan 03-02: 6). De-duplicated where they overlap.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Under sbx capabilities the primary workspace mount has `working_dir == VolumeMount.container_path == VolumeMount.host_path` (RT-04 SC-1) | VERIFIED | `container_config.rs:1281-1284` reassigns `config.volumes` + `config.working_dir` from `conform_workspace_paths`; tests at `container_config.rs:3308`, `3360`, `3399`, `3454` cover plain path, spaced path, bare-repo worktree, multi-repo workspace and each asserts `host_path == container_path` predicate across all surviving mounts. |
| 2 | Under Docker capabilities, `build_container_config` returns the same workspace + convenience mount shape it returned pre-Phase-3 (RT-04 SC-2) | VERIFIED | `container_config.rs:1261-1263` early-returns `Ok(config)` unchanged when `caps.supports_arbitrary_volume_paths == true`. Test `test_build_container_config_docker_baseline_unchanged` (`container_config.rs:3523`) asserts `working_dir == "/workspace/<dir_name>"`, workspace volume keeps `container_path == "/workspace/<dir_name>"`, AND at least one mount preserves `host != container` shape. |
| 3 | A user-declared extra_volumes entry of the form `host:container[:ro]` where host != container returns an `Err` that downcasts to `ContainerConfigError::InconformableExtraVolume` under sbx capabilities (RT-04 SC-3) | VERIFIED | `container_config.rs:1269-1278` runs the validator; test `test_build_container_config_sbx_inconformable_extra_volume_errors` (`container_config.rs:3595`) asserts `err.downcast_ref::<ContainerConfigError>()` matches `Some(InconformableExtraVolume { entry, .. })` with `entry == "/host/data:/container/data:ro"`. Companion tests at 3644 (conformant pass) and 3695 (Docker no-op) cover the surrounding branches. |
| 4 | Under sbx capabilities, all convenience mounts whose `container_path` differs from `host_path` are dropped silently | VERIFIED | `container_config.rs:1286-1298` `retain` closure drops `keep = v.container_path == v.host_path` mounts and emits `tracing::debug!` (not warn) per D-04. Test at `container_config.rs:3332-3338` asserts the predicate across all surviving mounts. |
| 5 | Under sbx capabilities, `anonymous_volumes` is cleared so downstream consumers do not see stale pre-conformance container paths | VERIFIED | `container_config.rs:1300-1302` `config.anonymous_volumes.clear()` when `!caps.supports_anonymous_volumes`. Test `test_build_container_config_sbx_clears_anonymous_volumes` (`container_config.rs:3746`) writes a config with `volume_ignores`, asserts `config.anonymous_volumes.is_empty()` after the call. |
| 6 | `compute_volume_paths` and `compute_workspace_volume_paths` remain capability-blind | VERIFIED | Neither function signature mentions `RuntimeCapabilities`; the post-pass is layered AFTER both functions return. Verified by inspection at `container_config.rs:917-940` (build_container_config calls both, then runs conform_for_capabilities last). |
| 7 | `instance.rs::get_container_for_instance` does not call `runtime.ensure_image` when `runtime.capabilities().supports_image_pull` is false; still calls when true (RT-06 SC-4) | VERIFIED | `instance.rs:1253-1255` reads `if runtime.capabilities().supports_image_pull { runtime.ensure_image(image)?; }`. Test `test_ensure_image_call_site_is_capability_gated` (`container_config.rs:3796`) source-substring asserts the 200 chars preceding `runtime.ensure_image(image)` contain `supports_image_pull`. Test `test_capability_matrices_drive_ensure_image_branch` (`container_config.rs:3810`) asserts Docker/Podman/Apple have `supports_image_pull = true`, sbx has `false`. |
| 8 | `Instance::container_workdir(caps)` returns the conformed host_path under sbx and `/workspace/...` under Docker | VERIFIED | `instance.rs:1272-1281` calls `compute_volume_paths` + `conform_workspace_paths` with the supplied caps. Test `test_instance_container_workdir_threads_capabilities` (`container_config.rs:3841`) asserts sbx caps return canonicalized host path, Docker caps return string starting with `/workspace/`. |
| 9 | All 19 existing call sites of `container_workdir` continue to compile via `container_workdir_now()` shim | VERIFIED | `grep -c "container_workdir_now()"` returns 15+2+1+1 = 19 across `instance.rs` (15), `creation_poller.rs` (2), `remove.rs` (1), `deletion.rs` (1). `grep -nE 'container_workdir\(\)' ...` returns zero matches (no parameterless calls remain). Shim defined at `instance.rs:1287-1290`. |
| 10 | `Instance::build_container_config` wrapper passes the resolved runtime's capabilities and `ContainerRuntimeName` into `container_config::build_container_config` | VERIFIED | `instance.rs:1310-1311` passes `runtime.capabilities()` and `runtime.name()`. The `name()` accessor on `ContainerRuntime` (concrete struct, not trait) is at `runtime.rs:75-82` and maps `RuntimeKind` discriminants 1:1 onto `ContainerRuntimeName`. Eliminates the Plan 03-01 temporary `Config::load()` round-trip. |
| 11 | Phase 1 capability matrix tests still pass; `runtime_base.rs` unchanged (D-13) | VERIFIED | `git log src/containers/runtime_base.rs` shows last touch is `8a81208` (Phase 1 fix); no Phase 3 commits modified the file. `RuntimeBase::ensure_image` at `runtime_base.rs:195-204` retains the defensive `Err(NotSupported)` safety net. |
| 12 | 14 new unit tests for build/conform helpers (Plan 03-01) + 3 SC-4 tests (Plan 03-02) all pass | VERIFIED | 17 Phase-3-added tests located by name (see Anti-Patterns / Tests below). Total test count in `container_config.rs` is 77. Orchestrator confirmed 1660 passing on Wave 2 worktree. |

**Score:** 12/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/session/container_config.rs` | ContainerConfigError + conform_workspace_paths + conform_for_capabilities + capabilities/runtime_name params + 14 new tests | VERIFIED | Enum at line 25, `conform_workspace_paths` at line 48, `conform_for_capabilities` at line 1255, `build_container_config` signature at line 917 carries both new params with `#[allow(clippy::too_many_arguments)]` at line 916. |
| `src/session/instance.rs` | ensure_image guard + container_workdir(caps) + container_workdir_now shim + build_container_config wrapper threading | VERIFIED | Guard at 1253, `container_workdir(caps)` at 1272, `container_workdir_now` at 1287, wrapper at 1292 passing `runtime.capabilities()` and `runtime.name()` at 1310-1311. |
| `src/containers/runtime.rs` | `ContainerRuntime::name() -> ContainerRuntimeName` accessor | VERIFIED | Inherent impl at `runtime.rs:75-82` maps RuntimeKind discriminants 1:1. |
| `src/cli/remove.rs` | call-site updated to `container_workdir_now()` | VERIFIED | Line 92. |
| `src/session/deletion.rs` | call-site updated to `container_workdir_now()` | VERIFIED | Line 361. |
| `src/tui/creation_poller.rs` | 2 call-sites updated to `container_workdir_now()` | VERIFIED | Lines 167 and 208. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `container_config::build_container_config` | `conform_for_capabilities` | End-of-function post-pass before returning `Ok(config)` | WIRED | `container_config.rs:1229-1234` calls `conform_for_capabilities(config, capabilities, runtime_name, &sandbox_config.extra_volumes)` as the function's terminal expression. |
| `conform_for_capabilities` | `conform_workspace_paths` | Owned-passthrough call rewriting workspace volumes + working_dir | WIRED | `container_config.rs:1281-1284` calls `conform_workspace_paths(config.volumes, config.working_dir, caps)` and reassigns both fields. |
| `Instance::container_workdir` | `container_config::conform_workspace_paths` | Pure call with caps arg returning conformed workdir | WIRED | `instance.rs:1278-1279`. |
| `Instance::container_workdir_now` | `Instance::container_workdir(caps)` | Shim that fetches caps off `get_container_runtime().capabilities()` | WIRED | `instance.rs:1287-1290`. |
| `Instance::get_container_for_instance` | `runtime.ensure_image` | Two-line if-guard gated on `runtime.capabilities().supports_image_pull` | WIRED | `instance.rs:1253-1255`. |
| `Instance::build_container_config` wrapper | `container_config::build_container_config` | Passes `runtime.capabilities()` + `runtime.name()` | WIRED | `instance.rs:1310-1311`. |
| 19 exec-time consumers (instance/poller/remove/deletion) | `Instance::container_workdir_now` | `.container_workdir_now()` append at every call site | WIRED | 15 + 2 + 1 + 1 = 19 grep matches; zero parameterless `container_workdir()` calls remain. |

### Data-Flow Trace (Level 4)

Phase 3 is a pure config-building / capability-gating change. No new rendered dynamic data was introduced; the artifacts feed downstream subprocess argv (Phase 5) and existing `ContainerConfig` consumers. Data flow validated via the test suite (1660 passing) and the structural wire-up above.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Phase-3 test set compiles + runs | `cargo test --lib session::container_config` (per orchestrator) | 1660 passing, 0 failed (orchestrator confirmed on Wave 2 worktree) | PASS |
| Library builds with new signatures | `cargo build --lib` (per Plan 03-02 task verify block + SUMMARY) | exit 0 | PASS |
| Clippy clean on new code | `cargo clippy --lib -- -D warnings` (per SUMMARY) | exit 0 | PASS |
| ensure_image guard is at exactly one call site in instance.rs | `grep -c "supports_image_pull" src/session/instance.rs` | 1 | PASS |
| Zero parameterless `container_workdir()` calls remain | `grep -nE 'container_workdir\(\)' src/session/instance.rs src/tui/creation_poller.rs src/cli/remove.rs src/session/deletion.rs` | 0 matches | PASS |
| 19 call sites of `container_workdir_now()` across 4 files | `grep -c container_workdir_now() <4 files>` | 15 + 2 + 1 + 1 = 19 | PASS |
| `runtime_base.rs` unchanged in Phase 3 (D-13) | `git log bc75d30..HEAD -- src/containers/runtime_base.rs` | no commits | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| RT-04 | 03-01-PLAN.md, 03-02-PLAN.md | `host_path == container_path` constraint for sbx enforced in `build_container_config` by capability-gated rewrite of BOTH `working_dir` AND every `VolumeMount.container_path`; inconformable configs surface clear validation error | SATISFIED | `conform_for_capabilities` post-pass at `container_config.rs:1255-1305`; `ContainerConfigError::InconformableExtraVolume` at `container_config.rs:25-35`; `Instance::container_workdir(caps)` threading at `instance.rs:1272`; 14 SC-1/2/3 tests + 1 threading test verify the contract. |
| RT-06 | 03-02-PLAN.md | sbx-specific image semantics: `ensure_image` skips silently when `!supports_image_pull`; Docker path unchanged | SATISFIED | Two-line guard at `instance.rs:1253-1255`; SC-4 source-substring test at `container_config.rs:3796`; capability-matrix test at `container_config.rs:3810`. RuntimeBase::ensure_image untouched per D-13. |

REQUIREMENTS.md maps exactly RT-04 and RT-06 to Phase 3. Both are covered by plan frontmatter `requirements:` fields. Zero orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | No `TODO`/`FIXME`/`XXX`/`TBD`/`HACK`/`PLACEHOLDER` markers introduced in Phase 3-modified files | — | — |

Single `#[allow(clippy::too_many_arguments)]` on `build_container_config` (9 params after D-09's purely additive design) is documented in SUMMARY Decisions Made with widespread precedent in the codebase (tui, cockpit, server). Plan-authorized.

### Tests Added (17 total)

Plan 03-01 (14):
- `test_conform_workspace_paths_docker_is_identity`
- `test_conform_workspace_paths_sbx_rewrites_container_to_host`
- `test_conform_workspace_paths_sbx_rewrites_workdir_under_volume_subdir`
- `test_conform_workspace_paths_sbx_multiple_volumes_longest_prefix_wins`
- `test_container_config_error_display_includes_entry_and_runtime`
- `test_build_container_config_sbx_conforms_workspace_mount_plain_path`
- `test_build_container_config_sbx_conforms_workspace_mount_path_with_spaces`
- `test_build_container_config_sbx_conforms_bare_repo_worktree`
- `test_build_container_config_sbx_conforms_multi_repo_workspace`
- `test_build_container_config_docker_baseline_unchanged`
- `test_build_container_config_sbx_inconformable_extra_volume_errors`
- `test_build_container_config_sbx_conformant_extra_volume_passes`
- `test_build_container_config_docker_inconformable_extra_volume_ok`
- `test_build_container_config_sbx_clears_anonymous_volumes`

Plan 03-02 (3):
- `test_ensure_image_call_site_is_capability_gated`
- `test_capability_matrices_drive_ensure_image_branch`
- `test_instance_container_workdir_threads_capabilities`

### Human Verification Required

None. Phase 3 is pure config-building / capability-gating with no UI surface, no new external service integration, and no real-time behavior. All checks are programmatic (compile + unit + structural). Live sbx integration is deferred to Phase 5 (RT-02/RT-05/RT-07/LIFE-01/TEST-01..03) per ROADMAP.

### Goal Achievement Summary

Phase 3 delivers exactly what the goal demands:
- `build_container_config` now consults `runtime.capabilities()` and rewrites both `working_dir` AND every `VolumeMount.container_path` consistently to `host_path` under `!supports_arbitrary_volume_paths` (sbx).
- Inconformable user `extra_volumes` surface a typed `ContainerConfigError::InconformableExtraVolume` (downcastable via `anyhow::Error::downcast_ref`).
- `ensure_image` is gated on `supports_image_pull`; sbx silently skips, Docker/Podman/AppleContainer call as before.
- Exec-time consumers (19 call sites) see the same conformed workdir via `Instance::container_workdir(caps)` / `container_workdir_now()`; no `sbx exec -w` mismatch later in Phase 5.
- Docker/Podman/AppleContainer paths are byte-identical to pre-Phase-3 (post-pass early-returns when `supports_arbitrary_volume_paths == true`); SC-2 baseline test verifies.

Two intentional deviations from the literal SC text, both documented and accepted:
1. SC-3 mentioned `volume_ignores` as the trigger; implementation uses `extra_volumes` per CONTEXT D-08 (authoritative narrowing). `volume_ignores` flows into `anonymous_volumes` which the post-pass clears under sbx caps.
2. SC-4 ROADMAP text suggests "verified by unit test with a mock runtime"; implementation uses source-substring + capability-matrix assertion pair per RESEARCH guidance (no precedent for mock runtimes; assertion cost proportional to two-line guard). Both checks together prove the gate is present AND the matrix values driving it are correct.

Neither deviation reduces RT-04 / RT-06 coverage; both were planner-authorized and explicit in PLAN/SUMMARY/Discussion-Log.

---

_Verified: 2026-05-16T01:30:00Z_
_Verifier: Claude (gsd-verifier)_
