# Phase 2 Discussion Log

**Date:** 2026-05-15
**Phase:** 02-sbxruntime-skeleton-and-pure-argv-builders

Human-readable record of the discuss-phase conversation. CONTEXT.md is the canonical decision record consumed by downstream agents; this file is for audit/retrospective only.

## Areas Selected

User chose all four gray areas presented:
1. Dispatch shape
2. Stub shape for unwired methods
3. Argv builder API + kit-path handling
4. is_available() test seam

## Area 1 — Dispatch Shape

**Question:** How should SbxRuntime plug into get_container_runtime() so the ~12 callers don't get rewritten?

**Options presented:**
- RuntimeHandle enum { Cli(ContainerRuntime), Sbx(SbxRuntime) } that implements the trait
- Box<dyn ContainerRuntimeInterface>
- Keep ContainerRuntime as facade; route Sbx dispatch arms to held SbxRuntime peer

**User feedback:** "i dont wanna be too radical since im adding to someone elses lib. thoughts on what we should do here"

**Reframe:** Phase 1's D-05 and D-06 had already implicitly chosen the facade path — stub-in-place via `RuntimeKind::Sbx` arms, with Phase 2 explicitly named as "the swap point" that does NOT re-touch callers. The other two options change `get_container_runtime()`'s return type, contradicting D-06. Apple Container already has `match self.kind` dispatch arms in `runtime.rs`; adding similar Sbx arms is pattern-consistent. The research's `RuntimeHandle` recommendation was on a *technical* argument (sbx breaks 4 RuntimeBase assumptions) that's solved by having `SbxRuntime` be a peer struct that doesn't inherit from `RuntimeBase` — orthogonal to whether the facade wraps it.

**User selection:** Facade with peer SbxRuntime.

## Area 2 — Stub Shape for Unwired Methods

**Question:** Now that is_available() will return true when sbx is on PATH, what should methods that Phase 5 wires up actually do in Phase 2?

**Options presented:**
- Honest split: pure/string methods do real work, queries Ok-empty, action verbs Err
- All Err for anything subprocess-touching
- Keep Phase 1's Ok-everywhere pattern

**User feedback:** "i dont wanna make too many changes to the lib, thoughts?"

**Reframe:** Audited what Phase 1 actually left in place. Three Sbx arms must change to satisfy Phase 2 success criteria (is_available, exec_command, build_create_args). Three existing safe-stub arms (does_container_exist, is_container_running, batch_running_states) are already honest because no container can exist. Action verbs fall through to `RuntimeBase::SBX` which produces broken sbx invocations *if reached*, but Phase 6 is the official wiring point for the settings dropdown — Phase 2 paths can't hit them in real flows.

Refined options to:
- Minimum surface (update only the 3 forced arms)
- Minimum + defensive Errs on 6–9 action verb arms
- Defer the question

**User selection:** "do what you think is best"

**Claude's call:** Minimum surface. Adding defensive Errs to action verbs that Phase 5 immediately rips out is pure churn; the codebase has already accepted the "unreachable in real flows" boundary via Phase 1's safe stubs.

## Area 3 — Argv Builder API + Kit-Path Handling

**Question:** Where do the pure argv builders live, and how do they handle the not-yet-existing kit path?

**Options presented:**
- Free functions in `src/containers/sbx/argv.rs` with `kit_path: Option<&Path>`
- Methods on `SbxRuntime`
- Trait method only (no separate argv module)

**User selection:** Free fns + Option<&Path> kit (Recommended). User clicked through with the rendered preview showing the concrete signature for `build_create_args`.

**Rationale captured:** Free functions are unit-testable without any SbxRuntime instance or sbx binary. Option<&Path> for kit cleanly defers Phase 4's materialization — Phase 2 tests pass None; Phase 4 plumbs Some(materialized_path). Methods-on-SbxRuntime would be symmetric with RuntimeBase but adds boilerplate for no Phase 2 gain. Trait-method-only loses the `argv::*` unit-test target named in success criterion #1.

## Area 4 — is_available() Test Seam

**Question:** How does the is_available() unit test inject a fake binary path?

**Options presented:**
- Configurable `binary: PathBuf` field on SbxRuntime (dependency injection)
- PATH env manipulation in tests
- `which` crate

**User feedback:** "i dont know man..."

**Claude's call:** Configurable binary on SbxRuntime. Reasoning:
- Phase 2 success criterion #3 explicitly requires "validated via a unit test that injects a fake binary path" — some seam is mandatory.
- DI via `PathBuf` field is the cheapest: no new crate dep, no global PATH state, no `#[serial]` requirement.
- PATH manipulation forces serial test execution; `which` adds an unnecessary crate dep (saved-memory feedback flagged "avoid new deps; prefer stdlib + existing runtime checks").
- Mild departure from how Docker/Podman/AppleContainer are constructed (their binaries are hardcoded on `RuntimeBase` consts), but acceptable because SbxRuntime is a peer struct that doesn't have to mirror the `RuntimeBase` pattern.

## Deferred Ideas Captured

- Real `is_daemon_running` composite probe — Phase 5 (RT-02)
- Real subprocess wiring of action verbs — Phase 5
- `pull_image` returns Err(NotSupported) per REQ-RT-06 — Phase 3
- Workspace-path conformance (host_path == container_path) — Phase 3 (RT-04)
- Kit-path resolution via KitMaterializer — Phase 4
- Cross-product editability test (FieldKey × ContainerRuntimeName) — Phase 6 (SET-02)
- End-to-end integration test (`cargo test --test sbx_integration -- --ignored`) — Phase 5

## Notes

- No scope creep raised by the user.
- Two user responses requested clarification rather than picking an option ("i dont wanna be too radical", "i dont wanna make too many changes to the lib") — both expressed the same conservatism principle. Surfaced as a recurring preference; downstream phases should default to minimum-surface choices unless there's a strong reason otherwise.
- The Phase 1 carry-forward (D-05/D-06 specifically) did most of the architectural work for Phase 2; this discussion mostly re-confirmed and refined the boundary of what Phase 2 actually has to ship vs what stays as Phase 1 left it.
