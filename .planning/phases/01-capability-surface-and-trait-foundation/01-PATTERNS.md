# Phase 1: Capability Surface and Trait Foundation - Pattern Map

**Mapped:** 2026-05-15
**Files analyzed:** 6 (5 modified + 1 new struct location, planner choice)
**Analogs found:** 6 / 6 (every file has an in-tree precedent; this phase is a refactor of an existing pattern, not a greenfield surface)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/containers/container_interface.rs` (MOD) | trait definition + data shapes | request-response | self (existing trait at L52-97) | exact (extending self) |
| `src/containers/runtime_base.rs` (MOD) | shared runtime config + behavior | request-response | self (existing `DOCKER` / `APPLE_CONTAINER` / `PODMAN` consts at L29-60) | exact (extending self) |
| `src/containers/runtime.rs` (MOD) | trait dispatch via enum | request-response | self (existing `RuntimeKind` + `match self.kind` at L14-19, L88-251) | exact (extending self) |
| `src/containers/mod.rs` (MOD) | factory functions | request-response | self (existing `runtime_binary()` / `get_container_runtime()` at L15-37) | exact (extending self) |
| `src/session/config.rs` (MOD) | serde enum extension | config | self (existing `ContainerRuntimeName` at L734-741) | exact (extending self) |
| `src/containers/capabilities.rs` OR inline (NEW) | data struct (capability flags) | none | `ContainerConfig` struct in `container_interface.rs` L42-50; `RuntimeBase` struct in `runtime_base.rs` L11-26 | role-match (peer data carrier in same module) |
| Tests (added in `runtime.rs` or `runtime_base.rs`) | unit test | n/a | `runtime.rs::test_podman_supports_docker_compatible_features` L354-363 | exact |

Match quality is dominated by "extending self" because every modified file already implements the exact pattern Phase 1 needs to grow. The compiler-enforced exhaustiveness contract (D-02) is itself an extension of the existing const-literal pattern in `runtime_base.rs`; there is no analog for "default impl avoidance" because the codebase has never used trait default impls for capability flags.

## Pattern Assignments

### `src/containers/runtime_base.rs` (shared runtime config + behavior)

**Analog:** self (the existing `RuntimeBase` struct + 3 const literals; this is THE precedent the phase extends)

**Existing capability-flag fields** (lines 22-25):
```rust
/// Whether this runtime supports the `:ro` read-only volume flag
pub supports_read_only_volumes: bool,
/// Whether this runtime supports `-v` on remove to clean up anonymous volumes
pub supports_remove_volumes: bool,
```

**Existing const-literal pattern, exhaustive field init** (lines 29-60):
```rust
impl RuntimeBase {
    pub const DOCKER: Self = Self {
        binary: "docker",
        name: "Docker",
        daemon_check_args: &["info"],
        pull_prefix: &["pull"],
        remove_subcommand: "rm",
        supports_read_only_volumes: true,
        supports_remove_volumes: true,
    };

    pub const APPLE_CONTAINER: Self = Self {
        binary: "container",
        name: "Apple Container",
        daemon_check_args: &["system", "status"],
        pull_prefix: &["image", "pull"],
        remove_subcommand: "delete",
        supports_read_only_volumes: false,
        supports_remove_volumes: false,
    };

