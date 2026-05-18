---
phase: 02-sbxruntime-skeleton-and-pure-argv-builders
verified: 2026-05-15T09:16:24Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 2: SbxRuntime Skeleton and Pure Argv Builders — Verification Report

**Phase Goal:** A `src/containers/sbx/` sub-module exists with the `SbxRuntime` skeleton implementing `ContainerRuntimeInterface`, exposing capability constants and pure-function argv builders for every `sbx` subcommand aoe will invoke. No real subprocess work yet; every method that would shell out either returns a stub error or is testable as a string-builder in isolation.

**Verified:** 2026-05-15T09:16:24Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Success Criteria

| #   | SC                                                                                    | Status     | Evidence |
| --- | ------------------------------------------------------------------------------------- | ---------- | -------- |
| SC-1 | argv unit tests: `build_create_args`, `build_exec_args`, `build_ports_args` ≥10 shapes | PASS | `cargo test --lib containers::sbx::argv::` → **20 passed, 0 failed**. Coverage: 7 create-args shapes (minimal, kit, cpus+memory, ro/non-ro volumes, multi-volume, anonymous-volume skip, port-mapping skip), 9 exec-args shapes (minimal, -i, -t, -i+-t, -w, env-inherit, env-literal, mixed env, full permutation), 3 ports-args shapes (simple, host-ip, protocol). 20 ≥ 10 minimum. |
| SC-2 | `SbxRuntime::capabilities()` returns 5 new + 2 migrated flags with sbx-correct values | PASS | `containers::sbx::tests::capabilities_returns_locked_sbx_matrix` asserts all 7 flags: `supports_read_only_volumes=false`, `supports_remove_volumes=false`, `supports_port_publish_at_create=false`, `supports_image_pull=false`, `supports_anonymous_volumes=false`, `supports_arbitrary_volume_paths=false`, `supports_dynamic_port_publish=true`. Also asserts `caps == RuntimeBase::SBX.capabilities` (D-12 zero-drift via `RuntimeBase::SBX` const at runtime_base.rs:99-119). Test PASS. |
| SC-3 | `SbxRuntime::is_available()` returns false when sbx absent, true when present (injection seam) | PASS | Two deterministic tests in `src/containers/sbx/mod.rs`:<br>• `is_available_with_injected_fake_binary_returns_true` constructs `SbxRuntime { binary: <tempdir>/fake-sbx }` with 0o755 perms and asserts `is_available()` returns true. Uses TempDir + scoped File::create (CONTEXT.md NamedTempFile recipe deviation documented — Linux ETXTBSY).<br>• `is_available_with_missing_binary_returns_false` uses `/nonexistent/sbx-binary` and asserts false. Both PASS, neither depends on host PATH. |
| SC-4 | `RuntimeKind::Sbx` dispatch routes through `SbxRuntime` without Docker/Podman/AppleContainer regression | PASS | All 17 `containers::runtime::tests::*` PASS, including:<br>• `test_sbx_dispatch_arms_return_safe_stubs` with **5 discriminating assertions**: `cmd.starts_with("sbx exec ")`, `cmd.contains("foo")`, `cmd.contains("bar")` (proves cmd is no longer dropped), `!cmd.contains("/* Phase 1")` (rules out sentinel), `!cmd.contains("dropped")`.<br>• `test_sbx_build_create_args_emits_sbx_shape_not_docker_shape` asserts `args[0]=="create"`, `args[1]=="shell"`, `!args.contains("run")`, `!args.contains("-d")`, `args.contains("--template")`, `args.contains("alpine:latest")`.<br>• Docker/Podman/AppleContainer matrix tests (`test_docker_capability_matrix`, `test_podman_capability_matrix`, `test_apple_container_capability_matrix`, `test_podman_exec_command_format_matches_docker`) all PASS — zero regression. |

**Score:** 4/4 SCs verified.

### Storage Shape (D-03 Option A)

