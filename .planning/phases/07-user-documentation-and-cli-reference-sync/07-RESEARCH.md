# Phase 7: User Documentation and CLI Reference Sync - Research

**Researched:** 2026-05-18
**Domain:** Documentation authoring, website integration, CI enforcement
**Confidence:** HIGH

## Summary

Phase 7 is a documentation-only phase with no runtime code changes. The work is: (1) author `docs/guides/sbx.md` following the established structure of peer runtime guides (Podman, Apple Containers), (2) wire it into the website via `sync-docs.mjs` and `docsNav.ts`, (3) add a callout in `docs/guides/sandbox.md`, and (4) run `cargo xtask gen-docs` and commit if there's a diff.

The existing codebase provides strong structural templates. `docs/guides/podman.md` and `docs/guides/apple-containers.md` follow identical section skeletons (Overview, Prerequisites, Configuration, Usage, Key Differences/Notes, Troubleshooting). The sbx guide mirrors this skeleton with sbx-specific content (secrets table, network policy, disk-usage warning, kit-format-experimental warning, upgrade path). All content sources are well-documented: the sbx CLI reference is at `.planning/sbx-cli-reference.md`, the kit spec is at `src/containers/sbx_kit/spec.yaml`, and agent-to-service mappings are derivable from the sbx secret service list and aoe's agent registry.

**Primary recommendation:** Author the guide as a direct structural peer to podman.md and apple-containers.md, using the exact same section headings. Keep it concise per D-06; users need setup steps, not sbx internals.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Guide lives at `docs/guides/sbx.md`, peer with `podman.md` and `apple-containers.md`. NOT a new `docs/sandbox/` subdirectory.
- **D-02:** Title: "Docker Sandboxes (sbx)". Matches Docker's own product naming with the CLI name in parentheses.
- **D-03:** Website route: `/guides/sbx/`. Added to sync-docs.mjs PAGES and URL_MAP arrays. Nav entry in docsNav.ts Guides section; exact position is planner discretion (group near Docker Sandbox entry).
- **D-04:** Mirror the structure of existing runtime guides (Podman, Apple Containers): Overview, Prerequisites, Configuration, Usage, Key Differences / Important Notes, Troubleshooting. sbx-specific content (secrets table, network policy choice, disk-usage warning, kit-format-experimental warning) slots into Prerequisites and a concise "Important Notes" section within this structure.
- **D-05:** Secrets table lists only kit-supported agents (the agents whose install commands the kit can actually run). Matches the set determined in Phase 4 D-10. Maps each agent to its `sbx secret set -g <service>` name. Keep it minimal; don't over-document.
- **D-06:** Keep the guide concise and parallel to peer guides. No verbose explanations of sbx internals; users don't need to understand microVM architecture to use it.
- **D-07:** Add a one-line callout to `docs/guides/sandbox.md` pointing users to the sbx guide, following the same pattern as the existing Podman and Apple Containers callouts already in that file (lines 7-9).
- **D-08:** Document upgrade path via Docker Desktop update mechanism (sbx ships with Docker Desktop). Explicitly warn against `sbx reset` (destroys all sandboxes and data). Link to Docker's own upgrade docs for details. Keep it brief.
- **D-09:** Run `cargo xtask gen-docs` and commit the result. If there's no diff (Phases 5/6 didn't change clap help), commit nothing for this part. The CI docs job must pass with zero diff.

### Claude's Discretion
- Exact nav position in docsNav.ts (group logically near Docker Sandbox)
- Exact wording of the disk-usage warning (should convey "each sandbox is a full microVM; expect ~ImageSize x ActiveSessions of host disk")
- Exact wording of the kit-format-experimental warning (sbx kit format is experimental per Docker docs; schema changes may require aoe updates)
- Whether to include a "Verify Installation" subsection under Prerequisites (following Apple Containers pattern) with `sbx version` and `sbx ls` commands
- Debug-log location text (reference the existing `AGENT_OF_EMPIRES_DEBUG=1` mechanism from AGENTS.md)
- Level of detail in the network policy section (recommend `allow-all` for simplicity or present the three options with trade-offs)

