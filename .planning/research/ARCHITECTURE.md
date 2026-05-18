# Architecture Research: sbx Container Runtime Backend

**Domain:** Brownfield extension to `aoe`'s container runtime layer
**Researched:** 2026-05-15
**Confidence:** HIGH (all conclusions traced to existing source files; no speculative external dependencies)

---

## Architectural Delta

This milestone is a brownfield insert: the wider system architecture (CLI/TUI/server, session layer, tmux layer, ACP cockpit) is unchanged. The work fits into one existing slot — the `Container Layer` in `.planning/codebase/ARCHITECTURE.md` — and threads through three small surfaces around it:

```
                                  ┌─────────────────────────┐
   Settings TUI / new-session ─── │  Session Layer          │
                                  │  - SandboxConfig        │
                                  │  - SandboxConfigOverride│
                                  │  - container_config.rs  │
                                  │  - builder.rs / instance│
                                  └────────────┬────────────┘
                                               │ ContainerConfig
                                               ▼
                            ┌──────────────────────────────────────┐
                            │  Container Layer (`src/containers/`) │
                            │  ┌────────────────────────────────┐  │
                            │  │ ContainerRuntimeInterface trait│  │
                            │  └─────────────┬──────────────────┘  │
                            │  ┌─────────────┼────────────┐        │
                            │  │ runtime.rs  │            │        │  NEW:
                            │  │ ┌──────┐ ┌──┴──┐ ┌──────┐│        │  ┌─────┐
                            │  │ │Docker│ │Apple│ │Podman││────────┼──│ Sbx │
                            │  │ └──────┘ └─────┘ └──────┘│        │  └──┬──┘
                            │  │ (RuntimeBase, shared)    │        │     │
                            │  └──────────────────────────┘        │     │
                            └──────────────────────────────────────┘     │
                                                                         ▼
                                                          ┌───────────────────────┐
                                                          │  sbx subprocess       │
                                                          │  + kit cache dir      │
                                                          │  (app_dir/sbx-kit/)   │
                                                          └───────────────────────┘
```

The change does NOT touch: `src/tui/home/`, `src/server/`, `src/cockpit/`, `src/tmux/`, `src/git/`, `src/process/`. They consume `Instance` / `DockerContainer` / `ContainerRuntime` opaquely and continue to do so.

---

## Module Placement: `src/containers/sbx/` (sub-module, not a single file)

### Decision

Add **`src/containers/sbx/`** as a peer to `runtime.rs` / `runtime_base.rs`, NOT a single `sbx.rs`. Sub-divide:

```
src/containers/
├── mod.rs                  # MODIFIED: register sbx in runtime_binary() / get_container_runtime()
├── error.rs                # MODIFIED: add Sbx-specific error variants (KitInstallFailed, PortPublishFailed)
├── container_interface.rs  # MODIFIED: trait gets Capabilities + ports method (see below)
├── runtime.rs              # MODIFIED: RuntimeKind gains Sbx variant; dispatch arms added
├── runtime_base.rs         # MODIFIED: add capability flags struct (or keep flat); Sbx has only token reuse
└── sbx/
    ├── mod.rs              # SbxRuntime façade implementing ContainerRuntimeInterface
    ├── argv.rs             # build_create_args / build_exec_argv / build_ports_argv (pure, unit-testable)
    ├── kit.rs              # KitMaterializer: embedded bytes → cache dir, version-aware
    ├── ports.rs            # publish_ports(name, &[String]) → Result<()> (subprocess loop)
    └── parse.rs            # parse `sbx ls --json` output for batch_running_states
```

### Why a sub-module, not a single file

The Podman precedent is misleading here. Podman is a true Docker drop-in: it shares argv shape, daemon model, image semantics, and exec semantics — so it lives as a single `RuntimeBase::PODMAN` constant and a `RuntimeKind::Podman` arm in `runtime.rs` (~25 lines total added). Apple Container is the closer precedent: enough delta from Docker that it gets its own `match self.kind { RuntimeKind::AppleContainer => ... }` arms in two places (`does_container_exist` uses `logs` not `inspect`; `is_container_running` parses JSON; `exec_command` wraps in `sh -c`).

sbx is **further** from Docker than Apple Container is:

1. **Image semantics inverted.** `pull_image` / `ensure_image` / `image_exists_locally` / `default_sandbox_image` / `effective_default_image` all operate on Docker registry tags. sbx uses templates (a saved snapshot) and kits (an install script), not registries. These trait methods become stubs/no-ops — but the stub behavior must be honest: `ensure_image` cannot lie and return `Ok(())` if the image actually doesn't exist, because callers expect the image to be ready post-call.
2. **Argv shape diverges.** Docker is `docker run -d --name X -w WD -v HOST:CTR -e K=V -p H:C IMAGE sleep infinity`. sbx is `sbx create shell --name X --kit KIT --template IMAGE -m MEM --cpus N WORKDIR [EXTRA:ro ...]` — positional workspace paths replace `-v`, `--template` replaces the image positional, no `-w`, no `-d`, no `-p`, no trailing `sleep infinity`.
3. **Out-of-band port publish.** Ports require a separate `sbx ports SANDBOX --publish` call after create. None of the existing runtimes have this two-step shape, so it warrants its own module rather than smuggling shell-out logic into `runtime.rs`.
4. **Kit materialization.** A new persistent on-disk concern (cache dir, version stamp, embedded bytes) that has no precedent in `src/containers/`. Inlining it into `mod.rs` would make the file the most complex in the directory.

