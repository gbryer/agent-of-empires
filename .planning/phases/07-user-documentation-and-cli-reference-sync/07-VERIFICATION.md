---
phase: 07-user-documentation-and-cli-reference-sync
verified: 2026-05-18T04:03:03Z
status: human_needed
score: 6/7 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Run cargo xtask gen-docs && git diff --exit-code docs/cli/reference.md"
    expected: "Exit code 0 (no diff between generated and committed CLI reference)"
    why_human: "Rust toolchain not available in verification environment; cannot run cargo xtask gen-docs"
---

# Phase 7: User Documentation and CLI Reference Sync Verification Report

**Phase Goal:** A user-facing guide at docs/guides/sbx.md explains every host-side prerequisite aoe does NOT manage: sbx login, sbx policy set-default, and sbx secret set -g per agent. The doc covers the upgrade path (do NOT recommend sbx reset), kit-format-experimental warning, disk-usage warning, and debug-log location. cargo xtask gen-docs regenerates docs/cli/reference.md to reflect any new clap help and CI enforces the regeneration. The doc is wired into the website per the AGENTS.md recipe.
**Verified:** 2026-05-18T04:03:03Z
**Status:** human_needed
**Re-verification:** No (initial verification)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | docs/guides/sbx.md exists and includes sections for: sbx login setup, default network policy choice with per-agent provider-domain table, sbx secret set -g per agent, upgrade path that explicitly avoids sbx reset, disk-usage warning, debug-log location, and kit-format-experimental warning | VERIFIED | File exists (157 lines). 6 H2 sections: Overview, Prerequisites, Configuration, Usage, Important Notes, Troubleshooting. sbx login at lines 16/131; policy set-default at lines 24/138; 5-row secrets table (anthropic/openai/google/mistral/droid) at lines 39-45; sbx reset WARNING blockquote at line 113; disk-usage at line 103 with "microVM"; kit-format-experimental at lines 105-107; debug.log paths at lines 155-156 with AGENT_OF_EMPIRES_DEBUG at line 150. |
| 2 | The doc is wired into the website per the AGENTS.md recipe: entry in sync-docs.mjs PAGES and URL_MAP, nav entry in docsNav.ts | VERIFIED | sync-docs.mjs PAGES entry at lines 43-49 (source: "docs/guides/sbx.md", dest: "guides/sbx.md", title: "Docker Sandboxes (sbx)"). URL_MAP entry at line 197 ("docs/guides/sbx.md": "/guides/sbx/"). docsNav.ts nav item at line 24 ({ title: "Docker Sandboxes (sbx)", href: "/guides/sbx/" }) immediately after Docker Sandbox. sync-docs.mjs nav validation would pass (href matches computed URL). |
| 3 | cargo xtask gen-docs produces no diff in docs/cli/reference.md | UNCERTAIN | Cannot verify: Rust toolchain not available in this environment. docs/cli/reference.md exists (1121 lines). Per plan analysis and RESEARCH.md, ContainerRuntimeName::Sbx is NOT a clap ValueEnum so gen-docs is expected to produce no diff. User must verify locally. |
| 4 | A first-time sbx user following only the new doc can go from "fresh aoe install" to "Sbx selected and a session running" without consulting source code | VERIFIED | Guide covers complete flow: Docker Desktop prerequisite (line 11), sbx login (line 16), policy set-default with all 3 options (lines 24-35), secret setup per agent (lines 37-47), config file edit with paths for both Linux and macOS (lines 64-69), profile-specific config (lines 76-81), TUI settings mention (line 83), and usage commands (lines 90-94). |
| 5 | docs/guides/sandbox.md contains a callout linking to sbx.md | VERIFIED | Line 11: `> **Looking for microVM-grade isolation?** AoE also supports [Docker Sandboxes (sbx)](sbx.md) for microVM-based sandboxing via Docker Desktop.` Follows existing callout pattern at lines 7-9. |
| 6 | website/scripts/sync-docs.mjs has both PAGES and URL_MAP entries for docs/guides/sbx.md | VERIFIED | "docs/guides/sbx.md" appears exactly 2 times in sync-docs.mjs: once in PAGES (line 44) and once in URL_MAP (line 197). |
| 7 | website/src/data/docsNav.ts has nav entry with href "/guides/sbx/" | VERIFIED | Line 24: `{ title: "Docker Sandboxes (sbx)", href: "/guides/sbx/" }` placed immediately after Docker Sandbox (line 23). |

