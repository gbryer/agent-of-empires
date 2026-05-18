# Phase 2: SbxRuntime Skeleton and Pure Argv Builders - Research

**Researched:** 2026-05-15
**Domain:** Rust container-runtime trait extension; pure-function argv builders; in-module unit tests
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Dispatch Shape**
- **D-01:** Keep the facade dispatch pattern Phase 1 established. `ContainerRuntime` stays the public type returned by `get_container_runtime()`; the existing `RuntimeKind::Sbx` arms in `runtime.rs` delegate to a held `SbxRuntime` peer struct in `src/containers/sbx/`. Zero caller churn at the ~12 sites that consume `get_container_runtime()`. Rejected `enum RuntimeHandle { Cli, Sbx }` and `Box<dyn ContainerRuntimeInterface>` because both change the factory return type and contradict D-06.
- **D-02:** `SbxRuntime` lives in `src/containers/sbx/` as a peer struct (not built on `RuntimeBase`). sbx breaks four `RuntimeBase` assumptions (image semantics, argv shape, port lifecycle, volume model); the facade keeps `RuntimeBase` clean for Docker/Podman/AppleContainer and lets `SbxRuntime` greenfield-implement only what it needs.
- **D-03 (planner discretion):** How `ContainerRuntime` stores the `SbxRuntime` instance (internal enum like `enum Backend { Cli(RuntimeBase), Sbx(SbxRuntime) }`, optional field `sbx: Option<SbxRuntime>` with an invariant, or lazy construction inside dispatch arms) is planner discretion. Constraint: must not require changing call sites and must keep `ContainerRuntime::docker()` / `podman()` / `apple_container()` constructors working unchanged.

**Stub Shape — Minimum Surface**
- **D-04:** Phase 2 updates only the three `RuntimeKind::Sbx` dispatch arms in `runtime.rs` that the success criteria force:
  1. `is_available()` stops short-circuiting to `false`; delegates to `SbxRuntime::is_available()` (real PATH probe).
  2. `exec_command()` replaces the Phase 1 sentinel string with a call into `sbx::argv::build_exec_args(...)`, joined into the returned `String`.
  3. `build_create_args()` replaces the fall-through to `self.base.build_create_args()` with a call into `sbx::argv::build_create_args(...)`.
- **D-05:** Existing Phase 1 safe-stub arms stay exactly as they are: `does_container_exist` to `Ok(false)`, `is_container_running` to `Ok(false)`, `batch_running_states` to empty `HashMap`.
- **D-06:** Action verbs (`create_container`, `start_container`, `stop_container`, `remove`, `exec`, `pull_image`, `get_version`, `is_daemon_running`, `ensure_image`) get no new `RuntimeKind::Sbx` arm in Phase 2. They continue to fall through to `RuntimeBase::SBX` methods. Phase 5 wires them with real subprocess code.

**Argv Builder API**
- **D-07:** Argv builders are free functions in `src/containers/sbx/argv.rs`, not methods on `SbxRuntime`. Signatures:
  - `pub fn build_create_args(name: &str, image: &str, kit_path: Option<&Path>, config: &ContainerConfig) -> Vec<String>`
  - `pub fn build_exec_args(name: &str, workdir: Option<&str>, env: &[EnvEntry], interactive: bool, tty: bool, cmd: &[&str]) -> Vec<String>` (planner refines)
  - `pub fn build_ports_args(name: &str, port_spec: &str) -> Vec<String>` (one call per port)
- **D-08:** `kit_path` is `Option<&Path>` because Phase 4 owns kit materialization. Phase 2 tests pass `None`. When `kit_path` is `None`, the builder still produces a valid (if incomplete) `sbx create shell --template <image>` shape for testing positional workspace path handling, `:ro` mounts, `--cpus`, `-m` independent of the kit path.
- **D-09:** Builders take `&ContainerConfig` directly rather than individual fields, except where a parameter is more natural at the call site (kit_path).

**is_available() Test Seam**
- **D-10:** `SbxRuntime` carries a configurable binary path: `pub struct SbxRuntime { pub(crate) binary: PathBuf }`. `SbxRuntime::new()` defaults to `PathBuf::from("sbx")`. Unit test constructs `SbxRuntime { binary: <tempfile path> }` with a chmod-+x'd shell script. Second test points at a non-existent path. No `#[serial]` needed.
- **D-11:** Mild departure from how Docker/Podman/AppleContainer are constructed (hardcoded on `RuntimeBase::*` consts). Acceptable because `SbxRuntime` is a peer struct and Phase 2's success criterion #3 explicitly requires the injection-based test.

**Capability Values**
- **D-12:** Sbx capability matrix is locked from Phase 1. `SbxRuntime::capabilities()` returns `RuntimeCapabilities { supports_read_only_volumes: false, supports_remove_volumes: false, supports_port_publish_at_create: false, supports_image_pull: false, supports_anonymous_volumes: false, supports_arbitrary_volume_paths: false, supports_dynamic_port_publish: true }`. Whether `SbxRuntime` re-declares its own const literal or routes through `RuntimeBase::SBX.capabilities` is planner discretion.

### Claude's Discretion

- File names within `src/containers/sbx/` beyond `mod.rs` and `argv.rs` (Phase 2 only ships these two; `kit.rs`, `ports.rs`, `parse.rs` arrive in Phases 4 and 5).
- Exact `build_exec_args` signature (curried params vs single struct); must yield ≥10 distinct argv shapes for success criterion #1.
- Whether `ContainerRuntime` stores `SbxRuntime` via internal enum, optional field, or lazy construction (per D-03 constraint).
- Whether dispatch arms in `runtime.rs` adapt `&ContainerConfig` for the argv builders, or whether the builders themselves accept the dispatch-arm-side types.
- Test layout: `#[cfg(test)]` in-module for `argv.rs` (purest, matches the rest of the codebase) vs separate `tests/sbx_argv.rs`. Reference: AGENTS.md "Use unit tests in-module (`#[cfg(test)]`) for pure logic."

### Deferred Ideas (OUT OF SCOPE)

- Real `is_daemon_running` composite probe (Phase 5, RT-02).
- Real subprocess wiring of action verbs (Phase 5).
- `pull_image` returns `Err(NotSupported)` per REQ-RT-06 (Phase 3).
- Workspace-path conformance (`host_path == container_path`) (Phase 3, RT-04).
- Kit-path resolution via `KitMaterializer` (Phase 4, KIT-01..06).
- Cross-product editability test (Phase 6, SET-02).
- End-to-end integration test against real sbx (Phase 5).
</user_constraints>

<phase_requirements>
## Phase Requirements

Per REQUIREMENTS.md traceability matrix: **Phase 2 maps to zero v1 requirements directly.** It is a scaffolding phase whose surface enables Phase 5 (RT-02, LIFE-01, TEST-01) and Phase 3 (RT-04, RT-06) to land cleanly. Success is gauged by the four Roadmap success criteria, not by REQ-IDs:

| ID | Description | Research Support |
|----|-------------|------------------|
| SC-1 | `cargo test -p aoe sbx::argv` passes ≥10 argv shapes across `build_create_args`, `build_exec_args`, `build_ports_args` | Pure-function builder pattern (mirrors `RuntimeBase::build_create_args` at `runtime_base.rs:223-317`); `:ro` suffix emission gated on `VolumeMount.read_only`; env-entry encoding pattern copied from `runtime_base.rs:277-287` |
| SC-2 | `SbxRuntime::capabilities()` returns the locked sbx matrix; verified by unit test | `RuntimeBase::SBX.capabilities` already declares the matrix at `runtime_base.rs:99-119`; D-12 leaves "re-declare vs route through `SBX.capabilities`" as planner choice |
| SC-3 | `SbxRuntime::is_available()` returns false when `sbx` absent, true when present, with injected binary path | `tempfile` 3.14 is a top-level dep (`Cargo.toml:104`); precedent for in-module tempfile use exists at `src/migrations/v003_*.rs`, `src/migrations/v004_*.rs`. The Unix `PermissionsExt::set_mode(0o755)` recipe is documented in CONTEXT.md `<specifics>` and matches existing codebase patterns |
| SC-4 | Trait dispatch through the existing `RuntimeKind::Sbx` arms routes every method correctly without regressing Docker/Podman/AppleContainer | The 3-arm minimum surface (D-04) keeps changes confined to `is_available` / `exec_command` / `build_create_args` arms in `runtime.rs`; existing per-backend capability tests at `runtime.rs:439-481` continue to pass |

