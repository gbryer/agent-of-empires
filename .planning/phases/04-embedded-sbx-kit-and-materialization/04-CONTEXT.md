# Phase 4: Embedded sbx Kit and Materialization - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Author the single aoe-authored sbx kit at `src/containers/sbx_kit/` (`spec.yaml`, `install.sh` template, status-hook shim files). Embed via stdlib `include_str!` / `include_bytes!` (no new crate dep). Materialize lazily on first use to `<app_dir>/sbx-kit/aoe-<aoe_version>-<spec_hash>/<agent>/`. Inside the materialized kit, the status-hook shim writes to `<workspace>/.aoe-hooks/<id>/status` so the host poller can read it across the microVM filesystem boundary. Add `cargo xtask check-kit` CI guard. Host-side, add a sibling `sbx_hook_status_dir(project_path, instance_id)` so the StatusPoller picks the right path when the resolved runtime is sbx. Phase 4 does NOT yet wire real `sbx create` calls — Phase 5 does.

Out of scope: real subprocess wiring of `create_container` / `exec` / `stop` / `remove` (Phase 5); the post-create `sbx ports --publish` orchestration (Phase 5); the Settings TUI dropdown surfacing `Sbx` (Phase 6); restoring gitconfig identity, agent-config dirs beyond the hook shim, GCP creds, and home-seed files that Phase 3 dropped silently (Phase 5/6 follow-up).

</domain>

<decisions>
## Implementation Decisions

### Status-Hook Shim Integration (KIT-04)
- **D-01:** Inside the VM, the kit's `initFiles` block writes each hook-supporting agent's settings.json with a shim command of shape `mkdir -p ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID && printf STATUS > ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status`. `${WORKDIR}` is sbx's documented kit template substitution applied at sandbox-start (one absolute host path; same path inside the VM because sbx mounts workspaces at the same path). `$AOE_INSTANCE_ID` is an env var aoe passes per `sbx exec` (parallel to the existing Docker/Podman path; Phase 2's `build_exec_args` already emits `-e KEY=VAL` per `EnvEntry::Literal`).
- **D-02:** Host-side, add a NEW free function `sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf` in `src/hooks/status_file.rs` returning `<project_path>/.aoe-hooks/<instance_id>/`. The existing `hook_status_dir(instance_id) -> PathBuf` (returns `/tmp/aoe-hooks/<id>/`) stays unchanged for Docker/Podman/AppleContainer. NO rename, NO method-on-Instance refactor, NO new capability flag — reuses the existing Phase 1 `supports_arbitrary_volume_paths` capability that already gates the same physical constraint.
- **D-03:** The StatusPoller / status-detection call site is the ONLY new dispatch point. It consults the resolved runtime via `caps.supports_arbitrary_volume_paths` (already in scope per Phase 1/3) and calls either `hook_status_dir(...)` or `sbx_hook_status_dir(...)`. Planner picks the exact site (likely a small `read_hook_status_for(instance: &Instance)` shim adjacent to the existing `read_hook_status`). `cleanup_hook_status_dir` follows the same dispatch shape when the runtime is sbx — point it at the workspace path instead of `/tmp/aoe-hooks/`.
- **D-04:** No new 6th capability flag (`host_visible_tmpfs` or similar). Phase 1 deferred this slot, but the constraint coincides exactly with `!supports_arbitrary_volume_paths`: a backend that can't mount arbitrary host paths inside the container also can't expose host `/tmp` to the agent. Adding a second flag for the same physical truth would be ceremonial overhead. If a future backend has different shape (e.g., sbx-with-a-host-visible-tmpfs proxy), revisit then.