| Item | Expected | Actual | Status |
| ---- | -------- | ------ | ------ |
| `ContainerRuntime` storage field | `pub(crate) sbx: Option<sbx::SbxRuntime>` | `pub(crate) sbx: Option<sbx::SbxRuntime>` at `runtime.rs:30` | PASS |
| `ContainerRuntime::docker()` sets `sbx: None` | None | `sbx: None` at `runtime.rs:38` | PASS |
| `ContainerRuntime::apple_container()` sets `sbx: None` | None | `sbx: None` at `runtime.rs:46` | PASS |
| `ContainerRuntime::podman()` sets `sbx: None` | None | `sbx: None` at `runtime.rs:54` | PASS |
| `ContainerRuntime::sbx()` sets `sbx: Some(SbxRuntime::new())` | Some | `sbx: Some(sbx::SbxRuntime::new())` at `runtime.rs:66` | PASS |

### Dispatch Minimum-Surface Audit (D-04)

| Trait method | RuntimeKind::Sbx arm? | Routes to | Expected (D-04 + D-05) | Status |
| ------------ | --------------------- | --------- | ---------------------- | ------ |
| `is_available` | YES (line 83-90) | `SbxRuntime::is_available()` | NEW peer-routed arm | PASS |
| `capabilities` | YES (line 96-105) | `SbxRuntime::capabilities()` | NEW peer-routed arm (D-12) | PASS |
| `build_create_args` | YES (line 203-215) | `sbx::argv::build_create_args` | NEW peer-routed arm | PASS |
| `exec_command` | YES (line 241-291) | `sbx::argv::build_exec_args` | NEW peer-routed arm | PASS |
| `does_container_exist` | YES (line 148-157) | safe stub (`Ok(false)`) | D-05 stub | PASS |
| `is_container_running` | YES (line 192-200) | safe stub (`Ok(false)`) | D-05 stub | PASS |
| `batch_running_states` | YES (line 338-345) | safe stub (`HashMap::new()`) | D-05 stub | PASS |
| `is_daemon_running` | NO | `self.base` (RuntimeBase::SBX) | Phase 5 wires | PASS |
| `get_version` | NO | `self.base` | Phase 5 wires | PASS |
| `pull_image` | NO | `self.base` (capability-gated by `supports_image_pull=false`) | Phase 1 WR-01 fix gates this | PASS |
| `ensure_image` | NO | `self.base` (capability-gated) | Phase 1 WR-01 fix gates this | PASS |
| `create_container` | NO | `self.base.run_create` (Docker shape) | Phase 5 wires; unreachable when `is_available()=false` | PASS (deferred to Phase 5) |
| `start_container` | NO | `self.base` | Phase 5 wires | PASS (deferred to Phase 5) |
| `stop_container` | NO | `self.base` | Phase 5 wires | PASS (deferred to Phase 5) |
| `remove` | NO | `self.base` | Phase 5 wires | PASS (deferred to Phase 5) |
| `exec` | NO | `self.base` | Phase 5 wires | PASS (deferred to Phase 5) |

