# Phase 1: Capability Surface and Trait Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-15
**Phase:** 01-capability-surface-and-trait-foundation
**Areas discussed:** Capability surface shape, Sixth flag: host_visible_tmpfs, Sbx variant scope this phase, Platform-support locking

---

## Capability Surface Shape

User initially asked for plain-English clarification on what "trait" and "mockability" meant. After narrowing the real trade-off (compile-time exhaustiveness + read-site cleanliness vs nothing meaningful in the codebase needs mocking), the choice landed on the hybrid.

| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid (trait method + RuntimeBase consts) | `fn capabilities(&self) -> RuntimeCapabilities` on the trait; values come from each runtime's RuntimeBase const. Mockable AND compile-time exhaustive when struct uses explicit field init. | ✓ |
| Trait method returning struct only | Each runtime impl writes its own method body. Mockable but adding a flag doesn't force every backend to update. | |
| Plain const fields on RuntimeBase + sbx const | Matches existing precedent exactly. Compile-time exhaustive but no trait-level surface. | |

**User's choice:** Hybrid.
**Notes:** Required two rounds of clarification — first to explain what a Rust trait is, then to honestly assess that mockability is a weak argument in this codebase (only one impl of the trait exists today; integration tests use the real Docker daemon, not mocks). Real decision driver: compile-time exhaustiveness + cleaner read sites in Phase 3/5. Planner note: capability literals must use explicit field init (`RuntimeCapabilities { ... full field list ... }`) — no `..Default::default()` shortcuts, otherwise the "adding a flag breaks every backend" guarantee fails.

---

## Sixth Flag: host_visible_tmpfs

| Option | Description | Selected |
|--------|-------------|----------|
| Add now in Phase 1 | Bundle the 6th flag with the 5 in REQ-RT-03. Lock the full capability surface in one phase. | |
| Defer to Phase 4 | Stay strictly to REQ-RT-03's 5 flags. Phase 4 adds host_visible_tmpfs when the kit hook fix needs it. | ✓ |

**User's choice:** Defer to Phase 4.
**Notes:** User chose to keep Phase 1's scope narrow (5 flags as REQ-RT-03 specifies). Cost: Phase 4 will need to re-sweep every backend to declare a value for the 6th flag. Benefit: clean phase boundaries — Phase 1 is "REQ-RT-03 as written", not "REQ-RT-03 plus a research-derived bonus".

---

## Sbx Variant Scope This Phase

| Option | Description | Selected |
|--------|-------------|----------|
| Stub-in-place (smaller delta) | Add Sbx enum variant + minimal stub returning is_available()=false. ~12 callers of get_container_runtime() stay untouched. Phase 2 swaps the stub. | ✓ |
| Introduce RuntimeHandle wrapper now | Change return type to enum RuntimeHandle { Cli(ContainerRuntime), Sbx(SbxRuntime) }. Touches all ~12 call sites in Phase 1. Phase 2 just fills in SbxRuntime. | |

**User's choice:** Stub-in-place.
**Notes:** Counted ~12 callers of `get_container_runtime()` across `src/cli/add.rs`, `src/session/instance.rs`, `src/session/builder.rs`, `src/tui/dialogs/new_session/`, `src/server/api/system.rs`, plus `src/containers/mod.rs` itself and tests. User favored keeping the surface change narrow in Phase 1; accepts that Phase 2 must replace the stub without re-touching the ~12 sites. Exact stub mechanism (4th `RuntimeKind` variant with short-circuit vs separate `SbxRuntime` shim) flagged to the planner as a Claude-discretion call.

---

## Platform-Support Locking

| Option | Description | Selected |
|--------|-------------|----------|
| Keep REQ-PLATFORM-01 as written | aoe compiles on all 4 targets. is_available() returns false on platforms where sbx isn't installable. No spec edit. | ✓ |
| Amend REQ-PLATFORM-01 to be sbx-honest | Edit the spec to call out that sbx is supported on macOS arm64 + Linux x86_64; aoe binary still compiles on all 4. | |

**User's choice:** Keep REQ-PLATFORM-01 as written.
**Notes:** Runtime behavior is identical between the two options — the question was purely whether to edit REQ-PLATFORM-01's wording. User chose no edit; `is_available()` carries the entire graceful-degradation contract on Intel Mac and Linux arm64. Phase 1 success criterion #3 (cargo build on macOS arm64 + Linux x86_64) names CI targets, not supported targets.

---

## Claude's Discretion

- **Capability struct file location** — `container_interface.rs` (next to trait) vs new `capabilities.rs` peer module. Either is fine; planner picks the cleanest diff.
- **Capability struct naming** — `RuntimeCapabilities` is the default (matches research vocabulary). Planner may rename if a collision arises.
- **Derived traits on `RuntimeCapabilities`** — `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq` all cheap and useful for tests. Planner picks the set.
- **Stub mechanism for Sbx in Phase 1** — 4th `RuntimeKind` variant with short-circuit in `is_available()` vs a separate tiny `SbxRuntime` shim. Constraint: Phase 2's real `SbxRuntime` peer struct must be droppable without re-touching the ~12 `get_container_runtime()` callers.
- **Compile-time exhaustiveness testing strategy for success criterion #1** — either a one-off test branch the planner spins up to verify the build breaks when a fake flag is added, or a doc comment explaining the contract. Planner picks.

## Deferred Ideas

- **`host_visible_tmpfs` capability flag (6th flag).** Phase 4 adds it when the embedded-kit status-hook fix needs to gate destination paths. All backends re-declare a value at that time: Docker/Podman/AppleContainer = `true`, sbx = `false`.
- **`RuntimeHandle { Cli(ContainerRuntime), Sbx(SbxRuntime) }` wrapper.** Considered for Phase 1; rejected. Phase 2 may revisit when introducing the real `SbxRuntime`.
- **`requires_capability` field on `SettingField`** (PITFALLS C7). Phase 6 (Settings TUI wiring) territory; Phase 1 just makes sure `RuntimeCapabilities` is `Copy + PartialEq` so the field can be referenced cleanly later.
- **PROJECT.md `bundled_sounds/` embedding-precedent correction.** Research caught that PROJECT.md cites bundled_sounds as an embedding precedent, but those are downloaded, not embedded. Real precedent is `rust-embed`. Doc fix, outside Phase 1 scope; surface during Phase 4 (kit) planning.