### Credential Delivery (Phase 4 minimum surface)
- **D-05:** `spec.yaml` declares `credentials.sources` for ssh-agent forwarding. One block; restores the SSH parity that Phase 3 dropped silently. No host-side aoe code; sbx handles the agent socket bridging.
- **D-06:** `spec.yaml` declares `environment.proxyManaged` listing known provider env vars (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, plus the standard set per agent). The sbx host-side proxy injects the real secret from `sbx secret set -g <service>` (set once by the user, host-side) so the credential never enters the microVM. The kit declares the contract; the user owns the secret. REQ-DOC-01 (Phase 7) documents the host-side `sbx secret set` step.
- **D-07:** Gitconfig identity (`user.name` / `user.email`), agent-config dirs beyond the hook shim (`~/.claude/`, `~/.local/share/opencode/`, etc.), GCP creds (`~/.config/gcloud/`), and home-seed files (`~/.claude.json`, `~/.sandbox-gitconfig`) — DEFERRED to Phase 5/6. Phase 3 already silently dropped these. Phase 4 doesn't attempt to restore them. Trade-off accepted: sbx v1 ships without auto-injected git identity and without per-agent config dirs; users either configure them inside the template image or via the agent's first-run flow. If user friction emerges post-v1, revisit via `files/home/` templating with `--env`-substituted variables (which requires plumbing through `sbx exec` at run-config time, not `sbx create`).

### Multi-Agent Install Dispatch (KIT-03)
- **D-08:** Materialized cache key includes the agent name: `<app_dir>/sbx-kit/aoe-<aoe_version>-<spec_hash>/<agent>/`. Cache key composition: aoe version (`env!("CARGO_PKG_VERSION")`) + SHA256 over the embedded kit bytes + agent name. Disk overhead: O(used_agents × aoe_versions); each per-agent dir is ~tens of KB.
- **D-09:** Per-agent dir layout: a static `spec.yaml` (identical across agents) plus a generated `install.sh` whose body is the chosen agent's `install_hint` from `src/agents.rs` (already exists per agent, lines 188–434). `KitMaterializer::ensure(agent_name) -> Result<PathBuf>` reads `install_hint` via `crate::agents::get_agent(agent_name).install_hint`, escapes/wraps it in a small shell harness, writes it to `<materialized>/install.sh`, and returns the kit dir for `sbx create shell --kit <path>`.
- **D-10:** Agents without a workable headless install path (e.g., Kiro via Docker desktop, settl via Homebrew Cask) get a kit that surfaces a clear error at install-hook time and fails-loud. Phase 4 does not try to make every agent in the registry sandbox-installable. Planner picks the exact subset; Claude Code, Codex, OpenCode, Gemini, Cursor CLI, droid, pi, Qwen Code are the obvious v1 targets (those with `npm install -g ...` / `curl | bash` install_hints already in `src/agents.rs`).

### Materialization Timing and Concurrency (KIT-01, KIT-05)
- **D-11:** Lazy on first use. `KitMaterializer::ensure(agent)` is called from `SbxRuntime::create_container` (Phase 5 wires the call site; Phase 4 ships the materializer). Non-sbx users never hit this code path. No eager startup cost.
- **D-12:** Concurrency-safe via stdlib only: materialize into `<final_path>.tmp.<pid>.<nanos>/` and atomic-rename to `<final_path>/` once written. Concurrent racers either (a) hit the cache-hit fast path on second-and-later calls because `<final_path>/` already exists, or (b) one wins the rename and the other's rename returns `EEXIST` → discard the tmp dir and use the winning final dir. No `flock`, no new crate dep, no global lock.
- **D-13:** Materialization writes are idempotent per (aoe_version, spec_hash, agent). A re-materialization with identical inputs produces the same dir contents byte-for-byte (no timestamps inside, no PID, no hostname). Aoe upgrade or kit-content change produces a different `<spec_hash>` → different dir → no in-place mutation of the old dir → no torn-write window.