### Deferred Ideas (OUT OF SCOPE)
None.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DOC-01 | User-facing docs explain host-side prerequisites that aoe does NOT manage: `sbx login`, `sbx policy set-default`, and `sbx secret set -g <service>` per agent. Includes upgrade path (do NOT recommend `sbx reset`), kit-format-experimental warning, and disk-usage note | Peer guide structure verified (podman.md, apple-containers.md); sbx CLI reference verified at `.planning/sbx-cli-reference.md`; kit-supported agents identified (9 agents); secret service names verified from sbx CLI reference; website wiring pattern verified (sync-docs.mjs, docsNav.ts) |
| DOC-02 | `cargo xtask gen-docs` regenerates `docs/cli/reference.md` to reflect any new clap help; CI-enforced | CI enforcement verified at `.github/workflows/ci.yml:76-80` (gen-docs + git diff check); `ContainerRuntimeName` is NOT a clap `ValueEnum` so gen-docs likely produces no diff from sbx addition alone; the guide explains runtime selection is via config/TUI, not CLI |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| sbx user guide authoring | Documentation (markdown) | -- | Pure content creation; no runtime code |
| Website integration | Static site build (Astro) | -- | sync-docs.mjs + docsNav.ts wiring; build-time only |
| sandbox.md callout | Documentation (markdown) | -- | One-line addition to existing file |
| CLI reference regeneration | Build tooling (xtask) | CI enforcement | `cargo xtask gen-docs` produces the artifact; CI validates freshness |

## Standard Stack

This phase involves no new library dependencies. The tools are all existing project infrastructure.

### Core
| Tool | Location | Purpose | Why Standard |
|------|----------|---------|--------------|
| Markdown (docs/) | `docs/guides/sbx.md` | User-facing guide content | All aoe guides are markdown; synced to website via sync-docs.mjs |
| sync-docs.mjs | `website/scripts/sync-docs.mjs` | Sync docs to Astro pages, rewrite links, validate nav coverage | Existing automation; all docs pages go through this |
| docsNav.ts | `website/src/data/docsNav.ts` | Sidebar navigation data | Single source of truth for website navigation |
| cargo xtask gen-docs | `xtask/src/main.rs` | Generate CLI reference from clap definitions | CI-enforced; existing tooling |

### Supporting
| Tool | Location | Purpose | When to Use |
|------|----------|---------|-------------|
| build-site.sh | `scripts/build-site.sh` | Full website build (calls npm run build which calls sync-docs) | Only for local verification of website build |

## Architecture Patterns

### System Architecture Diagram

```
docs/guides/sbx.md (NEW)
        |
        v
sync-docs.mjs (PAGES + URL_MAP entries)
        |
        +---> Astro page at website/src/pages/guides/sbx.md (generated, gitignored)
        |
        +---> Link rewriting (relative .md links -> website URLs)
        |
        +---> Nav validation (checks docsNav.ts has matching href)
        |
        v
docsNav.ts (NEW nav item in Guides section)
        |
        v
Astro build -> dist/ -> GitHub Pages deployment

docs/guides/sandbox.md (MODIFIED: add callout line)
docs/cli/reference.md (REGENERATED by cargo xtask gen-docs if diff exists)
```

### Recommended File Changes

```
docs/
  guides/
    sbx.md              # NEW: user-facing sbx guide
    sandbox.md          # MODIFIED: add callout to sbx guide
  cli/
    reference.md        # REGENERATED: cargo xtask gen-docs (if diff)

website/
  scripts/
    sync-docs.mjs       # MODIFIED: add PAGES entry + URL_MAP entry
  src/
    data/
      docsNav.ts        # MODIFIED: add nav item in Guides section
```

### Pattern 1: Adding a New Page to the Website

**What:** The AGENTS.md-prescribed 4-step recipe for wiring a new doc page into the website.
**When to use:** Every time a new docs/ file needs to appear on agent-of-empires.com.

**Steps:** [VERIFIED: codebase inspection of sync-docs.mjs, docsNav.ts, AGENTS.md]

1. Create the page in `docs/` (first line must be `# Title`).
2. Add an entry to `PAGES` array in `website/scripts/sync-docs.mjs`:
   ```javascript
   {
     source: "docs/guides/sbx.md",
     dest: "guides/sbx.md",
     title: "Docker Sandboxes (sbx)",
     description: "Set up Docker Sandboxes (sbx) as a microVM-based sandbox runtime for Agent of Empires.",
   },
   ```
