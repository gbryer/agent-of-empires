# Phase 7: User Documentation and CLI Reference Sync - Context

**Gathered:** 2026-05-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship a user-facing guide at `docs/guides/sbx.md` covering host-side prerequisites (`sbx login`, `sbx policy set-default`, `sbx secret set -g`), upgrade path, kit-format-experimental warning, disk-usage note, and debug-log location. Regenerate `docs/cli/reference.md` via `cargo xtask gen-docs`. Wire the new doc into the website (sync-docs.mjs PAGES + URL_MAP, docsNav.ts nav entry). Update the existing `docs/guides/sandbox.md` with a callout to the new guide.

Out of scope: code changes beyond `cargo xtask gen-docs` regeneration; new clap help text (should already be in place from Phase 6); any runtime or config changes.

</domain>

<decisions>
## Implementation Decisions

### Doc File Location and Identity (DOC-01)
- **D-01:** Guide lives at `docs/guides/sbx.md`, peer with `podman.md` and `apple-containers.md`. NOT a new `docs/sandbox/` subdirectory.
- **D-02:** Title: "Docker Sandboxes (sbx)". Matches Docker's own product naming with the CLI name in parentheses.
- **D-03:** Website route: `/guides/sbx/`. Added to sync-docs.mjs PAGES and URL_MAP arrays. Nav entry in docsNav.ts Guides section; exact position is planner discretion (group near Docker Sandbox entry).

### Doc Structure (DOC-01)
- **D-04:** Mirror the structure of existing runtime guides (Podman, Apple Containers): Overview, Prerequisites, Configuration, Usage, Key Differences / Important Notes, Troubleshooting. sbx-specific content (secrets table, network policy choice, disk-usage warning, kit-format-experimental warning) slots into Prerequisites and a concise "Important Notes" section within this structure.
- **D-05:** Secrets table lists only kit-supported agents (the agents whose install commands the kit can actually run). Matches the set determined in Phase 4 D-10. Maps each agent to its `sbx secret set -g <service>` name. Keep it minimal; don't over-document.
- **D-06:** Keep the guide concise and parallel to peer guides. No verbose explanations of sbx internals; users don't need to understand microVM architecture to use it.

### Existing Sandbox Guide Update
- **D-07:** Add a one-line callout to `docs/guides/sandbox.md` pointing users to the sbx guide, following the same pattern as the existing Podman and Apple Containers callouts already in that file (lines 7-9).

### Upgrade Path (DOC-01)
- **D-08:** Document upgrade path via Docker Desktop update mechanism (sbx ships with Docker Desktop). Explicitly warn against `sbx reset` (destroys all sandboxes and data). Link to Docker's own upgrade docs for details. Keep it brief.

### CLI Reference Sync (DOC-02)
- **D-09:** Run `cargo xtask gen-docs` and commit the result. If there's no diff (Phases 5/6 didn't change clap help), commit nothing for this part. The CI docs job must pass with zero diff.

### Claude's Discretion
- Exact nav position in docsNav.ts (group logically near Docker Sandbox)
- Exact wording of the disk-usage warning (should convey "each sandbox is a full microVM; expect ~ImageSize x ActiveSessions of host disk")
- Exact wording of the kit-format-experimental warning (sbx kit format is experimental per Docker docs; schema changes may require aoe updates)
- Whether to include a "Verify Installation" subsection under Prerequisites (following Apple Containers pattern) with `sbx version` and `sbx ls` commands
- Debug-log location text (reference the existing `AGENT_OF_EMPIRES_DEBUG=1` mechanism from AGENTS.md)
- Level of detail in the network policy section (recommend `allow-all` for simplicity or present the three options with trade-offs)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 7 — phase goal, dependencies (Phase 5, Phase 6), four success criteria
- `.planning/REQUIREMENTS.md` — DOC-01 (user-facing docs for host-side prereqs, upgrade path, kit warning, disk usage, debug-log), DOC-02 (CLI reference regeneration via cargo xtask gen-docs)
- `.planning/PROJECT.md` § Key Decisions — sbx host-side state (secrets, policies) stays user-owned; aoe documents but does not manage
- `.planning/PROJECT.md` § Out of Scope — explicit exclusions relevant to doc content (no per-agent native mapping, no sbx login flow inside aoe, etc.)