The total LOC for sbx will be roughly 4–6× Podman's footprint — file-per-concern keeps each file under ~300 lines and each function unit-testable, matching the rest of the codebase.

### Façade pattern (do NOT reuse `ContainerRuntime` enum dispatch)

`ContainerRuntime` in `runtime.rs` is a struct that pairs a `RuntimeBase` (shared behavior) with a `RuntimeKind` discriminant for the four ops that legitimately differ between Docker/Podman/Apple Container. **sbx breaks too many of `RuntimeBase`'s assumptions** to fit this model:

- `RuntimeBase::build_create_args` produces `docker run` argv. sbx needs `sbx create` argv.
- `RuntimeBase::pull_image` shells out to `pull` subcommand. sbx has no equivalent.
- `RuntimeBase::ensure_image` semantics depend on local image cache. sbx has none.
- `RuntimeBase::start_container` / `stop_container` work, but only `stop` is meaningful (sbx auto-starts on first `exec`).

Forcing sbx through `RuntimeBase` would require either turning every shared method into a `match self.kind` switch (defeating the abstraction) or padding `RuntimeBase` with sbx-specific args/branches (polluting Docker/Podman behavior).

**Better:** introduce `SbxRuntime` as a peer struct that implements `ContainerRuntimeInterface` directly, in parallel to `ContainerRuntime`. The two structs are united via `enum RuntimeHandle { Docker(ContainerRuntime), Sbx(SbxRuntime) }` (or trait object `Box<dyn ContainerRuntimeInterface>`) returned by `get_container_runtime()`.

```rust
// src/containers/mod.rs
pub enum RuntimeHandle {
    Cli(ContainerRuntime),  // Docker, Podman, AppleContainer (existing)
    Sbx(sbx::SbxRuntime),   // new
}

impl ContainerRuntimeInterface for RuntimeHandle {
    fn create_container(&self, name: &str, image: &str, c: &ContainerConfig) -> Result<String> {
        match self {
            RuntimeHandle::Cli(r) => r.create_container(name, image, c),
            RuntimeHandle::Sbx(r) => r.create_container(name, image, c),
        }
    }
    // ... etc, one delegator per trait method
}
```

This keeps `ContainerRuntime` unchanged (so Podman/Docker/AppleContainer regressions are impossible) and gives `SbxRuntime` a clean greenfield implementation in its own sub-module.

---

## Trait Evolution: Capabilities Struct on `ContainerRuntimeInterface`

### The five new flags

| Flag | Docker | Podman | Apple Container | sbx |
|------|--------|--------|-----------------|-----|
| `supports_port_publish_at_create` | true | true | true | **false** (use `sbx ports --publish` after create) |
| `supports_image_pull` | true | true | true | **false** (no `pull_image` equivalent; templates are saved or kit-built) |
| `supports_anonymous_volumes` | true | true | false (warns) | **false** (no anon-volume concept) |
| `supports_arbitrary_volume_paths` | true | true | true | **false** (host_path must equal container_path) |
| `supports_dynamic_port_publish` | false | false | false | **true** (`sbx ports SANDBOX --publish` works post-start) |

The first four are negative for sbx; the fifth is uniquely positive.

### Decision: one `fn capabilities(&self) -> RuntimeCapabilities` method on the trait

Three placement options were considered:

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| A. Five `fn supports_X(&self) -> bool` methods on the trait | Minimal change | Five new methods to mock; awkward at call sites that need >1 flag | Reject |
| B. Public flags on `RuntimeBase` (extend the existing pattern) | Matches `supports_read_only_volumes` / `supports_remove_volumes` precedent exactly | Fields live on a `pub(crate)` struct; sbx doesn't use `RuntimeBase` so the flags would be split across two locations (RuntimeBase for CLI runtimes, SbxRuntime fields for sbx) — call sites would need a switch | Reject |
| C. `fn capabilities(&self) -> RuntimeCapabilities` on `ContainerRuntimeInterface`, returning a single struct | One method, one struct, one source of truth; trivially mockable; works uniformly across `RuntimeHandle::Cli` and `RuntimeHandle::Sbx`; existing `supports_read_only_volumes` / `supports_remove_volumes` can migrate into it for consistency | Slightly wider surface change | **Accept** |

```rust
// src/containers/container_interface.rs
#[derive(Debug, Clone, Copy)]
pub struct RuntimeCapabilities {
    pub supports_read_only_volumes: bool,
    pub supports_remove_volumes: bool,
    pub supports_port_publish_at_create: bool,
    pub supports_image_pull: bool,
    pub supports_anonymous_volumes: bool,
    pub supports_arbitrary_volume_paths: bool,
    pub supports_dynamic_port_publish: bool,
}

pub trait ContainerRuntimeInterface {
    fn capabilities(&self) -> RuntimeCapabilities;
    // ... existing methods unchanged
}
```