3. Add the source-to-URL mapping in `URL_MAP`:
   ```javascript
   "docs/guides/sbx.md": "/guides/sbx/",
   ```
4. Add a nav entry in `website/src/data/docsNav.ts` in the Guides section:
   ```typescript
   { title: "Docker Sandboxes (sbx)", href: "/guides/sbx/" },
   ```

**Validation:** `sync-docs.mjs` exits with code 1 if a PAGES entry lacks a matching docsNav.ts href. The `npm run build` command in the website directory runs sync-docs.mjs automatically. [VERIFIED: sync-docs.mjs lines 308-323]

### Pattern 2: Peer Runtime Guide Structure

**What:** All existing runtime guides follow the same section skeleton. [VERIFIED: podman.md, apple-containers.md]
**When to use:** The sbx guide must follow this structure per D-04.

```markdown
# Title

## Overview
[1-2 paragraphs: what this runtime is, why users choose it]

## Prerequisites
[Numbered list of host-side requirements]
### Verify Installation (optional, per Apple Containers pattern)
[Commands to confirm the runtime is ready]

## Configuration
[How to set this runtime in aoe config]
### Profile-Specific Runtime
[How to scope to a profile]

## Usage
[Standard aoe commands; "identical to Docker sandbox"]

## Key Differences / Important Notes
[Runtime-specific behaviors users need to know]

## Troubleshooting
[Common problems and solutions]
```

### Pattern 3: Callout Pattern in sandbox.md

**What:** Existing callouts at lines 7-9 of sandbox.md use blockquote format. [VERIFIED: docs/guides/sandbox.md lines 7-9]
**When to use:** Adding the sbx callout per D-07.

```markdown
> **Looking for microVM-grade isolation?** AoE also supports [Docker Sandboxes (sbx)](sbx.md) for microVM-based sandboxing via Docker Desktop.
```

### Anti-Patterns to Avoid
- **Explaining sbx internals:** Per D-06, users don't need to understand microVM architecture. Keep it to setup steps and gotchas.
- **Documenting per-agent native sbx mapping:** This is explicitly Out of Scope. aoe uses `sbx create shell --kit ...` exclusively; the guide should not mention `sbx create claude` etc.
- **Recommending `sbx reset`:** Per D-08, this is destructive and must be warned against, never recommended.
- **Editing docs/cli/reference.md by hand:** This file is auto-generated; edit clap help in `src/cli/` and re-run `cargo xtask gen-docs` instead. [VERIFIED: AGENTS.md]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Link rewriting for website | Custom link-rewrite logic | sync-docs.mjs's `rewriteLinks()` | Handles relative-to-absolute, cross-doc, and anchor links automatically |
| Nav validation | Manual checking | sync-docs.mjs exit-code check at lines 308-323 | Build fails if nav entry is missing |
| CLI reference content | Manual markdown | `cargo xtask gen-docs` (uses `clap_markdown`) | CI enforces freshness; manual edits will drift |

## Common Pitfalls

### Pitfall 1: Missing URL_MAP Entry
**What goes wrong:** sync-docs.mjs rewrites `[text](sbx.md)` links using URL_MAP. If the new file isn't in URL_MAP, relative links from other docs (like sandbox.md's callout) won't rewrite correctly and will fall back to GitHub blob URLs.
**Why it happens:** Easy to add PAGES entry but forget URL_MAP.
**How to avoid:** Add both PAGES and URL_MAP entries in the same commit. The URL for `docs/guides/sbx.md` should be `/guides/sbx/`.
**Warning signs:** `sync-docs.mjs` output shows warnings about unresolved links (though this is best-effort, not a hard failure for link rewriting).

### Pitfall 2: Nav Entry href Mismatch
**What goes wrong:** `sync-docs.mjs` validates that every PAGES entry has a matching href in docsNav.ts. The href is computed as `"/" + dest.replace(/\.md$/, "/").replace(/\/index\/$/, "/")`. For `dest: "guides/sbx.md"`, the expected href is `/guides/sbx/`. [VERIFIED: sync-docs.mjs lines 314-315]
**Why it happens:** Typo in the href (missing trailing slash, wrong path segment).
**How to avoid:** Use exactly `/guides/sbx/` as the href.
**Warning signs:** `npm run build` in the website directory fails with "WARNING: /guides/sbx/ (from docs/guides/sbx.md) is not in docsNav.ts".