**Forward-coverage note:** RT-02 and LIFE-01 (Phase 5) consume the argv builders shipped in this phase. TEST-01 (Phase 5) will reuse the in-module unit-test pattern established here. Plan-checker should verify the argv-builder signatures here are stable enough that Phase 5 plans need not refactor them.

**SC-4 wording caveat (flagged risk, not blocker):** The Roadmap text says "Trait dispatch through the new `RuntimeHandle::Sbx(SbxRuntime)` variant". This wording is stale per ARCHITECTURE.md note that `RuntimeHandle` was rejected in favor of the facade (D-01). The actual mechanism is the existing `RuntimeKind::Sbx` dispatch in `runtime.rs`, which delegates to a held `SbxRuntime` per D-03 (storage shape planner-decided). **Action:** plan-checker should not be tripped by the stale Roadmap wording. Surface to user if they re-read the roadmap during plan review; do not edit the roadmap unilaterally.
</phase_requirements>

## Summary

Phase 2 stands up `src/containers/sbx/` as a sub-module with two files (`mod.rs`, `argv.rs`), a peer struct `SbxRuntime` carrying an injectable `binary: PathBuf`, and three pure free-function argv builders. It updates only three arms in `runtime.rs` (`is_available`, `exec_command`, `build_create_args`) to route through the new code. Every other Phase 1 stub arm stays as-is. The work is bounded, ~250-400 LOC total, with full unit-test coverage achievable without any `sbx` binary on the host. The facade pattern locked by D-01 is achievable in three distinct storage shapes (internal-enum / optional-field / lazy-construction); each has tradeoffs against diff size, dispatch ergonomics, and test injection.

**Primary recommendation:** Use the **optional-field** storage shape for `ContainerRuntime` (`pub struct ContainerRuntime { base: RuntimeBase, kind: RuntimeKind, sbx: Option<SbxRuntime> }`) — it produces the smallest diff against the existing Phase 1 code, requires no changes to non-sbx constructors, and the `match self.kind` invariant already in place at every dispatch site provides the safety guarantee (`expect("sbx kind without SbxRuntime"))` only triggers if the constructor is misused).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Pure argv string composition | `src/containers/sbx/argv.rs` (free fns) | — | Stateless transforms on `&ContainerConfig` / `&[EnvEntry]`; testable without trait, without struct, without subprocess. Mirrors `RuntimeBase::build_create_args` pattern at `runtime_base.rs:223-317`. |
| `is_available` (real PATH probe) | `src/containers/sbx/mod.rs` (SbxRuntime method) | — | Needs `&self` to read `self.binary` for the test-injection seam (D-10); cannot live as a free function. |
| Capability matrix declaration | `src/containers/sbx/mod.rs` OR `src/containers/runtime_base.rs::SBX` | — | D-12 planner choice. Routing through `RuntimeBase::SBX.capabilities` avoids drift; re-declaring keeps the peer struct self-contained. |
| Dispatch to argv builders | `src/containers/runtime.rs` (existing `match self.kind` arms) | — | Three arms updated (D-04). Adapter logic (e.g., `Option<&str>` to a workdir flag for `build_exec_args`) lives at the call site to keep builders pure. |
| Storage of `SbxRuntime` inside `ContainerRuntime` | `src/containers/runtime.rs` (`ContainerRuntime` struct definition) | — | D-03 planner choice (optional-field recommended above). |
| Trait surface (`ContainerRuntimeInterface`) | `src/containers/container_interface.rs` (unchanged) | — | Phase 1 already added `fn capabilities()`; Phase 2 adds no new trait methods. |
| Existing call sites (~12) | unchanged | — | D-01 guarantees zero churn; verified below in "Dispatch Site Inventory". |

## Standard Stack

### Core

This phase introduces **no new external dependencies**.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::path::PathBuf` / `std::path::Path` | stdlib | Binary path holder for test-injection seam | Already pervasive in codebase; `Command::new(PathBuf)` resolves bare names against `PATH` (matches existing `RuntimeBase::command()` at `runtime_base.rs:121-123`) |
| `std::process::Command` | stdlib | `<binary> --version` probe in `is_available()` | Matches `RuntimeBase::is_available()` shape at `runtime_base.rs:125-131` |
| `tempfile` | 3.14 | In-module unit test fake-binary creation | Already a top-level dep at `Cargo.toml:104`; precedent for in-module tests at `src/migrations/v003_*.rs`, `src/migrations/v004_*.rs` |
| `std::os::unix::fs::PermissionsExt` | stdlib | chmod-+x for the fake-binary test (`set_mode(0o755)`) | Standard Unix Rust pattern; Windows is out of scope per PLAT-01 |

### Supporting
No supporting libraries needed.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| In-module `#[cfg(test)]` | Separate `tests/sbx_argv.rs` integration test | AGENTS.md says "Use unit tests in-module (`#[cfg(test)]`) for pure logic" — every existing `src/containers/*.rs` follows this. In-module is strictly the project standard; reject separate file. |
| `pub(crate) binary: PathBuf` | `binary: Cow<'static, Path>` with default `Path::new("sbx")` | `PathBuf` is simpler; the allocation cost is one-time per process. `Cow` adds API complexity for no observable benefit. |
| Free-function `build_create_args` | Method on `SbxRuntime` | D-07 locked: free functions in `argv.rs`. Free functions are pure, testable without struct construction, and signal "this is just a string transformer" loudly. |
| Currying `build_exec_args(...)` with many positional bool params | Single `ExecOptions` struct | D-07 lists currying as the candidate. Currying is more idiomatic Rust for ≤6 params; struct adds boilerplate. Planner picks; either passes the 10-shape success criterion. |

**Installation:**
No new package installations. All work is in `src/containers/sbx/` (new directory) and `src/containers/runtime.rs` (3 arm updates).

**Version verification:**
- `tempfile 3.14` already at `Cargo.toml:104` (top-level dep, not dev-only). [VERIFIED: file read of `Cargo.toml`]
- Rust edition 2021, `rust-version = "1.85"` at `Cargo.toml:7-8`. [VERIFIED: file read]
- `Command::new(PathBuf)` resolves bare names against `PATH`. [VERIFIED: std docs; consistent with `RuntimeBase::command()` at `runtime_base.rs:121-123`]

## Package Legitimacy Audit

This phase installs **no new packages**. Every dependency is already in `Cargo.toml` (stdlib, `tempfile`). No slopcheck run required.

| Package | Registry | Status |
|---------|----------|--------|
| `tempfile` 3.14 | crates.io | Already pinned at `Cargo.toml:104`; no version change |

**Disposition:** No new packages. Skip slopcheck.

## Architecture Patterns

### System Architecture Diagram

```
                       ┌──────────────────────────────────────────┐
                       │ ContainerRuntimeInterface (trait)        │
                       │  - is_available()                        │
                       │  - capabilities()                        │
                       │  - exec_command() -> String              │
                       │  - build_create_args() -> Vec<String>    │
                       │  ... (all other methods)                 │
                       └──────────────┬───────────────────────────┘
                                      │ impl
                                      ▼
                       ┌──────────────────────────────────────────┐
                       │ ContainerRuntime (facade, runtime.rs)    │
                       │                                          │
                       │  match self.kind {                       │
                       │    Docker|Podman|AppleContainer =>       │
                       │      self.base.<method>(...)             │
                       │      (RuntimeBase shared impl)           │
                       │                                          │
                       │    Sbx =>                                │
                       │      ┌─ is_available  ─┐                 │
                       │      │ exec_command   │ Phase 2 routes   │
                       │      │ build_create_args ── into ────┐  │
                       │      └─                              │  │
                       │                                       │  │
                       │      ┌─ create_container, exec,  ─┐  │  │
                       │      │ stop, remove, pull_image,  │  │  │
                       │      │ get_version, ensure_image, │  │  │
                       │      │ is_daemon_running          │  │  │
                       │      │   fall through to          │  │  │
                       │      │   self.base.<method>       │  │  │
                       │      │   (returns wrong sbx       │  │  │
                       │      │    invocations, but        │  │  │
                       │      │    unreachable in normal   │  │  │
                       │      │    flows; Phase 5 wires)   │  │  │
                       │      └─                          │  │  │
                       │                                       │  │
                       │      ┌─ does_container_exist     ─┐  │  │
                       │      │ is_container_running       │  │  │
                       │      │ batch_running_states       │  │  │
                       │      │   Phase 1 safe stubs       │  │  │
                       │      │   (Ok(false) / empty map)  │  │  │
                       │      └─                          │  │  │
                       │  }                                    │  │
                       └───────────────────────────────────────┼──┘
                                                               │
                                                               ▼
                       ┌──────────────────────────────────────────┐
                       │ src/containers/sbx/                      │
                       │                                          │
                       │  mod.rs                                  │
                       │   ├─ pub struct SbxRuntime               │
                       │   │  { pub(crate) binary: PathBuf }      │
                       │   ├─ SbxRuntime::new()                   │
                       │   ├─ SbxRuntime::is_available(&self)     │
                       │   │   -> Command::new(&self.binary)      │
                       │   │      .arg("--version").output()...   │
                       │   └─ SbxRuntime::capabilities()          │
                       │      -> RuntimeBase::SBX.capabilities    │
                       │         (or re-declared literal)         │
                       │                                          │
                       │  argv.rs (PURE FREE FUNCTIONS)           │
                       │   ├─ pub fn build_create_args(           │
                       │   │    name, image, kit_path, config     │
                       │   │  ) -> Vec<String>                    │
                       │   ├─ pub fn build_exec_args(             │
                       │   │    name, workdir, env, interactive,  │
                       │   │    tty, cmd                          │
                       │   │  ) -> Vec<String>                    │
                       │   └─ pub fn build_ports_args(            │
                       │        name, port_spec                   │
                       │      ) -> Vec<String>                    │
                       └──────────────────────────────────────────┘
```

