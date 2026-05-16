---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: ready_to_plan
stopped_at: Phase 03 complete (2/2) — ready to discuss Phase 4
last_updated: 2026-05-16T03:10:44.936Z
last_activity: 2026-05-16
progress:
  total_phases: 7
  completed_phases: 3
  total_plans: 6
  completed_plans: 6
  percent: 43
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-15)

**Core value:** Picking `sbx` from the container runtime setting must be a true drop-in: every aoe flow that works on Docker today (TUI, web dashboard, cockpit, cross-machine, multi-agent) keeps working transparently on sbx, with sbx-specific quirks (out-of-band port publishing, microVM lifecycle) absorbed inside the runtime layer rather than leaked to the user.
**Current focus:** Phase 4 — embedded sbx kit and materialization

## Current Position

Phase: 4
Plan: Not started
Status: Ready to plan
Last activity: 2026-05-16

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**

- Total plans completed: 2
- Average duration: n/a
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 03 | 2 | - | - |

**Recent Trend:**

- Last 5 plans: n/a
- Trend: n/a

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work (from PROJECT.md and research):

- Foundation: sbx is a 4th `ContainerRuntime` peer, not a parallel abstraction (maximizes reuse of UI/settings/profile/integration plumbing)
- Foundation: Add capability flags to the runtime trait rather than `if runtime == Sbx` branches (extends existing `supports_read_only_volumes` precedent)
- Kit: Single embedded aoe kit, single Docker image (eliminates per-agent native sbx mapping; matches user's Docker mental model)
- Kit: Stdlib `include_str!`/`include_bytes!` for embedding (no new crate dep; rejected `include_dir`/`rust-embed`)
- Port publish: Auto-publish post-create via `sbx ports --publish`; failures surface but do NOT roll back the sandbox
- Scope: No new UI fields this milestone (existing `SandboxConfigOverride` covers everything via profile/global settings)

### Pending Todos

None yet.

### Blockers/Concerns

Two open architectural questions deferred to plan-phase decisions, not roadmap lock-in:

1. **Capability-flag placement** (Phase 1): trait method returning `RuntimeCapabilities` struct vs plain const fields on `RuntimeBase`. Both satisfy REQ-RT-03; trade-off is mockability vs compile-time exhaustiveness. Decide in `/gsd:plan-phase 1`.
2. **Kit materializer shape** (Phase 4): lazy-on-first-use vs eager-at-startup. Stdlib embedding mechanism is locked; the materialization timing is open. Decide in `/gsd:plan-phase 4`.

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-05-15T11:50:25.071Z
Stopped at: Phase 3 context gathered
Resume file: .planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-CONTEXT.md