`RuntimeBase`'s two existing fields stay where they are (they're still consumed by `build_create_args` internally), but `ContainerRuntime::capabilities()` constructs the struct from them plus three new constants (true/true/true for all CLI runtimes). `SbxRuntime::capabilities()` returns its own constant.

### Why this is justified, not just listed

- **Single mockable surface.** Tests for `container_config.rs` will need to verify "if `supports_arbitrary_volume_paths == false`, do path validation"; a single struct field is easier to override in a test than five trait methods.
- **Cohesion.** All five flags answer the same question ("what does this runtime support?") and tend to be consumed in clusters (the create-args builder needs to consult three of them simultaneously).
- **Future-proof.** New backends (e.g., a hypothetical `containerd` impl) will likely want to express their own capability matrix; a struct extends with `Default` impls without breaking existing impls.
- **Existing precedent migrates cleanly.** The two flags already on `RuntimeBase` slot into the struct without behavior change; this is the natural evolution of the existing pattern, not a competing one.

### Where capabilities are consulted

1. **`session/container_config.rs::build_container_config`** — when building `ContainerConfig.volumes`:
   - If `!supports_arbitrary_volume_paths`: every `VolumeMount.container_path` must equal `VolumeMount.host_path`. Validate; return error if any mount has been remapped (today every mount IS remapped to `/workspace/<name>`, so this is a hard refactor — see Pitfalls).
   - If `!supports_anonymous_volumes`: clear `ContainerConfig.anonymous_volumes` and emit a warning, OR reject the config if any are present.
2. **`SbxRuntime::create_container`** — checks `supports_port_publish_at_create == false` to know it must invoke `publish_ports` after create.
3. **`session/builder.rs` (and TUI status surfaces)** — checks `supports_image_pull == false` to skip `runtime.ensure_image()` calls and the "pulling image..." UI.

---

## Port Publish Orchestration: Inside `SbxRuntime::create_container`, Atomically

### Decision

The `sbx ports --publish` loop lives **inside `SbxRuntime::create_container`**, called immediately after `sbx create` succeeds. It does NOT live as a separate `publish_ports` method on the trait.

### Why inside (not separate)

The trait contract is "create_container takes a `ContainerConfig` (which has `port_mappings`) and returns a container ID once the container is ready to be used." Splitting port publication into a separate caller-invoked step would:

1. Require every existing call site (`session/instance.rs::get_container_for_instance`, `session/builder.rs` cleanup paths, future cockpit reconciler) to learn a runtime-specific second step. That's exactly the leakage REQ-INTEGRATION-01 prohibits.
2. Break the symmetry where `ContainerConfig.port_mappings` is the single source of truth for "ports the user wants published" — Docker honors it via `-p` at create, sbx must honor it via `sbx ports --publish` after create. Same input, different mechanism, same trait contract.
3. Create a window where the container exists but ports aren't published, observable to anything that polls `is_container_running`.

### Why inside (not blocking forever)

Each `sbx ports SANDBOX --publish PORT` invocation is bounded (sub-second per port in practice). The loop blocks `create_container` for `O(N_ports * round_trip_ms)`, which is acceptable for create — the existing `docker run` already blocks for the daemon round trip plus image inspection.

### Failure semantics

```rust
// src/containers/sbx/mod.rs (sketch)
fn create_container(&self, name: &str, image: &str, config: &ContainerConfig) -> Result<String> {
    self.ensure_kit_materialized()?;             // step 1 (cached, see below)
    let id = self.run_sbx_create(name, image, config)?;  // step 2: sbx create
    if let Err(e) = self.publish_ports(name, &config.port_mappings) {
        // REQ-RT-05: surface but do NOT roll back the sandbox
        tracing::warn!(target: "containers.sbx", %name, error = %e, "port publish failed; sandbox kept");
        // Returning Ok here intentionally: the sandbox is usable, ports are reportable separately.
        // Caller can detect via `sbx ports` later if it cares.
    }
    Ok(id)
}
```

REQ-RT-05 explicitly requires "failures surface clearly without rolling back the sandbox" — implementing this inside `create_container` keeps the trait contract while honoring that semantic. The `publish_ports` failure goes through `tracing::warn!` (consumed by `debug.log` and the runtime log filter) and could optionally surface via `BuildResult.warnings` if `builder.rs` is taught to capture container-layer warnings.

### Where `publish_ports` lives mechanically

`src/containers/sbx/ports.rs` exports `pub fn publish_ports(binary: &str, name: &str, ports: &[String]) -> Result<Vec<PublishedPort>>` (pure-ish, takes the binary name to enable test injection). `SbxRuntime` calls it from inside `create_container`. It is not part of the trait, so callers cannot invoke it directly — sbx-specific concern, sbx-internal API.

