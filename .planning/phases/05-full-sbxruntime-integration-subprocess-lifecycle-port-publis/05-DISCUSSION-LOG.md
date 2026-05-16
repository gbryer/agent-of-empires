# Phase 5: Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 05-full-sbxruntime-integration-subprocess-lifecycle-port-publis
**Areas discussed:** Port-publish orchestration, Daemon health, Subprocess pattern, Module layout, Readiness probe, Testing strategy

---

## Discussion Mode

User requested Claude make all decisions. Guiding constraint: minimize diff surface, no trait signature changes, keep it clean for a public repo PR.

## Port-Publish Orchestration

| Option | Description | Selected |
|--------|-------------|----------|
| Inside create_container | Port-publish as part of the create flow; changes return type | |
| Separate method on SbxRuntime | sbx-specific method; trait unchanged; session layer calls it | ✓ |
| New trait method | Adds publish_ports to ContainerRuntimeInterface | |

**User's choice:** Separate method (no trait change)
**Notes:** User explicitly wanted minimal codebase changes for public PR

## Daemon Health / SbxNotConfigured

| Option | Description | Selected |
|--------|-------------|----------|
| Change trait to Result<bool> | Propagates typed errors; touches all backends | |
| Keep bool, add sbx-specific diagnostic method | Trait unchanged; richer info available via SbxRuntime directly | ✓ |
| New trait method daemon_health() | Adds to interface; all backends must implement | |

**User's choice:** Keep trait as bool, add sbx-specific method
**Notes:** Avoids touching Docker/Podman/AppleContainer code paths

## Subprocess Pattern

| Option | Description | Selected |
|--------|-------------|----------|
| Inline Command::new in each method | Repetitive but explicit | |
| Private run() helper on SbxRuntime | DRY; single error-mapping point | ✓ |
| Shared trait method with default impl | Over-engineered for this case | |

**User's choice:** Private helper (minimal, DRY)

## Module Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Everything in mod.rs | Simple but grows large | |
| mod.rs + parse.rs + ports.rs | Focused files for JSON parsing and port orchestration | ✓ |
| Full split (lifecycle.rs, health.rs, parse.rs, ports.rs) | Over-structured for the scope | |

**User's choice:** Moderate split (parse.rs + ports.rs; action verbs stay in mod.rs)

## Claude's Discretion

- Exact backoff timing values within 30s budget
- Agent name threading mechanism (ContainerConfig field vs separate param)
- Stderr substring matching for "not ready" detection
- batch_running_states filtering strategy (server-side vs client-side)
- Test fixture JSON shape

## Deferred Ideas

- Async subprocess calls (large refactor, no user benefit this milestone)
- Rename DockerError to ContainerError (cosmetic churn)
- Richer aoe doctor sbx diagnostics surface
- Gitconfig identity / agent-config dirs (still deferred from Phase 4)