    pub const PODMAN: Self = Self {
        binary: "podman",
        name: "Podman",
        daemon_check_args: &["info"],
        pull_prefix: &["pull"],
        remove_subcommand: "rm",
        supports_read_only_volumes: true,
        supports_remove_volumes: true,
    };
    // ...
}
```

**Pattern to copy (the "hybrid" mechanism per D-01, D-02, D-03):**
- Replace the two `pub bool` fields with a single `pub capabilities: RuntimeCapabilities` field on `RuntimeBase`.
- Each const literal initializes `capabilities: RuntimeCapabilities { ... }` with **every** field named explicitly (no `..Default::default()` per D-02).
- Add a 4th const `RuntimeBase::SBX = Self { binary: "sbx", name: "Docker Sandboxes", ... capabilities: RuntimeCapabilities { ... honest sbx values ... } }`.
- The compiler refuses to build the next time a flag is added unless every const literal is updated. This is the success-criterion-1 mechanism.

**`is_available()` pattern that the sbx stub MUST short-circuit** (lines 66-72):
```rust
pub fn is_available(&self) -> bool {
    self.command()
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

For the Phase 1 sbx stub: do NOT reuse this `--version` probe. Either (a) the `Sbx` arm in `ContainerRuntime::is_available` short-circuits to `false` regardless of `self.base`, or (b) `RuntimeBase::SBX.is_available()` is overridden via the dispatch enum. Per D-07: "no new `#[cfg(target_os = ...)]` branches anywhere in `src/containers/`" — the platform gate is implicit in `which sbx` failing on unsupported platforms, but Phase 1 returns `false` unconditionally regardless.

**Test-block pattern** (lines 335-530): in-module `#[cfg(test)] mod tests { ... }` with each test using a const-literal `RuntimeBase::DOCKER` / `RuntimeBase::APPLE_CONTAINER` directly. Phase 1 tests assert each backend's `capabilities` field matches the honest matrix (5 new + 2 migrated flags × 4 backends = 28 assertions, but the exhaustive struct literal in the const body is the actual safety net).

---

### `src/containers/runtime.rs` (trait dispatch via enum)

**Analog:** self (the existing 3-arm `RuntimeKind` enum + `match self.kind` dispatch; this is THE precedent the phase extends)

**`RuntimeKind` enum** (lines 14-19):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Docker,
    AppleContainer,
    Podman,
}
```

**Constructor pattern, one per kind** (lines 26-47):
```rust
impl ContainerRuntime {
    pub fn docker() -> Self {
        Self {
            base: RuntimeBase::DOCKER,
            kind: RuntimeKind::Docker,
        }
    }

    pub fn apple_container() -> Self {
        Self {
            base: RuntimeBase::APPLE_CONTAINER,
            kind: RuntimeKind::AppleContainer,
        }
    }

    pub fn podman() -> Self {
        Self {
            base: RuntimeBase::PODMAN,
            kind: RuntimeKind::Podman,
        }
    }
}
```

**Trait-method-via-`match self.kind` dispatch** (lines 88-106 — `does_container_exist`; same shape repeats at L108-141 `is_container_running`, L171-203 `exec_command`, L209-251 `batch_running_states`):
```rust
fn does_container_exist(&self, name: &str) -> Result<bool> {
    match self.kind {
        RuntimeKind::Docker | RuntimeKind::Podman => {
            let output = self
                .base
                .command()
                .args(["container", "inspect", name])
                .output()?;
            Ok(output.status.success())
        }
        RuntimeKind::AppleContainer => {
            // Apple Container's `inspect` returns success(0) for non-existent
            // containers, so we use `logs` which properly fails for missing
            // containers.
            let output = self.base.command().args(["logs", name]).output()?;
            Ok(output.status.success())
        }
    }
}
```

**Pattern to copy for sbx stub (D-05, D-06):**
- Add `RuntimeKind::Sbx` as the 4th variant in the enum (line 14-19).
- Add `pub fn sbx() -> Self { Self { base: RuntimeBase::SBX, kind: RuntimeKind::Sbx } }` constructor (line 26-47 pattern).
- Every `match self.kind` block (4 of them: L88, L108, L172, L210) MUST gain a `RuntimeKind::Sbx => { ... }` arm. For Phase 1 stub, the arms can return safe placeholder values:
  - `does_container_exist` → `Ok(false)` (no sbx container can exist if the runtime isn't real yet)
  - `is_container_running` → `Ok(false)`
  - `exec_command` → empty string or `format!("sbx exec {} {}", name, cmd)` — must be addressable from settings code per integration point in CONTEXT.md, but is unreachable until Phase 2.
  - `batch_running_states` → `HashMap::new()` (mirrors the `AppleContainer` arm at L246-249: `let _ = prefix; HashMap::new()`)
- For the new trait method `fn capabilities(&self) -> RuntimeCapabilities`: implement it as `self.base.capabilities` (no kind switch needed — the field lives on `RuntimeBase` per D-01's hybrid placement).

**Test pattern for capability honesty** (lines 354-363):
```rust
#[test]
fn test_podman_supports_docker_compatible_features() {
    // Podman is a drop-in for Docker, so it must support the same feature
    // set the shared base relies on. If this regresses, the create-args
    // builder will silently produce broken output for podman users.
    let rt = ContainerRuntime::podman();
    assert!(rt.base.supports_read_only_volumes);
    assert!(rt.base.supports_remove_volumes);
    assert_eq!(rt.base.remove_subcommand, "rm");
    assert_eq!(rt.base.pull_prefix, &["pull"]);
}
```

**Pattern to copy for the Phase 1 capability-matrix tests:**
- One test per backend (`test_docker_capabilities`, `test_podman_capabilities`, `test_apple_container_capabilities`, `test_sbx_capabilities_honest`).
- Each asserts the full 7-field `RuntimeCapabilities` struct value.
- After D-02 migration, accessing the field is `rt.base.capabilities.supports_X` (not `rt.base.supports_X`).
- The sbx test asserts `is_available() == false` AND the capability matrix is honest (`supports_image_pull: false`, `supports_dynamic_port_publish: true`, etc., per the table in `RESEARCH.md/ARCHITECTURE.md` Trait Evolution section).

---

### `src/containers/container_interface.rs` (trait definition + data shapes)

**Analog:** self (the existing trait + `ContainerConfig` data struct)

**Existing trait surface** (lines 52-97):
```rust
pub trait ContainerRuntimeInterface {
    /// Check if the container runtime CLI is available
    fn is_available(&self) -> bool;