### Pitfall 3: Podman/Apple Container Guides Are NOT Wired into the Website
**What goes wrong:** Assuming Podman and Apple Container guides are already in sync-docs.mjs as a pattern to follow.
**Why it happens:** These guides exist as `docs/guides/podman.md` and `docs/guides/apple-containers.md` but are NOT in the sync-docs.mjs PAGES array, NOT in URL_MAP, and NOT in docsNav.ts. [VERIFIED: grep found zero matches]
**How to avoid:** Don't reference the Podman/Apple Container website wiring as a template. Use the sandbox.md entry in sync-docs.mjs as the structural template for PAGES/URL_MAP entries. The sbx guide will be the first runtime-alternative guide wired into the website.
**Warning signs:** N/A (just a knowledge gap, not a runtime error).

### Pitfall 4: cargo xtask gen-docs May Produce No Diff
**What goes wrong:** Expecting a diff from gen-docs and committing a no-op.
**Why it happens:** `ContainerRuntimeName::Sbx` is a serde enum variant and a TUI setting, but it is NOT a clap `ValueEnum` and does not appear in any CLI subcommand's `#[arg]` definitions. The `aoe add --sandbox` flag is a boolean; the runtime is set via config.toml or TUI settings. Therefore, `cargo xtask gen-docs` (which uses `clap_markdown::help_markdown`) will likely NOT produce a diff from the sbx addition. [VERIFIED: grep for ValueEnum and ContainerRuntimeName in src/cli/ found no matches]
**How to avoid:** Run `cargo xtask gen-docs` and check `git diff docs/cli/reference.md`. If no diff, skip the commit for this part (per D-09). The CI check at `.github/workflows/ci.yml:76-80` will pass as long as the committed reference.md matches what gen-docs produces.
**Warning signs:** `git diff --exit-code docs/cli/reference.md` exits 0 (no diff).

### Pitfall 5: First Line Must Be a Heading
**What goes wrong:** sync-docs.mjs strips the leading `# Title` line (line 283: `content.replace(/^# .+\n\n?/, "")`). If the file doesn't start with `# Title`, the stripping regex won't match and the title will be duplicated (once in Astro frontmatter, once in content).
**Why it happens:** Markdown file doesn't follow the convention.
**How to avoid:** Start `docs/guides/sbx.md` with exactly `# Docker Sandboxes (sbx)` as the first line, matching D-02.

## Code Examples

### sync-docs.mjs PAGES Entry Shape

```javascript
// Source: website/scripts/sync-docs.mjs lines 20-170
// Existing entry for sandbox.md as structural template:
{
  source: "docs/guides/sandbox.md",
  dest: "guides/sandbox.md",
  title: "Docker Sandbox: Quick Reference",
  description:
    "Run AI coding agents in isolated Docker containers with Agent of Empires.",
},
```

### docsNav.ts Entry Shape

```typescript
// Source: website/src/data/docsNav.ts lines 24-35
// Guides section items array:
{
  title: "Guides",
  items: [
    { title: "Docker Sandbox", href: "/guides/sandbox/" },
    // sbx entry should be placed immediately after Docker Sandbox
    // Other entries follow...
  ],
},
```

### sandbox.md Callout Pattern

```markdown
// Source: docs/guides/sandbox.md lines 7-9
> **Linux users:** AoE also supports [Podman](podman.md) as a daemonless, rootless-friendly alternative to Docker.
>
> **macOS users:** AoE also supports [Apple Containers](apple-containers.md) as a native alternative to Docker Desktop.
```

### Kit-Supported Agents and Secret Service Mapping

The kit materializer supports 9 agents [VERIFIED: src/containers/sbx/kit.rs line 339]:
`claude`, `codex`, `opencode`, `gemini`, `droid`, `pi`, `qwen`, `kiro`, `hermes`

The sbx secret available services [VERIFIED: .planning/sbx-cli-reference.md sbx secret set section]:
`anthropic`, `aws`, `cursor`, `droid`, `github`, `google`, `groq`, `mistral`, `nebius`, `openai`, `xai`

Agent-to-service mapping for the secrets table:

