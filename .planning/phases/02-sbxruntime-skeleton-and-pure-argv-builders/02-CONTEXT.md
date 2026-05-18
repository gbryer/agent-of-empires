# Phase 2: SbxRuntime Skeleton and Pure Argv Builders - Context

**Gathered:** 2026-05-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Stand up `src/containers/sbx/` as a sub-module with `SbxRuntime` as a peer struct that implements `ContainerRuntimeInterface` directly. Add pure-function argv builders (`build_create_args`, `build_exec_args`, `build_ports_args`) as free functions in `src/containers/sbx/argv.rs` so they are unit-testable without any `SbxRuntime` instance, without any `sbx` binary present, and without any subprocess execution. Make `is_available()` honest: real PATH check (via a `<binary> --version` probe), with an injection seam on `SbxRuntime` so a test can point at a temp-file fake. The minimum-necessary set of `RuntimeKind::Sbx` dispatch arms in `runtime.rs` is updated to route to the new builders; everything else Phase 1 left as safe stubs stays exactly as it is.

Out of scope for this phase: subprocess wiring of `create_container` / `exec_command` (real subprocess) / `stop_container` / `remove` / `exec` / `pull_image` / `get_version` / `is_daemon_running` (composite probe) / `batch_running_states` (real `sbx ls --json` parse) — all Phase 5. Workspace-path conformance and `ensure_image` capability gating — Phase 3. Embedded kit and materialization — Phase 4. Settings TUI exposure of `Sbx` — Phase 6.

</domain>

<decisions>
## Implementation Decisions

### Dispatch Shape
- **D-01:** Keep the **facade** dispatch pattern Phase 1 established (D-05/D-06). `ContainerRuntime` stays the public type returned by `get_container_runtime()`; the existing `RuntimeKind::Sbx` arms in `runtime.rs` delegate to a held `SbxRuntime` peer struct in `src/containers/sbx/`. Zero caller churn — the ~12 sites that consume `get_container_runtime()` are not touched. Rejected `enum RuntimeHandle { Cli, Sbx }` and `Box<dyn ContainerRuntimeInterface>` because both change the factory return type and contradict D-06.
- **D-02:** `SbxRuntime` lives in `src/containers/sbx/` as a peer struct (not built on `RuntimeBase`). Research argued this on the technical grounds that sbx breaks four `RuntimeBase` assumptions (image semantics, argv shape, port lifecycle, volume model); the facade keeps `RuntimeBase` clean for Docker/Podman/AppleContainer and lets `SbxRuntime` greenfield-implement only what it needs.
- **D-03:** How `ContainerRuntime` stores the `SbxRuntime` instance (internal enum like `enum Backend { Cli(RuntimeBase), Sbx(SbxRuntime) }`, optional field `sbx: Option<SbxRuntime>` with an invariant, or lazy construction inside dispatch arms) is **planner discretion**. Constraint: must not require changing call sites and must keep `ContainerRuntime::docker()` / `podman()` / `apple_container()` constructors working unchanged.

### Stub Shape — Minimum Surface
- **D-04:** Phase 2 updates **only the three** `RuntimeKind::Sbx` dispatch arms in `runtime.rs` that the success criteria force:
  1. `is_available()` — stop short-circuiting to `false`; delegate to `SbxRuntime::is_available()` (real PATH probe).
  2. `exec_command()` — replace the Phase 1 sentinel string with a call into `sbx::argv::build_exec_args(...)`, joined into the returned `String`.
  3. `build_create_args()` — replace the fall-through to `self.base.build_create_args()` (which produces docker-shaped argv) with a call into `sbx::argv::build_create_args(...)`.
- **D-05:** Existing Phase 1 safe-stub arms stay exactly as they are:
  - `does_container_exist` → `Ok(false)`
  - `is_container_running` → `Ok(false)`
  - `batch_running_states` → empty `HashMap`