    /// Check if the container runtime daemon is running
    fn is_daemon_running(&self) -> bool;

    /// Get the container runtime version string
    fn get_version(&self) -> Result<String>;
    // ... 14 more methods, no default impls
}
```

**Existing data-struct pattern in same file** (lines 5-9, 42-50):
```rust
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: bool,
}

pub struct ContainerConfig {
    pub working_dir: String,
    pub volumes: Vec<VolumeMount>,
    pub anonymous_volumes: Vec<String>,
    pub environment: Vec<EnvEntry>,
    pub cpu_limit: Option<String>,
    pub memory_limit: Option<String>,
    pub port_mappings: Vec<String>,
}
```

Note: existing structs in this file have **no** `#[derive(...)]`. The planner should derive `Clone, Copy, Debug, PartialEq, Eq` on `RuntimeCapabilities` per D-discretion (all fields are `bool`, so `Copy` is cheap and useful for tests).

**Pattern to copy:**
- Add `RuntimeCapabilities` struct (7 `pub bool` fields, exactly named per `<specifics>` in CONTEXT.md) at the top of the file alongside `VolumeMount` (L5-9) — or in a peer `capabilities.rs` if planner prefers (D-discretion). Inline keeps the diff smaller and matches the file's existing role as "trait + its data shapes."
- Add `fn capabilities(&self) -> RuntimeCapabilities;` to the trait (line 52-97). NO default impl (PITFALLS C6).
- Re-export from `mod.rs` alongside `ContainerConfig`, `EnvEntry`, `VolumeMount` (line 10): `pub use container_interface::{ContainerConfig, ContainerRuntimeInterface, EnvEntry, RuntimeCapabilities, VolumeMount};`.

**Field list (from `<specifics>` and ARCHITECTURE table):**
```rust
pub struct RuntimeCapabilities {
    pub supports_read_only_volumes: bool,            // migrated from RuntimeBase
    pub supports_remove_volumes: bool,                // migrated from RuntimeBase
    pub supports_port_publish_at_create: bool,        // NEW
    pub supports_image_pull: bool,                    // NEW
    pub supports_anonymous_volumes: bool,             // NEW
    pub supports_arbitrary_volume_paths: bool,        // NEW
    pub supports_dynamic_port_publish: bool,          // NEW
}
```

