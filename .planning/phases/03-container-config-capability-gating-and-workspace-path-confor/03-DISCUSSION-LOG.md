# Phase 3: container_config Capability Gating and Workspace Path Conformance - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-15
**Phase:** 03-container-config-capability-gating-and-workspace-path-confor
**Areas discussed:** Convenience mount handling, Capability injection seam, Validation error scope & shape, ensure_image skip mechanism

---

## Pre-question research (user-requested)

User pushed back on the framing of the first question ("host_path == container_path vs /workspace like current") and asked for research before locking. Findings via WebSearch + WebFetch against docs.docker.com/ai/sandboxes:

- `sbx create AGENT PATH` mounts the workspace at the absolute host path — there is no `HOST:CONTAINER` syntax and no remap flag. The current Docker `/workspace/<name>` model is unachievable on sbx without kit-side symlink shims that would contradict Phase 1's `supports_arbitrary_volume_paths=false` capability flag.
- Kits inject files (`files/home/`, `initFiles`) but CANNOT define volume mounts.
- Symlinks pointing outside the sandbox don't follow; symlinks into mounted paths do work.
- sbx's native design intent: host-path mounts so error messages, build outputs, and gitignored paths reference paths the user sees outside the sandbox (explicit UX argument in the docs).
- Credentials flow via `sbx secret set -g <service>` + kit `credentials/serviceAuth/proxyManaged` blocks — the real secret never enters the VM. SSH keys via host SSH agent forwarding.

Conclusion: `host_path == container_path` is forced by sbx AND is the better UX. The real Phase 3 decisions are downstream of this — what to do with non-workspace mounts and the validation/error surface.

---

## Convenience mount handling

### Q1: What happens to convenience mounts (gitconfig/SSH/agent-config/GCP/home-seed) under sbx?

| Option | Description | Selected |
|--------|-------------|----------|
| Drop in build_container_config (Recommended) | End-of-function post-pass filters out mounts whose container_path != host_path; tracing::debug on each drop; Phase 4 kit owns credential delivery via native sbx mechanisms | ✓ |
| Drop with explicit Inconformable error path | Same drop but with diagnostic surface for `aoe doctor` later | |
| Keep them; let build_create_args skip silently | Leaves ContainerConfig containing mounts that will never materialize on sbx | |

**User's choice:** Drop in build_container_config.

### Q2: Where in the pipeline does the conformance pass live?

| Option | Description | Selected |
|--------|-------------|----------|
| End-of-function post-pass (Recommended) | One concentrated capability-aware step at the end of build_container_config; Docker path is a no-op | ✓ |
| Early in compute_volume_paths + post-pass for convenience mounts | Two touch points, each smaller | |
| Per-insertion-site capability gates | Scattered if/else around every volumes.push() | |

**User's choice:** End-of-function post-pass.

### Q3: User-declared extra_volumes that can't conform (e.g., `~/.aws:/root/.aws`)?

| Option | Description | Selected |
|--------|-------------|----------|
| Hard error with typed variant (Recommended) | Return Err(ContainerConfigError::InconformableExtraVolume {..}); aoe-internal convenience mounts still drop silently | ✓ |
| Drop with tracing::warn | Silently degrade the sandbox vs the user's stated intent | |
| Auto-conform when possible, drop+warn otherwise | Surprising — silently relocates user-declared mount | |

**User's choice:** Hard error with typed variant.

---

## Capability injection seam

### Q1: How does build_container_config get the runtime capabilities?

| Option | Description | Selected |
|--------|-------------|----------|
| Add `capabilities: RuntimeCapabilities` parameter (Recommended) | One param; instance.rs already has `runtime` in scope; RuntimeCapabilities is Copy+PartialEq+Eq; tests use synthetic literals | ✓ |
| Pass `&dyn ContainerRuntimeInterface` | Heavier signature; requires mock trait impls in tests | |
| Look up via get_container_runtime() inside | Couples function to global singleton; harder to test | |

**User's choice:** Add `capabilities: RuntimeCapabilities` parameter.

### Q2: How does Instance::container_workdir() get the conformed working_dir?

| Option | Description | Selected |
|--------|-------------|----------|
| Shared conformance helper takes (paths, caps) (Recommended) | Extract conform_workspace_paths(); both build_container_config and container_workdir call it | ✓ |
| container_workdir() looks up runtime internally | Reaches into global runtime singleton | |
| Push capabilities into compute_volume_paths | More invasive; every existing test grows a Default::default() | |