| Agent | Service Name | Command |
|-------|-------------|---------|
| Claude Code | anthropic | `sbx secret set -g anthropic` |
| Codex | openai | `sbx secret set -g openai` |
| Gemini CLI | google | `sbx secret set -g google` |
| Hermes | mistral | `sbx secret set -g mistral` |
| Qwen Code | -- (uses QWEN_API_KEY via proxyManaged; no matching sbx service yet) | N/A |
| OpenCode | -- (multi-provider; user sets whichever provider secret they use) | Depends on provider |
| droid | droid | `sbx secret set -g droid` |
| Kiro CLI | -- (uses AWS credentials typically) | `sbx secret set -g aws` [ASSUMED] |
| Pi | -- (multi-provider) | Depends on provider |

**Note on proxyManaged:** The kit's `spec.yaml` declares these env vars as `proxyManaged`: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `MISTRAL_API_KEY`, `QWEN_API_KEY`, `DEEPSEEK_API_KEY`. The sbx proxy injects the real secret from the user's `sbx secret set -g <service>` store; the credential never enters the microVM. [VERIFIED: src/containers/sbx_kit/spec.yaml lines 156-164]

**Recommendation for the secrets table:** List only agents with a clear 1:1 mapping (Claude/anthropic, Codex/openai, Gemini/google, Hermes/mistral, droid/droid). For multi-provider agents (OpenCode, Pi, Qwen Code, Kiro), add a brief note that users should set the secret for whichever provider they use, consulting `sbx secret set` interactive mode. This keeps the table concise per D-06.

### Debug Log Location

Debug logging is enabled via `AGENT_OF_EMPIRES_DEBUG=1`. The log file is written to `<app_dir>/debug.log` where `<app_dir>` is: [VERIFIED: src/main.rs line 71, AGENTS.md "Debug logging" section]
- **Linux:** `~/.config/agent-of-empires/debug.log`
- **macOS:** `~/.agent-of-empires/debug.log`
- **Debug builds:** `~/.config/agent-of-empires-dev/debug.log` (Linux) or `~/.agent-of-empires-dev/debug.log` (macOS)

### Network Policy Options

The three default network policies [VERIFIED: .planning/sbx-cli-reference.md sbx policy set-default section]:

| Policy | Description |
|--------|-------------|
| `allow-all` | All outbound network traffic is allowed |
| `balanced` | Common dev traffic allowed (AI services, package registries, etc.) |
| `deny-all` | All outbound network traffic is blocked |

**Recommendation for the guide:** Present all three options with a one-sentence description each, and recommend `allow-all` for simplicity or `balanced` as a sensible default. Don't go deep into custom policy rules (that's sbx documentation territory, not aoe's).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-runtime docs in `docs/sandbox/` subdirectory | Flat `docs/guides/` peer files | Decision D-01 (this phase) | sbx.md lives alongside podman.md, not in a new subdirectory |
| Podman/Apple Container guides not on website | Still not on website | Pre-existing gap | sbx guide will be the first runtime-alternative guide wired into the website; Podman/Apple Container remain docs-only for now |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Kiro CLI uses AWS credentials and maps to `sbx secret set -g aws` | Code Examples (secrets table) | Low; the secrets table can omit Kiro or note "provider-dependent" instead |
| A2 | `cargo xtask gen-docs` will produce no diff after Phases 5/6 | Common Pitfalls (Pitfall 4) | Low; if it does produce a diff, the task is simply "commit the regenerated file" |

## Open Questions

1. **Should Podman and Apple Container guides be wired into the website in this phase?**
   - What we know: Both guides exist as `docs/guides/podman.md` and `docs/guides/apple-containers.md` but are NOT in sync-docs.mjs or docsNav.ts. [VERIFIED: grep found zero matches]
   - What's unclear: Whether this is intentional (perhaps they were added after the website launched and the wiring was overlooked) or deliberate (perhaps they're considered too niche for the website).
   - Recommendation: Out of scope for Phase 7 per the phase boundary. If desired, it's a separate follow-up task. Phase 7 only adds the sbx guide.

