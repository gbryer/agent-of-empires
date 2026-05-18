---
phase: 07-user-documentation-and-cli-reference-sync
reviewed: 2026-05-18T18:42:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - docs/guides/sbx.md
  - docs/guides/sandbox.md
  - website/scripts/sync-docs.mjs
  - website/src/data/docsNav.ts
findings:
  critical: 0
  warning: 1
  info: 1
  total: 2
status: issues_found
---

# Phase 7: Code Review Report

**Reviewed:** 2026-05-18T18:42:00Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Phase 7 adds a new Docker Sandboxes (sbx) user guide (`docs/guides/sbx.md`), a cross-reference callout in the existing Docker sandbox guide (`docs/guides/sandbox.md`), and website wiring in `sync-docs.mjs` (PAGES entry, URL_MAP entry) and `docsNav.ts` (sidebar nav item). The website integration follows the established pattern correctly: PAGES array, URL_MAP, and docsNav all have matching entries, and the sync-docs.mjs validation gate will pass. No emdash or `--` separator violations were found in the new content. All referenced sibling guides (`podman.md`, `apple-containers.md`, `workflow.md`, `configuration.md`) exist on disk. The sbx.md technical claims about runtime capabilities (no read-only volumes, no anonymous volumes, post-creation port publishing) are accurate per the source code (`RuntimeBase::SBX` capabilities). The one substantive issue is an incomplete agent coverage table in the new sbx guide.

## Warnings

### WR-01: sbx guide omits three supported agents from the secrets table

**File:** `docs/guides/sbx.md:39-47`
**Issue:** The "Store API secrets" table lists five agents (Claude Code, Codex, Gemini CLI, Hermes, droid) and the follow-up paragraph mentions four multi-provider agents (OpenCode, Pi, Qwen Code, Kiro). This accounts for nine agents total. However, the project supports eleven agents; Cursor CLI, Copilot CLI, and Mistral Vibe are absent from both the table and the multi-provider note. All three have sbx kit settings entries in the source (`SETTINGS_CURSOR` at `container_config.rs:1087`, and agent config mounts for `vibe` and `copilot` at lines 236 and 260). A user running Cursor CLI, Copilot CLI, or Mistral Vibe inside sbx will not know which `sbx secret set -g <service>` command to run, since these agents are not listed anywhere in the guide.

**Fix:** Add the missing agents to the table or the multi-provider note. If their sbx service names are known, add rows to the table. Otherwise, add them to the parenthetical list with a note to use `sbx secret set` interactive mode:

```markdown
   For multi-provider agents (OpenCode, Pi, Qwen Code, Kiro, Cursor CLI, Copilot CLI, Mistral Vibe), set the secret for whichever provider you use. Run `sbx secret set` without arguments to launch interactive mode, which lists all available services.
```

## Info

### IN-01: Nav placement groups sbx guide immediately after Docker Sandbox

**File:** `website/src/data/docsNav.ts:24`
**Issue:** The sbx guide is placed as the second item in the Guides section, directly after "Docker Sandbox". This is a sensible placement for discoverability (users see both Docker isolation options together). No action needed; noting for completeness that this was a deliberate ordering choice rather than an alphabetical insertion.

**Fix:** None required.

---

_Reviewed: 2026-05-18T18:42:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