### Recommended Project Structure

```
src/containers/
├── container_interface.rs   # unchanged
├── error.rs                 # unchanged
├── mod.rs                   # unchanged (D-01 guarantees no factory-return-type change)
├── runtime.rs               # MODIFIED: 3 arm bodies (is_available, exec_command, build_create_args); ContainerRuntime struct gains 1 field per D-03 storage choice
├── runtime_base.rs          # unchanged (RuntimeBase::SBX const stays in place)
└── sbx/                     # NEW DIRECTORY
    ├── mod.rs               # NEW: SbxRuntime struct + impl (is_available, capabilities, new)
    └── argv.rs              # NEW: build_create_args, build_exec_args, build_ports_args + #[cfg(test)] mod
```

The directory must be registered in `mod.rs` (or via parent module). Per existing `src/containers/mod.rs:3`, the convention is `mod runtime;` for crate-private modules. **Add `mod sbx;`** (private to `containers`) so `runtime.rs` can `use super::sbx;` to reach the new symbols. `SbxRuntime` may be `pub` or `pub(crate)`; `pub(crate)` is sufficient since callers reach it via `ContainerRuntime`.

### Pattern 1: Pure Argv Builder (mirrors `runtime_base.rs:223-317`)

**What:** A free function returning `Vec<String>` from `(name, image, config, ...)` inputs. No `&self`. No subprocess. No file I/O.
**When to use:** Every sbx subcommand argv composition in Phase 2.
**Example (from runtime_base.rs:223-317):**
```rust
// Pattern reference — existing Docker shape
pub fn build_create_args(
    &self,
    name: &str,
    image: &str,
    config: &ContainerConfig,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--name".to_string(),
        name.to_string(),
        ...
    ];
    // ... capability-gated branches consult self.capabilities.supports_X ...
    args
}
```

**Phase 2 sbx variant (in `src/containers/sbx/argv.rs`):**
```rust
use std::path::Path;
use crate::containers::container_interface::{ContainerConfig, EnvEntry};

// Builds: sbx create shell --name <name> [--kit <kit_path>] --template <image>
//                          [--cpus <n>] [-m <mem>] <workspace_path>[:ro] ...
pub fn build_create_args(
    name: &str,
    image: &str,
    kit_path: Option<&Path>,
    config: &ContainerConfig,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "create".to_string(),
        "shell".to_string(),
        "--name".to_string(),
        name.to_string(),
    ];

    if let Some(kp) = kit_path {
        args.push("--kit".to_string());
        args.push(kp.display().to_string());
    }

    args.push("--template".to_string());
    args.push(image.to_string());

    if let Some(cpu) = &config.cpu_limit {
        args.push("--cpus".to_string());
        args.push(cpu.clone());
    }
    if let Some(mem) = &config.memory_limit {
        args.push("-m".to_string());
        args.push(mem.clone());
    }

    // Positional workspace paths come from config.volumes.
    // Phase 3 enforces host_path == container_path; Phase 2 emits host_path.
    // Anonymous volumes are NOT emitted (supports_anonymous_volumes = false).
    // Port mappings are NOT emitted at create (supports_port_publish_at_create = false).
    for vol in &config.volumes {
        let suffix = if vol.read_only { ":ro" } else { "" };
        args.push(format!("{}{}", vol.host_path, suffix));
    }

    args
}
```

**Why this works:** Capability checks live inside `build_create_args` for Docker because Docker's builder is shared across runtimes that differ on those flags. The sbx builder is single-purpose — sbx capability values are constants — so the builder unconditionally emits the sbx shape. No `if config.has_X` capability gates needed in sbx::argv.

### Pattern 2: `build_exec_args` (per CONTEXT.md `<specifics>`)

```rust
pub fn build_exec_args(
    name: &str,
    workdir: Option<&str>,
    env: &[EnvEntry],
    interactive: bool,
    tty: bool,
    cmd: &[&str],
) -> Vec<String> {
    let mut args: Vec<String> = vec!["exec".to_string()];

    if interactive {
        args.push("-i".to_string());
    }
    if tty {
        args.push("-t".to_string());
    }

    if let Some(wd) = workdir {
        args.push("-w".to_string());
        args.push(wd.to_string());
    }

    // EnvEntry serialization mirrors runtime_base.rs:277-287 exactly:
    //   Inherit { key, .. }      -> "-e", "KEY"
    //   Literal { key, value }   -> "-e", "KEY=VALUE"
    for entry in env {
        args.push("-e".to_string());
        match entry {
            EnvEntry::Inherit { key, .. } => args.push(key.clone()),
            EnvEntry::Literal { key, value } => args.push(format!("{}={}", key, value)),
        }
    }

    args.push(name.to_string());
    for c in cmd {
        args.push((*c).to_string());
    }

    args
}
```

### Pattern 3: `build_ports_args` (one call per port)

```rust
pub fn build_ports_args(name: &str, port_spec: &str) -> Vec<String> {
    vec![
        "ports".to_string(),
        name.to_string(),
        "--publish".to_string(),
        port_spec.to_string(),
    ]
}
```

The per-port loop lives in Phase 5's `src/containers/sbx/ports.rs`. Phase 2 only emits the single-port argv shape.

### Pattern 4: Three Storage-Shape Options for `ContainerRuntime` (D-03 planner discretion)

Each option must satisfy:
- `ContainerRuntime::docker()` / `podman()` / `apple_container()` unchanged
- The 3 updated arms (`is_available`, `exec_command`, `build_create_args` for the `Sbx` branch) can reach a held `SbxRuntime`
- Zero churn at the ~12 call sites of `get_container_runtime()`

**Option A — Optional field (recommended):**
```rust
pub struct ContainerRuntime {
    pub(crate) base: RuntimeBase,
    pub(crate) kind: RuntimeKind,
    pub(crate) sbx: Option<sbx::SbxRuntime>,  // Some only when kind == Sbx
}

impl ContainerRuntime {
    pub fn docker() -> Self {
        Self { base: RuntimeBase::DOCKER, kind: RuntimeKind::Docker, sbx: None }
    }
    // ... podman, apple_container similarly with sbx: None ...
    pub fn sbx() -> Self {
        Self {
            base: RuntimeBase::SBX,
            kind: RuntimeKind::Sbx,
            sbx: Some(sbx::SbxRuntime::new()),
        }
    }
}
```

At dispatch sites:
```rust
RuntimeKind::Sbx => self.sbx.as_ref()
    .expect("kind == Sbx without SbxRuntime; ContainerRuntime::sbx() constructor invariant violated")
    .is_available(),
```

**Pros:** Smallest diff (3 constructors gain `sbx: None`; one gains `Some(SbxRuntime::new())`). Easy to extend later (Phase 5 may extend `SbxRuntime` with more state without changing storage shape). Existing arms that route through `self.base` need no edits.

**Cons:** Runtime-checked invariant (`expect` on `None`) rather than compile-time. Acceptable because the only constructor for `kind == Sbx` is `ContainerRuntime::sbx()`.

**Option B — Internal enum:**
```rust
enum Backend {
    Cli(RuntimeBase),
    Sbx(sbx::SbxRuntime),
}

pub struct ContainerRuntime {
    backend: Backend,
    pub(crate) kind: RuntimeKind,
}
```

**Pros:** Compile-time exhaustive match. No `expect()`.