2. **Exact subset of agents for the secrets table**
   - What we know: 9 agents are kit-supported. Only some have clear 1:1 sbx secret service mappings. [VERIFIED: kit.rs line 339, sbx-cli-reference.md]
   - What's unclear: Whether the guide should list all 9 with "provider-dependent" notes for multi-provider agents, or only the clear 1:1 mappings.
   - Recommendation: List the clear 1:1 mappings (Claude/anthropic, Codex/openai, Gemini/google, Hermes/mistral, droid/droid) and add a general note for multi-provider agents. This follows D-05 ("keep it minimal; don't over-document") and D-06 ("keep the guide concise").

## Environment Availability

Step 2.6: SKIPPED (no external dependencies identified). This phase is purely documentation authoring, website config file edits, and running an existing xtask command. No new tools, services, or runtimes are required beyond what the project already uses (Rust toolchain for `cargo xtask gen-docs`, Node.js for `npm run build` website verification).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo xtask gen-docs (built-in, no external test framework) |
| Config file | `.github/workflows/ci.yml` (CI enforcement) |
| Quick run command | `cargo xtask gen-docs && git diff --exit-code docs/cli/reference.md` |
| Full suite command | `cd website && npm install && npm run build` (validates sync-docs + nav) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOC-01 | sbx guide exists with required sections | manual-only | Visual inspection of `docs/guides/sbx.md` | N/A (new file) |
| DOC-01 | Guide wired into website (PAGES, URL_MAP, nav) | build validation | `cd website && npm run build` | sync-docs.mjs exits 1 on missing nav |
| DOC-01 | sandbox.md callout added | manual-only | `grep "sbx" docs/guides/sandbox.md` | Existing file |
| DOC-02 | CLI reference up to date | CI-enforced | `cargo xtask gen-docs && git diff --exit-code docs/cli/reference.md` | Existing CI job |

### Sampling Rate
- **Per task commit:** `cargo xtask gen-docs && git diff --exit-code docs/cli/reference.md`
- **Per wave merge:** `cd website && npm install && npm run build`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
None. Existing test infrastructure (CI gen-docs check, sync-docs.mjs nav validation) covers all automated phase requirements.

## Security Domain

security_enforcement: not applicable for this documentation-only phase. No code changes, no user input handling, no network operations, no authentication logic. All changes are static markdown content and JavaScript/TypeScript config file entries.

## Sources

### Primary (HIGH confidence)
- `docs/guides/podman.md` - peer guide structure template (read in full)
- `docs/guides/apple-containers.md` - peer guide structure template (read in full)
- `docs/guides/sandbox.md` - existing callout pattern at lines 7-9 (read in full)
- `website/scripts/sync-docs.mjs` - PAGES, URL_MAP, link rewriting, nav validation (read in full)
- `website/src/data/docsNav.ts` - navigation structure (read in full)
- `.planning/sbx-cli-reference.md` - sbx CLI commands, secret services, policy options (read in full)
- `src/containers/sbx_kit/spec.yaml` - proxyManaged env vars, kit structure (read in full)
- `src/containers/sbx/kit.rs` line 339 - kit-supported agent list (verified via grep)
- `src/session/config.rs` lines 734-742 - ContainerRuntimeName enum (Sbx is NOT a clap ValueEnum)
- `.github/workflows/ci.yml` lines 76-80 - gen-docs CI enforcement (verified)
- `.github/workflows/docs.yml` - website deployment workflow (verified)
- `scripts/build-site.sh` + `website/package.json` - build chain includes sync-docs (verified)
- `xtask/src/main.rs` - gen-docs implementation (verified)
- `src/main.rs` line 71 - debug.log location (`get_app_dir().join("debug.log")`)
- AGENTS.md - project conventions, "Adding a new page" recipe, debug logging section

### Secondary (MEDIUM confidence)
- `.planning/phases/04-embedded-sbx-kit-and-materialization/04-CONTEXT.md` D-10 - kit-supported agent subset

### Tertiary (LOW confidence)
- Kiro CLI to `aws` secret service mapping (A1 in Assumptions Log)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all tools are existing project infrastructure, verified by reading source
- Architecture: HIGH - documentation-only phase with clear file change list
- Pitfalls: HIGH - all pitfalls verified by reading the actual validation code in sync-docs.mjs and CI workflows

**Research date:** 2026-05-18
**Valid until:** 2026-06-18 (stable; docs infrastructure unlikely to change)