### Prior phase context (carry-forward)
- `.planning/phases/04-embedded-sbx-kit-and-materialization/04-CONTEXT.md` — D-06 (proxyManaged env vars; user sets secrets via `sbx secret set -g`), D-07 (gitconfig/agent-config deferred; sbx v1 ships without auto-injected git identity)
- `.planning/phases/05-full-sbxruntime-integration-subprocess-lifecycle-port-publis/05-CONTEXT.md` — D-04/D-05 (SbxDaemonHealth enum; `is_daemon_running` detects "not logged in" / "policy not configured")
- `.planning/phases/06-settings-tui-cross-machine-integration-and-sleep-wake/06-CONTEXT.md` — D-01 (field visibility gating; user sees only applicable settings for sbx)

### Peer documentation (the pattern to follow)
- `docs/guides/podman.md` — Podman runtime guide (structure template: Overview, Prerequisites, Configuration, Usage, Compatibility Notes, Troubleshooting)
- `docs/guides/apple-containers.md` — Apple Container runtime guide (structure template with Prerequisites including install + verify + initial setup)
- `docs/guides/sandbox.md` — Docker sandbox guide (the existing guide that gets a callout added)

### Website wiring (the recipe from AGENTS.md)
- `website/scripts/sync-docs.mjs` — PAGES array (source, dest, title, description) and URL_MAP (source path to website URL)
- `website/src/data/docsNav.ts` — navigation sections and items

### sbx CLI reference (content source for the guide)
- `.planning/sbx-cli-reference.md` — full sbx CLI reference; source for the `sbx login`, `sbx policy`, `sbx secret`, `sbx version` command syntax documented in the guide

### Repo policy
- `AGENTS.md` § "Adding a new page to the website" recipe — 4-step process (create docs page, add to PAGES, add to URL_MAP, add nav entry)
- `AGENTS.md` § Coding Style — no em-dashes in prose; comments explain non-obvious "why"
- `AGENTS.md` § Build, Test, and Development Commands — `cargo xtask gen-docs` regenerates CLI reference; CI enforces

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `docs/guides/podman.md` and `docs/guides/apple-containers.md` — direct structural templates for the new guide. Both follow Overview > Prerequisites > Configuration > Usage > Differences/Notes > Troubleshooting.
- `docs/guides/sandbox.md` lines 7-9 — existing callout pattern for alternative runtimes ("AoE also supports [X] as a ..."). The sbx callout follows this exact pattern.
- `website/scripts/sync-docs.mjs` PAGES array — existing entries show the exact shape: `{ source, dest, title, description }`. The sbx entry follows the same shape.
- `website/src/data/docsNav.ts` — existing nav items show `{ title, href }` pairs. The sbx entry follows identically.
- `xtask/src/main.rs::generate_cli_docs()` — existing `cargo xtask gen-docs` implementation. Phase 7 only runs it; no modification needed.

### Established Patterns
- **Runtime guide structure**: all three existing runtime guides (Docker, Podman, Apple Containers) share the same section skeleton. The sbx guide is the fourth peer.
- **Callout pattern in sandbox.md**: existing lines reference Podman and Apple Containers with a one-line blockquote. The sbx callout follows identically.
- **sync-docs.mjs grouping**: guides are grouped by source directory (`docs/guides/ -> pages/guides/`). The sbx entry slots into the same group.

### Integration Points
- `docs/guides/sandbox.md` line 7-9 area — add the sbx callout here alongside the existing Podman/Apple Containers callouts.
- `website/scripts/sync-docs.mjs` PAGES array — add new entry for `docs/guides/sbx.md`.
- `website/scripts/sync-docs.mjs` URL_MAP — add source-to-URL mapping.
- `website/src/data/docsNav.ts` Guides section items array — add nav entry.
- `docs/cli/reference.md` — regenerated by `cargo xtask gen-docs`; no manual edits.

</code_context>

<specifics>
## Specific Ideas

- **Callout in sandbox.md** (following existing pattern at lines 7-9):
  ```markdown
  > **Looking for microVM-grade isolation?** AoE also supports [Docker Sandboxes (sbx)](sbx.md) for microVM-based sandboxing via Docker Desktop.
  ```

- **Secrets table shape** (minimal, in Prerequisites):
  ```markdown
  | Agent | Service Name | Command |
  |-------|-------------|---------|
  | Claude Code | anthropic | `sbx secret set -g anthropic` |
  | Codex | openai | `sbx secret set -g openai` |
  | Gemini CLI | google | `sbx secret set -g google` |
  ```

- **Upgrade warning** (in Important Notes or Troubleshooting):
  ```markdown
  > **Warning:** Do NOT run `sbx reset` to fix issues. This destroys all sandboxes and cached data. Instead, update Docker Desktop to get the latest sbx version.
  ```

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 07-user-documentation-and-cli-reference-sync*
*Context gathered: 2026-05-18*