- **D-06:** Action verbs (`create_container`, `start_container`, `stop_container`, `remove`, `exec`, `pull_image`, `get_version`, `is_daemon_running`, `ensure_image`) get **no new `RuntimeKind::Sbx` arm in Phase 2**. They continue to fall through to `RuntimeBase::SBX` methods that would produce wrong sbx invocations *if reached*. They are unreachable through real flows because Phase 6 is the official wiring point for the settings dropdown; the only way to hit them today is direct programmatic construction, e.g. tests. Phase 5 wires them with real subprocess code. Adding defensive `Err(NotImplemented)` arms now would be pure churn — Phase 5 immediately rips them out.

### Argv Builder API
- **D-07:** Argv builders are **free functions** in `src/containers/sbx/argv.rs`, not methods on `SbxRuntime`. Signatures:
  - `pub fn build_create_args(name: &str, image: &str, kit_path: Option<&Path>, config: &ContainerConfig) -> Vec<String>`
  - `pub fn build_exec_args(name: &str, workdir: Option<&str>, env: &[EnvEntry], interactive: bool, tty: bool, cmd: &[&str]) -> Vec<String>` (planner refines exact signature; must cover `-i`/`-t`/`-e`/`-w` permutations per success criterion #1)
  - `pub fn build_ports_args(name: &str, port_spec: &str) -> Vec<String>` (one call per port; the per-port loop happens in Phase 5's `ports.rs`)
- **D-08:** `kit_path` is `Option<&Path>` because Phase 4 owns kit materialization. Phase 2 tests pass `None`. Phase 4/5 plumbs `Some(materialized_path)` from the materializer. When `kit_path` is `None`, the builder still produces a valid (if incomplete) `sbx create shell --template <image>` shape — useful for testing positional workspace path handling, `:ro` mounts, `--cpus`, `-m` independent of the kit path.
- **D-09:** Builders take `&ContainerConfig` directly rather than individual fields, except where a parameter is more natural at the call site (kit_path is not on `ContainerConfig`; it's resolved from the kit materializer).

### is_available() Test Seam
- **D-10:** `SbxRuntime` carries a configurable binary path: `pub struct SbxRuntime { pub(crate) binary: PathBuf }`. `SbxRuntime::new()` defaults to `PathBuf::from("sbx")` — `Command::new(PathBuf)` resolves bare names against `PATH`, matching today's behavior. Unit test constructs `SbxRuntime { binary: <tempfile path> }` with a tiny chmod-+x'd shell script (`exit 0`) to assert `is_available() == true`. A second test points at a non-existent path to assert `false`. No `#[serial]` needed — no global state.
- **D-11:** This is a mild departure from how Docker/Podman/AppleContainer are constructed (their binaries are hardcoded on `RuntimeBase::*` consts). Acceptable because `SbxRuntime` is a peer struct, not a `RuntimeBase` user — it doesn't have to mirror that pattern, and Phase 2's success criterion #3 explicitly requires the injection-based test.

### Capability Values
- **D-12:** Sbx capability matrix is locked from Phase 1 (D-04, `RuntimeBase::SBX.capabilities`). `SbxRuntime::capabilities()` returns the same `RuntimeCapabilities { supports_read_only_volumes: false, supports_remove_volumes: false, supports_port_publish_at_create: false, supports_image_pull: false, supports_anonymous_volumes: false, supports_arbitrary_volume_paths: false, supports_dynamic_port_publish: true }`. Whether `SbxRuntime` re-declares its own const literal or routes through `RuntimeBase::SBX.capabilities` is planner discretion. Re-declaring is cleaner (peer struct owns its own data); routing avoids drift if Phase 1's values ever change. Both satisfy the success criterion.

### Claude's Discretion
- File names within `src/containers/sbx/` beyond `mod.rs` and `argv.rs` — Phase 2 only ships these two. `kit.rs`, `ports.rs`, `parse.rs` arrive in Phases 4 and 5.
- Exact `build_exec_args` signature (curried params vs single struct) — planner picks; must yield at least 10 distinct argv shapes for success criterion #1.
- Whether `ContainerRuntime` stores `SbxRuntime` via internal enum, optional field, or lazy construction — planner picks per D-03 constraint.
- Whether the dispatch arms in `runtime.rs` adapt `&ContainerConfig` for the argv builders, or whether the builders themselves accept the dispatch-arm-side types — planner picks; either works.
- Test layout: `#[cfg(test)]` in-module for `argv.rs` (purest, matches the rest of the codebase) vs separate `tests/sbx_argv.rs` — planner picks. Reference: AGENTS.md "Use unit tests in-module (`#[cfg(test)]`) for pure logic".

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 2 — phase goal, dependencies, success criteria (4 listed)
- `.planning/REQUIREMENTS.md` — Phase 2 stands up scaffolding; no requirement formally completes here. Coverage moves to Phase 5 for RT-02 and LIFE-01. Capability values for sbx (from RT-03) are referenced but locked in Phase 1.
- `.planning/PROJECT.md` § Key Decisions — sbx as 4th `ContainerRuntime` peer; capability flags over `if runtime == Sbx` branches

### Prior phase context (carry-forward)
- `.planning/phases/01-capability-surface-and-trait-foundation/01-CONTEXT.md` — D-05 (stub-in-place), D-06 (Phase 2 swap point without re-touching callers), D-03 (capability values live next to backend declaration), D-04 (5 new flags + 2 migrated), sbx capability values

### Codebase (the surface being modified or extended)
- `src/containers/container_interface.rs` — `ContainerRuntimeInterface` trait (Phase 2 implements it for SbxRuntime), `RuntimeCapabilities` struct (Phase 1), `ContainerConfig` / `VolumeMount` / `EnvEntry` data shapes
- `src/containers/runtime.rs` — existing `RuntimeKind::Sbx` dispatch arms (Phase 2 updates `is_available`, `exec_command`, `build_create_args` arms; leaves the rest)
- `src/containers/runtime_base.rs` — `RuntimeBase::SBX` const declared in Phase 1; Phase 2 keeps it (D-03 says values live next to backend declaration). SbxRuntime may route through it for capabilities, or re-declare (D-12)
- `src/containers/mod.rs` — `get_container_runtime()` factory; Phase 2 does NOT change its return type per D-01

### Research (decision rationale)
- `.planning/research/ARCHITECTURE.md` § Module Placement — `src/containers/sbx/` sub-module layout (Phase 2 owns mod.rs + argv.rs; kit.rs, ports.rs, parse.rs are later phases)
- `.planning/research/ARCHITECTURE.md` § Façade pattern (do NOT reuse `ContainerRuntime` enum dispatch) — *partially overridden by Phase 1's D-05/D-06*: research recommended `RuntimeHandle` enum; project chose facade. SbxRuntime is still a peer struct per research, but reached via existing `ContainerRuntime` dispatch
- `.planning/research/ARCHITECTURE.md` § Suggested Build Order — confirms Phase 2 (P2 in research) is "SbxRuntime skeleton + argv.rs (pure) — is_available, capabilities(), build_create_args (NO subprocess yet)"

### External CLI shapes (lock the argv format)
- `.planning/sbx-cli-reference.md` § `sbx create` — `[flags] AGENT PATH [PATH...]`; relevant flags: `--name`, `--cpus`, `--kit` (repeatable), `-m, --memory`, `-t, --template`. `:ro` suffix on positional PATH for read-only mounts. AGENT is `shell` per KIT-02
- `.planning/sbx-cli-reference.md` § `sbx exec` — `[flags] SANDBOX COMMAND [ARG...]`; relevant flags: `-i, --interactive`, `-t, --tty`, `-e, --env`, `-w, --workdir`
- `.planning/sbx-cli-reference.md` § `sbx ports` — `SANDBOX [flags]`; `--publish` flag (repeatable), port spec format `[[HOST_IP:]HOST_PORT:]SANDBOX_PORT[/PROTOCOL]`

### Repo policy
- `AGENTS.md` § Coding Style — no dead code, no `--` em-dashes/separators, no scattered `#[cfg(target_os = ...)]` outside `src/process/` (Phase 2 adds zero)
- `AGENTS.md` § Testing Guidelines — unit tests in-module (`#[cfg(test)]`) for pure logic; tests must be deterministic and clean up after themselves

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `RuntimeBase::SBX` (`src/containers/runtime_base.rs:99-119`): already declares sbx's binary name, capability matrix, and a (placeholder) `daemon_check_args`. SbxRuntime can route capabilities through this const or re-declare; either works.
- `RuntimeKind::Sbx` (`src/containers/runtime.rs:19`): dispatch discriminant already wired. Phase 2 updates three of the existing arms; no new variant added.
- `ContainerRuntime::sbx()` constructor (`src/containers/runtime.rs:53-58`): already pairs `RuntimeBase::SBX` with `RuntimeKind::Sbx`. Phase 2 may extend it to also construct/hold a `SbxRuntime` instance (D-03 storage shape is planner discretion).
- `ContainerConfig`, `VolumeMount`, `EnvEntry` (`src/containers/container_interface.rs:5-86`): the existing data shapes argv builders consume. No new types needed.
- `RuntimeCapabilities` struct (`src/containers/container_interface.rs:59-86`): Phase 1's locked struct. `SbxRuntime::capabilities()` returns an instance with sbx values.

### Established Patterns
- **Pure argv builder pattern**: `RuntimeBase::build_create_args` (`src/containers/runtime_base.rs:223-317`) is a pure function returning `Vec<String>`, separated from subprocess execution. Tests cover argv composition without running Docker. Phase 2's `sbx::argv::*` mirrors this exactly — same shape, different argv content.
- **Capability-gated branching inside builders**: `build_create_args` already consults `self.capabilities.supports_read_only_volumes`, `supports_arbitrary_volume_paths`, `supports_anonymous_volumes`, `supports_port_publish_at_create` to drive `-v`/`-p` emission. Phase 2's sbx builder is the *positive* counterpart to capability=false in `RuntimeBase::build_create_args`: for sbx, the positional workspace shape and `:ro` suffix replaces `-v`; no `-p` is emitted at create; etc.
- **In-module `#[cfg(test)]` for pure logic**: every existing `src/containers/*.rs` file has its tests in-module. AGENTS.md confirms this is the project standard.
- **Test skip pattern for runtime availability**: tests like `docker_if_available()` (`src/containers/runtime.rs:319-326`) gate by `is_available() && is_daemon_running()`. For Phase 2's sbx, argv-builder tests are pure (no skip needed); is_available unit test uses the injection seam (D-10), no host sbx needed.

### Integration Points
- `runtime.rs` dispatch arms are the **only** site where Phase 2's changes are visible outside `src/containers/sbx/`. Three arms updated; no other file in `src/` touched.
- `get_container_runtime()` (`src/containers/mod.rs`) — its match arm for `ContainerRuntimeName::Sbx` already calls `ContainerRuntime::sbx()`. Phase 2 does not modify it (D-01).
- Cargo: no new dependencies. `tempfile` is already a dev-dep (used elsewhere); reuse for the is_available injection test.

</code_context>

<specifics>
## Specific Ideas

- **`sbx create shell` argv skeleton** (Phase 2 produces, Phases 4/5 plumb the kit path):
  ```
  ["create", "shell",
   "--name", <name>,
   ("--kit", <kit_path>)?      // emitted only when Some
   "--template", <image>,
   ("--cpus", <n>)?            // from config.cpu_limit
   ("-m", <memory>)?           // from config.memory_limit
   <workspace_path> (":ro")?   // positional; one per VolumeMount; host_path is used (Phase 3 enforces host_path == container_path)
   ...                         // additional positional mounts
  ]
  ```
  Note: positional workspace paths come from `config.volumes`. Anonymous volumes from `config.anonymous_volumes` are NOT emitted (capability=false). Port mappings from `config.port_mappings` are NOT emitted at create (capability=false); Phase 5 emits them via `build_ports_args`.

- **`sbx exec` argv skeleton**:
  ```
  ["exec",
   ("-i")?  ("-t")?            // interactive/tty flags
   ("-w", <workdir>)?          // workdir
   ("-e", <key>)... or ("-e", <key=value>)...   // per EnvEntry; Inherit emits "-e KEY", Literal emits "-e KEY=VALUE" (parallel to RuntimeBase's pattern at runtime_base.rs:277-287)
   <name>, <cmd0>, <cmd1>, ...
  ]
  ```
  At least 10 distinct argv shapes for success criterion #1: { interactive, tty, both, neither } × { workdir, no workdir } × { env, no env } gives 16 combinations; pick 10+ covering all edges.

- **`sbx ports --publish` argv skeleton** (single call; per-port loop is Phase 5):
  ```
  ["ports", <name>, "--publish", <port_spec>]
  ```
  Where `<port_spec>` is the existing `config.port_mappings` entry shape (`"3000:3000"`, `"127.0.0.1:3000:3000/tcp"`). Phase 2's builder takes the spec verbatim; no parsing.

- **`is_available` injection test recipe** (per D-10):
  ```rust
  #[test]
  fn is_available_with_injected_fake_binary() {
      let tmp = tempfile::NamedTempFile::new().unwrap();
      std::fs::write(tmp.path(), "#!/bin/sh\nexit 0\n").unwrap();
      let mut perms = std::fs::metadata(tmp.path()).unwrap().permissions();
      use std::os::unix::fs::PermissionsExt;
      perms.set_mode(0o755);
      std::fs::set_permissions(tmp.path(), perms).unwrap();
      let sbx = SbxRuntime { binary: tmp.path().to_path_buf() };
      assert!(sbx.is_available());
  }

  #[test]
  fn is_available_with_missing_binary_returns_false() {
      let sbx = SbxRuntime { binary: PathBuf::from("/nonexistent/sbx-binary") };
      assert!(!sbx.is_available());
  }
  ```
  Unix-only chmod; the cross-platform shape is planner discretion (Windows is out of scope per PLAT-01).

- **Storage shape options for `ContainerRuntime` holding `SbxRuntime`** (D-03 leaves to planner):
  - Optional field: `pub struct ContainerRuntime { base: RuntimeBase, kind: RuntimeKind, sbx: Option<SbxRuntime> }`. `sbx()` constructor sets `sbx: Some(...)`; dispatch arms unwrap via `.expect()` with a kind-invariant message.
  - Internal enum: `enum Backend { Cli(RuntimeBase), Sbx(SbxRuntime) }` with `pub struct ContainerRuntime { backend: Backend, kind: RuntimeKind }`. Exhaustive match in dispatch arms.
  - Lazy: `kind == Sbx` arms construct `SbxRuntime::new()` on demand. Simplest but wasteful; rejected for tests that need to inject a binary path (the lazy path would need a global override).
  Both option 1 and option 2 satisfy D-01/D-03; planner picks based on which produces the smallest, cleanest diff.

</specifics>

<deferred>
## Deferred Ideas

- **Real `is_daemon_running` composite probe** — Phase 5 (RT-02). Phase 1 left `is_daemon_running` falling through to `RuntimeBase::SBX.is_daemon_running()` which runs `sbx version`. Phase 5 replaces with `sbx version` + `sbx ls --json` parseable + `SbxNotConfigured` on "not logged in" / "policy not configured" stderr.
- **Real subprocess wiring of action verbs** — Phase 5. Phase 2 deliberately leaves them falling through to `RuntimeBase::SBX` per D-06.
- **`pull_image` returns `Err(NotSupported)` per REQ-RT-06** — Phase 3 (capability gating; RT-06). Phase 2 does not add a Sbx arm to `pull_image`; today it falls through to `RuntimeBase::SBX.pull_image()` which runs `sbx ` + image (empty `pull_prefix`) and produces an error. Not pretty, but unreachable in real flows until Phase 6.
- **Workspace-path conformance (host_path == container_path)** — Phase 3 (RT-04). Phase 2's `build_create_args` accepts whatever `config.volumes` holds and emits the host_path positionally. Phase 3 rewrites `config` upstream of the dispatch arm so the values are correct.
- **Kit-path resolution via `KitMaterializer`** — Phase 4 (KIT-01..06). Phase 2 only accepts `kit_path: Option<&Path>`; the wiring from "materialized kit dir" to "value passed to `build_create_args`" lands in Phase 4/5.
- **Cross-product editability test (every FieldKey × every ContainerRuntimeName)** — Phase 6 (SET-02). Phase 2 doesn't touch settings.
- **End-to-end integration test** (`cargo test --test sbx_integration -- --ignored`) — Phase 5. Phase 2's tests are 100% unit; no `#[ignore]`, no sbx binary required to run them.

</deferred>

---

*Phase: 02-sbxruntime-skeleton-and-pure-argv-builders*
*Context gathered: 2026-05-15*