**User's choice:** Shared conformance helper takes (paths, caps).

---

## Validation error scope & shape

### Q1: How should the typed validation error be expressed? (User asked clarification first.)

Initial framing rejected; user asked "which changes the codebase the least while doing what we need here?" Claude answered: Option A (new thiserror enum + keep anyhow::Result return) is minimum-churn that satisfies SC-3 "typed variant" — no signature change, one small new type, one downcast_ref test.

| Option | Description | Selected |
|--------|-------------|----------|
| New thiserror enum, anyhow return preserved (Recommended) | ContainerConfigError enum + Err(typed.into()); tests use downcast_ref() | ✓ |
| Change return to Result<_, ContainerConfigError> | Stronger typing but signature ripples through callers | |
| anyhow only, no typed enum | Smallest but violates SC-3 typed-variant wording | |

**User's choice:** A (new thiserror enum, anyhow return preserved).
**Notes:** User explicitly framed this as minimum-churn vs SC compliance trade-off; A satisfies both.

### Q2: Beyond InconformableExtraVolume, are there other Err cases?

User asked Claude to take the lead. Claude picked: just InconformableExtraVolume. Rationale: YAGNI + AGENTS.md explicit "no dead code, never add fields/functions that nothing reads". Phase 3 only has one real Err trigger today; variants get added when a real trigger emerges.

| Option | Description | Selected |
|--------|-------------|----------|
| Just InconformableExtraVolume (Recommended) | One variant; convenience-mount drops are silent; smallest viable contract | ✓ (Claude discretion) |
| InconformableExtraVolume + InconformableWorkspaceMount | Second variant for future-proofing; no known trigger today | |
| Single generic variant Inconformable { mount, runtime, reason } | One variant generic over mount source | |

**User's choice:** (deferred to Claude) — Just InconformableExtraVolume.

### Q3: How does the error identify the runtime?

| Option | Description | Selected |
|--------|-------------|----------|
| ContainerRuntimeName enum (Recommended) | Existing enum; Display/Debug derived; tests assert on the variant value | ✓ |
| Static str/String describing the constraint | Skip the runtime tag; rely on reason field only | |

**User's choice:** ContainerRuntimeName enum.

---

## ensure_image skip mechanism

### Q1: How should the !supports_image_pull skip be implemented and tested?

| Option | Description | Selected |
|--------|-------------|----------|
| Inline guard + capability-level test (Recommended) | Two-line `if runtime.capabilities().supports_image_pull { ... }` at instance.rs:1249-1250; test verifies capability matrix + call-site behavior; no mock runtime needed | ✓ |
| Extract helper fn(runtime, image) | New free fn; test still needs ~30 lines of trait-impl boilerplate | |
| Push gate into ensure_image() itself | Changes semantics of an existing method | |

**User's choice:** Inline guard + capability-level test.

---

## Claude's Discretion

- ContainerConfigError module location (inline at top of container_config.rs vs new peer module) — both fine; inline minimizes churn.
- Anonymous_volumes cleanup for !supports_anonymous_volumes — clear at conformance pass (recommended) vs leave for argv-time skip; either correct.
- Snapshot test mechanism for SC-2 baseline-unchanged — `insta` (new dep, more ergonomic) vs hand-written fixture (zero deps); planner picks.
- Error-variant set (the "Q2" of validation errors) — user explicitly delegated to Claude; chose single-variant per YAGNI/no-dead-code rule.

## Deferred Ideas

- Phase 4 kit `files/home/` + credential proxy + SSH agent forwarding + `sbx secret set -g` integration replaces the convenience mounts dropped here.
- Hook-status-dir relocation under workspace (KIT-04) — Phase 4's call; Phase 3 doesn't touch the existing host_path == container_path hook mount.
- `aoe doctor` diagnostic surface listing dropped mounts — v2 (DIAG-01 already noted in REQUIREMENTS.md).
- Auto-conforming extra_volumes (rewriting `~/.aws:/root/.aws` to `~/.aws:~/.aws`) — rejected for Phase 3; could be revisited if user friction emerges.
- `insta` adoption as a project-wide snapshot-testing dep — decoupled from Phase 3 scope.