**Cons:** Larger diff — every existing arm that consults `self.base` must now destructure `self.backend` (e.g., `let Backend::Cli(base) = &self.backend else { unreachable!() };`). Or `RuntimeBase` is duplicated. Or the enum is built differently. The diff balloons into every existing arm. **Rejected for diff size.**

**Option C — Lazy construction:**
```rust
RuntimeKind::Sbx => sbx::SbxRuntime::new().is_available(),
```

**Pros:** Zero struct change. Smallest possible diff to `ContainerRuntime` struct.

**Cons:** Defeats D-10's test-injection seam — the lazy path always constructs `SbxRuntime::new()` with the default binary, so a test cannot inject a `SbxRuntime { binary: <tempfile> }` and observe it through `ContainerRuntime::is_available()`. Tests can still exercise `SbxRuntime::is_available()` directly, which is what success criterion #3 actually requires. But the dispatch arm is then untested for the injected case. **Acceptable if tests focus on `SbxRuntime` directly**, but suboptimal for diff longevity (Phase 5 will need to add stored state anyway).

**Recommendation:** **Option A (optional field)** for the planner. It is the smallest delta consistent with both D-10 (injection seam works through `ContainerRuntime`) and D-03 constraint (constructors unchanged). The `expect()` is defense-in-depth, not a real failure mode.

### Anti-Patterns to Avoid

- **Don't reintroduce `RuntimeHandle` enum at the public surface.** D-01 explicitly rejected it. The Roadmap text mentioning `RuntimeHandle::Sbx(SbxRuntime)` is stale wording; the real mechanism is `RuntimeKind::Sbx` dispatch.
- **Don't make argv builders generic over runtime kind.** They are sbx-only. `build_create_args` in `sbx/argv.rs` knows it is producing `sbx create shell ...` argv. Don't accept a `RuntimeKind` parameter or branch on it.
- **Don't duplicate the EnvEntry serialization logic.** Copy the pattern from `runtime_base.rs:277-287` verbatim into `sbx::argv::build_exec_args`. Phase 2 should **not** extract a shared helper — that's premature abstraction across two different argv shapes (`docker run -e` vs `sbx exec -e`). If Phase 5 finds a third use site, then refactor. Until then, two ~10-line copies are clearer than one indirection.
- **Don't gate sbx argv on capability flags.** sbx capabilities are constants. `build_create_args` in sbx unconditionally emits the sbx shape; it does not check `supports_anonymous_volumes` before deciding whether to emit anonymous-volume paths (the answer is always "no").
- **Don't add defensive `Err(NotImplemented)` arms for the action verbs.** D-06 forbids this. The action verbs continue to fall through to `RuntimeBase::SBX` methods (which produce wrong invocations *if reached*) until Phase 5 swaps them. They are unreachable in normal flows because Phase 6 is the wiring point for the settings dropdown.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Checking if a binary is on `PATH` | Manual `$PATH` parsing + filesystem stat | `Command::new(<path-or-bare-name>).arg("--version").output()` | `Command::new` already handles bare names via `PATH`; this exact pattern is at `runtime_base.rs:125-131` and is the project standard |
| Cross-platform "make file executable" | Forking on `cfg!(unix)` | `std::os::unix::fs::PermissionsExt::set_mode(0o755)` directly | Windows is out of scope per PLAT-01; no need to gate |
| Temporary fake-binary cleanup | Manual `tempdir` + drop guards | `tempfile::NamedTempFile` | Drop impl cleans up; already pervasive (`src/migrations/v003_*` etc.) |
| Generic argv composition | A "DSL" or builder for argv | Plain `Vec<String>` with `push()` calls | Matches `RuntimeBase::build_create_args` pattern exactly; clarity over cleverness |

**Key insight:** Every component this phase needs is already in stdlib or `Cargo.toml`. No new crate dependencies and no clever abstractions. The discipline is **matching existing patterns** (`runtime_base.rs` for argv, `migrations/v003_*.rs` for tempfile, `runtime_base.rs:121-131` for is-available probe).

## Common Pitfalls

### Pitfall 1: Hidden EnvEntry inheritance side-channel

**What goes wrong:** `RuntimeBase::run_create` at `runtime_base.rs:319-353` does more than just emit argv: for `EnvEntry::Inherit` entries, it ALSO `.env()` the value onto the `Command` so the subprocess can read it (the value never appears in argv, preventing secrets from leaking into `ps`).

**Why it happens:** Phase 2's `build_exec_args` is a pure argv builder — it emits the `-e KEY` (no value) for Inherit entries. But the dispatch arm in `runtime.rs::exec_command` returns a `String`, not a `Command`. The Inherit-key side-channel cannot be honored when the consumer is just rendering a `String` for tmux to exec.

**How to avoid:** **This is not a Phase 2 concern.** `exec_command` returns a `String` that tmux exec-runs in a shell context — Inherit entries rely on the calling shell having `KEY` set in its environment, which is the existing Docker behavior at `runtime_base.rs:407-413`. Phase 5's real subprocess `exec()` method (not `exec_command`) will handle the side-channel for `sbx exec`. Phase 2's `build_exec_args` correctly emits `-e KEY` (matching the Inherit semantic) and `-e KEY=VALUE` (matching Literal); the caller decides whether to also pipe the value through the process environment.

**Warning signs:** A reviewer asking "where does the Inherit value get passed?" — answer: it doesn't, in Phase 2's `String`-returning path. The shell that tmux launches inherits the host env, which is where the Inherit value sits.

### Pitfall 2: `:ro` suffix attaches to wrong path

**What goes wrong:** Docker's positional mount is `HOST:CONTAINER[:ro]`. sbx's positional mount is `PATH[:ro]` (single path because host_path == container_path). A drive-by reader might emit `HOST:CONTAINER:ro` for sbx, producing a sandbox path of `/path/to/container:ro` (broken).

**Why it happens:** Muscle memory from Docker mount syntax.

**How to avoid:** The builder unconditionally uses `vol.host_path` for sbx — never both `host_path` and `container_path`. The `:ro` suffix is appended only to the host path. Phase 3 will enforce that `vol.host_path == vol.container_path` so it doesn't matter which one is read; in Phase 2, the test fixtures intentionally use mismatched paths and assert that `host_path` (not `container_path`) ends up in argv.

**Warning signs:** A test that constructs a `VolumeMount { host_path: "/host", container_path: "/ctr", read_only: true }` and asserts the argv contains `"/host:ro"` — not `"/host:/ctr:ro"`.

### Pitfall 3: `Command::new(PathBuf)` PATH-resolution misconception

**What goes wrong:** Belief that `Command::new(PathBuf::from("sbx"))` does NOT consult `PATH` because `PathBuf` is "more concrete" than `&str`.

**Why it happens:** Easy to misread `Command::new`'s signature.

**How to avoid:** `Command::new<S: AsRef<OsStr>>` resolves bare names against `PATH` regardless of input type. `PathBuf::from("sbx")` produces a relative single-component path; `Command::new` consults `PATH`. `PathBuf::from("/tmp/sbx-fake")` is an absolute path; `Command::new` uses it directly. Both behaviors are what we want. The test injection works because the test passes the absolute tempfile path.

**Warning signs:** A test that fails with "no such file" when pointing to `/nonexistent/sbx-binary` — that's a passing test (we want `false`); not a misconfiguration.

### Pitfall 4: Test ordering and parallelism

**What goes wrong:** Two tests both `chmod 0o755` a tempfile and the second sees the first's file (impossible with `NamedTempFile`, but easy to assume).

**Why it happens:** Cargo runs tests in parallel.

**How to avoid:** `tempfile::NamedTempFile::new()` creates a uniquely-named file. Two parallel tests get independent files. No `#[serial]` is needed (D-10 confirms). The `is_available` probe spawns a subprocess; each test owns its subprocess and its tempfile. Use `let _guard = tmp;` (or the binding name) to keep the tempfile alive until after the assertion.

**Warning signs:** Flaky tests in CI that pass locally. Almost always a parallelism bug; the fix is to ensure each test owns its tempfile.

### Pitfall 5: Forgetting to register the `sbx` module

**What goes wrong:** Phase 2 ships `src/containers/sbx/mod.rs` but no `mod sbx;` declaration in `src/containers/mod.rs` (or wherever the parent is). The new files compile in isolation but are unreachable from `runtime.rs`.

**Why it happens:** Easy oversight when adding a new sub-module.

**How to avoid:** The plan must include an explicit task: "Add `mod sbx;` to `src/containers/mod.rs`". Verify with `cargo build` — the compiler will say `unresolved import` at the use site in `runtime.rs` if missing.

