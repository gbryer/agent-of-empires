# Phase 1: Capability Surface and Trait Foundation - Context

**Gathered:** 2026-05-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Express runtime capabilities as a first-class surface on `ContainerRuntimeInterface`: migrate the existing two flags (`supports_read_only_volumes`, `supports_remove_volumes`) and add five new flags (`supports_port_publish_at_create`, `supports_image_pull`, `supports_anonymous_volumes`, `supports_arbitrary_volume_paths`, `supports_dynamic_port_publish`) into the same place. Register `Sbx` as a `ContainerRuntimeName` variant with a stub that reports `is_available() = false`. Lock platform-support semantics: aoe compiles on all four supported targets; sbx graceful-degrades via `is_available()` on platforms where the `sbx` CLI is not installed.

Out of scope for this phase: real `SbxRuntime` impl (Phase 2), workspace-path conformance and `ensure_image` gating (Phase 3), embedded kit (Phase 4), Settings TUI wiring (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Capability Surface Shape
- **D-01:** Use the **hybrid** placement. Add `fn capabilities(&self) -> RuntimeCapabilities` to `ContainerRuntimeInterface`. The struct's field values come from each runtime's `RuntimeBase` const (and an sbx-side const later). Gives the trait a mockable surface AND preserves the compile-time exhaustiveness of today's `RuntimeBase` field pattern.
- **D-02:** Capability struct literals MUST use explicit field init for every flag. `..Default::default()` shortcuts are forbidden because they defeat the "adding a new flag breaks every backend until they declare a value" guarantee that Phase 1's success criterion #1 depends on.
- **D-03:** Each backend's capability values live next to where the backend itself is declared (Docker/Podman/AppleContainer values on the existing `RuntimeBase::DOCKER` / `PODMAN` / `APPLE_CONTAINER` consts; sbx values on a sbx-side const introduced here as part of the stub). Adding a new flag is one PR that touches every backend constant — the compiler enforces it.

### Capability Flag Count
- **D-04:** Phase 1 ships exactly the **five** new flags called out in REQ-RT-03, plus migration of the existing two. No `host_visible_tmpfs` flag in this phase — Phase 4 adds the 6th flag when the kit hook fix needs it.

### Sbx Variant Scope
- **D-05:** **Stub-in-place.** Phase 1 adds `Sbx` to `ContainerRuntimeName` and introduces a minimal stub so that `is_available()` for sbx returns `false` on every platform. The ~12 callers of `get_container_runtime()` (`src/cli/add.rs`, `src/session/instance.rs`, `src/session/builder.rs`, `src/tui/dialogs/new_session/`, `src/server/api/system.rs`, plus tests) stay untouched.
- **D-06:** The stub mechanism (4th `RuntimeKind` variant + sbx `RuntimeBase` const with a short-circuit in `is_available()`, vs a separate tiny `SbxRuntime` shim) is a planner decision. Constraint: whatever Phase 1 ships, Phase 2 must be able to swap it for the real `SbxRuntime` peer struct (per research/ARCHITECTURE.md) without re-touching the ~12 caller sites — otherwise the "smaller delta" win evaporates.

### Platform-Support Locking
- **D-07:** REQ-PLATFORM-01 stays as written. aoe code compiles on macOS arm64, macOS x86_64, Linux x86_64, Linux arm64. sbx CLI itself ships only for macOS arm64 + Linux x86_64; on the other two targets, `which sbx` fails and `is_available()` returns false — handled by the same code path as "user hasn't installed sbx". No new `#[cfg(target_os = ...)]` branches anywhere in `src/containers/` or `src/session/` (REQ-RT-08).
- **D-08:** Phase 1 success criterion #3 (cargo build on macOS arm64 + Linux x86_64) names the two CI platforms; it does not narrow the supported-platform list. macOS x86_64 + Linux arm64 must still compile — verified locally or in a follow-up CI matrix if not currently covered.

### Claude's Discretion
- Struct file location (`container_interface.rs` next to the trait, vs new `capabilities.rs` peer module) — planner choice. Either is fine; pick what produces the cleanest diff.
- Naming: `RuntimeCapabilities` vs `Capabilities` vs `ContainerCapabilities` — planner choice. `RuntimeCapabilities` is what research used and is unambiguous; default to that unless there's a collision.
- Derived traits on `RuntimeCapabilities` (`#[derive(Clone, Copy, Debug, PartialEq, Eq)]`) — planner choice. All four are cheap and useful for tests.
- The exact testing strategy for success criterion #1 ("adding a fake flag fails the build") — could be a test branch the planner spins up to verify, or a doc comment explaining the contract. Planner picks.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 1 — phase goal, dependencies, success criteria
- `.planning/REQUIREMENTS.md` — REQ-RT-03 (5 new + 2 migrated flags), REQ-RT-08 (no scattered cfg branches), REQ-PLAT-01 (compile on all 4 targets, is_available gates sbx), REQ-TEST-05 (fmt/clippy/deny gates)
- `.planning/PROJECT.md` § Key Decisions — sbx as 4th `ContainerRuntime` peer; capability flags over `if runtime == Sbx` branches; no backwards-compat shims for the trait change

### Codebase (the surface being modified)
- `src/containers/container_interface.rs` — the `ContainerRuntimeInterface` trait that gains `fn capabilities(&self) -> RuntimeCapabilities`
- `src/containers/runtime_base.rs` § L11-60 — `RuntimeBase` struct with the existing `supports_read_only_volumes` / `supports_remove_volumes` fields and the three runtime consts (`DOCKER`, `APPLE_CONTAINER`, `PODMAN`)
- `src/containers/runtime.rs` — `RuntimeKind` enum + `ContainerRuntime` dispatch; where the Phase 1 stub for sbx lands
- `src/containers/mod.rs` § L15-37 — `runtime_binary()` and `get_container_runtime()`; gain a `ContainerRuntimeName::Sbx` arm
- `src/session/config.rs` § L730-741 — `ContainerRuntimeName` enum that gains the `Sbx` variant; check `#[serde(rename_all = "snake_case")]` so the on-disk value is `"sbx"`

### Research (decision rationale)
- `.planning/research/SUMMARY.md` § Open Questions — capability-placement debate (ARCHITECTURE struct vs PITFALLS const fields); embedding-crate debate (out of Phase 1 scope)
- `.planning/research/ARCHITECTURE.md` — `RuntimeCapabilities` placement recommendation; `RuntimeHandle` wrapper rationale (NOT adopted in Phase 1 per D-05)
- `.planning/research/PITFALLS.md` — C6 (capability-flag placement), C7 (UI honesty via `requires_capability`)

### Repo policy
- `AGENTS.md` § Coding Style — no dead code, no emdash separators, no scattered `#[cfg(target_os = ...)]` outside `src/process/`
- `AGENTS.md` § Settings & Configuration — settings wiring checklist (relevant for Phase 6, but trait choices here must not paint Phase 6 into a corner)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `RuntimeBase` const-pattern (`DOCKER`, `APPLE_CONTAINER`, `PODMAN` at `src/containers/runtime_base.rs:29-60`): each backend declares its config as a `pub const Self`. The new capability fields slot into this pattern directly; the existing two flags (`supports_read_only_volumes`, `supports_remove_volumes`) are the precedent.
- `RuntimeKind` enum (`src/containers/runtime.rs:14-19`): the existing 3-variant dispatch enum for the four runtime-specific operations. If the Phase 1 stub mechanism adds a 4th variant, this is the touch point.
- `EnvEntry` / `VolumeMount` / `ContainerConfig` shapes (`src/containers/container_interface.rs`): not changing in Phase 1, but the planner should be aware these are the trait's data shape — capability flags in Phase 3 will gate which of these shapes are honored.

### Established Patterns
- Trait + dispatch enum: `ContainerRuntimeInterface` (the trait) + `ContainerRuntime` (single struct holding `RuntimeBase + RuntimeKind`) + `match self.kind` for the four runtime-specific cases. Phase 1's stub for sbx must not break this pattern; Phase 2 may evolve it into `RuntimeHandle`.
- `is_available()` uses `--version` exit code (`runtime_base.rs:66-72`). Sbx's stub `is_available()` must return false unconditionally for Phase 1, regardless of whether `sbx` binary happens to be on PATH.
- No `cfg(target_os = ...)` outside `src/process/`: enforced by AGENTS.md and REQ-RT-08. Phase 1 adds zero.

### Integration Points
- `get_container_runtime()` (`src/containers/mod.rs:27`) is the **only** factory; ~12 callers consume it. The Phase 1 stub plus the new `Sbx` arm here is the single integration point that must not break the trait dispatch.
- Settings TUI (`src/tui/settings/fields.rs`) currently lists `ContainerRuntimeName` choices for the runtime dropdown. Phase 6 wires Sbx into that dropdown; Phase 1 must not block that — the `Sbx` enum variant must be addressable from settings code even if non-selectable until Phase 6.
- `cargo deny check` already runs in CI (REQ-TEST-05). The trait change adds no new deps; no allow-list edits expected.

</code_context>

<specifics>
## Specific Ideas

- Compile-time exhaustiveness contract: the literal pattern is `RuntimeCapabilities { supports_read_only_volumes: ..., supports_remove_volumes: ..., supports_port_publish_at_create: ..., supports_image_pull: ..., supports_anonymous_volumes: ..., supports_arbitrary_volume_paths: ..., supports_dynamic_port_publish: ..., }` — every field named, no defaults. Adding an 8th field forces every existing literal to be edited or the build fails. This is the success-criterion-1 mechanism in Phase 1.
- "Hybrid" mechanism sketch (planner can iterate): on `RuntimeBase`, replace the two `pub bool` fields with a single `pub capabilities: RuntimeCapabilities` field. Each const initializes the full struct. The trait's `fn capabilities(&self) -> RuntimeCapabilities` is implemented once on `ContainerRuntime` as `self.base.capabilities`. Sbx's stub provides its own const (or its own struct) returning the same shape.
- Sbx stub honest values: per research and REQ surface — `supports_port_publish_at_create=false`, `supports_image_pull=false`, `supports_anonymous_volumes=false`, `supports_arbitrary_volume_paths=false`, `supports_dynamic_port_publish=true`. Migrated `supports_read_only_volumes` / `supports_remove_volumes` — set whatever is honest for sbx (likely both `false` based on the microVM/kit model; planner confirms during planning).
- Honest values for Docker/Podman on the five new flags: `supports_port_publish_at_create=true`, `supports_image_pull=true`, `supports_anonymous_volumes=true`, `supports_arbitrary_volume_paths=true`, `supports_dynamic_port_publish=false` (Docker requires `-p` at run time; not dynamic without `docker run --network=host` workarounds). AppleContainer values may differ — planner confirms during research.

</specifics>

<deferred>
## Deferred Ideas

- **`host_visible_tmpfs` capability flag (6th flag).** Defers to Phase 4 (Embedded kit). When Phase 4 adds it, all existing backends must re-declare a value: Docker/Podman/AppleContainer = `true`, sbx = `false`. Update REQ-RT-03 to list 6 flags at that point (or add a new requirement; planner choice). This is the prerequisite for the status-hook-shim destination switch (workspace-relative path instead of `/tmp/aoe-hooks/`).
- **`RuntimeHandle { Cli(ContainerRuntime), Sbx(SbxRuntime) }` wrapper.** Considered for Phase 1 and rejected (D-05 chose stub-in-place). Phase 2 may revisit this when introducing the real `SbxRuntime` — at that point, the trade-off is whether to change `get_container_runtime()`'s return type or to grow `RuntimeKind`/`ContainerRuntime` to encompass sbx. Phase 2 decides; not a Phase 1 concern.
- **`requires_capability: Option<fn(&RuntimeCapabilities) -> bool>` on `SettingField`** (PITFALLS C7). Belongs in Phase 6 (Settings TUI wiring). Phase 1 only needs to ensure the capability struct shape is something `SettingField` can reference cleanly later — `Copy + PartialEq` should be sufficient.
- **`bundled_sounds/` embedding-precedent correction in PROJECT.md.** Research surfaced that PROJECT.md cites bundled_sounds as an embedding precedent, but those are downloaded, not embedded. Real precedent is `rust-embed` for `web/dist/`. Doc-only correction; outside Phase 1 trait/capability scope. Surface during Phase 4 (kit) planning.

</deferred>

---

*Phase: 01-capability-surface-and-trait-foundation*
*Context gathered: 2026-05-15*