Total NEW peer-routed Sbx arms: **4** (is_available, capabilities, build_create_args, exec_command). Plus 3 safe-stub arms (D-05). The action verbs (create_container, start_container, stop_container, remove, exec, pull_image, ensure_image, get_version, is_daemon_running, image_exists_locally) remain on `self.base` per D-05 deferral to Phase 5.

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `src/containers/sbx/mod.rs` | SbxRuntime struct + is_available + capabilities + 3 tests | PASS | 90 lines; struct with PathBuf binary, ::new(), ::is_available() (real `Command::new(&self.binary).arg("--version")` probe), ::capabilities() routes through RuntimeBase::SBX.capabilities; 3 tests in #[cfg(test)] |
| `src/containers/sbx/argv.rs` | 3 pure builders + 20 tests | PASS | 441 lines; `build_create_args`, `build_exec_args`, `build_ports_args` all return `Vec<String>` with no `&self` and no subprocess; 20 tests cover ≥10 distinct argv shapes |
| `src/containers/mod.rs` | `pub mod sbx;` registered | PASS | Line 5: `pub mod sbx;` (visibility deviation documented; see Notes) |
| `src/containers/runtime.rs` | storage field + 4 NEW Sbx dispatch arms + tests | PASS | Field at line 30; 4 NEW arms at lines 83, 96, 203, 241; new test `test_sbx_build_create_args_emits_sbx_shape_not_docker_shape` at line 599; strengthened `test_sbx_dispatch_arms_return_safe_stubs` at line 573 |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `ContainerRuntime::sbx()` factory | `SbxRuntime::new()` | `Some(sbx::SbxRuntime::new())` | WIRED | runtime.rs:66 |
| `<ContainerRuntime as ContainerRuntimeInterface>::is_available` (Sbx arm) | `SbxRuntime::is_available()` | `self.sbx.as_ref().expect(...).is_available()` | WIRED | runtime.rs:83-89 |
| `<ContainerRuntime as ContainerRuntimeInterface>::capabilities` (Sbx arm) | `SbxRuntime::capabilities()` | `self.sbx.as_ref().expect(...).capabilities()` | WIRED | runtime.rs:96-105 |
| `<ContainerRuntime as ContainerRuntimeInterface>::build_create_args` (Sbx arm) | `sbx::argv::build_create_args` | `sbx::argv::build_create_args(name, image, None, config)` | WIRED | runtime.rs:211 |
| `<ContainerRuntime as ContainerRuntimeInterface>::exec_command` (Sbx arm) | `sbx::argv::build_exec_args` | `sbx::argv::build_exec_args(name, workdir, &[], true, true, &cmd_parts)` joined with `"sbx"` prefix | WIRED | runtime.rs:284 |
| `SbxRuntime::capabilities()` | `RuntimeBase::SBX.capabilities` | direct read | WIRED (D-12 zero-drift) | sbx/mod.rs:40 |
| `containers::mod` | `containers::sbx` module | `pub mod sbx;` | WIRED | containers/mod.rs:5 |
| `containers::sbx::mod` | `containers::sbx::argv` module | `pub mod argv;` | WIRED | sbx/mod.rs:7 |
| `build_ports_args` | (Phase 5 caller) | none yet | ORPHANED-by-design | No internal caller until Phase 5 ports.rs; tested in isolation. Module visibility promoted to `pub` (deviation) to keep clippy clean. |

### Quality Gates

| Gate | Status | Details |
| ---- | ------ | ------- |
| `grep -c 'allow(dead_code)'` across sbx/mod.rs, sbx/argv.rs, runtime.rs | PASS | 0 occurrences in all three files |
| `cargo fmt --check` exit 0 | PASS | no output, exit 0 |
| `cargo clippy --lib -- -D warnings` exit 0 | PASS | no warnings, exit 0 |
| No `cfg(target_os = ...)` in `src/containers/` | PASS | grep returns no matches; platform gating is correctly absent (AGENTS.md: OS-specific logic belongs in `src/process/`) |
| `cargo test --lib containers::sbx::argv::` | PASS | 20/20 |
| `cargo test --lib containers::sbx::tests::` | PASS | 3/3 |
| `cargo test --lib containers::runtime::tests::` | PASS | 17/17 |
| `cargo test --lib containers::` (full suite) | PASS | 56/56 |
| Docker/Podman/AppleContainer regression | PASS | All capability-matrix, exec-command, image-pull, binary-name tests pass |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| (none) | – | – | – | No TBD/FIXME/XXX/HACK/PLACEHOLDER markers in modified files |

### Notes (Non-Blocking)

**1. Module visibility deviation (`pub` vs `pub(crate)`)**

Plan 02-01 promoted `pub(crate) mod sbx` to `pub mod sbx` (`src/containers/mod.rs:5`) and `pub(crate) mod argv` to `pub mod argv` (`src/containers/sbx/mod.rs:7`). This deviates from the original PLAN frontmatter acceptance criteria but is documented in `02-01-SUMMARY.md` Deviations § Rule 3 with the justification that `build_ports_args` has no internal lib caller until Phase 5's `ports.rs` lands, and `cargo clippy -- -D warnings` (enforced by the cargo-husky pre-commit hook) flags `pub(crate)` items that are unused at the lib level. This deviation does NOT affect goal achievement — all 4 SCs pass, the module is correctly wired, and the action-verb stubs are intact.