---

### `src/containers/mod.rs` (factory functions)

**Analog:** self (the existing 3-arm `match cfg.sandbox.container_runtime`)

**`runtime_binary()` factory** (lines 15-25):
```rust
pub fn runtime_binary() -> &'static str {
    if let Ok(cfg) = Config::load() {
        match cfg.sandbox.container_runtime {
            ContainerRuntimeName::AppleContainer => "container",
            ContainerRuntimeName::Docker => "docker",
            ContainerRuntimeName::Podman => "podman",
        }
    } else {
        "docker"
    }
}
```

**`get_container_runtime()` factory** (lines 27-37):
```rust
pub fn get_container_runtime() -> ContainerRuntime {
    if let Ok(cfg) = Config::load() {
        match cfg.sandbox.container_runtime {
            ContainerRuntimeName::AppleContainer => ContainerRuntime::apple_container(),
            ContainerRuntimeName::Docker => ContainerRuntime::docker(),
            ContainerRuntimeName::Podman => ContainerRuntime::podman(),
        }
    } else {
        ContainerRuntime::default()
    }
}
```

**Pattern to copy:**
- Both functions gain a 4th arm: `ContainerRuntimeName::Sbx => "sbx"` and `ContainerRuntimeName::Sbx => ContainerRuntime::sbx()` respectively.
- The `else` branches (`"docker"` / `ContainerRuntime::default()`) are unchanged — Phase 1 does NOT change the default runtime.
- D-05 explicitly says "the ~12 callers of `get_container_runtime()` stay untouched" — the return type stays `ContainerRuntime`, NOT `RuntimeHandle` (RuntimeHandle is rejected for Phase 1; deferred to Phase 2).

---

### `src/session/config.rs` (serde enum)

**Analog:** self (the existing 3-variant `ContainerRuntimeName` enum)

**Existing enum** (lines 733-741):
```rust
/// Container runtime options for sandboxing
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerRuntimeName {
    AppleContainer,
    #[default]
    Docker,
    Podman,
}
```

**Field that uses it** (lines 728-730):
```rust
/// Container runtime to use for sandboxing (docker, podman, or apple_container)
#[serde(default)]
pub container_runtime: ContainerRuntimeName,
```

**Pattern to copy:**
- Add `Sbx` variant. With `#[serde(rename_all = "snake_case")]` already present, the on-disk TOML value is automatically `"sbx"` (no explicit `#[serde(rename = "sbx")]` attribute needed; "snake_case" lowercases single-word variants to themselves).
- Update the doc comment on `pub container_runtime` to mention sbx: `/// Container runtime to use for sandboxing (docker, podman, apple_container, or sbx)`.
- Variant order is alphabetical-ish in the existing enum (`AppleContainer`, `Docker`, `Podman`); inserting `Sbx` after `Podman` keeps the pattern.
- The `#[default]` stays on `Docker`. Phase 1 does NOT change the default.
- Adding an enum variant is serde-backwards-compatible (existing TOML deserializes unchanged), so NO migration in `src/migrations/` is needed (matches how Podman was added — see ARCHITECTURE.md Migration check).

---

### `src/containers/capabilities.rs` OR inline in `container_interface.rs` (new struct location)

**Analog:** `ContainerConfig` (`container_interface.rs:42-50`) — peer data struct in the same module, no derives, all `pub` fields.

**Decision criteria for inline vs peer module:**
- Inline (in `container_interface.rs`): smaller diff, struct lives with the trait that references it, matches file's existing role as "trait + data shapes." Recommended.
- Peer `capabilities.rs`: keeps the file size flat, easier to grep, leaves room for capability-related helpers (e.g., a future `requires_capability` predicate from PITFALLS C7).

Either is fine per CONTEXT.md `<decisions>` Claude's Discretion. Inline is the smaller delta; planner should default to it unless there's a concrete reason to peer-modularize.

**Derives:** `#[derive(Clone, Copy, Debug, PartialEq, Eq)]` per D-discretion. All four are cheap (struct is 7 bytes), all useful for tests, and `Copy + PartialEq` is the minimum surface PITFALLS C7 expects from the future Phase 6 `requires_capability` hook.