**Warning signs:** "`use super::sbx::...` errors out" — the parent module didn't declare it.

### Pitfall 6: Re-declaring vs routing capability matrix (D-12 drift risk)

**What goes wrong:** If `SbxRuntime::capabilities()` re-declares a literal `RuntimeCapabilities { ... }`, a future change to `RuntimeBase::SBX.capabilities` (e.g., Phase 4 adds the 6th flag `host_visible_tmpfs`) updates one but not the other; the two diverge silently.

**Why it happens:** D-12 calls this "planner discretion."

**How to avoid:** **Recommend routing through `RuntimeBase::SBX.capabilities`** rather than re-declaring. `RuntimeBase` is `pub(crate)` (verified at `runtime_base.rs:11`), so `SbxRuntime` can read `RuntimeBase::SBX.capabilities` directly. Single source of truth, zero drift risk, two-line method body. The peer-struct ownership argument from D-12 is real but weak: `SbxRuntime` not using `RuntimeBase` for execution does not mean it cannot reference the const for one read-only data accessor.

**Warning signs:** A future PR that updates sbx capabilities in `runtime_base.rs::SBX` and a test still passes against the stale literal in `SbxRuntime::capabilities()`. The plan-checker should flag the duplication if the planner picks re-declare.

## Runtime State Inventory

> Not applicable — Phase 2 is a greenfield code-addition phase. No renames, no migrations, no string replacements across the codebase. The only "moved" surface is `RuntimeKind::Sbx` dispatch arms in `runtime.rs` that change body content, not location.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no on-disk state added | None |
| Live service config | None — no external services touched | None |
| OS-registered state | None — no task scheduler / launchd / systemd integration | None |
| Secrets/env vars | None — no env var names referenced | None |
| Build artifacts | None — no new build artifacts; `cargo build` continues to work | None |

## Code Examples

### Example 1: `SbxRuntime` struct + `is_available` (per D-10 recipe)

```rust
// src/containers/sbx/mod.rs
// Source: pattern derived from runtime_base.rs:121-131 with binary path indirection per D-10.
use std::path::PathBuf;
use std::process::Command;

use crate::containers::container_interface::RuntimeCapabilities;
use crate::containers::runtime_base::RuntimeBase;

pub(crate) struct SbxRuntime {
    /// Injectable for the unit test that asserts is_available with a fake
    /// binary. Defaults to PathBuf::from("sbx") which Command::new resolves
    /// against PATH, matching the behavior of every other backend in this
    /// codebase.
    pub(crate) binary: PathBuf,
}

impl SbxRuntime {
    pub fn new() -> Self {
        Self {
            binary: PathBuf::from("sbx"),
        }
    }

    pub fn is_available(&self) -> bool {
        Command::new(&self.binary)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn capabilities(&self) -> RuntimeCapabilities {
        // Route through the existing const to avoid drift (see Pitfall 6).
        // Alternative per D-12: re-declare the literal here.
        RuntimeBase::SBX.capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn is_available_with_injected_fake_binary_returns_true() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "#!/bin/sh\nexit 0\n").unwrap();
        let path = tmp.path().to_path_buf();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();

        let sbx = SbxRuntime { binary: path };
        assert!(sbx.is_available());
        // tmp dropped here, file deleted.
        drop(tmp);
    }

    #[test]
    fn is_available_with_missing_binary_returns_false() {
        let sbx = SbxRuntime {
            binary: PathBuf::from("/nonexistent/sbx-binary-aoe-test"),
        };
        assert!(!sbx.is_available());
    }

    #[test]
    fn new_uses_bare_sbx_for_path_resolution() {
        let sbx = SbxRuntime::new();
        assert_eq!(sbx.binary, PathBuf::from("sbx"));
    }

    #[test]
    fn capabilities_matches_runtime_base_sbx() {
        let sbx = SbxRuntime::new();
        assert_eq!(sbx.capabilities(), RuntimeBase::SBX.capabilities);
    }
}
```

### Example 2: The three `runtime.rs` arm replacements

**Before (Phase 1) — `is_available` arm at `runtime.rs:74-79`:**
```rust
fn is_available(&self) -> bool {
    match self.kind {
        RuntimeKind::Sbx => false,
        _ => self.base.is_available(),
    }
}
```

**After (Phase 2):**
```rust
fn is_available(&self) -> bool {
    match self.kind {
        RuntimeKind::Sbx => self.sbx
            .as_ref()
            .expect("ContainerRuntime::sbx() invariant: sbx field must be Some when kind == Sbx")
            .is_available(),
        _ => self.base.is_available(),
    }
}
```

**Before (Phase 1) — `exec_command` Sbx arm at `runtime.rs:240-254`:**
```rust
RuntimeKind::Sbx => {
    let _ = options;
    let _ = cmd;
    format!(
        "sbx exec {} /* Phase 1 stub: cmd and options dropped */",
        name
    )
}
```

**After (Phase 2):**
```rust
RuntimeKind::Sbx => {
    // Parse the `options` &str (used by Docker as a passthrough like
    // "-w /workspace") into discrete fields for the sbx builder.
    // Phase 2 simplification: pass options through as a single workdir
    // when it starts with "-w ", otherwise drop. Phase 5 hardens this.
    let (workdir, env): (Option<&str>, &[EnvEntry]) = match options {
        Some(opt) if opt.starts_with("-w ") => (Some(&opt[3..]), &[]),
        _ => (None, &[]),
    };
    let cmd_parts = [cmd];
    let args = sbx::argv::build_exec_args(
        name,
        workdir,
        env,
        /* interactive */ true,
        /* tty */ true,
        &cmd_parts,
    );
    std::iter::once("sbx".to_string())
        .chain(args)
        .collect::<Vec<_>>()
        .join(" ")
}
```

**Note:** The exact adapter at the dispatch site is planner discretion. The Phase 2 success criterion only requires that `exec_command` no longer returns the sentinel string and the test in `runtime.rs` (`test_sbx_dispatch_arms_return_safe_stubs`) still passes (the existing assertion is loose: `cmd.contains("sbx") && cmd.contains("foo")`).

**Before (Phase 1) — `build_create_args` falls through unconditionally at `runtime.rs:181-183`:**
```rust
fn build_create_args(&self, name: &str, image: &str, config: &ContainerConfig) -> Vec<String> {
    self.base.build_create_args(name, image, config)
}
```

**After (Phase 2):**
```rust
fn build_create_args(&self, name: &str, image: &str, config: &ContainerConfig) -> Vec<String> {
    match self.kind {
        RuntimeKind::Sbx => {
            // Phase 2 passes None for kit_path; Phase 4's KitMaterializer
            // becomes the source of Some(materialized_path) in Phase 5.
            sbx::argv::build_create_args(name, image, None, config)
        }
        _ => self.base.build_create_args(name, image, config),
    }
}
```

### Example 3: Unit tests for argv builders (in-module per AGENTS.md)

