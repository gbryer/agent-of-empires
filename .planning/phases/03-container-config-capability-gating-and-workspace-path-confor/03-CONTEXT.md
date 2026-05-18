# Phase 3: container_config Capability Gating and Workspace Path Conformance - Context

**Gathered:** 2026-05-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Rewrite `build_container_config` and `compute_volume_paths` to honor the `RuntimeCapabilities` declared by the resolved runtime. For `!supports_arbitrary_volume_paths` runtimes (sbx), both `working_dir` AND every workspace `VolumeMount.container_path` are conformed to the host path (sbx mounts at the absolute host path; no remap exists). Convenience mounts that target `/root/.gitconfig`, `/root/.ssh`, `/root/.claude/*`, GCP creds, home-seed files are dropped silently (Phase 4 kit delivers credentials via sbx's native proxy + `files/home/`). User-declared `extra_volumes` with `host_path != container_path` surface a typed error. `instance.rs::get_container_for_instance` skips `runtime.ensure_image()` when `!supports_image_pull`. Existing Docker/Podman/AppleContainer code paths produce byte-identical output to pre-Phase-3.

Out of scope: real `sbx` subprocess wiring of action verbs (Phase 5); embedded kit and credential proxy mapping (Phase 4); Settings TUI gating of capability-incompatible fields (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Workspace Path Conformance (RT-04)
- **D-01:** Sbx forces `host_path == container_path` — verified by the Docker docs (`sbx create AGENT PATH` mounts at the absolute host path; no `HOST:CONTAINER` syntax; kits cannot define mounts). The current `/workspace/<dir_name>` strategy is unachievable on sbx without kit-side symlink shims that would contradict Phase 1's `supports_arbitrary_volume_paths=false` capability flag. The native host-path model is also the better UX per sbx's own design rationale (tracebacks, build outputs, gitignored paths all reference paths the user sees outside the sandbox).
- **D-02:** Conformance happens in a single end-of-function post-pass inside `build_container_config`. Function body builds the ContainerConfig the way it does today (workspace + every convenience mount), then a final `conform_for_capabilities(&mut config, caps)` step rewrites workspace `container_path` to `host_path`, rewrites `working_dir`, and drops non-conformable mounts. Docker capability matrix makes the pass a no-op — SC-2 baseline-unchanged falls out for free.
- **D-03:** A shared `conform_workspace_paths(volumes, working_dir, caps) -> (Vec<VolumeMount>, String)` free function in `src/session/container_config.rs` is the single source of truth for workspace conformance. Both `build_container_config`'s post-pass AND `Instance::container_workdir()` (line 1263, used to feed `sbx exec -w`) call it so the workdir stays consistent across mount-time and exec-time call sites.

### Convenience Mount Handling (RT-04)
- **D-04:** Mounts whose `container_path` is not equal to (or under) one of the workspace `host_path` prefixes are DROPPED silently with `tracing::debug!` when `!supports_arbitrary_volume_paths`. This covers the existing gitconfig (`/root/.gitconfig`), SSH (`/root/.ssh`), GCP creds, agent-config (`/root/.claude`, `/root/.local/share/opencode`, etc.), and home-seed-file mounts (`/root/.claude.json`, `/root/.sandbox-gitconfig`). Phase 4 kit delivers these via sbx's native mechanism (`sbx secret set -g <service>` + kit `credentials/network/serviceAuth` proxy injection + `files/home/`).
- **D-05:** The hook-status-dir mount (`src/session/container_config.rs:1001-1006`) already uses `host_path == container_path` shape. It survives the conformance pass unchanged. Phase 4 may relocate it under the workspace path (`<workspace>/.aoe-hooks/<id>/status` per KIT-04), but Phase 3 does not touch it.

### Validation Error Shape (RT-04 SC-3)
- **D-06:** Introduce a new `thiserror`-based enum `ContainerConfigError` in `src/session/container_config.rs` (or a small peer module — planner discretion) with exactly one variant for Phase 3: `InconformableExtraVolume { entry: String, runtime: ContainerRuntimeName, reason: String }`. `entry` is the raw user-supplied string from `sandbox.extra_volumes` (e.g., `"~/.aws:/root/.aws"`). `runtime` uses the existing `ContainerRuntimeName` enum for free Display/Debug. `reason` is a static-ish string ("requires host_path == container_path").
- **D-07:** `build_container_config` keeps its `anyhow::Result<ContainerConfig>` return signature. The error site converts via `Err(ContainerConfigError::InconformableExtraVolume { ... }.into())`; anyhow accepts the typed variant transparently. SC-3 unit test asserts via `err.downcast_ref::<ContainerConfigError>()`. No call site (`instance.rs::build_container_config`, line 1269) needs touching.
- **D-08:** Only `extra_volumes` (user-declared via `sandbox.extra_volumes`) trigger the typed error. Aoe-internal convenience mounts the user did not author are silently dropped. Rationale: the user explicitly named `/root/.aws` for `~/.aws`; failing loud lets them fix it (`~/.aws:~/.aws` conforms) rather than silently degrading. No second variant for inconformable workspace or anonymous-volume cases — per AGENTS.md "no dead code", we add variants only when a real trigger emerges.

### Capability Injection Seam
- **D-09:** `build_container_config` gains a `capabilities: RuntimeCapabilities` parameter. The single call site at `src/session/instance.rs:1269` already has `runtime` in scope (line 1249); it passes `runtime.capabilities()`. `RuntimeCapabilities` is `Copy + PartialEq + Eq` (Phase 1) so passing by value is cheap. Tests construct synthetic `RuntimeCapabilities` literals (or use `RuntimeBase::DOCKER.capabilities` / `RuntimeBase::SBX.capabilities`) directly — no mock trait impl needed.
- **D-10:** `Instance::container_workdir()` (`src/session/instance.rs:1263`) gains a `capabilities: RuntimeCapabilities` argument too. All call sites (Claude/Codex/etc. sandboxed poll fns near `:1308`) have `Instance` and thus the runtime via `containers::get_container_runtime()`; they read capabilities at call time. Keeps `container_workdir()` a pure derivation, no global lookup.
- **D-11:** `compute_volume_paths` stays capability-blind. The capability-aware logic lives in the shared `conform_workspace_paths` helper (D-03), called from both `build_container_config`'s post-pass and `container_workdir()`.

### ensure_image Skip (RT-06)
- **D-12:** Inline call-site guard at `src/session/instance.rs:1249-1250`:
  ```rust
  let runtime = containers::get_container_runtime();
  if runtime.capabilities().supports_image_pull {
      runtime.ensure_image(image)?;
  }
  ```
  Two-line change. No new helper, no mock-runtime boilerplate. SC-4 unit test asserts the capability-driven branch by exercising `Instance::ensure_container()` with a session whose resolved runtime reports `!supports_image_pull` and verifying `ensure_image` is not invoked (capability matrix is already locked in Phase 1 tests; the test surface is the call-site behavior, not the trait method). Docker behavior path is unchanged (Docker reports `supports_image_pull=true` so the existing call still fires).
- **D-13:** `RuntimeBase::ensure_image` (`src/containers/runtime_base.rs:195`) keeps its existing defensive `Err(NotSupported)`-when-can't-pull-and-not-local behavior — if a future code path forgets to gate at the call site, it gets a typed error rather than a silent skip. Phase 3 does not modify `RuntimeBase::ensure_image`.

### Claude's Discretion
- Exact module location for `ContainerConfigError` — inline at the top of `container_config.rs` vs new `container_config/error.rs` peer module. Both fine; inline keeps churn smallest. Planner picks.
- Anonymous_volumes (`Vec<String>`) handling on the resolved `ContainerConfig` for `!supports_anonymous_volumes` runtimes — clear them in the post-pass (cleaner; `ContainerConfig` matches what the runtime will actually emit) vs leave them and rely on Phase 2's `build_create_args` skip with warn. Recommend clearing for consistency with the convenience-mount drop, but planner picks.
- Port_mappings (`Vec<String>`) treatment for `!supports_port_publish_at_create` runtimes — Phase 5 (RT-05) owns the post-create `sbx ports --publish` orchestration. Phase 3 leaves `port_mappings` as-is in `ContainerConfig` so Phase 5 has the data to iterate over.
- Snapshot/baseline test mechanism for SC-2 (Docker output unchanged) — `insta` is not yet in the dep tree; recommend hand-written fixture comparison (e.g., `assert_eq!(deduped_volumes, expected_volumes)`) for a representative input. Planner picks whether to add `insta` (new dep, more ergonomic) vs hand-written (zero deps, slightly more boilerplate).
- Working_dir picking under sbx multi-mount cases (sibling worktree, multi-repo workspace) — the existing functions already pick one `*_container` value; conformance just rewrites it to the corresponding `host_path`. Behavior is mechanical, no decision needed.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 3 — phase goal, dependencies (Phase 1), four success criteria
- `.planning/REQUIREMENTS.md` — RT-04 (host_path == container_path enforcement + typed validation error), RT-06 (ensure_image skip via capability gate; pull_image is `Err(NotSupported)` callers gate via capability)
- `.planning/PROJECT.md` § Key Decisions — capability flags over `if runtime == Sbx` branches; sbx-specific quirks absorbed inside the runtime layer

### Prior phase context (carry-forward)
- `.planning/phases/01-capability-surface-and-trait-foundation/01-CONTEXT.md` — D-01 (hybrid placement: `fn capabilities(&self) -> RuntimeCapabilities` returns each runtime's matrix), D-04 (5 new flags + 2 migrated), sbx capability values locked: `supports_arbitrary_volume_paths=false`, `supports_image_pull=false`, `supports_anonymous_volumes=false`
- `.planning/phases/02-sbxruntime-skeleton-and-pure-argv-builders/02-CONTEXT.md` — D-01..D-06 (facade dispatch; Phase 2 stub-in-place for action verbs); D-12 (sbx capability matrix routed through `RuntimeBase::SBX.capabilities` or re-declared on `SbxRuntime`)

### Codebase (the surface being modified)
- `src/session/container_config.rs` § `compute_volume_paths` (lines 630-734) — the workspace-path computer for the default / bare-repo-worktree / sibling-worktree cases; stays capability-blind, conformance happens downstream
- `src/session/container_config.rs` § `compute_workspace_volume_paths` (lines 744-800) — multi-repo workspace path computer; also stays capability-blind
- `src/session/container_config.rs` § `build_container_config` (lines 825-1134) — the orchestrator that builds `ContainerConfig`. Phase 3 adds a `capabilities: RuntimeCapabilities` param and an end-of-function `conform_for_capabilities(&mut config, caps)` post-pass before returning. Mount push sites of interest: gitconfig (line 884), SSH (line 894), GCP creds (line 916), agent config mount (line 963), agent home seed files (line 974), hooks dir (line 1001, already conformant), extra_volumes (line 1079, source of the typed Err)
- `src/session/instance.rs` § `get_container_for_instance` (lines 1248-1260) — call site for `runtime.ensure_image(image)?`; Phase 3 adds the `if runtime.capabilities().supports_image_pull` guard around line 1250
- `src/session/instance.rs` § `container_workdir` (lines 1263-1267) — second consumer of `compute_volume_paths` output; needs the capability arg per D-10
- `src/session/instance.rs` § `build_container_config` (lines 1269-1283) — wrapper that calls into `container_config::build_container_config`; needs the new `capabilities` arg threaded through
- `src/containers/container_interface.rs` § `RuntimeCapabilities` (lines 59-86) — the capability struct (Phase 1); `Copy + PartialEq + Eq`; used by D-09
- `src/containers/runtime_base.rs` § `RuntimeBase::DOCKER.capabilities` / `::SBX.capabilities` (lines 35-44, 110-119) — already-locked capability matrices; tests can use these literals directly
- `src/containers/runtime_base.rs` § `ensure_image` (lines 195-210) — KEEP AS-IS for Phase 3 (D-13). Defensive `Err(NotSupported)` when can't pull and not local stays in place.
- `src/containers/error.rs` — existing `DockerError` thiserror enum; pattern reference for the new `ContainerConfigError` (D-06)
- `src/session/projects.rs` § `RegistryError`, `src/session/instance.rs` § `EnsureReadyError` — other thiserror precedents in the session module

### sbx model (the constraint source)
- `.planning/sbx-cli-reference.md` § `sbx create` (lines 216-264) — "The workspace path is required and will be mounted inside the sandbox at the same path as on the host." No `HOST:CONTAINER` syntax; positional `PATH [PATH...]` with `:ro` suffix only
- Docker Sandboxes architecture docs (https://docs.docker.com/ai/sandboxes/architecture/) — workspaces mount via filesystem passthrough at absolute host paths; explicit UX rationale for `host_path == container_path` (preserves error messages, build outputs, gitignored paths)
- Docker Sandboxes FAQ — "A sandboxed agent can't follow symlinks to paths outside the sandbox." Symlinks into mounted paths work; symlinks pointing outside the mount don't
- Docker Sandboxes kits docs (https://docs.docker.com/ai/sandboxes/customize/kits/) — kits inject files via `files/home/` and `initFiles` with `${WORKDIR}` substitution. Kits CANNOT define additional volume mounts. Credentials flow via `credentials.sources` + `network.serviceDomains` + `network.serviceAuth` + `environment.proxyManaged` so the real secret never enters the VM. SSH keys via host SSH agent forwarding.

### Repo policy
- `AGENTS.md` § Coding Style — no dead code (drives D-08 single-variant decision); no inline `#[cfg(target_os = ...)]` outside `src/process/` (Phase 3 adds zero)
- `AGENTS.md` § Settings & Configuration — Phase 3 changes no settings fields; SET-01 wiring is Phase 6
- `AGENTS.md` § Testing Guidelines — unit tests in-module (`#[cfg(test)]`) for pure logic; tests must be deterministic and clean up after themselves

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `RuntimeCapabilities` struct (`src/containers/container_interface.rs:59-86`): Phase 1's locked struct, `Copy + PartialEq + Eq`. Pass by value as the new `build_container_config` param (D-09).
- `RuntimeBase::DOCKER.capabilities` / `RuntimeBase::SBX.capabilities` (`src/containers/runtime_base.rs:35-44, 110-119`): already-honest capability literals; tests use these directly instead of constructing synthetic matrices (saves boilerplate).
- `ContainerRuntime::capabilities()` (`src/containers/runtime.rs:96`): the trait accessor; the call site at `instance.rs:1249` already has `runtime` in scope.
- `ContainerRuntimeName` enum (`src/session/config.rs:730-741`): Display/Debug already derived; reused inside `ContainerConfigError::InconformableExtraVolume` (D-06).
- `tracing::debug!` / `tracing::warn!`: already used throughout `container_config.rs` (e.g., line 866, 922, 1085); the drop-pass logs use the same pattern.
- `tempfile`: already a dev-dep (used in `container_config.rs` tests at line 1206+); no new test deps for fixtures.

### Established Patterns
- **thiserror at module boundaries**: `DockerError` (`src/containers/error.rs`), `RegistryError` (`src/session/projects.rs`), `EnsureReadyError` (`src/session/instance.rs:68`). `ContainerConfigError` follows the same shape: `#[derive(Debug, Error)] pub enum ... { #[error("..")] Variant { ... } }`.
- **anyhow at function returns**: `build_container_config` returns `anyhow::Result<ContainerConfig>`; typed enums slot in via `Err(typed.into())` and tests `downcast_ref()`. No precedent for changing existing return signatures to typed Results.
- **Capability-gated branching inside pure functions**: `RuntimeBase::build_create_args` (`src/containers/runtime_base.rs:223-317`) already gates `-v`/`-p` emission on `supports_arbitrary_volume_paths` / `supports_anonymous_volumes` / `supports_port_publish_at_create`. Phase 3's `conform_for_capabilities` post-pass is the upstream counterpart — clears at config-build time what `build_create_args` would skip at argv-build time.
- **In-module `#[cfg(test)]` for pure logic**: `container_config.rs` already has extensive in-module tests starting around line 1156. Phase 3's new tests slot into the same `mod tests` block.

### Integration Points
- `src/session/instance.rs:1269-1283` (the `build_container_config` wrapper on `Instance`) is the SINGLE call site that needs the new `capabilities` param threaded. Threading scope is mechanical: read `runtime.capabilities()` once from the same `runtime` already constructed at `1249`, pass it down.
- `src/session/instance.rs:1308+` (sandboxed poll fns: `claude_poll_fn_sandboxed`, etc.) call `self.container_workdir()`. Each gains a `runtime.capabilities()` lookup (or accepts a capabilities param if the poll fn signature changes). Planner decides whether to thread caps into the poll fn or read them inside `container_workdir()` from a runtime arg.
- No other `src/` files are touched by Phase 3. `src/containers/` is read-only (capability values, trait surface). `src/server/`, `src/tui/`, `src/cockpit/`, `src/cli/` are untouched.
- No new crate deps. `tempfile` and `tracing` are already in scope.

### Testing Surface
- `compute_volume_paths` already has 8 unit tests (`container_config.rs:1202-1492`); Phase 3 adds parallel cases that exercise the conformance helper through `build_container_config` rather than touching `compute_volume_paths`.
- `build_container_config` has integration-style tests (`:2301`, `:2395`, `:2601`, `:2692`, `:2857`); Phase 3 adds:
  - SC-1 cases (3+ workspace shapes under sbx caps; assert `working_dir == VolumeMount.container_path == VolumeMount.host_path`)
  - SC-2 baseline-unchanged case (Docker caps; hand-written fixture comparison)
  - SC-3 inconformable `extra_volumes` case (assert `Err.downcast_ref::<ContainerConfigError>()` matches `InconformableExtraVolume`)
  - SC-4 ensure_image skip case (Phase 1 capability tests already prove `!supports_image_pull` for sbx; the new test exercises the call-site guard)

</code_context>

<specifics>
## Specific Ideas

- **`conform_for_capabilities` post-pass skeleton:**
  ```rust
  fn conform_for_capabilities(
      config: &mut ContainerConfig,
      caps: RuntimeCapabilities,
      runtime_name: ContainerRuntimeName,
      extra_volumes: &[String],
  ) -> Result<(), ContainerConfigError> {
      if caps.supports_arbitrary_volume_paths {
          return Ok(()); // Docker / Podman / AppleContainer — no-op
      }

      // 1. Validate user-declared extra_volumes
      for entry in extra_volumes {
          let parts: Vec<&str> = entry.splitn(3, ':').collect();
          if parts.len() >= 2 && parts[0] != parts[1] {
              return Err(ContainerConfigError::InconformableExtraVolume {
                  entry: entry.clone(),
                  runtime: runtime_name,
                  reason: "requires host_path == container_path".into(),
              });
          }
      }

      // 2. Conform workspace mounts: rewrite container_path to host_path
      let (conformed_volumes, conformed_workdir) =
          conform_workspace_paths(std::mem::take(&mut config.volumes), config.working_dir.clone(), caps);
      config.volumes = conformed_volumes;
      config.working_dir = conformed_workdir;

      // 3. Drop convenience mounts (container_path != host_path after conform)
      config.volumes.retain(|v| {
          let keep = v.container_path == v.host_path;
          if !keep {
              tracing::debug!(
                  "Dropping mount {} -> {} for runtime {} (!supports_arbitrary_volume_paths)",
                  v.host_path, v.container_path, runtime_name,
              );
          }
          keep
      });

      // 4. Optional: clear anonymous_volumes if !supports_anonymous_volumes (planner discretion per D-08 note)
      if !caps.supports_anonymous_volumes {
          config.anonymous_volumes.clear();
      }

      Ok(())
  }
  ```

- **`ContainerConfigError` enum:**
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum ContainerConfigError {
      #[error(
          "extra_volume `{entry}` cannot conform to host_path == container_path \
           under runtime {runtime}; {reason}. Fix: use `<host>:<host>` or remove the entry."
      )]
      InconformableExtraVolume {
          entry: String,
          runtime: ContainerRuntimeName,
          reason: String,
      },
  }
  ```

- **ensure_image call site (instance.rs:1248-1250):**
  ```rust
  // Ensure image is available (always pulls to get latest, when supported)
  let runtime = containers::get_container_runtime();
  if runtime.capabilities().supports_image_pull {
      runtime.ensure_image(image)?;
  }
  ```

- **Threading capabilities through container_workdir():**
  ```rust
  pub fn container_workdir(&self, caps: RuntimeCapabilities) -> String {
      let (volumes, working_dir) = container_config::compute_volume_paths(
          Path::new(&self.project_path), &self.project_path,
      ).unwrap_or_else(|_| (vec![], "/workspace".to_string()));
      let (_, conformed) = container_config::conform_workspace_paths(volumes, working_dir, caps);
      conformed
  }
  ```

</specifics>

<deferred>
## Deferred Ideas

- **Phase 4 kit `files/home/` and credential proxy delivery** — gitconfig identity (name/email), agent settings.json shims, SSH agent forwarding, and `sbx secret set -g <service>` integration. Phase 3 drops these mounts silently because Phase 4 is the right place to design the replacement.
- **Hook-status-dir relocation under workspace** — KIT-04 calls for shims to write to `<workspace>/.aoe-hooks/<id>/status` (workspace-relative) rather than `<app_dir>/hooks/<id>` (currently uses `host_path == container_path` so it conforms today; relocation is Phase 4's call).
- **`!supports_anonymous_volumes` ContainerConfig cleanup** — Phase 3 may clear `anonymous_volumes` (consistent with the convenience-mount drop), or leave it for Phase 2's `build_create_args` to skip. Planner discretion (D-08 note).
- **`!supports_port_publish_at_create` post-create publish** — Phase 5 (RT-05) owns the per-port `sbx ports --publish` orchestration. Phase 3 leaves `port_mappings` untouched in `ContainerConfig`.
- **`insta` snapshot dep** — adopt for the SC-2 baseline-unchanged test? Adds a new dev-dep; planner picks vs hand-written fixture.
- **Auto-conform extra_volumes** (`~/.aws:/root/.aws` → `~/.aws:~/.aws`) — rejected for Phase 3 in favor of hard-error. Could be revisited if user friction emerges, but silently relocating user-declared mounts is surprising.
- **Diagnostic surface for dropped mounts** — `aoe doctor` listing which mounts the resolved runtime won't honor. Speculative; defer until a real diagnostic flow exists (v2 DIAG-01 already noted in REQUIREMENTS.md).

</deferred>

---

*Phase: 03-container-config-capability-gating-and-workspace-path-confor*
*Context gathered: 2026-05-15*
