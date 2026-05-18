# Agent of Empires — sbx Container Runtime Backend

## What This Is

Agent of Empires (`aoe`) is a Rust-based TUI/CLI/web tool for spawning and managing parallel AI coding agent sessions (Claude Code, Codex, OpenCode, Gemini, etc.) in isolated sandboxes on top of `tmux`. This milestone adds **Docker Sandboxes (`sbx`)** as a fourth container runtime backend alongside Docker, Podman, and Apple Container — giving users microVM-grade isolation for agent sessions without changing how they interact with `aoe`.

## Core Value

Picking `sbx` from the container runtime setting must be a true drop-in: every aoe flow that works on Docker today (TUI, web dashboard, cockpit, cross-machine, multi-agent) keeps working transparently on sbx, with sbx-specific quirks (out-of-band port publishing, microVM lifecycle) absorbed inside the runtime layer rather than leaked to the user.

## Requirements

### Validated

<!-- Existing capabilities aoe already ships, derived from .planning/codebase/. The sbx backend must not regress any of these. -->

- ✓ Multi-runtime container abstraction with Docker, Podman, Apple Container backends — existing
- ✓ `ContainerRuntimeInterface` trait covering availability, daemon health, version, image pull/exists, container CRUD, exec, batch state — existing (`src/containers/container_interface.rs`)
- ✓ Per-runtime capability flags (`supports_read_only_volumes`, `supports_remove_volumes`) in `runtime_base.rs` — existing
- ✓ Per-session/per-profile/global override resolution for sandbox config (`SandboxConfigOverride`) — existing (`src/session/profile_config.rs`)
- ✓ Settings TUI exposes every sandbox field (image, env, port_mappings, cpu/memory, mount_ssh, runtime selection) — existing (`src/tui/settings/fields.rs`)
- ✓ New-session sandbox dialog exposes Image + Environment overrides; other fields inherit — existing (`src/tui/dialogs/new_session/render.rs:589`)
- ✓ Cockpit, web dashboard, cross-machine, ACP integration all work over the runtime abstraction — existing (`src/cockpit/`, `src/server/`)
- ✓ **REQ-KIT-01:** Single aoe-authored sbx kit at `src/containers/sbx_kit/` embedded via stdlib `include_str!`/`include_bytes!`, materialized to `<app_dir>/sbx-kit/aoe-<ver>-<spec_hash>/<agent>/` on first use — validated in Phase 4
- ✓ **REQ-KIT-02:** Kit invocation uses `sbx create shell --kit <materialized-path> --template <user's image>`, reusing the same image as Docker/Podman — validated in Phase 4 (`kit_path_threads_through_build_create_args` test)
- ✓ **REQ-KIT-03:** Per-agent install dispatch in spec.yaml; status-hook shims write workspace-relative `<workspace>/.aoe-hooks/<id>/status`; kit drops `.gitignore` for `.aoe-hooks/` — validated in Phase 4
- ✓ **REQ-KIT-04:** Content-addressed cache (SHA256 over embedded bytes) invalidates on aoe upgrade or kit content change; 30-day stale-dir GC fires fire-and-forget on cache miss — validated in Phase 4
- ✓ **REQ-RT-01:** `Sbx` is selectable in `ContainerRuntimeName` and is wired through global Settings + profile + session override resolution — validated in Phase 6
- ✓ **REQ-INTEGRATION-01:** Cockpit, web dashboard PTY relay, cross-machine TUI, and ACP cockpit clients all function transparently when the configured runtime is sbx (no surface gets sbx-specific code paths beyond the runtime layer) — validated in Phase 6

### Active

<!-- v1 scope for the sbx backend. Each item is a hypothesis until shipped. -->

