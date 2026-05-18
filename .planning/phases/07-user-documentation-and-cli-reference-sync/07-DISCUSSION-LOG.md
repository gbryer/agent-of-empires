# Phase 7: User Documentation and CLI Reference Sync - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md; this log preserves the alternatives considered.

**Date:** 2026-05-18
**Phase:** 07-user-documentation-and-cli-reference-sync
**Areas discussed:** Doc file location, Scope and structure, Existing sandbox guide, Upgrade path detail

---

## Doc File Location

### Question: Where should the sbx guide live?

| Option | Description | Selected |
|--------|-------------|----------|
| docs/guides/sbx.md | Peer with podman.md, apple-containers.md. Consistent with existing structure. | ✓ |
| docs/sandbox/sbx.md | New docs/sandbox/ subdirectory as ROADMAP.md suggests. | |

**User's choice:** docs/guides/sbx.md

### Question: Where in the website nav?

| Option | Description | Selected |
|--------|-------------|----------|
| Right after Docker Sandbox | Groups all sandbox/container runtime guides together. | |
| After Apple Containers group | Add all alternative runtimes as a group. | |
| You decide | Let planner pick the nav position. | ✓ |

**User's choice:** You decide

### Question: What should the guide's title be?

| Option | Description | Selected |
|--------|-------------|----------|
| Docker Sandbox (sbx) | Clarifies sbx IS Docker's sandbox product. | |
| sbx (Docker Sandboxes) | Leads with the CLI name. | |
| Docker Sandboxes (sbx) | Leads with the brand, matches Docker's naming. | ✓ |
| You decide | Let planner pick. | |

**User's choice:** Docker Sandboxes (sbx)

---

## Scope and Structure

### Question: Section structure?

| Option | Description | Selected |
|--------|-------------|----------|
| Same structure | Mirror Podman/Apple Container guides exactly. | ✓ |
| Expanded structure | Add extra top-level sections for sbx-unique topics. | |
| You decide | Let planner pick. | |

**User's choice:** Same structure (mirror peer guides)

### Remaining questions (Secrets table, Existing sandbox guide, Upgrade path)

**User's choice:** User requested fast resolution; all remaining decisions locked by Claude based on peer guide patterns. User explicitly wanted no over-documentation; keep it like the existing guides.

---

## Claude's Discretion

- Nav placement in docsNav.ts (group logically near Docker Sandbox)
- Secrets table scope (kit-supported agents only)
- Exact disk-usage and kit-format-experimental warning wording
- Whether to include "Verify Installation" subsection
- Debug-log location text
- Network policy section level of detail

## Deferred Ideas

None; discussion stayed within phase scope.