```rust
// src/containers/sbx/argv.rs (#[cfg(test)] tail)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::container_interface::{ContainerConfig, EnvEntry, VolumeMount};

    fn empty_config() -> ContainerConfig {
        ContainerConfig {
            working_dir: "/workspace".to_string(),
            volumes: vec![],
            anonymous_volumes: vec![],
            environment: vec![],
            cpu_limit: None,
            memory_limit: None,
            port_mappings: vec![],
        }
    }

    // ----- build_create_args shapes -----

    #[test]
    fn create_args_minimal_no_kit_no_volumes() {
        let cfg = empty_config();
        let args = build_create_args("sbx-test", "alpine:latest", None, &cfg);
        assert_eq!(args, vec![
            "create", "shell", "--name", "sbx-test",
            "--template", "alpine:latest",
        ].into_iter().map(String::from).collect::<Vec<_>>());
    }

    #[test]
    fn create_args_with_kit_path() {
        let cfg = empty_config();
        let kit = std::path::PathBuf::from("/tmp/sbx-kit/aoe-1.7.0");
        let args = build_create_args("sbx-test", "alpine:latest", Some(&kit), &cfg);
        let kit_idx = args.iter().position(|a| a == "--kit").unwrap();
        assert_eq!(args[kit_idx + 1], "/tmp/sbx-kit/aoe-1.7.0");
        // --kit appears BEFORE --template
        let tmpl_idx = args.iter().position(|a| a == "--template").unwrap();
        assert!(kit_idx < tmpl_idx);
    }

    #[test]
    fn create_args_workspace_positional_no_ro() {
        let mut cfg = empty_config();
        cfg.volumes.push(VolumeMount {
            host_path: "/Users/g/proj".to_string(),
            container_path: "/Users/g/proj".to_string(),
            read_only: false,
        });
        let args = build_create_args("sbx-test", "alpine", None, &cfg);
        assert!(args.contains(&"/Users/g/proj".to_string()));
        assert!(!args.iter().any(|a| a.ends_with(":ro")));
    }

    #[test]
    fn create_args_workspace_positional_with_ro() {
        let mut cfg = empty_config();
        cfg.volumes.push(VolumeMount {
            host_path: "/Users/g/docs".to_string(),
            container_path: "/Users/g/docs".to_string(),
            read_only: true,
        });
        let args = build_create_args("sbx-test", "alpine", None, &cfg);
        assert!(args.contains(&"/Users/g/docs:ro".to_string()));
    }

    #[test]
    fn create_args_multiple_workspaces_in_order() {
        let mut cfg = empty_config();
        cfg.volumes.push(VolumeMount {
            host_path: "/a".to_string(),
            container_path: "/a".to_string(),
            read_only: false,
        });
        cfg.volumes.push(VolumeMount {
            host_path: "/b".to_string(),
            container_path: "/b".to_string(),
            read_only: true,
        });
        let args = build_create_args("sbx-test", "alpine", None, &cfg);
        let a_idx = args.iter().position(|x| x == "/a").unwrap();
        let b_idx = args.iter().position(|x| x == "/b:ro").unwrap();
        assert!(a_idx < b_idx);
    }

    #[test]
    fn create_args_cpus_and_memory() {
        let mut cfg = empty_config();
        cfg.cpu_limit = Some("4".to_string());
        cfg.memory_limit = Some("8g".to_string());
        let args = build_create_args("t", "alpine", None, &cfg);
        let cpu_idx = args.iter().position(|a| a == "--cpus").unwrap();
        assert_eq!(args[cpu_idx + 1], "4");
        let mem_idx = args.iter().position(|a| a == "-m").unwrap();
        assert_eq!(args[mem_idx + 1], "8g");
    }

    #[test]
    fn create_args_anonymous_volumes_not_emitted() {
        let mut cfg = empty_config();
        cfg.anonymous_volumes.push("/workspace/target".to_string());
        let args = build_create_args("t", "alpine", None, &cfg);
        // sbx doesn't support anonymous volumes; they're silently dropped
        // because supports_anonymous_volumes == false.
        assert!(!args.iter().any(|a| a.contains("/workspace/target")));
    }

    #[test]
    fn create_args_port_mappings_not_emitted_at_create() {
        let mut cfg = empty_config();
        cfg.port_mappings.push("3000:3000".to_string());
        let args = build_create_args("t", "alpine", None, &cfg);
        // supports_port_publish_at_create == false; ports go through
        // sbx::argv::build_ports_args post-create (Phase 5).
        assert!(!args.contains(&"-p".to_string()));
        assert!(!args.contains(&"3000:3000".to_string()));
    }

    // ----- build_exec_args shapes (16 combinations, 10+ exercised) -----

    #[test]
    fn exec_args_minimal_no_flags() {
        let args = build_exec_args("box1", None, &[], false, false, &["bash"]);
        assert_eq!(args, vec!["exec", "box1", "bash"]
            .into_iter().map(String::from).collect::<Vec<_>>());
    }

    #[test]
    fn exec_args_interactive_only() {
        let args = build_exec_args("box1", None, &[], true, false, &["bash"]);
        assert!(args.contains(&"-i".to_string()));
        assert!(!args.contains(&"-t".to_string()));
    }

    #[test]
    fn exec_args_tty_only() {
        let args = build_exec_args("box1", None, &[], false, true, &["bash"]);
        assert!(args.contains(&"-t".to_string()));
        assert!(!args.contains(&"-i".to_string()));
    }

    #[test]
    fn exec_args_interactive_and_tty() {
        let args = build_exec_args("box1", None, &[], true, true, &["bash"]);
        let i_idx = args.iter().position(|a| a == "-i").unwrap();
        let t_idx = args.iter().position(|a| a == "-t").unwrap();
        assert!(i_idx < t_idx);
    }

    #[test]
    fn exec_args_workdir() {
        let args = build_exec_args("box1", Some("/work"), &[], true, true, &["bash"]);
        let w_idx = args.iter().position(|a| a == "-w").unwrap();
        assert_eq!(args[w_idx + 1], "/work");
    }

    #[test]
    fn exec_args_env_literal() {
        let env = [EnvEntry::Literal {
            key: "TERM".to_string(),
            value: "xterm".to_string(),
        }];
        let args = build_exec_args("box1", None, &env, false, false, &["bash"]);
        assert!(args.contains(&"TERM=xterm".to_string()));
    }

    #[test]
    fn exec_args_env_inherit_emits_key_only() {
        let env = [EnvEntry::Inherit {
            key: "GH_TOKEN".to_string(),
            value: "ghp_secret".to_string(),
        }];
        let args = build_exec_args("box1", None, &env, false, false, &["bash"]);
        assert!(args.contains(&"GH_TOKEN".to_string()));
        assert!(!args.iter().any(|a| a.contains("ghp_secret")));
    }

    #[test]
    fn exec_args_mixed_env() {
        let env = [
            EnvEntry::Inherit { key: "S".to_string(), value: "v".to_string() },
            EnvEntry::Literal { key: "L".to_string(), value: "x".to_string() },
        ];
        let args = build_exec_args("box1", None, &env, false, false, &["bash"]);
        assert!(args.contains(&"S".to_string()));
        assert!(args.contains(&"L=x".to_string()));
    }

    #[test]
    fn exec_args_workdir_and_env_and_flags() {
        let env = [EnvEntry::Literal {
            key: "K".to_string(),
            value: "V".to_string(),
        }];
        let args = build_exec_args("box1", Some("/w"), &env, true, true, &["bash", "-l"]);
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"-t".to_string()));
        assert!(args.contains(&"-w".to_string()));
        assert!(args.contains(&"K=V".to_string()));
        let name_idx = args.iter().position(|a| a == "box1").unwrap();
        let bash_idx = args.iter().position(|a| a == "bash").unwrap();
        assert!(name_idx < bash_idx);
    }

    #[test]
    fn exec_args_cmd_with_multiple_argv() {
        let args = build_exec_args("box1", None, &[], false, false, &["echo", "hello", "world"]);
        let echo_idx = args.iter().position(|a| a == "echo").unwrap();
        assert_eq!(args[echo_idx + 1], "hello");
        assert_eq!(args[echo_idx + 2], "world");
    }

    // ----- build_ports_args shapes -----

    #[test]
    fn ports_args_simple_spec() {
        let args = build_ports_args("box1", "3000:3000");
        assert_eq!(args, vec!["ports", "box1", "--publish", "3000:3000"]
            .into_iter().map(String::from).collect::<Vec<_>>());
    }

    #[test]
    fn ports_args_host_ip_spec() {
        let args = build_ports_args("box1", "127.0.0.1:8080:8080/tcp");
        let p_idx = args.iter().position(|a| a == "--publish").unwrap();
        assert_eq!(args[p_idx + 1], "127.0.0.1:8080:8080/tcp");
    }

    #[test]
    fn ports_args_ephemeral_spec() {
        let args = build_ports_args("box1", "8080");
        assert!(args.contains(&"--publish".to_string()));
        assert!(args.contains(&"8080".to_string()));
    }
}
```

That's **15 argv-shape tests** for `build_create_args` (8) + `build_exec_args` (10) + `build_ports_args` (3) = well over the success-criterion-1 minimum of 10.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `RuntimeBase`-only capability flag (Phase 0) | `RuntimeCapabilities` struct on the trait + per-backend const + sbx peer | Phase 1 (2026-05-15) | Trait now mockable; peer backends can declare capabilities without using `RuntimeBase` |
| Sbx `is_available` short-circuits to `false` (Phase 1 stub) | Real `<binary> --version` probe via injectable `SbxRuntime::binary` (Phase 2) | This phase | Tests can inject a fake binary; production runs `which sbx` semantically |
| `exec_command` Sbx arm returns sentinel string (Phase 1 stub) | Composes a real `sbx exec` argv via `sbx::argv::build_exec_args` (Phase 2) | This phase | Plumbed for Phase 5 subprocess wiring |

**Deprecated/outdated:**
- The Roadmap's `RuntimeHandle::Sbx(SbxRuntime)` wording (Phase 2 success criterion #4) — superseded by D-01's facade choice. Do not propagate. [CITED: ROADMAP.md L48; ARCHITECTURE.md L82-110 documents the original `RuntimeHandle` proposal that was rejected]