- [x] **REQ-RT-01:** `Sbx` is selectable in `ContainerRuntimeName` and is wired through global Settings + profile + session override resolution — validated in Phase 6
- [ ] **REQ-RT-02:** `SbxRuntime` implements `ContainerRuntimeInterface` end-to-end (availability, daemon health via `sbx ls`, version via `sbx version`, container CRUD via `sbx create`/`exec`/`stop`/`rm`, batch state via `sbx ls --json`)
- [ ] **REQ-RT-03:** `ContainerRuntimeInterface` (or `RuntimeBase`) gains capability flags so each backend can declare unsupported semantics: `supports_port_publish_at_create`, `supports_image_pull`, `supports_anonymous_volumes`, `supports_arbitrary_volume_paths`, `supports_dynamic_port_publish`
- [ ] **REQ-RT-04:** Workspace mounts that violate sbx's `host_path == container_path` constraint either auto-conform or surface a clear validation error — never silently mismount
- [ ] **REQ-RT-05:** Resolved `port_mappings` are auto-published via `sbx ports SANDBOX --publish ...` immediately after `sbx create` succeeds; failures surface clearly without rolling back the sandbox
- [ ] **REQ-RT-06:** sbx-specific image semantics (no `pull_image`; templates instead) are no-ops/stubs that match the trait contract without warning the user
- [ ] **REQ-LIFE-01:** sbx lifecycle maps cleanly: `create_container` → `sbx create` (no auto-attach), `start_container` → no-op (sbx auto-starts on first `exec`), `exec` → `sbx exec [-i] [-t] [-e] [-w]`, `stop_container` → `sbx stop`, `remove` → `sbx rm [-f]`
- [x] **REQ-INTEGRATION-01:** Cockpit, web dashboard PTY relay, cross-machine TUI, and ACP cockpit clients all function transparently when the configured runtime is sbx (no surface gets sbx-specific code paths beyond the runtime layer) — validated in Phase 6
- [ ] **REQ-PLATFORM-01:** Backend works on macOS arm64, macOS x86_64, Linux x86_64, Linux arm64
- [ ] **REQ-DOCS-01:** User-facing docs explain host-side prerequisites that aoe does NOT manage: `sbx login`, `sbx policy set-default`, `sbx secret set -g <service>`
- [ ] **REQ-TEST-01:** Unit tests cover sbx argv builders, capability gating, port-publish orchestration, and kit materialization. Integration tests gated behind a feature/`#[ignore]` like existing Docker tests; CI skips when `sbx` is unavailable

### Out of Scope

<!-- Explicit boundaries to prevent scope creep. Each entry includes the why. -->

- **Per-agent native sbx mapping** (`sbx create claude`, `sbx create codex`, etc.) — Decided against in favor of one-kit/one-image; eliminates the "fall back to shell for unsupported aoe agents" branch and matches the user's existing Docker model
- **Authoring or fetching kits at runtime** — The single aoe kit is embedded and shipped with the binary; users never write or pull kits
- **Managing sbx host-side global state** (secrets, policies) — These are user/host concerns; aoe documents them but does not own them. Avoids bleeding aoe state into the user's host sbx daemon
- **Adding port_mappings (or cpu/memory/mount_ssh) to the new-session TUI dialog** — Out of scope for this milestone; the data layer already supports per-profile/per-session overrides via `SandboxConfigOverride`. Promote later if needed
- **Migrating existing Docker-backed sessions to sbx** — Users opt in by changing the runtime setting; no data migration story for in-flight sessions
- **Templating/snapshotting via `sbx template save/load`** — Useful but separate; this milestone focuses on parity, not new capabilities
- **Backwards compatibility shims** — Per repo guidelines (`AGENTS.md`): the trait change to add capability flags is a breaking change; existing backends update in place
- **Windows support** — `aoe` already does not support Windows; sbx does not change that

## Context

**Codebase baseline.** `aoe` is a single-binary Rust app at `src/`, with a clean `ContainerRuntime` abstraction in `src/containers/` that already supports three runtimes (Docker, Podman, AppleContainer). Implementations all shell out to the runtime CLI — no Docker SDK. The `runtime_base.rs` module already has a precedent for capability flags (`supports_read_only_volumes`, `supports_remove_volumes`) that gates command construction. This milestone extends that precedent.

**Sandbox config is layered.** `SandboxConfig` (global, `src/session/config.rs`) → `SandboxConfigOverride` (per-profile/per-session, `src/session/profile_config.rs`) → resolved `ContainerConfig` (passed to runtime, `src/containers/container_interface.rs`). The override struct already supports every field the new-session UI doesn't currently expose, so per-session port mappings work today via profiles or global settings — they just aren't editable in the new-session dialog.