### Stale-Kit GC (KIT-05)
- **D-14:** GC folded into `KitMaterializer::ensure`. On a cache miss (new dir just materialized), after the new dir is in place, scan siblings under `<app_dir>/sbx-kit/` and `rmdir -r` any `aoe-*` whose top-level dir mtime is older than 30 days. Log via `tracing::info!`; never propagate errors (a stale dir that can't be removed is not a session-blocking condition). Non-sbx users never trigger GC because they never call `ensure`.
- **D-15:** GC runs at most once per materialization (not per aoe startup, not per session create on cache-hit). Wraps in a tokio task spawn or a `std::thread::spawn` so the rmdir tree walk doesn't block session create. Planner picks task vs thread; the `tokio::task::spawn_blocking` pattern fits because session create is already on the tokio runtime.

### Kit Validation (KIT-06)
- **D-16:** New xtask subcommand `check-kit` in `xtask/src/main.rs` (alongside `gen-docs` and `check-skill`). Parses the embedded `spec.yaml` (via existing `serde_yaml`, already in the dep tree), asserts `schemaVersion == "1"`, asserts required fields (`kind`, `name`, `commands.install`, `commands.startup`), and lints `commands.startup[*].command` for naked install verbs (`apt-get`, `apt`, `dpkg`, `pip install`, `npm install -g`, `curl ... | bash`, `wget`) without an idempotency guard (line must start with `[ -f ... ]` / `command -v ... ||` / `test -x ...`). CI runs `cargo xtask check-kit` in the existing CI workflow.
- **D-17:** `check-kit` parses the embedded bytes from the source files at `src/containers/sbx_kit/` directly (not the materialized form, which is per-agent and built at runtime). The static `spec.yaml` is the artifact under test; the generated `install.sh` body comes from `src/agents.rs` and is covered by separate unit tests.

### Claude's Discretion
- Exact module location for `KitMaterializer`: new `src/containers/sbx/kit.rs` (per Phase 2 D-03 / research ARCHITECTURE.md) is the obvious slot; planner confirms.
- Whether `spec.yaml` is embedded via `include_str!` (string) vs `include_bytes!` (bytes-then-validate). `include_str!` is fine because spec.yaml is text; the hash is computed over `bytes()` either way.
- Exact set of agent install commands packaged into `install.sh` templates (which agents are v1, which fail-loud). REQUIREMENTS.md doesn't enumerate; planner picks based on `install_hint` shape per agent.
- Whether the per-agent dir contains a single `install.sh` (current plan) vs a `spec.yaml` with `commands.install` inlined. Inlining `install_hint` directly into `spec.yaml` is also viable (and skips the separate script); planner picks whichever the kit schema accepts most cleanly.
- Hook-shim file format inside `initFiles` (one combined settings.json per hook-supporting agent vs per-agent files). Both work; planner picks based on what kit schema allows for path-keyed initFiles.
- Whether `ensure_kit_materialized` returns the per-agent dir or a tuple `(kit_dir, agent_install_marker)`. Planner picks based on what `build_create_args` needs (Phase 2 already takes `kit_path: Option<&Path>` — just hand it the materialized agent dir).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 4 — phase goal, dependencies (Phase 1), five success criteria
- `.planning/REQUIREMENTS.md` — KIT-01 (source + embedding mechanism), KIT-02 (`sbx create shell --kit ... --template ...` invocation shape), KIT-03 (per-agent install via documented commands), KIT-04 (workspace-relative status path + `.gitignore` drop), KIT-05 (cache invalidation + 30-day GC), KIT-06 (xtask check-kit guard)
- `.planning/PROJECT.md` § Key Decisions — single embedded kit / single Docker image; stdlib `include_str!`/`include_bytes!` for embedding (rejected `include_dir`/`rust-embed`); sbx host-side state (secrets, policies) stays user-owned

### Prior phase context (carry-forward)
- `.planning/phases/01-capability-surface-and-trait-foundation/01-CONTEXT.md` — D-01 (`fn capabilities() -> RuntimeCapabilities` on the trait), D-04 (5 new flags + 2 migrated; sbx values: `supports_arbitrary_volume_paths=false`, `supports_image_pull=false`, etc.), and the deferred 6th flag note that Phase 4 reviewed and ultimately did NOT need (see D-04 above)
- `.planning/phases/02-sbxruntime-skeleton-and-pure-argv-builders/02-CONTEXT.md` — D-08 (`kit_path: Option<&Path>` already accepted by `build_create_args`; Phase 4 hands it the materialized agent dir), D-07 (free-function argv builders in `src/containers/sbx/argv.rs`)
- `.planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-CONTEXT.md` — D-04 (convenience mounts dropped silently under sbx — Phase 4's credential strategy explicitly addresses gitconfig/SSH/agent-config/home-seed deferrals from that drop), D-05 (hook-status-dir mount survived Phase 3; Phase 4 relocates it under the workspace per KIT-04)

### Codebase (the surface being modified or extended)
- `src/containers/sbx/` — directory created in Phase 2; Phase 4 adds `kit.rs` (KitMaterializer + cache key + GC). Per Phase 2 D-07, file naming is planner discretion within this dir.
- `src/containers/sbx/argv.rs` (Phase 2) — `build_create_args(name, image, kit_path: Option<&Path>, &ContainerConfig)`. Phase 4 plumbs the materialized kit dir into `kit_path`.
- `src/containers/sbx_kit/` — NEW directory with `spec.yaml`, hook-shim files for hook-supporting agents (Claude Code, Cursor, Gemini, Qwen Code). Embedded via stdlib `include_str!`/`include_bytes!` at build time.
- `src/hooks/mod.rs` § L19 `HOOK_STATUS_BASE = "/tmp/aoe-hooks"` — STAYS for Docker/Podman/AppleContainer. Phase 4 does not touch this constant.
- `src/hooks/status_file.rs` § L13 `hook_status_dir(instance_id)` — STAYS unchanged. Phase 4 adds a sibling `sbx_hook_status_dir(project_path, instance_id)` returning `<project_path>/.aoe-hooks/<id>/`.
- `src/hooks/status_file.rs` § L23 `read_hook_status` and § L39 `cleanup_hook_status_dir` — Phase 4 adds sbx-aware variants or a single `_for_instance` shim that dispatches via capabilities (planner picks; minimum-churn target).
- `src/session/poller.rs` § StatusPoller — single new dispatch site (`if caps.supports_arbitrary_volume_paths { /* legacy */ } else { /* sbx */ }`). Planner picks exact line.
- `src/agents.rs` § L67 `AgentDef`, § L103 `install_hint` — already populated for every agent. Phase 4 reads `install_hint` per agent at materialization time; no schema change to the registry.
- `src/session/mod.rs` § L73 `get_app_dir()` — returns the per-platform `agent-of-empires[-dev]` dir. Phase 4's KitMaterializer roots `<app_dir>/sbx-kit/` from here. Already handles debug-vs-release isolation; no platform code added.
- `src/main.rs` — env var `CARGO_PKG_VERSION` reachable via `env!("CARGO_PKG_VERSION")`. Used in the cache key.
- `xtask/src/main.rs` § `Commands` enum — Phase 4 adds `CheckKit` variant; pattern parallels `CheckSkill` (line 59).
- `Cargo.toml` § L95 `sha2 = "0.11"` — already a dep; reuse for the spec_hash. No new crate.
- `Cargo.toml` § L120 `rust-embed` — already a dep (for `web/dist/`). NOT used for the sbx kit per PROJECT.md / REQUIREMENTS.md (stdlib only).

### sbx kit format (the constraint source)
- `.planning/sbx-cli-reference.md` § `sbx create` (lines 216-264) — `--kit` flag is repeatable; `--template` for image; positional workspace paths only; **no `-e/--env` flag at create** (line 249 area: `-e` is only on `sbx exec`). Confirms install-time agent dispatch must be baked into the materialized kit.
- `.planning/research/FEATURES.md` § "sbx Kits Ecosystem Reference" (lines 189-202) — confirmed schema fields: `schemaVersion`, `kind` (`mixin`|`agent`), `commands.{install,startup}`, `initFiles`, `network.{allowedDomains,deniedDomains,serviceDomains,serviceAuth}`, `environment.{variables,proxyManaged}`, `credentials.sources`. Note: don't declare `allowedDomains`/`deniedDomains` (host policy wins per AF/C10).
- `.planning/research/PITFALLS.md` § C1 (status across VM boundary), C5 (cache invalidation), C9 (idempotent startup), C10 (network policy), N5 (install-hook output retention), M5 (per-sandbox disk usage) — every concern Phase 4 implements against.
- Docker Sandboxes kits docs (https://docs.docker.com/ai/sandboxes/customize/kits/) — `${WORKDIR}` substitution at sandbox-start; `commands.install` runs once as root; `commands.startup` runs every start as agent user (uid 1000); `initFiles` writes files into the sandbox at install time. Kit format explicitly experimental — `cargo xtask check-kit` is the early-warning system.
- Docker Sandboxes architecture docs (https://docs.docker.com/ai/sandboxes/architecture/) — each sandbox has its own Docker daemon and filesystem; workspaces mount at the same absolute host path inside the VM (the basis for the `${WORKDIR}/.aoe-hooks/...` shim path).

### Repo policy
- `AGENTS.md` § Coding Style — no dead code (D-08/D-09 keep cache key minimal); no `--` em-dashes; no inline `#[cfg(target_os = ...)]` outside `src/process/` (Phase 4 adds zero)
- `AGENTS.md` § Settings & Configuration — Phase 4 changes no settings fields; SET-01 wiring is Phase 6
- `AGENTS.md` § Testing Guidelines — unit tests in-module (`#[cfg(test)]`) for pure logic; tests must be deterministic and clean up after themselves (`tempfile` used throughout the codebase)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/agents.rs::AGENTS` (the agent registry) + `install_hint` field per agent — pre-existing, populated, and per-agent-specific. KitMaterializer reads from this directly via `crate::agents::get_agent(name).install_hint`; no parallel mapping to maintain.
- `src/hooks/status_file.rs::hook_status_dir`, `read_hook_status`, `cleanup_hook_status_dir` — pre-existing path resolution + I/O. Phase 4 adds sbx siblings, doesn't touch the existing trio.
- `src/session/mod.rs::get_app_dir()` — pre-existing per-platform + dev/release-aware app dir resolution. KitMaterializer roots its cache under this.
- `env!("CARGO_PKG_VERSION")` — standard cargo build-time substitution; used in the cache key alongside `spec_hash`.
- `sha2 = "0.11"` (Cargo.toml:95) — already in the dep tree for auth tokens; reused for the kit content hash. No new crate.
- `serde_yaml` (already in dep tree, used in `src/hooks/mod.rs` for Hermes config) — Phase 4's `xtask check-kit` parses spec.yaml via this; no new dep.
- `Phase 2's build_create_args` (`src/containers/sbx/argv.rs`) — already accepts `kit_path: Option<&Path>` per D-08. Phase 4 hands it the materialized agent dir; no signature change.
- `Phase 1's RuntimeCapabilities + supports_arbitrary_volume_paths` — already gates the same physical constraint Phase 4 uses to dispatch hook path resolution. No new flag.
- `tempfile` (dev-dep, used throughout) — KitMaterializer tests use this for temp `<app_dir>` substitutes.

### Established Patterns
- **xtask subcommand pattern**: `xtask/src/main.rs` already has `GenDocs` and `CheckSkill` variants; Phase 4 adds `CheckKit` following the same shape (single `fn check_kit()` peer to `fn check_skill()` at line 59).
- **Free-function helpers in `src/hooks/status_file.rs`**: existing functions are free fns over `instance_id: &str`; Phase 4's `sbx_hook_status_dir(project_path: &Path, instance_id: &str)` matches that shape exactly. No methods-on-Instance refactor.
- **In-module `#[cfg(test)]`**: `src/hooks/status_file.rs:48` already has extensive in-module tests with the `tempfile` pattern. Phase 4's tests for `sbx_hook_status_dir`, the materializer cache key, and the GC sweep slot into the same `mod tests` block (kit.rs) or a peer one (status_file.rs).
- **Stdlib-only embedding**: PROJECT.md / REQUIREMENTS.md / the user's `feedback_dependencies_and_gates` preference all point at `include_str!`/`include_bytes!`. `web/dist/` uses `rust-embed`; Phase 4 explicitly does not.
- **Atomic-rename for cache writes**: pattern already used in `src/logging.rs:447` (`persist_runtime_filter` writes to `runtime_filter.tmp` then `rename`). Phase 4's materializer follows the same idiom for thread-safe materialization.
- **Tokio spawn_blocking for background sweeps**: used elsewhere in `src/server/`; appropriate for the 30-day GC walk so it doesn't block create_container.

### Integration Points
- `SbxRuntime::create_container` (Phase 5 wires this) — calls `KitMaterializer::ensure(agent_name)` once before `build_create_args`. The materialized dir flows into `kit_path: Option<&Path>` on the existing Phase 2 argv builder.
- `StatusPoller` (`src/session/poller.rs`) — the ONE new host-side dispatch site. Reads `caps.supports_arbitrary_volume_paths` from the resolved runtime; calls either the existing `read_hook_status(instance_id)` or a new sbx-aware variant that uses `sbx_hook_status_dir(project_path, instance_id)`. Planner picks whether to wrap into a `read_hook_status_for(instance: &Instance)` shim or to inline the dispatch.
- `xtask/src/main.rs::Commands` — add `CheckKit` variant; CI workflow runs `cargo xtask check-kit` alongside `cargo xtask check-skill`.
- `.github/workflows/*` — CI invocation point for `cargo xtask check-kit`. Planner confirms the exact file (likely the same workflow that already runs `cargo xtask check-skill`).
- No changes to `src/containers/runtime.rs` dispatch arms beyond what Phase 2 already shipped. The Phase 5 wiring will add real subprocess calls; Phase 4 changes nothing in `runtime.rs`.

</code_context>

<specifics>
## Specific Ideas

- **Materialized layout**: `<app_dir>/sbx-kit/aoe-<CARGO_PKG_VERSION>-<spec_hash_hex>/<agent>/{spec.yaml, install.sh, files/...}`. Top-level dir per (version, hash); inner dir per agent. GC scans the top level (drop stale `aoe-*`); within a live top-level dir, all agent subdirs are kept.

- **Cache key composition (pseudocode)**:
  ```rust
  fn cache_dir(app_dir: &Path, agent: &str) -> PathBuf {
      let spec_hash = sha2::Sha256::digest(include_bytes!("../sbx_kit/spec.yaml"));
      let hash_hex = format!("{:x}", spec_hash);
      app_dir
          .join("sbx-kit")
          .join(format!("aoe-{}-{}", env!("CARGO_PKG_VERSION"), &hash_hex[..16]))
          .join(agent)
  }
  ```
  Truncating hash to 16 hex chars keeps paths short (64 bits of collision resistance against accidental drift; SHA256 of identical bytes is deterministic).

- **Atomic-rename materializer**:
  ```rust
  pub fn ensure(agent: &str) -> Result<PathBuf> {
      let target = cache_dir(&get_app_dir()?, agent);
      if target.exists() { return Ok(target); }
      let tmp = target.with_extension(format!("tmp.{}.{}", std::process::id(), nanos_since_epoch()));
      std::fs::create_dir_all(&tmp)?;
      write_embedded_files(&tmp)?;
      write_install_sh(&tmp, agent)?;
      match std::fs::rename(&tmp, &target) {
          Ok(()) => {}
          Err(e) if e.kind() == ErrorKind::AlreadyExists => { let _ = std::fs::remove_dir_all(&tmp); }
          Err(e) if target.exists() => { let _ = std::fs::remove_dir_all(&tmp); }
          Err(e) => return Err(e.into()),
      }
      gc_stale_dirs_in_background(target.parent().unwrap().parent().unwrap().to_path_buf());
      Ok(target)
  }
  ```
  Planner refines error matching for the OS-specific EEXIST shape (`std::io::ErrorKind::AlreadyExists` may be unstable for `rename` on some platforms; fall back to a post-rename `target.exists()` check as the authoritative signal).

- **Status-hook shim inside `initFiles`** (illustrative; planner picks exact YAML shape):
  ```yaml
  initFiles:
    - path: /home/agent/.claude/settings.json
      content: |
        {
          "hooks": {
            "PreToolUse": [{"hooks": [{"type": "command", "command": "sh -c '[ -n \"$AOE_INSTANCE_ID\" ] || exit 0; mkdir -p ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID && printf running > ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status'"}]}]
          }
        }
    - path: ${WORKDIR}/.gitignore.aoe-hooks
      content: |
        .aoe-hooks/
  ```
  Note: `${WORKDIR}` is sbx's documented substitution; `$AOE_INSTANCE_ID` is a runtime sandbox env aoe passes via `sbx exec -e AOE_INSTANCE_ID=<id>`. The `.gitignore` drop site is workspace-rooted; planner picks whether to merge it into an existing `.gitignore` or write a separate file.

- **Host-side dispatch shim** (illustrative):
  ```rust
  // src/hooks/status_file.rs
  pub fn sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf {
      project_path.join(".aoe-hooks").join(instance_id)
  }

  // poller / status_detection caller (one site):
  let caps = runtime.capabilities();
  let status_dir = if caps.supports_arbitrary_volume_paths {
      hook_status_dir(&instance.id)
  } else {
      sbx_hook_status_dir(Path::new(&instance.project_path), &instance.id)
  };
  ```

- **xtask check-kit skeleton**:
  ```rust
  fn check_kit() {
      let spec_src = include_str!("../../src/containers/sbx_kit/spec.yaml");
      let spec: serde_yaml::Value = serde_yaml::from_str(spec_src).expect("invalid yaml");
      assert_eq!(spec["schemaVersion"].as_str(), Some("1"), "schemaVersion must be \"1\"");
      let startup = spec["commands"]["startup"].as_sequence().expect("commands.startup required");
      for cmd in startup {
          let s = cmd["command"].as_str().unwrap_or_default();
          let install_verbs = ["apt-get", "apt ", "dpkg", "pip install", "npm install -g", "wget ", "curl "];
          for verb in &install_verbs {
              if s.contains(verb) && !s.contains("|| ") && !s.starts_with("[ ") && !s.starts_with("command -v") {
                  panic!("naked install verb `{}` in startup without idempotency guard: {}", verb, s);
              }
          }
      }
      println!("check-kit: OK");
  }
  ```
  Planner refines verb list and idempotency-guard detection.

</specifics>

<deferred>
## Deferred Ideas

- **Gitconfig identity (`user.name` / `user.email`) for sbx sessions** — Phase 3 dropped this mount silently for sbx. Phase 4 does NOT restore it. Future option: pass via `sbx exec -e GIT_AUTHOR_NAME=... -e GIT_AUTHOR_EMAIL=...` at every exec, or use `files/home/.gitconfig.template` with `--var` substitution (would require a kit-schema feature aoe doesn't currently use). Revisit in Phase 5/6 if user friction emerges.
- **Agent config dirs beyond hook shim** (`~/.claude/` excluding settings.json, `~/.local/share/opencode/`, `~/.config/git/`, etc.) — Phase 3 silently dropped these for sbx. Phase 4 only restores the hook-shim slice via `initFiles`. Other config recovery is per-agent first-run state, deferred to Phase 5/6 (or accepted as a user-onboarding step).
- **GCP creds and home-seed files** (`~/.config/gcloud/`, `~/.claude.json`, `~/.sandbox-gitconfig`) — same: dropped in Phase 3, not restored in Phase 4. Cloud creds belong in `sbx secret set -g gcp` per the proxyManaged model (D-06); home-seed files belong in `files/home/` if a future phase wants to ship them.
- **Hook-shim relocation for Docker/Podman/AppleContainer** (workspace-relative `<workspace>/.aoe-hooks/<id>/status` for ALL runtimes) — would unify the host-side path resolution and let us delete `HOOK_STATUS_BASE = "/tmp/aoe-hooks"`. Not Phase 4 scope; would change Docker session ergonomics and trigger a migration. Surface separately if desired.
- **6th `host_visible_tmpfs` capability flag** (Phase 1 deferred) — Phase 4 reviewed and declined to add it; `!supports_arbitrary_volume_paths` covers the same condition. Revisit only if a future backend has a different shape.
- **Kit-shipped network allow-lists** (`network.allowedDomains` for provider API domains) — Phase 4 does NOT declare these. C10 documents the reasoning: deny wins host-side, so kit allow-lists provide false confidence; users own their host policy. Revisit only if first-run friction proves significant.
- **`sbx template save` snapshotting integration** (v2 TPL-01 from REQUIREMENTS.md) — Phase 4 does not touch templates; templating is the v2 polish.
- **Per-session `sbx ports --publish` orchestration** (RT-05) — Phase 5 owns this; Phase 4's argv-builder integration only emits `--kit <path>` and `--template <image>`, leaving port-publish for the post-create orchestration.
- **Real `pull_image` `Err(NotSupported)` semantics** (REQ-RT-06) — Phase 3 wired the call-site guard; Phase 4 doesn't touch `pull_image` itself. Phase 5 ships the trait-method body if needed.

</deferred>

---

*Phase: 04-embedded-sbx-kit-and-materialization*
*Context gathered: 2026-05-16*
