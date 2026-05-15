---
phase: 01-capability-surface-and-trait-foundation
plan: 02
subsystem: infra
tags: [rust, container-runtime, sbx, stub, compile-time-exhaustiveness]

# Dependency graph
requires: [01-01]
provides:
  - "ContainerRuntimeName::Sbx as serde-round-trip-clean 4th enum variant; on-disk TOML value is \"sbx\""
  - "RuntimeBase::SBX const literal with the full 7-field honest sbx capability matrix"
  - "RuntimeKind::Sbx + ContainerRuntime::sbx() stub constructor"
  - "Sbx arms in every match-self.kind dispatch site in runtime.rs (does_container_exist, is_container_running, exec_command, batch_running_states)"
  - "is_available short-circuit on RuntimeKind::Sbx returning false unconditionally (D-05 stub contract)"
  - "Factory arms for Sbx in runtime_binary() and get_container_runtime() so the 12 downstream callers (CONTEXT.md D-05) reach a stub runtime without re-touching"
affects: [02-sbx-runtime-skeleton, 03-workspace-conformance-and-ensure-image, 06-settings-tui-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Inline match-arm short-circuit (RuntimeKind::Sbx => false) instead of platform cfg gating; preserves REQ-RT-08 (containers/ baseline 0 cfg(target_os))"
    - "Const-literal SBX with explicit 7-field RuntimeCapabilities init; no Default::default fill (D-02 contract from Plan 01-01)"
    - "Safe-stub dispatch arms (Ok(false), empty HashMap, addressable string) so every trait method has a defined answer even though is_available short-circuits all real callers"

key-files:
  created:
    - .planning/phases/01-capability-surface-and-trait-foundation/01-02-SUMMARY.md
  modified:
    - src/session/config.rs
    - src/containers/runtime_base.rs
    - src/containers/runtime.rs
    - src/containers/mod.rs
    - src/tui/settings/fields.rs

key-decisions:
  - "Inline is_available match-arm (RuntimeKind::Sbx => false) rather than per-RuntimeBase override; keeps the short-circuit semantically tied to the kind discriminant so Phase 2 only deletes the arm to re-enable base.is_available()"
  - "Sbx capability matrix declared on RuntimeBase::SBX const rather than on a separate sbx-specific struct; matches Plan 01-01's Docker/Podman/AppleContainer placement and reuses the compile-time exhaustiveness contract verbatim"
  - "Settings TUI fields.rs Sbx arms included as a Rule 3 deviation; the build does not compile without them and Phase 6 would otherwise inherit a data-corruption bug (selecting index 3 round-trips back to AppleContainer)"
  - "Per-task RED test commits even though the build compiled red-or-green at task boundaries; matches Plan 01-01's TDD Gate Compliance precedent that intermediate non-compile is the exhaustiveness contract firing"

patterns-established:
  - "Adding a new ContainerRuntime variant: (1) add `Sbx,` to ContainerRuntimeName, (2) add `pub const SBX: Self` with full RuntimeCapabilities literal, (3) add `Sbx` to RuntimeKind, (4) add `pub fn sbx()` constructor, (5) extend is_available to short-circuit OR delegate, (6) add Sbx arms to every match-self.kind dispatch site, (7) wire ContainerRuntimeName::Sbx in runtime_binary() and get_container_runtime() factories, (8) extend ANY exhaustive enum match outside the prescribed file set (build will fail until done)."
  - "Stub-in-place pattern for Phase 1 future-backend land: is_available returns false unconditionally; every dispatch arm returns a safe stub; the 12 caller sites stay untouched. Phase 2 swaps the const + the is_available arm to enable real probing without re-visiting callers."

requirements-completed: [RT-03, RT-08, PLAT-01, TEST-05]

# Metrics
duration: ~30 min (excluding initial cargo build cache rebuild)
completed: 2026-05-15
---

# Phase 1 Plan 2: Sbx as the 4th ContainerRuntime variant Summary

**Sbx lands as a fully-stubbed 4th runtime: enum variant in `ContainerRuntimeName`, `RuntimeKind`, and `RuntimeBase::SBX` const with the honest 7-flag capability matrix, threaded through the factory and every dispatch site; `is_available` short-circuits to false unconditionally per the D-05 Phase 1 contract.**

## Performance

- **Duration:** ~30 min of plan execution (full cargo build + clippy rebuild on fresh worktree added ~3 min on top)
- **Started:** 2026-05-15T06:03:00Z (Task 1 RED commit timestamp)
- **Completed:** 2026-05-15T06:33:49Z
- **Tasks:** 3 of 3 completed
- **Commits:** 6 (per-task RED/GREEN pairs)
- **Files modified:** 5 (4 from plan's files_modified + 1 deviation, see below)

## Accomplishments

- `ContainerRuntimeName::Sbx` added as the 4th variant of the serde enum in `src/session/config.rs`; lowered to `"sbx"` on disk via the existing `#[serde(rename_all = "snake_case")]` attribute; `#[default]` remains on `Docker`.
- `RuntimeBase::SBX` added as the 4th const literal with `binary: "sbx"`, `name: "Docker Sandboxes"`, and the full honest 7-field `RuntimeCapabilities` matrix from CONTEXT.md `<specifics>`.
- `RuntimeKind::Sbx` added as the 4th variant; `ContainerRuntime::sbx()` constructor pairs `RuntimeBase::SBX` with `RuntimeKind::Sbx`.
- `is_available` overridden to `match self.kind { RuntimeKind::Sbx => false, _ => self.base.is_available(), }`; locks the D-05 stub contract structurally (cannot be tricked into returning true via `sbx --version` on PATH).
- Safe-stub `RuntimeKind::Sbx => ...` arms added to all four `match self.kind` dispatch blocks: `does_container_exist` (Ok(false)), `is_container_running` (Ok(false)), `exec_command` (`format!("sbx exec {}", name)`), `batch_running_states` (empty HashMap).
- `runtime_binary()` and `get_container_runtime()` factories in `src/containers/mod.rs` each have a `ContainerRuntimeName::Sbx` arm; the 12 downstream callers enumerated in CONTEXT.md D-05 are untouched.
- 11 new tests pin every contract: 4 serde-round-trip tests, the SBX const matrix, the constructor binary/name/kind triple, the is_available short-circuit, the per-flag capability matrix, the dispatch arms safe-stub shape, the trait-method routing through SBX, and the factory integration.
- Whole-tree `cargo test --lib` reports **1619 passed / 0 failed** (1608 baseline + 11 new). `cargo fmt --check`, `cargo build`, and `cargo clippy --all-targets -- -D warnings` exit 0.

## Task Commits

Six commits land this plan; each task has a RED test commit followed by a GREEN implementation commit:

1. **Task 1 RED:** `1e0edd0` test(01-02): add failing tests for ContainerRuntimeName::Sbx
2. **Task 1 GREEN:** `e4f34d7` feat(01-02): add ContainerRuntimeName::Sbx variant
3. **Task 2 RED:** `f3767bb` test(01-02): add failing tests for RuntimeBase::SBX and ContainerRuntime::sbx()
4. **Task 2 GREEN:** `a427a92` feat(01-02): add RuntimeBase::SBX, RuntimeKind::Sbx, and ContainerRuntime::sbx() stub
5. **Task 3 RED:** `2f286f9` test(01-02): add failing test for get_container_runtime/Sbx factory wiring
6. **Task 3 GREEN:** `a4f1abe` feat(01-02): wire ContainerRuntimeName::Sbx through factories; restore green build

## Files Created/Modified

- `src/session/config.rs` — Added `Sbx` as the 4th `ContainerRuntimeName` variant after `Podman`; updated the doc comment on `pub container_runtime` to list `or sbx`; added 4 round-trip tests (serializes to `"sbx"`, deserializes from `"sbx"`, default stays Docker, existing 3 variants still round-trip).
- `src/containers/runtime_base.rs` — Added `pub const SBX: Self` after `RuntimeBase::PODMAN`. Initializer: `binary: "sbx"`, `name: "Docker Sandboxes"`, `daemon_check_args: &["version"]` (placeholder; never called in Phase 1 because is_available short-circuits), `pull_prefix: &[]` (empty: supports_image_pull is false), `remove_subcommand: "rm"`, and a full 7-field `RuntimeCapabilities` literal. Added `test_sbx_runtime_base_const` pinning the binary, name, and per-flag matrix.
- `src/containers/runtime.rs` — Added `Sbx` to `RuntimeKind` (now 4 variants); added `pub fn sbx() -> Self` constructor at the end of `impl ContainerRuntime`; rewrote `is_available` to short-circuit on `RuntimeKind::Sbx` and otherwise delegate to `self.base.is_available()`; added `RuntimeKind::Sbx` arms to all four `match self.kind` dispatch sites with comments naming the Phase 2 successor; added 5 unit tests (`test_sbx_runtime_uses_sbx_binary`, `test_sbx_is_available_returns_false_unconditionally`, `test_sbx_capability_matrix`, `test_sbx_dispatch_arms_return_safe_stubs`, `test_sbx_capabilities_via_trait_method`).
- `src/containers/mod.rs` — Added `ContainerRuntimeName::Sbx => "sbx"` arm to `runtime_binary()` and `ContainerRuntimeName::Sbx => ContainerRuntime::sbx()` arm to `get_container_runtime()`; added `test_get_container_runtime_constructs_sbx_when_kind_matches` to the in-module tests.
- `src/tui/settings/fields.rs` — **(Deviation file: not in plan's `files_modified` list)** Added `ContainerRuntimeName::Sbx => 3` to the two `match container_runtime` selected-index lookups in `build_sandbox_fields()`; appended `"Docker Sandboxes".into()` to `container_runtime_options` so index 3 renders correctly. Updated the two reverse-mapping `match selected` blocks in `apply_field_to_global()` and `apply_field_to_profile()` from `_ => AppleContainer` to explicit `2 => AppleContainer, _ => Sbx` so a user selecting "Docker Sandboxes" round-trips back to `ContainerRuntimeName::Sbx` instead of silently saving as AppleContainer.

## Honest Values Matrix (Sbx row appended to Plan 01-01's table)

| Flag | Docker | Podman | AppleContainer | Sbx |
|---|---|---|---|---|
| `supports_read_only_volumes` | true | true | false | **false** |
| `supports_remove_volumes` | true | true | false | **false** |
| `supports_port_publish_at_create` | true | true | true | **false** |
| `supports_image_pull` | true | true | true | **false** |
| `supports_anonymous_volumes` | true | true | true | **false** |
| `supports_arbitrary_volume_paths` | true | true | true | **false** |
| `supports_dynamic_port_publish` | false | false | false | **true** |

This is the matrix Phase 2's real `SbxRuntime` peer struct must claim. The sbx-specific cells are sourced from CONTEXT.md `<specifics>` and align with the Docker Sandboxes design (the microVM kit model lacks docker-style volume/pull verbs; ports are published out-of-band via `sbx port publish`).

## Stub Patterns (so Phase 2 knows what to replace)

The four dispatch sites in `src/containers/runtime.rs` have these Sbx arms today; Phase 2 (RT-02 / RT-04) replaces each with a real subprocess call:

| Dispatch site | Current stub | Phase 2 successor |
|---|---|---|
| `does_container_exist(name)` | `Ok(false)` | `sbx inspect <name>` exit-code probe |
| `is_container_running(name)` | `Ok(false)` | sbx running-state probe (per RESEARCH.md Q5) |
| `exec_command(name, options, cmd)` | `format!("sbx exec {}", name)` (addressable but unreachable) | full `sbx exec` argv construction with TTY flags |
| `batch_running_states(prefix)` | empty `HashMap` | `sbx ls --filter` parse |
| `is_available()` | `RuntimeKind::Sbx => false,` short-circuit in the `match self.kind` block | delete the Sbx arm; the default `self.base.is_available()` path runs `sbx --version`, gated by the typed `SbxNotConfigured` error per Phase 2's RT-02 |

Phase 2 also replaces `RuntimeBase::SBX.daemon_check_args` (currently `&["version"]` placeholder) with the real composite probe, and `pull_prefix` stays empty because `supports_image_pull` is `false` and Phase 2's kit model uses `sbx kit` instead of image pull.

## D-05 Caller-Sites Confirmation

The 12 caller sites enumerated in CONTEXT.md D-05 are confirmed untouched. `git diff --stat HEAD~6 HEAD -- src/tui/dialogs/new_session/ src/server/api/system.rs src/cli/add.rs src/session/instance.rs src/session/builder.rs` reports zero changes. Phase 2 can swap the stub for the real `SbxRuntime` peer struct without re-visiting any of them.

## Decisions Made

- **Inline match-arm short-circuit over RuntimeBase::is_available override.** Putting the `false` short-circuit on `match self.kind` in `runtime.rs` rather than overriding `RuntimeBase::is_available` keeps the stub contract semantically tied to the discriminant. Phase 2 deletes a single arm to re-enable the base path; alternatives would have required surgery on `RuntimeBase` impl.
- **No `#[cfg(target_os = ...)]` gating on the SBX const.** D-07 (REQ-RT-08) bans new cfg-gates in `src/containers/` and the `src/session/` config file. The platform-availability question is handled implicitly: `is_available` returns `false` everywhere in Phase 1; Phase 2's `which sbx` probe naturally returns false on platforms where sbx is not installed.
- **Capability flag count gate is 28 (7 flags x 4 backends).** Plan 01-01 left it at 21 (7x3); after adding SBX it's 28. The plan's structural grep gate confirms this; any future flag addition will force the count to advance to 32 (7x4 + 7x1 new field x 4 backends = 32), or 35 (8 flags x 4 backends), depending on whether a new flag is added.
- **fields.rs deviation included rather than deferred to Phase 6.** Without the Sbx arms in the two `match container_runtime` blocks in `build_sandbox_fields()`, the build does not compile. Beyond compile fix, the reverse-mapping `_ => AppleContainer` arms would silently save Sbx as AppleContainer if Phase 6 shipped the dropdown without revisiting them, which is a data-corruption bug. Fixing both directions here costs 6 lines and prevents a subtle regression Phase 6 might miss.

## Deviations from Plan

### Rule 3: blocking issue auto-fix

**1. `src/tui/settings/fields.rs` was modified despite not appearing in the plan's `files_modified` list.**

- **Found during:** Task 1 GREEN. After adding `ContainerRuntimeName::Sbx`, `cargo build --tests` reported four `E0004: non-exhaustive patterns` errors. Two were in `src/containers/mod.rs` (factory matches; covered by Task 3 explicitly). Two were in `src/tui/settings/fields.rs` at lines 932-936 and 944-948: exhaustive `match container_runtime { ... }` blocks that map the variant to a dropdown index.
- **Issue:** The plan's `files_modified` did not enumerate ALL exhaustive matches on `ContainerRuntimeName`. The build cannot compile without `Sbx` arms in those two sites, and once you map `Sbx => 3`, the inverse `match selected { ..., _ => AppleContainer }` blocks at lines 1818-1820 and 2106-2108 silently save the user's choice as `AppleContainer` instead of `Sbx` (data-corruption bug).
- **Fix:** Added `ContainerRuntimeName::Sbx => 3` arms in both `build_sandbox_fields()` matches; appended `"Docker Sandboxes".into()` to `container_runtime_options` so index 3 has a renderable label; updated the two reverse-mapping `match selected` blocks from `_ => AppleContainer` to explicit `2 => AppleContainer, _ => Sbx` so round-trip is correct.
- **Files modified:** `src/tui/settings/fields.rs`
- **Commit:** `a4f1abe` (Task 3 GREEN)
- **Why this is safe vs. Phase 6's scope:** Phase 6 (RT-09) owns the full settings TUI wiring (icons, descriptions, defaults, possibly hiding Sbx behind a feature flag). The Phase 1 stub fix here is the minimum-correct compile + data-integrity change; Phase 6 can expand the label, add a tooltip, or otherwise refine without re-touching the match logic. The `container_runtime_options` literal vec is the only Phase-6-territory line; the other four arms are purely defensive.

### `cargo deny check` not run

The plan's verify block includes `cargo deny check` (line 346). The `cargo-deny` subcommand is not installed in this worktree's sandbox (`error: no such command: deny`); Plan 01-01's SUMMARY did not run it either. Since this plan adds **zero new crates** (the diff includes only stdlib + first-party code), any cargo-deny outcome must equal the baseline, so the absence of the tool does not change the conclusion. The whole-tree `Cargo.lock` is unchanged.

## Issues Encountered

- The `cargo` binary was not on `PATH` for non-login shells inside the worktree's bash environment, even though `~/.cargo/env` exists and `bash -l -c` works. Worked around by prefixing every cargo invocation with `bash -l -c '...'`. Resolution was infra-only; no impact on the plan output.
- `cargo fmt` re-formatted `test_container_runtime_name_default_is_docker` across three lines after the initial commit (the inline form was too long). Re-staged the auto-fmt diff into Task 3's GREEN commit so `cargo fmt --check` exits 0.

## TDD Gate Compliance

Each of the three tasks was marked `tdd="true"` in the plan, and each has a per-task RED → GREEN pair in `git log`:

1. `1e0edd0` test(01-02) → `e4f34d7` feat(01-02): Task 1 (config.rs variant + 4 tests)
2. `f3767bb` test(01-02) → `a427a92` feat(01-02): Task 2 (SBX const + RuntimeKind::Sbx + sbx() ctor + dispatch arms + 6 tests)
3. `2f286f9` test(01-02) → `a4f1abe` feat(01-02): Task 3 (factory arms + fields.rs deviation + 1 test)

Intermediate non-compiling states between commits are accepted per Plan 01-01's TDD Gate Compliance precedent ("the literal RED test for Task 1's contract is the cargo-check E0046 error... the literal GREEN is Task 2's `cargo build` success"). The compile-time exhaustiveness contract from Plan 01-01 fires as designed in this plan: adding `RuntimeKind::Sbx` without the 4 dispatch arms would fail the build; adding `ContainerRuntimeName::Sbx` without the factory arms (and the fields.rs arms) would fail the build.

No REFACTOR commit; the GREEN code in each task was already idiomatic and clippy-clean on first compile.

## Diagnostics That Surfaced

- `cargo fmt --check` initially failed on a single 105-char line; auto-fmt resolved it.
- `cargo clippy --all-targets -- -D warnings` was clean on first GREEN compile; no warnings about the new const, the new enum variant, the match arms, or the test bodies.
- All 1619 lib tests pass. The 11 new tests run in microseconds (pure data assertions, no subprocess calls).
- `grep -rh "#[cfg(target_os" src/containers/ | wc -l` returns **0**. The diff-vs-prior-state form on `src/session/config.rs` reports **0** newly-added cfg(target_os) lines (REQ-RT-08 honored).
- `grep -rE "RuntimeHandle" src/` returns **0** matches (D-05 honored; Phase 2 introduces the wrapper if needed).

### Structural Gate Note

The plan's verification gate at line 275 expects `grep -A1 "RuntimeKind::Sbx =>" src/containers/runtime.rs | grep -E "^\s*(false|Ok\(false\))" | head -1` to return at least one match for the is_available short-circuit. In practice the idiomatic Rust form `RuntimeKind::Sbx => false,` puts the body on the same line as the `=>`, not on the next line, so `grep -A1` returns `_ => self.base.is_available()` as the second line rather than `false`. The structural intent of the gate is honored: the dedicated unit test `test_sbx_is_available_returns_false_unconditionally` directly asserts `!ContainerRuntime::sbx().is_available()`, which is the real contract. If a future regression converts the arm to `RuntimeKind::Sbx => self.base.is_available()`, that test will fail. The structural grep gate is a lossy proxy for the semantic test.

## Next Phase Readiness

- **Plan 02 (sbx-runtime-skeleton):** Has a stable enum surface to extend. `ContainerRuntimeName::Sbx`, `RuntimeKind::Sbx`, and `RuntimeBase::SBX` are stable; Phase 2 introduces the real `SbxRuntime` peer struct, swaps the `Sbx` arm in `get_container_runtime()` to construct it, and replaces the four dispatch-arm safe stubs (and the is_available short-circuit) with real subprocess calls. The 12 caller sites stay untouched.
- **Plan 03 (workspace-conformance-and-ensure-image):** Can gate code on `runtime.capabilities().supports_arbitrary_volume_paths` and `runtime.capabilities().supports_image_pull` for the sbx-aware workspace path conformance and ensure-image short-circuit. Sbx claims `false` on both; consumers should treat them as authoritative.
- **Plan 04 (embedded-kit):** Can gate the embedded-kit installation path on the same `supports_image_pull == false` flag. Sbx claims `true` on `supports_dynamic_port_publish`, which Plan 04 / Phase 5 can use for the late-publish port flow.
- **Plan 06 (settings-tui-wiring):** The `Sbx` arm in `src/tui/settings/fields.rs` is already in place with a basic "Docker Sandboxes" label. Phase 6 can expand the dropdown UX (icons, descriptions, possibly hiding behind a feature flag for users without `sbx` on PATH), but does NOT need to invent a new enum variant. The reverse-mapping arms are already correct; Phase 6 only needs to refine presentation.

## Self-Check: PASSED

- File `src/session/config.rs` contains `Sbx,` variant (1 match), `or sbx` doc text (1 match), no per-variant `#[serde(rename = "sbx")]` attribute.
- File `src/containers/runtime_base.rs` contains `pub const SBX: Self` (1 match), `28` supports_*: lines across 4 backends, `0` `..Default::default()` occurrences.
- File `src/containers/runtime.rs` contains `RuntimeKind::Sbx` 8 occurrences (variant + ctor + is_available arm + 4 dispatch arms + 1 trait-test reference); 5 `RuntimeKind::Sbx => ` arm bodies (is_available + 4 dispatch); `is_available` Sbx arm body is the literal `false`.
- File `src/containers/mod.rs` contains `ContainerRuntimeName::Sbx => "sbx"` (1 match) and `ContainerRuntimeName::Sbx => ContainerRuntime::sbx()` (1 match); 2 inner match blocks (no collapse).
- File `src/tui/settings/fields.rs` contains 4 new `ContainerRuntimeName::Sbx` references across the two forward matches and the two reverse matches; `container_runtime_options` vec now lists 4 items.
- `cargo build` exits 0; `cargo test --lib` reports 1619 passed / 0 failed; `cargo fmt --check` exits 0; `cargo clippy --all-targets -- -D warnings` exits 0.
- `grep -rh "#[cfg(target_os" src/containers/ | wc -l` returns `0`; `git diff HEAD~6 -- src/session/config.rs | grep -E "^\+[^+]" | grep -E "#\[cfg\(target_os" | wc -l` returns `0`.
- `grep -rE "RuntimeHandle" src/` returns `0` matches.
- Commits exist in `git log`: `1e0edd0`, `e4f34d7`, `f3767bb`, `a427a92`, `2f286f9`, `a4f1abe`.

---
*Phase: 01-capability-surface-and-trait-foundation*
*Plan: 02*
*Completed: 2026-05-15*