**Recommended follow-up:** Phase 5's `ports.rs` should restore `pub(crate)` once `build_ports_args` has an internal caller. The user may want to track this with a TODO comment or backlog item, but it is not a Phase 2 blocker.

**2. Acceptance-criteria grep drift (documented in 02-01-SUMMARY)**

Four grep-based plan acceptance criteria need re-interpretation:
- `grep -c 'pub(crate) mod sbx' ... returns 1` → now returns 0 (visibility deviation above)
- `grep -c 'pub(crate) mod argv' ... returns 1` → now returns 0 (visibility deviation above)
- `grep -c 'anonymous_volumes' src/containers/sbx/argv.rs returns 0` → returns 4 (1 doc-comment + 3 test references); the SPIRIT (impl does not emit anonymous-volume `-v` flags) IS satisfied — verified by reading `build_create_args`, which has no anonymous_volumes emission and is covered by `create_args_skips_anonymous_volumes` test.
- `grep -c '"-p"' src/containers/sbx/argv.rs returns 0` → returns 1 (in the test `create_args_skips_port_mappings` that asserts ABSENCE of `-p`); the SPIRIT (impl does not emit `-p PORT`) IS satisfied — `build_create_args` body has no `-p` push.

Both grep-as-proxy criteria are over-strict; the underlying invariants hold.

**3. Action-verb stubs documented as deferred to Phase 5**

`does_container_exist`, `is_container_running`, `batch_running_states` for `RuntimeKind::Sbx` return safe stubs (false / false / empty HashMap) per D-05/D-06. The Phase 1 sentinel comments were refreshed by commit `8aaeb96` to point at the correct deferral target (Phase 5 per D-05/D-06, not the original "Phase 2 RT-04" text). Verified by reading runtime.rs:148-157, 192-200, 338-346.

**4. `create_container` for Sbx falls through to `RuntimeBase::run_create` (Docker shape)**

When `ContainerRuntime::create_container()` is called with `RuntimeKind::Sbx`, the dispatch checks `does_container_exist` (stub: false) then calls `self.base.run_create(name, image, config)`, which uses `RuntimeBase::build_create_args` (Docker `run -d` shape) rather than the new `sbx::argv::build_create_args`. This is dead-path in Phase 2 (Phase 1's WR-01 capability gating prevents image pull errors from leaking, and `is_available()` returns false on hosts without sbx) but would emit Docker-shape argv on a host with sbx installed if the call path were exercised before Phase 5 lands. Per CONTEXT.md D-05 and ROADMAP Phase 5 success criteria, this is correctly scoped as Phase 5 work (RT-02, LIFE-01). **Not a Phase 2 gap.** Note that `<ContainerRuntime as ContainerRuntimeInterface>::build_create_args` (the trait method itself) DOES dispatch correctly to sbx-shape argv — the discrepancy is only via the `create_container → run_create → RuntimeBase::build_create_args` internal call chain, which D-05 explicitly defers.

### Human Verification Required

None. All criteria are deterministic and verified via `cargo test` + `cargo clippy` + `cargo fmt --check` + file inspection. No visual, real-time, or external-service behavior under test.

---

## Gaps Summary

No gaps. All 4 Success Criteria verified by passing tests with discriminating assertions. The phase goal — "src/containers/sbx/ submodule exists with SbxRuntime skeleton + pure argv builders, every method either returns a stub error or is testable in isolation" — is fully achieved.

The visibility deviation (Note 1) and the acceptance-criteria grep drift (Note 2) are documented in 02-01-SUMMARY.md and do not block phase completion. Phase 5 should tighten visibility back to `pub(crate)` when `build_ports_args` gets its first internal caller.

**Recommended action:** PROCEED to Phase 3 (sbx-specific capability-gating and volume-path enforcement).

---

_Verified: 2026-05-15T09:16:24Z_
_Verifier: Claude (gsd-verifier)_