---

## Shared Patterns

### Compile-Time Exhaustiveness via Const Literals
**Source:** `src/containers/runtime_base.rs:29-60` (existing `DOCKER`/`APPLE_CONTAINER`/`PODMAN` consts)
**Apply to:** Every `RuntimeBase::*` const, including the new `RuntimeBase::SBX`. Every `RuntimeCapabilities { ... }` literal inside those consts.

```rust
pub const DOCKER: Self = Self {
    binary: "docker",
    name: "Docker",
    daemon_check_args: &["info"],
    pull_prefix: &["pull"],
    remove_subcommand: "rm",
    capabilities: RuntimeCapabilities {
        supports_read_only_volumes: true,
        supports_remove_volumes: true,
        supports_port_publish_at_create: true,
        supports_image_pull: true,
        supports_anonymous_volumes: true,
        supports_arbitrary_volume_paths: true,
        supports_dynamic_port_publish: false,
    },
};
```

The contract: every flag named explicitly, no `..Default::default()`, no missing fields. The compiler refuses to build the next time a flag is added unless every const is updated. This IS the success-criterion-1 mechanism (per D-02 and PITFALLS C6).

### Trait Dispatch via `match self.kind`
**Source:** `src/containers/runtime.rs:88-106` (and L108-141, L171-203, L209-251)
**Apply to:** Every existing `match self.kind` block must grow a `RuntimeKind::Sbx => { ... }` arm. The compiler enforces exhaustiveness — adding the variant breaks every match block until each gets the new arm.

For the new `fn capabilities(&self) -> RuntimeCapabilities` method, NO match needed: the body is `self.base.capabilities` because the data lives on `RuntimeBase` per D-01.

### Stub Behavior: `Ok(false)` / Empty Collections
**Source:** `src/containers/runtime.rs:246-249` (the existing `AppleContainer` arm of `batch_running_states`)
```rust
RuntimeKind::AppleContainer => {
    let _ = prefix;
    HashMap::new()
}
```
**Apply to:** Every Phase 1 sbx stub arm. For methods returning `Result<bool>`, return `Ok(false)`; for `HashMap`, return `HashMap::new()`; for `String` (exec_command), return an addressable but unreachable string. Pattern: gracefully degrade, never panic, never lie about success.

### `#[serde(rename_all = "snake_case")]` for runtime enums
**Source:** `src/session/config.rs:734-735`
**Apply to:** The existing attribute already covers `Sbx → "sbx"` automatically. NO explicit `#[serde(rename = "sbx")]` is needed (single-word PascalCase variants snake-case to themselves). Verify by running an existing config-deserialization test or by adding an assertion: `serde_json::to_string(&ContainerRuntimeName::Sbx).unwrap() == "\"sbx\""`.

### Tests in `#[cfg(test)] mod tests` In-Module
**Source:** `src/containers/runtime_base.rs:335-530` and `src/containers/runtime.rs:254-373`
**Apply to:** Phase 1 capability-matrix tests live in the same files as the things they test. No new `tests/*.rs` integration test needed for Phase 1 (the trait change has no observable runtime behavior beyond what unit tests can cover).

## No Analog Found

None. Phase 1 is a refactor of an existing const-literal + dispatch-enum pattern with concrete in-tree precedents for every modification. The closest thing to "no analog" is the design choice between inline-struct vs peer-module for `RuntimeCapabilities`, but both options have analogs (inline = `ContainerConfig` in `container_interface.rs`; peer = `runtime_base.rs` itself, which is a peer of `container_interface.rs`).

## Metadata

**Analog search scope:** `src/containers/`, `src/session/config.rs`
**Files scanned:** 5 (4 in `src/containers/`, 1 in `src/session/`)
**Pattern extraction date:** 2026-05-15
**Phase boundary respected:** New file location is planner choice (D-discretion); the recommendation here (inline in `container_interface.rs`) is non-binding.