**sbx fundamentals (per docs.docker.com/ai/sandboxes and the `sbx` CLI reference saved at `.planning/sbx-cli-reference.md`):**
- microVM-based isolation; each sandbox runs its own Docker daemon and filesystem
- Workspaces are bind-mounted at the same host path inside the sandbox via positional `sbx create AGENT PATH [PATH:ro]` args
- Ports are published out-of-band: `sbx ports SANDBOX --publish [HOST:]SANDBOX/PROTO` (any time after start)
- `sbx exec` mirrors `docker exec` semantics (`-i`, `-t`, `-e`, `-w`)
- Native agent identifiers (claude, codex, copilot, cursor, docker-agent, droid, gemini, kiro, opencode, shell) — aoe will use `shell` exclusively and layer agent installs via the embedded kit
- Kits provide `install`/`startup`/`initFiles` hooks plus credentials/network/environment blocks; perfect fit for status-hook shimming and per-agent install

**Settings UI shape.** Per `AGENTS.md`: every configurable field must be editable in the settings TUI. Adding `Sbx` to `ContainerRuntimeName` requires the standard wiring (FieldKey, build_*_fields, apply/clear, profile override, merge logic).

**Test substrate.** Existing Docker integration tests are `#[ignore]` in CI and run locally when Docker is present. The same pattern applies for sbx tests; CI gracefully skips. E2E tests that don't need a real container runtime stay green.

## Constraints

- **Tech stack:** Rust 2021, edition `1.85`. Shell out to the `sbx` CLI; no daemon SDK (matches existing runtimes). Per `AGENTS.md`: no `--` em-dashes or dead code; pass `cargo fmt` + `cargo clippy`.
- **Compatibility:** No backwards-compatibility shims for the trait change; this is a breaking internal change called out per `AGENTS.md` policy.
- **Platform:** macOS arm64/x86_64, Linux x86_64/arm64. Windows excluded (`aoe` does not support Windows).
- **External dependency:** Requires `sbx` CLI installed on the host. aoe must detect absence and produce a clear error rather than crashing.
- **Host-side state ownership:** sbx secrets and network policies are user-owned global state; aoe documents requirements but does not modify them.
- **CI:** Real-sbx integration tests must skip when `sbx` is unavailable (matches Docker test pattern); unit tests for argv construction and capability gating must run unconditionally.
- **Settings UI completeness:** Adding `Sbx` runtime requires the full FieldKey/profile/merge/clear wiring per `AGENTS.md` — no half-finished settings.
- **Docs generation:** `cargo xtask gen-docs` keeps `docs/cli/reference.md` in sync; CI enforces.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| sbx is a 4th `ContainerRuntime` peer, not a parallel abstraction | Maximizes reuse of existing UI, settings, profile, and integration plumbing; users see a familiar dropdown | — Pending |
| Add capability flags to the runtime trait rather than `if runtime == Sbx` branches | Existing precedent (`supports_read_only_volumes`); keeps backend differences declarative and testable | — Pending |
| Single embedded aoe kit, single Docker image (reuses existing Image setting) | Eliminates per-agent native sbx mapping and the "fall back to shell" branch; matches the user's Docker mental model | — Pending |
| Kit ships embedded in the binary via stdlib `include_str!`/`include_bytes!`, materialized to cache on first use | No external authoring/fetching; no new crate dep (rejected `include_dir`/`rust-embed` since stdlib suffices for a few small files); cache invalidated on aoe upgrade via aoe-version + content-hash path key | — Pending |
| Auto-publish resolved `port_mappings` post-create via `sbx ports --publish` | Preserves existing UX (user declares ports once, they Just Work); failure surfaces but does not roll back the sandbox | — Pending |
| No new UI fields for this milestone | Scope discipline; `SandboxConfigOverride` already supports everything via profile/global settings; new-session dialog stays as-is | — Pending |
| sbx host-side state (secrets, policy) stays user-owned | Avoids aoe stomping on user/team-level sbx config; documented as prerequisites instead | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-17 after Phase 6 (settings TUI, cross-machine integration, sleep/wake) — REQ-RT-01, REQ-INTEGRATION-01 validated*