## Dispatch Site Inventory (Open Question 10)

D-01 claims "the ~12 sites that consume `get_container_runtime()` are not touched." Actual census of `grep get_container_runtime` across `src/` and `tests/`:

| File | Line | Usage | Phase 2 Touches? |
|------|------|-------|------------------|
| `src/containers/mod.rs` | 30 | factory definition | no |
| `src/containers/mod.rs` | 46 | `batch_container_health()` helper | no |
| `src/containers/mod.rs` | 60 | `DockerContainer::new()` | no |
| `src/containers/mod.rs` | 72 | `DockerContainer::from_session_id()` | no |
| `src/containers/mod.rs` | 177 | test only | no |
| `src/cli/add.rs` | 463 | sandbox availability check | no |
| `src/session/builder.rs` | 304 | sandbox precondition check | no |
| `src/session/instance.rs` | 1249 | `ensure_image` call | no |
| `src/server/api/system.rs` | 366 | `/docker_status` API | no |
| `src/tui/dialogs/new_session/mod.rs` | 327 | dialog availability check | no |
| `src/tui/dialogs/new_session/mod.rs` | 413 | default image render | no |
| `src/tui/dialogs/new_session/mod.rs` | 668 | default image render | no |
| `src/tui/dialogs/new_session/mod.rs` | 731 | default image render | no |
| `src/tui/dialogs/new_session/tests.rs` | 688 | test only | no |
| `src/tui/dialogs/new_session/tests.rs` | 763 | test only | no |
| `src/tui/dialogs/new_session/tests.rs` | 821 | test only | no |
| `tests/sandbox_integration.rs` | 12 | integration test | no |

**Verified:** 17 call sites total (12 production code, 4 test code, 1 factory definition). Zero are touched by Phase 2. D-01's "~12 sites" guarantee holds; the actual number including tests is 17, but the substance of the claim (no caller churn) is verified. [VERIFIED: bash grep of repo, 2026-05-15]

## Assumptions Log

> All concrete claims in this research are grounded in file reads against the repo at the time of research. The only items below are training-data-flavored caveats, not unverified facts.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| (none) | — | — | — |

**This table is empty:** Every claim in this research traces to a file read (`runtime.rs`, `runtime_base.rs`, `container_interface.rs`, `mod.rs`, `Cargo.toml`, `AGENTS.md`, `CONTEXT.md`, `ROADMAP.md`, `REQUIREMENTS.md`, `STATE.md`, `PROJECT.md`, `01-CONTEXT.md`, `ARCHITECTURE.md`, `sbx-cli-reference.md`) or a `bash grep` over the repo. No user confirmation needed on top of CONTEXT.md's locked decisions.

## Open Questions (RESOLVED)

1. **D-12: re-declare vs route `capabilities()` const** — Research recommends routing through `RuntimeBase::SBX.capabilities` (Pitfall 6) to avoid drift. Planner decides.
   - **RESOLVED:** Route through `RuntimeBase::SBX.capabilities` (per plan 02-01 Task 1). Single source of truth; zero drift risk between Phase 1's locked matrix and Phase 2's accessor.

2. **D-03: storage shape for `SbxRuntime` inside `ContainerRuntime`** — Research recommends Option A (optional field) for diff size and test-injection ergonomics. Planner decides.
   - **RESOLVED:** Option A — `pub(crate) sbx: Option<SbxRuntime>` field on `ContainerRuntime` (per plan 02-02 Task 1). Smallest diff, no caller churn, kind-invariant enforced locally via `.as_ref().expect(...)` at the three modified dispatch arms.

3. **`build_exec_args` signature: positional bool params vs single struct** — Both pass success criterion #1. Research's example uses positional params (matches `RuntimeBase::exec_command` shape at `runtime_base.rs:407-413`). Planner decides.
   - **RESOLVED:** Positional params per D-07 (per plan 02-01 Task 2). Matches the `RuntimeBase::exec_command` shape; reads cleanly at the call site in the strengthened `runtime.rs::tests` block.

4. **Exact `runtime.rs` `exec_command` arm body** — How the dispatch arm parses the existing `options: Option<&str>` parameter (used by Docker as "-w /workspace" passthrough) into discrete `workdir` / `env` / flag arguments for `build_exec_args`. Research provides a reasonable Phase 2 simplification (parse `"-w "` prefix into workdir; drop other content). Planner may pick a different adapter or do it minimally; either way the existing test `test_sbx_dispatch_arms_return_safe_stubs` (loose assertion: `cmd.contains("sbx") && cmd.contains("foo")`) continues to pass.
   - **RESOLVED:** Adopt the `-w ` prefix-parse simplification (per plan 02-02 Task 1 Action Step 6). Phase 2's real call sites for `exec_command` are unreachable per D-06 (action-verb arms remain Phase 1 stubs until Phase 5); the adapter exists only so the strengthened test in `runtime.rs::tests` proves dispatch routes through to the new builder.

5. **SC-4 Roadmap wording risk** — The Roadmap text "Trait dispatch through the new `RuntimeHandle::Sbx(SbxRuntime)` variant" is stale (D-01 rejected `RuntimeHandle`). Flagged so plan-checker does not interpret SC-4 literally; the working mechanism is the existing `RuntimeKind::Sbx` dispatch in `runtime.rs`. Do not rewrite the Roadmap unilaterally; surface to user if it confuses review.
   - **RESOLVED:** Both plans flag this as `risks: R-01` and implement SC-4 via the existing `RuntimeKind::Sbx` dispatch. Surfaced to user via plan-checker run; no Roadmap rewrite. The intent of SC-4 ("trait dispatch routes through to SbxRuntime without regression") is preserved.

## Environment Availability

> Skipped — Phase 2 is a pure code/config change. No external dependencies required at research, plan, or implementation time. The `sbx` binary is **not** required to run any Phase 2 test (the injection seam D-10 lets the unit test work with a tempfile fake-binary).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (no external test crate); `serial_test 3.4` for tmux-affected tests (not used here); `tempfile 3.14` for fake-binary tests |
| Config file | `Cargo.toml` only; no `pytest.ini`/`jest.config` equivalents in Rust |
| Quick run command | `cargo test -p agent-of-empires sbx::` |
| Full suite command | `cargo test` (TUI-only) or `cargo test --features serve` (web dashboard included; not required for Phase 2) |

### Phase Requirements → Test Map

Phase 2 has no requirements that formally complete here; the validation map is keyed off the four Roadmap success criteria.

| SC ID | Behavior | Test Type | Automated Command | File Exists? |
|-------|----------|-----------|-------------------|-------------|
| SC-1 (create) | `build_create_args` emits `create shell --name <name> [--kit ...] --template <image> [--cpus N] [-m M] <ws>[:ro] ...` for ≥4 distinct shapes | unit | `cargo test -p agent-of-empires sbx::argv::tests::create_args_` | ❌ Wave 0 (new module) |
| SC-1 (exec) | `build_exec_args` emits `exec [-i] [-t] [-w <wd>] [-e ...] <name> <cmd>...` for ≥4 distinct shapes | unit | `cargo test -p agent-of-empires sbx::argv::tests::exec_args_` | ❌ Wave 0 (new module) |
| SC-1 (ports) | `build_ports_args` emits `ports <name> --publish <spec>` for ≥2 distinct specs | unit | `cargo test -p agent-of-empires sbx::argv::tests::ports_args_` | ❌ Wave 0 (new module) |
| SC-2 | `SbxRuntime::capabilities()` returns sbx-correct matrix per D-12 | unit | `cargo test -p agent-of-empires sbx::tests::capabilities_matches_runtime_base_sbx` | ❌ Wave 0 (new module) |
| SC-3 | `SbxRuntime::is_available()` returns true with injected fake binary, false with nonexistent path | unit | `cargo test -p agent-of-empires sbx::tests::is_available_with_` | ❌ Wave 0 (new module) |
| SC-4 (no regression) | All existing `runtime.rs` tests continue to pass after the three arm edits | unit | `cargo test -p agent-of-empires --lib containers::runtime` | ✅ exists at `runtime.rs:316-552` |
| SC-4 (dispatch routes) | The `RuntimeKind::Sbx` arms in `runtime.rs` route to `SbxRuntime` and produce non-stub output for `is_available`, `exec_command`, `build_create_args` | unit | Extend `runtime.rs::tests::test_sbx_dispatch_arms_return_safe_stubs` (loose) and add stricter shape assertions | ✅ partial — exists at `runtime.rs:534-542`, needs strengthening |

### Sampling Rate