**Score:** 6/7 truths verified (1 UNCERTAIN requiring human verification)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `docs/guides/sbx.md` | User-facing sbx runtime guide | VERIFIED | 157 lines, all 6 required H2 sections, all DOC-01 content present. Commit 0a95e67. |
| `docs/guides/sandbox.md` | Callout to sbx guide | VERIFIED | Line 11 contains callout with `[Docker Sandboxes (sbx)](sbx.md)` link. Commit f9a8bc4. |
| `website/scripts/sync-docs.mjs` | PAGES + URL_MAP entries for sbx guide | VERIFIED | PAGES entry at lines 43-49, URL_MAP entry at line 197. Commit f9a8bc4. |
| `website/src/data/docsNav.ts` | Nav entry for sbx guide | VERIFIED | `/guides/sbx/` nav item at line 24. Commit f9a8bc4. |
| `docs/cli/reference.md` | Up-to-date CLI reference | UNCERTAIN | File exists (1121 lines, 30KB). Cannot run cargo xtask gen-docs to verify zero-diff. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `website/scripts/sync-docs.mjs` | `docs/guides/sbx.md` | PAGES source field | WIRED | Line 44: `source: "docs/guides/sbx.md"` |
| `website/scripts/sync-docs.mjs` | `website/src/data/docsNav.ts` | Nav validation (navHrefs.has) | WIRED | Lines 317-331: validation loop checks all PAGES have matching docsNav href. `/guides/sbx/` present in docsNav.ts at line 24. |
| `docs/guides/sandbox.md` | `docs/guides/sbx.md` | Relative markdown link in callout | WIRED | Line 11: `[Docker Sandboxes (sbx)](sbx.md)` |
| `docs/guides/sbx.md` | `docs/guides/sandbox.md` | Relative markdown link (PLAN key_link) | NOT_WIRED | No markdown link from sbx.md back to sandbox.md. Line 87 says "usage is identical to the Docker sandbox" but no hyperlink. Minor: PLAN key_link defined this but roadmap SC does not require it. |

### Data-Flow Trace (Level 4)

Not applicable. Phase 7 artifacts are documentation and website configuration files, not components rendering dynamic data.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| sync-docs.mjs nav validation would pass | grep for "/guides/sbx/" in docsNav.ts | Found at line 24 | PASS |
| sbx.md has no emdashes | grep for U+2014 or " -- " | None found | PASS |
| Commits exist in git history | git log --oneline 0a95e67 / f9a8bc4 | Both found | PASS |
| sbx.md appears exactly 2x in sync-docs.mjs | grep -c "docs/guides/sbx.md" | 2 | PASS |

### Probe Execution

No probes defined for this documentation phase. Step 7c: SKIPPED (no probe scripts found or declared).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| DOC-01 | 07-01, 07-02 | User-facing docs explain host-side prerequisites (sbx login, policy set-default, secret set -g), upgrade path (no sbx reset), kit-format-experimental warning, disk-usage note | SATISFIED | All items present in docs/guides/sbx.md. sbx login: line 16. policy set-default: line 24. secret set -g: lines 41-45 (5 agents). Upgrade path with sbx reset warning: lines 109-113. Kit-format-experimental: lines 105-107. Disk-usage: line 103. Doc wired into website: sync-docs.mjs + docsNav.ts entries verified. sandbox.md callout verified. |
| DOC-02 | 07-02 | cargo xtask gen-docs regenerates docs/cli/reference.md; CI-enforced | NEEDS HUMAN | docs/cli/reference.md exists (1121 lines). Cannot run cargo xtask gen-docs in this environment (no Rust toolchain). Per plan analysis, sbx is not a clap ValueEnum so gen-docs should produce no diff. User must verify locally. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none found) | - | - | - | All modified files clean of TODO/FIXME/TBD/XXX/HACK/PLACEHOLDER markers |

### Human Verification Required

### 1. Verify CLI reference is up to date via cargo xtask gen-docs

**Test:** Run `cargo xtask gen-docs && git diff --exit-code docs/cli/reference.md` from the repository root.
**Expected:** Exit code 0 (committed docs/cli/reference.md matches generated output). No diff.
**Why human:** Rust toolchain is not available in the verification environment. This is the CI-enforced check for DOC-02. Per plan analysis, ContainerRuntimeName::Sbx is NOT a clap ValueEnum, so gen-docs is expected to produce no diff from the sbx addition.

### Gaps Summary

No blockers found. All documentation content, website wiring, and cross-linking verified present and correct.

One UNCERTAIN truth (cargo xtask gen-docs zero-diff) cannot be verified without the Rust toolchain. This is routed to human verification. The expectation based on codebase analysis is that it will pass (sbx is not a clap ValueEnum).

One minor plan-level key_link not wired: sbx.md does not contain a markdown link back to sandbox.md. This is not a roadmap success criterion and does not block phase goal achievement.

---

_Verified: 2026-05-18T04:03:03Z_
_Verifier: Claude (gsd-verifier)_
