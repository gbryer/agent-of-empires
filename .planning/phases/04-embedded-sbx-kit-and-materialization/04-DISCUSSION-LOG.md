# Phase 4: Embedded sbx Kit and Materialization - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 04-embedded-sbx-kit-and-materialization
**Areas discussed:** Status-hook shim integration, Credential delivery scope, Multi-agent install dispatch, Materialization timing and GC scheduling

---

## Area selection

| Option | Description | Selected |
|--------|-------------|----------|
| Status-hook shim integration | How the in-VM shim writes status to a path the host poller can read across the microVM filesystem boundary | ✓ |
| Credential delivery scope | Which Phase-3-dropped convenience mounts (gitconfig / SSH / GCP / agent-config / home-seed) to restore now vs defer | ✓ |
| Multi-agent install dispatch | How a single kit installs whichever agent the user chose (env var dispatch vs per-agent materialized dirs vs --var) | ✓ |
| Materialization timing and GC scheduling | Lazy-vs-eager materialization + when the 30-day GC sweep runs | ✓ |

**User's choice:** All four areas selected.

---

## Direction-setting message from user

User explicitly redirected after the first deep-dive question: "i dont care too much, just make it fucking work and dont make huge changes to the codebase."

Existing memory `feedback_minimum_lib_churn.md` already captures this preference (user is contributing to someone else's lib; minimum delta preferred). Applied that guidance to lock all four areas with the smallest-delta approach rather than soliciting per-area preferences.

---

## Status-Hook Shim Integration

Considered three host-side dispatch options:

| Option | Description | Selected |
|--------|-------------|----------|
| Method on Instance, capability-derived | Method `Instance::hook_status_dir(&self)` consults resolved runtime capabilities; switches on `!supports_arbitrary_volume_paths`. No new flag but a wider refactor of the existing free-function API. | |
| Add the 6th capability flag | Add `host_visible_tmpfs: bool` to `RuntimeCapabilities` (Phase 1 deferred slot). Precise semantic, more ceremony. | |
| Sibling free function + one dispatch site | New `sbx_hook_status_dir(project_path, instance_id) -> PathBuf` next to existing `hook_status_dir(instance_id)`. Existing trio stays unchanged. One new dispatch site in StatusPoller switches via existing capability flag. | ✓ |

**User's choice:** Minimum-delta path — sibling free function, no new flag, no API rename.

**Notes:** `!supports_arbitrary_volume_paths` already gates the same physical truth (the backend's path constraint AND its tmpfs visibility are both consequences of the microVM boundary). Adding a second flag for the same condition would be ceremonial overhead. Phase 1's deferred 6th flag stays deferred.

---

## Credential Delivery Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Restore all 5 categories | Replace every Phase-3-dropped mount via sbx-native mechanisms (proxyManaged + secrets + credentials.sources + files/home/). Large kit complexity. | |
| Phase 4 minimum: SSH agent + provider API keys via proxy; defer the rest | `credentials.sources` for SSH agent (one line); `environment.proxyManaged` declaring known provider env vars so the host-side `sbx secret set -g <service>` flow handles the secret; gitconfig identity / agent-config dirs / GCP creds / home-seed files DEFERRED. | ✓ |
| Defer everything to Phase 5/6 | Phase 4 ships only the hook shim and agent install; no credential delivery at all. | |

**User's choice:** Phase 4 minimum.

**Notes:** Phase 3 already chose to silently drop convenience mounts; Phase 4 restores the two that are cheap-and-honest (SSH agent forwarding and proxyManaged provider keys). Anything that would require kit-schema features we don't yet use (`--var` substitution, gitconfig identity templating) is deferred.

---

## Multi-Agent Install Dispatch

| Option | Description | Selected |
|--------|-------------|----------|
| Env-var dispatch inside install.sh | Kit's `install.sh` reads `$AOE_AGENT` set via sbx env. Needs `sbx create` to accept env vars at create time. | |
| Per-agent materialized kit dir | Cache key includes agent name; KitMaterializer writes a per-agent `install.sh` whose body is the chosen agent's `install_hint` from `src/agents.rs`. Single spec.yaml across agents. | ✓ |
| sbx `--var KEY=VAL` substitution at create | Use sbx's kit-var mechanism if exists. | |

**User's choice:** Per-agent materialized dirs.

**Notes:** Verified against `.planning/sbx-cli-reference.md`: `sbx create` has NO `-e/--env` flag (line 512's `-e` belongs to `sbx exec` only). No `--var` either. So runtime env-var dispatch at install-time isn't possible. Per-agent materialized dirs sidestep this entirely — agent name is baked in at materialize-time. Disk overhead is tens of KB per (aoe-version, agent) tuple — trivial.

---

## Materialization Timing and GC Scheduling

| Option | Description | Selected |
|--------|-------------|----------|
| Lazy on first use; GC folded into materializer | `KitMaterializer::ensure` is called from `SbxRuntime::create_container`; non-sbx users pay zero cost. Atomic-rename for thread safety. GC runs once per cache-miss in the background, walks `<app_dir>/sbx-kit/` and rmdir's `aoe-*` mtime > 30 days. Log-only. | ✓ |
| Eager at aoe startup | Materialize all known agents' kits at startup; faster first session creation. Higher base cost for non-sbx users. | |
| Explicit subcommand only | `aoe doctor sbx-gc` plus explicit `aoe sandbox prep`. No automatic behavior. | |

**User's choice:** Lazy + folded GC.

**Notes:** Concurrency-safe via `<dir>.tmp.<pid>.<nanos>` → atomic `rename` → `EEXIST` fallback. Stdlib only, no `flock`, no new crate. Pattern already used in `src/logging.rs::persist_runtime_filter`. GC runs in a `tokio::task::spawn_blocking` (planner picks) so the tree-walk rmdir doesn't block session create.

---

## Claude's Discretion

- Exact module location for `KitMaterializer` within `src/containers/sbx/` (likely `kit.rs` per Phase 2 D-03 / research ARCHITECTURE.md).
- `include_str!` vs `include_bytes!` for `spec.yaml` embedding.
- Which subset of agents from `src/agents.rs` get usable `install.sh` bodies in v1 (those with `npm install -g ...` / `curl | bash` install_hints are obvious; agents with desktop-app installs (Kiro via Docker desktop, settl via Homebrew Cask) get fail-loud kits).
- Whether to inline `install_hint` directly into `spec.yaml::commands.install[]` vs keep a separate `install.sh` file.
- Hook-shim `initFiles` exact YAML shape (one entry per agent vs combined; planner picks based on kit schema).
- Whether `read_hook_status` / `cleanup_hook_status_dir` get sbx-aware variants or a single `_for_instance` shim that dispatches via capabilities.
- Exact verb list and idempotency-guard detection in `xtask check-kit`.
- `tokio::task::spawn_blocking` vs `std::thread::spawn` for the GC background sweep.
- Whether to add a `min-sbx-version` field to `spec.yaml` for the version-mismatch failure mode (Pitfall N6); planner decides whether to add now or defer.

## Deferred Ideas

- Gitconfig identity, agent-config dirs beyond hook shim, GCP creds, home-seed files — deferred to Phase 5/6 (revisit only if user friction emerges).
- Hook-shim relocation for Docker/Podman/AppleContainer (workspace-relative for ALL runtimes) — would unify path resolution and let us delete `HOOK_STATUS_BASE`, but changes Docker ergonomics and triggers a migration. Out of Phase 4 scope.
- 6th `host_visible_tmpfs` capability flag — Phase 4 reviewed and declined; existing `supports_arbitrary_volume_paths` covers the same condition.
- Kit-shipped network allow-lists (`network.allowedDomains` for provider API domains) — declined per Pitfall C10 (deny wins host-side; kit allow-lists give false confidence).
- `sbx template save` snapshotting (v2 TPL-01).
- Per-session `sbx ports --publish` orchestration (Phase 5 RT-05).