- **Per task commit:** `cargo test -p agent-of-empires sbx::` (sub-second; pure-function tests + 3 subprocess probes)
- **Per wave merge:** `cargo test -p agent-of-empires --lib containers::` (covers all backend regression tests)
- **Phase gate:** `cargo test` (full suite green); plus `cargo fmt --check` and `cargo clippy -- -D warnings` per REQ-TEST-05

### Wave 0 Gaps

- [ ] `src/containers/sbx/mod.rs` — covers SC-2 (`capabilities`), SC-3 (`is_available`), and the test-injection seam (D-10). Includes `#[cfg(test)] mod tests` per AGENTS.md.
- [ ] `src/containers/sbx/argv.rs` — covers SC-1 (`build_create_args`, `build_exec_args`, `build_ports_args`). Includes `#[cfg(test)] mod tests` with ≥15 argv-shape tests (exceeds the 10-shape minimum).
- [ ] `src/containers/mod.rs` — add `mod sbx;` declaration (one-line).
- [ ] `src/containers/runtime.rs` — modify three arm bodies; modify `ContainerRuntime` struct per D-03 storage choice (1 added field for Option A); modify `ContainerRuntime::sbx()` constructor; modify other constructors to populate the new field with default (`None` for Option A).
- [ ] Strengthen `runtime.rs::tests::test_sbx_dispatch_arms_return_safe_stubs` to assert that `exec_command` emits the new builder shape (e.g., `cmd.contains("exec") && cmd.contains("foo")`), not the stale sentinel comment.
- [ ] Add a fresh test in `runtime.rs::tests` asserting `build_create_args` for `RuntimeKind::Sbx` emits `"create" + "shell" + "--name"` (proves the new path is reached, distinct from `RuntimeBase::build_create_args`'s `"run" + "-d"` shape).
- Framework install: none — Rust's built-in test runner and existing `tempfile` dep are sufficient.

## Security Domain

> `security_enforcement` is not set in `.planning/config.json` (verified at `.planning/config.json`). Treating as enabled per the contract.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase 2 adds no authentication surface. `sbx login` is a user-managed prerequisite (DOC-01, Phase 7). |
| V3 Session Management | no | Phase 2 manages no sessions. |
| V4 Access Control | no | Phase 2 manages no access control. |
| V5 Input Validation | yes | The argv builders accept `&str` and `&ContainerConfig` from internal callers (no user input crosses the layer). `name` and `port_spec` flow from upstream config; Phase 3 enforces workspace-path conformance. |
| V6 Cryptography | no | No cryptographic operations introduced. |

### Known Threat Patterns for Rust subprocess argv composition

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Argv injection via unescaped `&str` interpolation | Tampering | Use `Command::args(&[...])` rather than building a shell string. Rust's `Command` does not invoke a shell; each argv element is passed as a discrete `execve` argument. Phase 2's builders return `Vec<String>`, which Phase 5 will pass to `Command::args`. No shell interpolation possible. [VERIFIED: matches existing `RuntimeBase::build_create_args` -> `RuntimeBase::run_create` flow at `runtime_base.rs:319-353`] |
| Secret leakage into argv (e.g., `-e KEY=secret` in `ps`) | Information Disclosure | `EnvEntry::Inherit` deliberately emits only `-e KEY` in argv; the value travels through the child's process environment (see `RuntimeBase::run_create` at `runtime_base.rs:325-332`). Phase 2's `build_exec_args` matches this pattern. The side-channel for Inherit values in the `String`-returning `exec_command` path is not a Phase 2 concern (see Pitfall 1). |
| Symbolic link redirection of the injected fake binary | Tampering | The Phase 2 test creates a `tempfile::NamedTempFile` (owned by the test process, unique name) and chmods it to 0o755. The `Command::new(&self.binary)` call resolves the absolute path with no PATH-search ambiguity. `NamedTempFile` cleans up on drop. No symlink chase. |
| Path traversal in `kit_path` argument | Tampering | Phase 2 accepts `kit_path: Option<&Path>` from internal callers only (in Phase 5, from `KitMaterializer`). Phase 2 itself passes `None`. Phase 4's materializer is the entity that chooses the kit path; it builds against a content-addressed cache dir, not user input. Phase 2 has nothing to validate. |

## Project Constraints (from CLAUDE.md → AGENTS.md)

`./CLAUDE.md` is a symlink to `AGENTS.md`. Extracted actionable directives:

- **No dead code.** Never `#[allow(dead_code)]`. If a field isn't used yet, don't add it. Phase 2 implication: `SbxRuntime` must not contain fields that no method reads. The single `binary: PathBuf` field is read by `is_available()`; that's the bare minimum and required by D-10.
- **No emdashes or `--`** as separators in human-authored prose. (Auto-generated content is exempt; clippy/rustfmt output is exempt.)
- **No platform gating in `src/containers/`.** OS-specific code lives in `src/process/{macos,linux}.rs`. Phase 2's chmod-0o755 in the test is Unix-only via `PermissionsExt`; per PLAT-01 Windows is out of scope, so no `#[cfg(target_os = ...)]` gating is added. Verified: the chmod sits inside `#[cfg(test)]` and uses `std::os::unix::fs::PermissionsExt` which is itself Unix-only and inert on Windows builds.
- **Use unit tests in-module (`#[cfg(test)] mod tests { ... }`) for pure logic.** Phase 2's `argv.rs` follows this exactly. Integration tests go in `tests/*.rs`; nothing in Phase 2 requires that surface.
- **No backwards-compatibility shims by default.** Phase 2's `runtime.rs` arm edits do not preserve the Phase 1 stub semantics for `exec_command` or `build_create_args`. Plan must note this as an internal-breaking change against Phase 1's stubs, but it is the explicit Phase 2 step and CONTEXT.md D-04 endorses it.
- **`cargo fmt` + `cargo clippy` must pass.** REQ-TEST-05 lock from Phase 1.
- **Settings TUI completeness rule** does not apply: Phase 2 adds no `SandboxConfig` fields. Phase 6 owns the settings wiring.

## Sources

### Primary (HIGH confidence)
- `.planning/phases/02-sbxruntime-skeleton-and-pure-argv-builders/02-CONTEXT.md` — D-01..D-12 locks
- `.planning/REQUIREMENTS.md` — traceability table (Phase 2 = 0 requirements)
- `.planning/ROADMAP.md` § Phase 2 — success criteria SC-1..SC-4 (with SC-4 stale wording risk)
- `.planning/PROJECT.md` § Key Decisions — sbx as 4th `ContainerRuntime` peer
- `.planning/phases/01-capability-surface-and-trait-foundation/01-CONTEXT.md` — carry-forward (D-05/D-06/D-03/D-04)
- `.planning/research/ARCHITECTURE.md` § Module Placement, § Façade pattern, § Suggested Build Order
- `.planning/sbx-cli-reference.md` § `sbx create`, `sbx exec`, `sbx ports`
- `src/containers/container_interface.rs` (full read) — trait shape, `RuntimeCapabilities`, `ContainerConfig`, `EnvEntry`, `VolumeMount`
- `src/containers/runtime.rs` (full read) — existing dispatch arms, Phase 1 sbx test coverage
- `src/containers/runtime_base.rs` (full read) — `RuntimeBase::SBX` const, `build_create_args` pattern, `EnvEntry` encoding pattern at lines 277-287
- `src/containers/mod.rs` (full read) — factory, `DockerContainer`, `get_container_runtime`
- `Cargo.toml` (read) — `tempfile = "3.14"` at line 104 (top-level dep), `rust-version = "1.85"`
- `AGENTS.md` (full read) — coding style, testing guidelines, no-dead-code rule, in-module unit-test rule

### Secondary (MEDIUM confidence)
- `bash grep get_container_runtime` over `src/` and `tests/` — verified 17 call sites; D-01's "~12" claim holds

### Tertiary (LOW confidence)
- None. All claims in this research are grounded in file reads or repo grep.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every component (stdlib, `tempfile`) is in `Cargo.toml` at a verified version; no external crate decisions to make
- Architecture: HIGH — D-01..D-12 lock the architecture; only D-03 storage shape is open and three concrete options are presented with tradeoffs
- Pitfalls: HIGH — each pitfall is grounded in a specific code reference (`runtime_base.rs:277-287` for env encoding, `runtime_base.rs:121-131` for the probe shape, etc.)
- Test layout: HIGH — AGENTS.md mandates in-module `#[cfg(test)]` for pure logic; every existing `src/containers/*.rs` file follows it

**Research date:** 2026-05-15
**Valid until:** 2026-06-14 (30 days; the codebase patterns are stable and locked decisions D-01..D-12 hold the architecture in place)