---

## Kit Materialization: Lazy First-Use, App-Dir Cached, Version-Stamped

### Decision

Materialize the embedded kit lazily on the first `SbxRuntime` operation that needs it (`create_container`, transitively through `ensure_kit_materialized`). Cache to `<app_dir>/sbx-kit/<version>/`. Version comes from a const in the binary; mismatch triggers re-materialization.

### `bundled_sounds/` is the wrong precedent

The PROJECT.md mentions `bundled_sounds/` as the precedent for embedding. **It is not** — `src/sound/bundled.rs` shows the sound files are NOT compiled into the binary; they're downloaded on demand from a hardcoded GitHub raw URL. That pattern is unsuitable for the kit:

1. The kit is a hard dependency of every sandboxed sbx session. A network-required first-launch is a regression vs Docker (where `ensure_image` is the only network step and it's cacheable).
2. We control kit versioning; a kit shipped with `aoe v1.7.1` may not be compatible with the materialized v1.7.0 cache, and we need a version stamp on disk.
3. The kit needs to live in a stable on-disk location for the lifetime of every sandbox — `sbx create shell --kit <PATH>` requires `<PATH>` to exist at create time and (depending on whether sbx copies the kit or references it) potentially for the lifetime of the sandbox.

### Real precedent: `src/server/mod.rs::StaticAssets`

The actual embedded-asset pattern in this codebase is `rust-embed`:

```rust
// src/server/mod.rs:48-50
#[derive(Embed)]
#[folder = "web/dist/"]
struct StaticAssets;
```

This compiles `web/dist/` into the binary at build time and provides byte access at runtime. `rust-embed = "8"` is already in `Cargo.toml`. We extend the same pattern, extracting to disk when needed:

```rust
// src/containers/sbx/kit.rs
#[derive(Embed)]
#[folder = "containers/sbx-kit/"]   // new directory, repo-rooted
struct EmbeddedKit;

const KIT_VERSION: &str = env!("CARGO_PKG_VERSION");  // or a dedicated const

pub fn ensure_kit_materialized() -> Result<PathBuf> {
    let cache = get_app_dir()?.join("sbx-kit").join(KIT_VERSION);
    let stamp = cache.join(".version");
    if stamp.exists() && std::fs::read_to_string(&stamp).ok().as_deref() == Some(KIT_VERSION) {
        return Ok(cache);  // hot path
    }
    // Cold path: stale or missing. Wipe and re-materialize.
    if cache.exists() { std::fs::remove_dir_all(&cache)?; }
    std::fs::create_dir_all(&cache)?;
    for path in EmbeddedKit::iter() {
        let bytes = EmbeddedKit::get(&path).context("missing kit file")?.data;
        let dest = cache.join(path.as_ref());
        if let Some(p) = dest.parent() { std::fs::create_dir_all(p)?; }
        std::fs::write(&dest, &bytes)?;
    }
    std::fs::write(&stamp, KIT_VERSION)?;
    Ok(cache)
}
```

### Why lazy on first use, not eager at startup

- **TUI cold start cost.** `tui::run()` and CLI subcommand dispatch happen before any sandbox decision; eager materialization adds disk I/O to every single `aoe` invocation, including ones that never touch sbx.
- **Cleaner conditional behavior.** If the user never selects sbx, we never write the kit dir.
- **Migration parity.** The migration system in `src/migrations/` is for *moving* user data; kit materialization is a write-on-need cache, not a migration. It belongs in the runtime, not in `migrations::run_migrations()`.

The first-use cost is bounded: the kit is small (script + a few files), one-time per version, on a fast local filesystem. Worth measuring during build, but no architectural reason for concern.

### Why version-stamped (REQ-KIT-04)

Three failure modes are prevented:

1. **Kit upgrade after `aoe` upgrade.** User installs `aoe v1.8.0` with a new kit version. Without the stamp check, the v1.7.x cache would be reused silently, and the user's sandboxes would behave inconsistently (new kit features missing).
2. **Partial materialization.** If a previous run crashed mid-write, the stamp is missing — next call wipes and re-materializes. Idempotent and self-healing.
3. **Manual editing.** A user editing files in the cache dir (e.g., poking at the install hook) does not get clobbered until the next `aoe` upgrade — they're explicitly modifying a cache, the contract is documented.

### Cache dir placement

`<app_dir>/sbx-kit/<version>/` — uses the existing `session::get_app_dir()` helper, which already handles debug-vs-release isolation (`agent-of-empires-dev` vs `agent-of-empires`) and the macOS/Linux split. No new platform code.

### Where the kit source lives in the repo

New top-level directory `containers/sbx-kit/` (sibling to existing `docker/` and `bundled_sounds/`):

```
containers/sbx-kit/
├── spec.yaml         # kit manifest (sbx schema)
├── files/            # initFiles dropped into the sandbox (status-hook shims for claude/codex/gemini/qwen)
└── install.sh        # install hook: installs the chosen agent binary
```

The Cargo `include` list and any `.gitignore` rules need to whitelist this directory.

---

## Component Boundaries: Full Reach of Files Touched

### New files

| File | Purpose | LOC estimate |
|------|---------|--------------|
| `src/containers/sbx/mod.rs` | `SbxRuntime` struct + `ContainerRuntimeInterface` impl | ~250 |
| `src/containers/sbx/argv.rs` | Pure argv builders (create, exec, ports) — unit-testable | ~200 |
| `src/containers/sbx/kit.rs` | `EmbeddedKit` rust-embed struct + `ensure_kit_materialized()` | ~80 |
| `src/containers/sbx/ports.rs` | Port-publish loop subprocess wrapper | ~80 |
| `src/containers/sbx/parse.rs` | `parse_sbx_ls_json()` for `batch_running_states` | ~100 |
| `containers/sbx-kit/spec.yaml` | Kit manifest | ~30 |
| `containers/sbx-kit/install.sh` | Agent installer hook | ~50 |
| `containers/sbx-kit/files/...` | Status-hook shims | varies |

### Modified files

| File | Why | Change shape |
|------|-----|--------------|
| `src/containers/mod.rs` | Wire `Sbx` into `runtime_binary()` and `get_container_runtime()`; introduce `RuntimeHandle` | Small; add 3rd match arm |
| `src/containers/container_interface.rs` | Add `RuntimeCapabilities` struct + `fn capabilities()` to trait | Trait extension; breaking |
| `src/containers/runtime.rs` | Implement `capabilities()` for existing `ContainerRuntime`; consume new flags in `build_create_args` if needed | Additive |
| `src/containers/runtime_base.rs` | Optional: collapse the two existing flags into `RuntimeCapabilities` constants per kind | Refactor; safe |
| `src/containers/error.rs` | `KitMaterializationFailed`, `PortPublishFailed`, `SbxNotInstalled` variants | Additive |
| `src/session/config.rs` | Add `Sbx` variant to `ContainerRuntimeName` enum (line 736) | One enum variant |
| `src/session/profile_config.rs` | Already typed as `Option<ContainerRuntimeName>` (line 193); no schema change, but the value `Sbx` becomes valid | Implicit |
| `src/session/container_config.rs` | Consult `runtime.capabilities()` to: (a) validate `supports_arbitrary_volume_paths` (REQ-RT-04), (b) skip anonymous_volumes, (c) skip ensure_image when `!supports_image_pull` | Conditional logic in 2-3 places |
| `src/session/instance.rs` | `get_container_for_instance` (line 1228) skips `runtime.ensure_image()` when `!supports_image_pull` | One conditional |
| `src/session/builder.rs` | Cleanup path uses `runtime.remove()` — no change; sbx handles `--force` itself | None |
| `src/tui/settings/fields.rs` | Add `"sbx"` (or `"Docker Sandboxes"`) to `container_runtime_options` (line 950); add 4th `container_runtime_selected` arm (line 932-936); same for `global_container_runtime_selected` (line 944-948) | 4 small edits |
| `src/tui/settings/input.rs` | Add `Sbx` arm to two `match selected` blocks (lines 1816-1820, 2104-2108) | 2 small edits |
| `Cargo.toml` | `rust-embed` dependency promoted out of the `serve`-feature gate (kit needs it unconditionally) | One feature edit |

### Files explicitly unchanged

`src/server/`, `src/cockpit/`, `src/tmux/`, `src/git/`, `src/process/`, `src/hooks/`, `src/tui/dialogs/new_session/`, `src/tui/home/`, `src/cli/add.rs` — they all consume the trait via `DockerContainer` / `ContainerRuntime`, which now transparently support sbx.

### Migration check

Adding `Sbx` to `ContainerRuntimeName` is a serde-backwards-compatible enum extension (existing TOML with `container_runtime = "docker"` keeps deserializing). **No `src/migrations/vNNN_*.rs` is needed.** This is consistent with how Podman was added.

---

## Data Flow: User Creates Sandboxed Session with Runtime=sbx

```
User in TUI → New Session dialog (Enter to confirm)
  │
  ▼
src/tui/dialogs/new_session/render.rs
  │  builds InstanceParams { sandbox: true, sandbox_image: "ghcr.io/.../aoe-sandbox:latest", ... }
  ▼
src/cli/add.rs::run() OR src/tui/dialogs/new_session/* (same path)
  │
  ▼
src/session/builder.rs::build_instance(InstanceParams)
  │  resolve_config(profile) → merged Config (config.sandbox.container_runtime == Sbx)
  │  creates worktree (unchanged)
  │  populates instance.sandbox_info { image, container_name, ... }   ◄── name still "aoe-sandbox-<id>"
  │  calls cleanup_instance only on failure
  ▼
src/session/instance.rs::get_container_for_instance()        ◄── triggered on first launch / ensure_pane_ready
  │  let container = DockerContainer::new(&self.id, image)   ◄── DockerContainer is misnamed; works for sbx too
  │  container.is_running() → false                          ◄── routes through ContainerRuntime → SbxRuntime
  │  container.exists()    → false                           ◄── sbx ls --json | grep name
  │  runtime.capabilities().supports_image_pull == false     ◄── NEW: skip runtime.ensure_image(image)
  │  let config = self.build_container_config()
  ▼
src/session/container_config.rs::build_container_config()
  │  produces ContainerConfig {
  │    working_dir,                                          ◄── e.g. "/workspace/myproject"  ⚠️ see Pitfalls
  │    volumes: vec![VolumeMount { host: /Users/g/proj, ctr: /workspace/proj }, ...],
  │    anonymous_volumes: vec!["/workspace/proj/target", ...],
  │    port_mappings: vec!["3000:3000"],
  │    ...
  │  }
  │  NEW: if !runtime.capabilities().supports_arbitrary_volume_paths:
  │    → either rewrite container_path = host_path (auto-conform) AND working_dir to host path
  │    → OR return Err(ConfigError::IncompatibleVolumeMapping) per REQ-RT-04
  │  NEW: if !runtime.capabilities().supports_anonymous_volumes:
  │    → drop anonymous_volumes (warn) or fold into volume_ignores semantically
  ▼
container.create(&config)
  │  → ContainerRuntime → RuntimeHandle::Sbx(sbx_runtime)
  ▼
src/containers/sbx/mod.rs::SbxRuntime::create_container()
  │  step 1: ensure_kit_materialized() → /Users/g/.agent-of-empires/sbx-kit/1.7.1/
  │  step 2: build_create_argv (see argv.rs)
  │           sbx create shell
  │             --name aoe-sandbox-<id>
  │             --kit /Users/g/.agent-of-empires/sbx-kit/1.7.1/
  │             --template ghcr.io/.../aoe-sandbox:latest
  │             --cpus 4 -m 8g
  │             /Users/g/proj /Users/g/docs:ro
  │  step 3: subprocess execute, capture sandbox-id from stdout
  │  step 4: publish_ports(name, &config.port_mappings)      ◄── REQ-RT-05
  │           for port in ports: sbx ports <name> --publish <port>
  │           on failure: tracing::warn!, do NOT rollback
  │  return Ok(sandbox_id)
  ▼
back in instance.rs: sandbox_info.container_id = Some(id)
  │
  ▼
instance.start() → tmux::Session::start()
  │  pane runs the agent's exec_command (e.g. "sbx exec -it aoe-sandbox-<id> claude")
  │  ▲ exec_command produced by SbxRuntime::exec_command — different argv from Docker:
  │    "sbx exec -it -w /workspace/proj aoe-sandbox-<id> claude"
  ▼
First exec auto-starts the sandbox (sbx semantic) → agent runs → status detection unchanged
```

The dataflow is dominated by the call into `SbxRuntime::create_container`. Everything upstream and downstream of it is shared with the existing runtimes. The only externally visible difference is in `build_container_config` (volume validation/conformance) and the skipped `ensure_image` call — both gated by `capabilities()`.

---

## Suggested Build Order with Dependency Graph

The phases are NOT linear; some can parallelize. Dependency edges:

```
                    ┌──────────────────────────────────────────────┐
                    │ P1: Capability struct + trait extension      │
                    │     - container_interface.rs                 │
                    │     - runtime.rs (impl for ContainerRuntime) │
                    │     - existing flags migrated                │
                    └──────────────────┬───────────────────────────┘
                                       │ blocks
                ┌──────────────────────┼─────────────────────────┐
                ▼                      ▼                         ▼
   ┌────────────────────────┐  ┌──────────────────────┐  ┌─────────────────────┐
   │ P2: SbxRuntime skeleton│  │ P3: container_config │  │ P5: TUI settings +  │
   │     + argv.rs (pure)   │  │     volume validation│  │     ContainerRuntime│
   │     - is_available     │  │     + capability gate│  │     Name::Sbx       │
   │     - capabilities()   │  │     for !arbitrary   │  │                     │
   │     - build_create_args│  │     paths, !pull,    │  │                     │
   │     (NO subprocess yet)│  │     !anon volumes    │  │                     │
   └─────────┬──────────────┘  └──────────────────────┘  └─────────────────────┘
             │ blocks                                              ▲
             │                                                     │ ENABLES end-user
             ▼                                                     │ visibility
   ┌────────────────────────┐                                      │
   │ P4a: Kit materialization│  (independent of P2 once trait is ──┘
   │      kit.rs             │   stable; can develop in parallel)
   │      + sbx-kit/spec.yaml│
   │      + install.sh       │
   │      + files/ shims     │
   └─────────┬──────────────┘
             │ blocks
             ▼
   ┌────────────────────────┐
   │ P4b: SbxRuntime full   │
   │      create_container  │
   │      + ports.rs publish│
   │      + parse.rs ls JSON│
   │      + start/stop/rm   │
   │      + exec_command    │
   │      + integration test│
   │      gated #[ignore]   │
   └─────────┬──────────────┘
             │
             ▼
   ┌────────────────────────┐
   │ P6: Documentation:     │  (independent of code; needs P5 + P4b for screenshots)
   │     prerequisite docs  │
   │     (sbx login,        │
   │     policy set-default,│
   │     secret set -g)     │
   │     + xtask gen-docs   │
   └────────────────────────┘
```

### Why this order

- **P1 first, blocking everything else.** The capability struct is the architectural keystone; until the trait has `fn capabilities()`, neither `container_config.rs` nor `SbxRuntime` can opt into capability-gated behavior. P1 is small (~50 LOC + tests), uncontroversial, and can land in a single PR with full Docker/Podman/Apple Container regression coverage via existing tests.
- **P2 + P3 + P5 parallelize after P1.** P2 is the SbxRuntime skeleton with capability flags returned as constants but no real subprocess yet — argv builders are pure functions, fully unit-testable without sbx installed. P3 is the `container_config.rs` validation work, exercised against fake capability constants. P5 is the TUI plumbing, also purely cosmetic until P4b.
- **P4a (kit) is independent of the runtime trait.** The kit materialization logic only depends on `rust-embed` and `get_app_dir()` — neither of which changes. Can develop in parallel with P2/P3/P5 entirely.
- **P4b is the integration phase.** It joins SbxRuntime (P2), the kit (P4a), and the volume conformance (P3). Real `sbx` subprocess invocations land here. Integration tests are `#[ignore]`-gated so CI doesn't require sbx.
- **P6 (docs) last.** Needs the actual UX to document and screenshot.

### Phase content summary

| Phase | Content | Test surface |
|-------|---------|--------------|
| P1 | `RuntimeCapabilities` struct, trait method, ContainerRuntime impl, optional RuntimeBase migration | Existing runtime tests must still pass; new test: each existing kind reports honest capabilities |
| P2 | `src/containers/sbx/mod.rs` skeleton, `argv.rs` pure builders, `capabilities()` returning sbx constants, stubs for subprocess methods | Unit tests on argv.rs only; can land before sbx is even installable |
| P3 | `container_config.rs` capability gating: REQ-RT-04 path validation, anon volume drop, ensure_image skip | Unit tests with fake `Capabilities` |
| P4a | `containers/sbx-kit/` content, `sbx/kit.rs` materializer with `rust-embed`, version stamp logic | Unit tests with `tempfile` for app dir |
| P4b | Wire SbxRuntime to subprocess: `create_container`, `publish_ports`, `parse_sbx_ls_json`, `exec_command` | Unit tests for parse.rs; `#[ignore]` integration tests for full flow |
| P5 | TUI settings options, ContainerRuntimeName::Sbx variant, runtime_binary() / get_container_runtime() wiring | TUI snapshot tests; e2e test (creates session with sbx selected, asserts no panic) |
| P6 | docs/sandbox/sbx.md user guide; xtask gen-docs run | Doc CI check |

### Critical path

P1 → P4b is the critical path; everything else can proceed in parallel branches. P4b is the longest single phase because it integrates three independent inputs (P2, P3, P4a) and is the first place real `sbx` is exercised. Recommend allocating the most validation budget here.

---

## Component Responsibility Matrix

| Component | Existing Responsibility | Delta for sbx |
|-----------|-------------------------|---------------|
| `src/cli/add.rs`, `src/tui/dialogs/new_session/` | Build `InstanceParams`, call builder | None |
| `src/session/builder.rs` | Resolve config, create worktree, populate `sandbox_info` | None (the sandbox container is created lazily in `instance.rs`, not in builder) |
| `src/session/instance.rs::get_container_for_instance` | Decide ensure_image / create / start path | Skip `ensure_image` when `!supports_image_pull` |
| `src/session/container_config.rs::build_container_config` | Build `ContainerConfig` from sandbox + workspace | Validate volume paths against capabilities; drop or warn on anon volumes |
| `src/containers/mod.rs` | Resolve runtime via config, expose `DockerContainer` | Add `Sbx` to `runtime_binary()` and `get_container_runtime()`; introduce `RuntimeHandle` |
| `src/containers/container_interface.rs` | Trait definition | Add `RuntimeCapabilities` and `fn capabilities()` |
| `src/containers/runtime.rs` (`ContainerRuntime`) | Docker/Podman/Apple impl | Implement `capabilities()` from constants |
| `src/containers/sbx/` (NEW) | sbx impl of trait | All of P2 + P4a + P4b |
| `src/tui/settings/fields.rs` + `input.rs` | Render and apply settings | Add `Sbx` to runtime select; wire to enum |
| `src/session/config.rs::ContainerRuntimeName` | Enum of valid runtimes | Add `Sbx` variant |
| `src/server/`, `src/cockpit/`, `src/tmux/` | Consume container layer opaquely | None (transparent reuse) |

---

## Anti-Patterns to Avoid

### Inlining sbx logic into `runtime.rs`

**Trap:** "Apple Container has special arms in `runtime.rs`; just add a `RuntimeKind::Sbx` arm to every method."
**Why it's wrong:** sbx breaks four of `RuntimeBase`'s assumptions (image semantics, argv shape, port lifecycle, volume model). Adding `RuntimeKind::Sbx` arms would force `RuntimeBase::build_create_args` to switch on the kind (defeating the abstraction) or duplicate the entire builder. The Apple Container precedent works because Apple Container shares Docker's argv shape — sbx does not.
**Do this instead:** SbxRuntime as a peer struct, joined via `enum RuntimeHandle`. New code in `src/containers/sbx/`; existing `ContainerRuntime` only gains the trait method `capabilities()`.

### Putting capability flags on `RuntimeBase` only

**Trap:** "Existing precedent: `supports_read_only_volumes` lives on `RuntimeBase`. Add the new five there."
**Why it's wrong:** `SbxRuntime` does not use `RuntimeBase`. Flags would split across two locations (RuntimeBase fields for CLI runtimes, SbxRuntime fields for sbx), and call sites would need to switch on `RuntimeHandle` to read them. Trait-level `capabilities()` collapses this to one access pattern.
**Do this instead:** `fn capabilities() -> RuntimeCapabilities` on `ContainerRuntimeInterface`. Existing `RuntimeBase` flags become *inputs* to constructing the capabilities struct.

### Materializing the kit eagerly at startup

**Trap:** "Run `ensure_kit_materialized()` from `main.rs` so it's always ready."
**Why it's wrong:** Adds disk I/O to every TUI cold start, including users who never select sbx. Conflicts with the lazy-everywhere style of the codebase (e.g., agent registry init, hook installation).
**Do this instead:** Lazy on first call into `SbxRuntime::create_container`. Self-healing if interrupted (version stamp gate).

### Splitting port publish into a separate trait method

**Trap:** "Add `fn publish_ports()` to `ContainerRuntimeInterface` so callers can invoke it explicitly."
**Why it's wrong:** Every existing call site (instance.rs, builder.rs, future cockpit reconciler) would need a sbx-specific second step. Breaks REQ-INTEGRATION-01 ("no surface gets sbx-specific code paths beyond the runtime layer").
**Do this instead:** Internal-only function in `src/containers/sbx/ports.rs`, called from inside `SbxRuntime::create_container`. Trait contract stays "after `create_container` returns Ok, the container is fully usable."

### Silently remapping `host_path` to `container_path` for sbx

**Trap:** "sbx requires host_path == container_path. Just rewrite container_path to match host_path inside SbxRuntime."
**Why it's wrong:** The downstream `working_dir` (`/workspace/myproject`) and any per-agent paths baked into commands assume the canonical `/workspace/...` layout. A silent rewrite at the runtime layer makes the runtime succeed but every subsequent `exec` lands in a directory that doesn't exist.
**Do this instead:** Push the conformance into `container_config.rs::build_container_config` — when `!supports_arbitrary_volume_paths`, BOTH the `working_dir` and all `VolumeMount.container_path` values are rewritten consistently to the host paths. The whole `ContainerConfig` is internally consistent before it leaves `build_container_config`.

---

## Sources

All findings traced to source in this repository:

- `src/containers/container_interface.rs` (trait definition, ContainerConfig, EnvEntry, VolumeMount)
- `src/containers/runtime.rs` (ContainerRuntime, RuntimeKind dispatch, Apple Container deltas)
- `src/containers/runtime_base.rs` (RuntimeBase, existing capability flag precedent at lines 22-25)
- `src/containers/mod.rs` (runtime_binary, get_container_runtime, DockerContainer)
- `src/session/config.rs` lines 679-741 (SandboxConfig, ContainerRuntimeName enum)
- `src/session/profile_config.rs` lines 138-194 (SandboxConfigOverride, container_runtime override at line 193)
- `src/session/instance.rs` lines 1228-1283 (get_container_for_instance, build_container_config)
- `src/session/container_config.rs` lines 630-735 (compute_volume_paths — host→container path remapping)
- `src/session/builder.rs` lines 517-584 (sandbox_info population, cleanup path)
- `src/sound/bundled.rs` (NEGATIVE evidence: bundled_sounds is downloaded, NOT embedded)
- `src/server/mod.rs` lines 25-50 (rust-embed pattern — actual embedded asset precedent)
- `src/tui/settings/fields.rs` lines 921-1112 (container_runtime select rendering)
- `src/tui/settings/input.rs` lines 1816-1820, 2104-2108, 724-727 (apply/clear runtime override)
- `Cargo.toml` lines 120 (rust-embed dependency, currently feature-gated)
- `src/session/mod.rs` lines 73-93 (get_app_dir helper for cache placement)
- `.planning/codebase/ARCHITECTURE.md` (existing architecture, layer boundaries)
- `.planning/codebase/STRUCTURE.md` (module layout)
- `.planning/sbx-cli-reference.md` (sbx command surface — `create AGENT PATH`, `ports --publish`, `exec`, `ls --json`)
- `.planning/PROJECT.md` (requirements REQ-RT-01 through REQ-INTEGRATION-01, key decisions)

Confidence is HIGH: every architectural claim is grounded in explicit code or explicit requirements; no speculative external integrations are required.
