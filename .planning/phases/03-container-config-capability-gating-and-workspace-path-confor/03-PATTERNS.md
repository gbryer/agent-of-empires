# Phase 3: container_config Capability Gating and Workspace Path Conformance - Pattern Map

**Mapped:** 2026-05-15
**Files analyzed:** 2 modified (0 new). Planner discretion may add 1 peer-module file.
**Analogs found:** 5 / 5 surfaces (all strong, in-tree)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/session/container_config.rs` (modify) | session-tier pure-config builder; adds typed-error enum, two free fns, one new param on `build_container_config`, ~9 new in-module tests | request-response (pure transform: inputs → `ContainerConfig`) | `RuntimeBase::build_create_args` (`src/containers/runtime_base.rs:223-317`) for the capability-gated branching shape; `DockerError` (`src/containers/error.rs:1-53`) for the thiserror enum shape | exact (same module tier, same pure-transform shape) |
| `src/session/instance.rs` (modify) | session-tier orchestrator; adds 2-line `ensure_image` guard, threads `capabilities` through `container_workdir()` + the inner `build_container_config` wrapper, +19 call-site updates | request-response (call-site gating) | `RuntimeBase::ensure_image` (`src/containers/runtime_base.rs:195-210`) for the capability-check shape, but inverted from "defensive at impl" to "preventive at call site" | role-match (call-site gate is a new shape; pattern is the boolean capability check) |
| `src/session/container_config/error.rs` (NEW, planner discretion) | error module peer | n/a (declarative types) | `src/containers/error.rs` (peer to `runtime_base.rs`) | exact if planner picks peer-module placement; inline placement at top of `container_config.rs` is the smaller-delta default per CONTEXT D-06 |

**No new test file.** All Phase 3 tests slot into the existing `mod tests` block in `container_config.rs` (starts line 1156). `instance.rs` has no `mod tests` today; SC-4 source-substring test lives in `container_config.rs::tests` per RESEARCH Open Question #2 to avoid spinning up a new test mod for one test.

## Pattern Assignments

### `src/session/container_config.rs` (pure-config builder, request-response)

#### Analog 1: `DockerError` thiserror enum shape

**Analog:** `src/containers/error.rs` (full file, 53 lines)

**Imports pattern** (line 1):
```rust
use thiserror::Error;
```

**Enum shape** (lines 3-51):
```rust
#[derive(Debug, Error)]
pub enum DockerError {
    #[error(
        "Docker is not installed or not in PATH.\n\
         Install Docker: https://docs.docker.com/get-docker/"
    )]
    NotInstalled,

    #[error("Container not found: {0}")]
    ContainerNotFound(String),

    #[error("Failed to create container: {0}")]
    CreateFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DockerError>;
```

**Apply to Phase 3:** `ContainerConfigError` mirrors this shape exactly: `#[derive(Debug, Error)] pub enum ContainerConfigError { #[error("...")] Variant { ... } }`. Difference: Phase 3's variant uses **struct-style fields** (named `entry`, `runtime`, `reason`) instead of `Tuple(String)`, because the variant carries three semantic fields that the error message interpolates by name. The `RegistryError` analog below shows the named-field shape; combine both.

#### Analog 2: `RegistryError` thiserror enum (named-field variants)

**Analog:** `src/session/projects.rs:18-33`

**Pattern** (lines 15-33):
```rust
/// Distinct failure modes for registry mutations. The web layer maps these to
/// HTTP status codes (Conflict → 409, NotFound → 404, Other → 500); CLI/TUI
/// callers convert via `Into<anyhow::Error>` and surface the message verbatim.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    NotFound(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

**Imports** (line 10):
```rust
use thiserror::Error;
```

**Apply to Phase 3:** Same `use thiserror::Error;` import. Same `#[derive(Debug, Error)]`. Same doc-comment style explaining who the variants serve. `ContainerConfigError::InconformableExtraVolume` uses named fields (struct-style) because the error message interpolates three identifiers. The `Into<anyhow::Error>` semantics RegistryError documents are exactly what Phase 3 needs: D-07 says `build_container_config` keeps `anyhow::Result` and the typed variant slots in via `.into()`.

#### Analog 3: `EnsureReadyError` (peer in instance.rs, third precedent)

**Analog:** `src/session/instance.rs:64-95`

**Pattern** (lines 64-95):
```rust
/// Errors `ensure_pane_ready` can return. Separating transient lifecycle
/// states from real tmux failures lets HTTP callers map them to 409 (retry)
/// vs 500 (real failure) instead of lumping everything as a tmux error.
#[derive(Debug)]
pub enum EnsureReadyError {
    /// Instance is mid-lifecycle (Creating/Deleting). Caller should retry.
    Transient(Status),
    /// Instance is cockpit-mode (no backing tmux pane); send is not supported.
    CockpitMode,
    /// Underlying tmux operation failed.
    Tmux(anyhow::Error),
}

impl std::fmt::Display for EnsureReadyError { /* hand-written */ }
impl std::error::Error for EnsureReadyError {}
```

**Apply to Phase 3:** This is the **anti-pattern reference**: `EnsureReadyError` hand-writes `Display` and `Error` instead of using `thiserror`. Phase 3 follows `DockerError` / `RegistryError` (thiserror), NOT `EnsureReadyError` (hand-rolled). Surface this excerpt so the planner does not mistakenly hand-roll the impls. Doc-comment-on-the-enum style is reusable, though.

#### Analog 4: Capability-gated branching inside a pure function

**Analog:** `RuntimeBase::build_create_args` — `src/containers/runtime_base.rs:223-317`

**Imports already present in scope** (file-level):
- `use crate::containers::{ContainerConfig, EnvEntry, VolumeMount};` (mirrored in `container_config.rs:11`)
- `use anyhow::Result;` (already in `container_config.rs:9`)

**Core gate pattern** (lines 238-274):
```rust
if self.capabilities.supports_arbitrary_volume_paths {
    for vol in &config.volumes {
        if !self.capabilities.supports_read_only_volumes && vol.read_only {
            tracing::warn!(
                "{} does not support read-only volumes, mounting {} read-write",
                self.name,
                vol.container_path
            );
        }
        let mount = if vol.read_only && self.capabilities.supports_read_only_volumes {
            format!("{}:{}:ro", vol.host_path, vol.container_path)
        } else {
            format!("{}:{}", vol.host_path, vol.container_path)
        };
        args.push("-v".to_string());
        args.push(mount);
    }
} else if !config.volumes.is_empty() {
    tracing::warn!(
        "{} does not support arbitrary volume paths; {} bind mount(s) skipped",
        self.name,
        config.volumes.len()
    );
}

if self.capabilities.supports_anonymous_volumes {
    for path in &config.anonymous_volumes {
        args.push("-v".to_string());
        args.push(path.clone());
    }
} else if !config.anonymous_volumes.is_empty() {
    tracing::warn!(
        "{} does not support anonymous volumes; {} path(s) skipped",
        self.name,
        config.anonymous_volumes.len()
    );
}
```

**Port-publish gate pattern** (lines 289-300):
```rust
if self.capabilities.supports_port_publish_at_create {
    for port in &config.port_mappings {
        args.push("-p".to_string());
        args.push(port.clone());
    }
} else if !config.port_mappings.is_empty() {
    tracing::warn!(
        "{} does not support port publish at create; {} mapping(s) skipped",
        self.name,
        config.port_mappings.len()
    );
}
```

**Apply to Phase 3:** `conform_for_capabilities` mirrors this gate shape at the upstream tier (config-build time, not argv-build time). The early-return-when-gate-is-true shape (`if caps.supports_arbitrary_volume_paths { return Ok(config); }`) makes the Docker matrix a true no-op, matching the SC-2 baseline-unchanged requirement. The `tracing::warn!` pattern when a feature is skipped is the same shape Phase 3 reuses for `tracing::debug!` on dropped mounts (debug, not warn, because aoe-internal drops are silent per D-04). The gate **MUST** be on `caps.supports_arbitrary_volume_paths`, never on `runtime_name == ContainerRuntimeName::Sbx` (per RESEARCH Anti-Patterns).

#### Analog 5: `tracing::debug!` drop-pass logging

**Analog:** `src/session/container_config.rs:865-870, 922-924, 1063-1066, 1085-1086`

**Pattern A — structured field per arg** (lines 865-870):
```rust
tracing::debug!(
    "Loaded sandbox config: extra_volumes={:?}, mount_ssh={}, volume_ignores={:?}",
    c.sandbox.extra_volumes,
    c.sandbox.mount_ssh,
    c.sandbox.volume_ignores
);
```

**Pattern B — single-line warn for malformed input** (lines 1085-1086):
```rust
} else {
    tracing::warn!("Ignoring malformed extra_volume entry: {}", entry);
}
```

**Pattern C — warn on non-existent file** (lines 922-925):
```rust
tracing::warn!(
    "GOOGLE_APPLICATION_CREDENTIALS points to non-existent file: {}",
    cred_path
);
```

**Apply to Phase 3:** The dropped-mount log uses **`debug!` not `warn!`** (D-04: silently with `tracing::debug!`). Recommend the structured-key form from RESEARCH (`target: "containers.config", runtime = %runtime_name, host = %v.host_path, ...`) over the positional `{}` form for greppability. Either is consistent with the file's existing style — both shapes already coexist.

#### Analog 6: `extra_volumes` `splitn(3, ':')` push site (DO NOT duplicate the warn)

**Analog:** `src/session/container_config.rs:1069-1087`

**Pattern** (lines 1067-1087):
```rust
let mut extra_volume_container_paths: std::collections::HashSet<String> =
    std::collections::HashSet::new();
for entry in &sandbox_config.extra_volumes {
    let parts: Vec<&str> = entry.splitn(3, ':').collect();
    if parts.len() >= 2 {
        tracing::info!(
            "Mounting extra volume: {} -> {} (ro: {})",
            parts[0],
            parts[1],
            parts.get(2) == Some(&"ro")
        );
        extra_volume_container_paths.insert(parts[1].to_string());
        volumes.push(VolumeMount {
            host_path: parts[0].to_string(),
            container_path: parts[1].to_string(),
            read_only: parts.get(2) == Some(&"ro"),
        });
    } else {
        tracing::warn!("Ignoring malformed extra_volume entry: {}", entry);
    }
}
```

**Apply to Phase 3:** The post-pass validator re-runs `entry.splitn(3, ':')` on `&sandbox_config.extra_volumes` to check `parts[0] != parts[1]`. **Reuse the same split shape, do not factor into a helper** (per RESEARCH "Don't Hand-Roll"; one parse site exists today, the post-pass adds one validate-only second pass on the same raw input). **Critical:** The post-pass MUST NOT re-emit a `warn!` for malformed entries (`parts.len() < 2`); the push loop already filtered+logged them (Pitfall #6 in RESEARCH). The post-pass branch is precisely `if parts.len() >= 2 && parts[0] != parts[1] { return Err(...); }`.

#### Analog 7: `build_container_config` signature + call site

**Analog:** `src/session/container_config.rs:820-846`

**Current signature** (lines 820-833):
```rust
/// Build a full `ContainerConfig` for creating a sandboxed container.
///
/// `profile` selects which profile's overrides (volumes, mount_ssh, volume_ignores)
/// are merged on top of the global config. An empty `profile` falls back to the
/// user's globally configured default profile.
pub(crate) fn build_container_config(
    project_path_str: &str,
    sandbox_info: &SandboxInfo,
    tool: &str,
    is_yolo_mode: bool,
    instance_id: &str,
    workspace_info: Option<&super::WorkspaceInfo>,
    profile: &str,
) -> Result<ContainerConfig> {
```

**Single call site** in `src/session/instance.rs:1269-1283`:
```rust
fn build_container_config(&self) -> Result<crate::containers::ContainerConfig> {
    let sandbox = self
        .sandbox_info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("sandbox_info missing for sandboxed session"))?;
    container_config::build_container_config(
        &self.project_path,
        sandbox,
        &self.tool,
        self.is_yolo_mode(),
        &self.id,
        self.workspace_info.as_ref(),
        &self.source_profile,
    )
}
```

**Apply to Phase 3:** Append **two** params per D-09 + the error-variant requirement: `capabilities: RuntimeCapabilities` AND `runtime_name: ContainerRuntimeName` (the error variant interpolates `runtime`, so the function needs both the matrix and the name; the matrix doesn't carry the name). The call site at instance.rs:1269 already has `runtime` constructed at line 1249 — reuse it: pass `runtime.capabilities()` and a name accessor. Note: `ContainerRuntime` trait may not expose `name()` directly; if not, planner reads it from `Config::load()`'s `container_runtime` field (already loaded for the session). Verify at plan time.

#### Analog 8: Representative integration-style test (build_container_config)

**Analog:** `src/session/container_config.rs:2299-2386` (`test_build_container_config_includes_repo_sandbox_settings`)

**Imports/setup pattern** (lines 2300-2334):
```rust
#[test]
#[serial_test::serial]
fn test_build_container_config_includes_repo_sandbox_settings() {
    // Isolate HOME so global/profile config doesn't interfere
    let temp_home = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_home.path());
    #[cfg(target_os = "linux")]
    std::env::set_var("XDG_CONFIG_HOME", temp_home.path().join(".config"));

    // Create a project directory with repo config
    let project_dir = TempDir::new().unwrap();
    let config_dir = project_dir.path().join(".agent-of-empires");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        r#"
[sandbox]
environment = ["MY_VAR=hello", "CI=true"]
volume_ignores = [".venv", "node_modules"]
extra_volumes = ["/host/data:/container/data:ro"]
"#,
    )
    .unwrap();

    // Initialize a git repo so compute_volume_paths works
    git2::Repository::init(project_dir.path()).unwrap();

    let sandbox_info = super::super::instance::SandboxInfo {
        enabled: true,
        container_id: None,
        image: "test:latest".to_string(),
        container_name: "test-container".to_string(),
        extra_env: None,
        custom_instruction: None,
    };
```

**Call + assertion** (lines 2335-2345):
```rust
    let project_path_str = project_dir.path().to_str().unwrap();
    let config = build_container_config(
        project_path_str,
        &sandbox_info,
        "claude",
        false,
        "test-instance-id",
        None,
        "",
    )
    .unwrap();
```

**Apply to Phase 3:** All 9 new tests follow this setup shape. Use `#[serial_test::serial]` (Pitfall #3) for any test that mutates HOME or calls `build_container_config`. Use `tempfile::TempDir` for fixtures (auto-cleanup). Construct `SandboxInfo` struct literal inline. Phase 3 tests pass the new `capabilities` + `runtime_name` params explicitly using `RuntimeBase::SBX.capabilities` / `RuntimeBase::DOCKER.capabilities` const literals (RESEARCH "Don't Hand-Roll"). For path comparisons, canonicalize via `path.canonicalize().unwrap()` before assertion (Pitfall #4, mirrors line 1259-1260).

#### Analog 9: `compute_volume_paths` unit test (smaller test shape)

**Analog:** `src/session/container_config.rs:1201-1222` (`test_compute_volume_paths_regular_repo`)

**Pattern**:
```rust
#[test]
fn test_compute_volume_paths_regular_repo() {
    let (_dir, repo_path) = setup_regular_repo();
    let project_path_str = repo_path.to_str().unwrap();

    let (volumes, working_dir) = compute_volume_paths(&repo_path, project_path_str).unwrap();

    assert_eq!(volumes.len(), 1);
    assert_eq!(
        volumes[0].host_path,
        repo_path.to_string_lossy().to_string()
    );
    assert_eq!(volumes[0].container_path, working_dir);
    let dir_name = repo_path.file_name().unwrap().to_string_lossy();
    assert_eq!(
        volumes[0].container_path,
        format!("/workspace/{}", dir_name)
    );
}
```

**Apply to Phase 3:** The `conform_workspace_paths` direct tests (RESEARCH validation map: `test_conform_workspace_paths_sbx_rewrites_workdir_under_volume`, `test_conform_workspace_paths_docker_is_identity`) follow this no-HOME-mutation shape because the helper is pure (takes `Vec<VolumeMount>`, returns `Vec<VolumeMount>`). No `#[serial]` needed for those two; no TempDir fixture needed; just construct `VolumeMount` literals inline. Mirror `setup_regular_repo()` for `compute_volume_paths` consumers (the SC-1 cases).

---

### `src/session/instance.rs` (orchestrator, request-response)

#### Analog 10: `ensure_image` defensive capability check (the inverse pattern)

**Analog:** `src/containers/runtime_base.rs:195-210`

**Pattern**:
```rust
pub fn ensure_image(&self, image: &str) -> Result<()> {
    if self.image_exists_locally(image) {
        tracing::info!("Using local {} image '{}'", self.name, image);
        return Ok(());
    }

    if !self.capabilities.supports_image_pull {
        return Err(DockerError::CommandFailed(format!(
            "{} does not support image pull; ensure the image '{}' is available through other means",
            self.name, image
        )));
    }

    tracing::info!("Pulling {} image '{}'", self.name, image);
    self.pull_image(image)
}
```

**Apply to Phase 3:** **Do NOT touch this function** (D-13). The defensive check stays as a safety net. Phase 3 adds the **preventive** check at the call site (`instance.rs:1249`) so the function isn't entered for sbx in the normal flow. The pattern to copy is the capability boolean check itself — `if !caps.supports_image_pull` becomes `if caps.supports_image_pull` (inverted, because the call-site guard runs the call when capable; the impl guard rejects when not capable).

#### Analog 11: Current `ensure_image` call site (the two-line patch target)

**Analog:** `src/session/instance.rs:1248-1252`

**Current code** (lines 1248-1252):
```rust
// Ensure image is available (always pulls to get latest)
let runtime = containers::get_container_runtime();
runtime.ensure_image(image)?;

let config = self.build_container_config()?;
```

**Apply to Phase 3:** Replace with the D-12 guard verbatim:
```rust
// Ensure image is available (always pulls to get latest, when supported)
let runtime = containers::get_container_runtime();
if runtime.capabilities().supports_image_pull {
    runtime.ensure_image(image)?;
}

let config = self.build_container_config()?;  // (Phase 3 also threads caps into this wrapper)
```

The doc-comment update ("when supported") is part of the planned diff so future readers don't think `ensure_image` always fires.

#### Analog 12: Current `container_workdir` shape (the threading target)

**Analog:** `src/session/instance.rs:1262-1267`

**Current code**:
```rust
/// Get the container working directory for this instance.
pub fn container_workdir(&self) -> String {
    container_config::compute_volume_paths(Path::new(&self.project_path), &self.project_path)
        .map(|(_, wd)| wd)
        .unwrap_or_else(|_| "/workspace".to_string())
}
```

**Apply to Phase 3:** Per D-10, gain a `capabilities: RuntimeCapabilities` arg. New shape:
```rust
pub fn container_workdir(&self, caps: RuntimeCapabilities) -> String {
    let (volumes, working_dir) =
        container_config::compute_volume_paths(Path::new(&self.project_path), &self.project_path)
            .unwrap_or_else(|_| (vec![], "/workspace".to_string()));
    let (_, conformed_wd) = container_config::conform_workspace_paths(volumes, working_dir, caps);
    conformed_wd
}
```

**Optional helper** (RESEARCH Integration Points recommendation): Add `container_workdir_now(&self) -> String` that fetches `caps` via the global lookup so the 19 call sites only append `_now`. Planner discretion; cuts churn.

---

## Shared Patterns

### Pattern A: `use thiserror::Error;` at module head

**Source:** `src/session/projects.rs:10`, `src/containers/error.rs:1`
**Apply to:** Whichever file hosts `ContainerConfigError` (inline at top of `container_config.rs` per D-06 default, or `container_config/error.rs` if planner picks peer-module).

```rust
use thiserror::Error;
```

### Pattern B: Doc-comment explaining who consumes the variants

**Source:** `src/session/projects.rs:15-17`
```rust
/// Distinct failure modes for registry mutations. The web layer maps these to
/// HTTP status codes (Conflict → 409, NotFound → 404, Other → 500); CLI/TUI
/// callers convert via `Into<anyhow::Error>` and surface the message verbatim.
```

**Apply to Phase 3:** `ContainerConfigError` gets a similar prefatory doc comment naming the consumer:
```rust
/// Validation failures `build_container_config` surfaces to callers when the
/// resolved runtime's capability matrix cannot honor the requested mount shape.
/// CLI/TUI callers receive these via `anyhow::Error::downcast_ref` (build_container_config
/// keeps its anyhow::Result<_> signature; typed variants slot in via `.into()`).
```

### Pattern C: Capability access via trait method

**Source:** `src/session/instance.rs:1249` (`runtime.ensure_image(image)?;` uses `runtime: Box<dyn ContainerRuntime>` from `containers::get_container_runtime()`)
**Trait method:** `src/containers/container_interface.rs:97` (`fn capabilities(&self) -> RuntimeCapabilities;`)

**Apply to:** Every Phase 3 call site that needs `caps`:
```rust
let runtime = containers::get_container_runtime();
let caps = runtime.capabilities();  // Copy, cheap to pass by value
```

`RuntimeCapabilities` is `Copy + Debug + PartialEq + Eq` (`container_interface.rs:59`); pass by value, not reference.

### Pattern D: Const-literal capability matrices for tests

**Source:** `src/containers/runtime_base.rs:35-44` (DOCKER), `:48-67` (APPLE_CONTAINER), `:70-90` (PODMAN), `:99-119` (SBX)

**Apply to:** Every Phase 3 test that needs caps:
```rust
let caps_sbx = crate::containers::runtime_base::RuntimeBase::SBX.capabilities;
let caps_docker = crate::containers::runtime_base::RuntimeBase::DOCKER.capabilities;
```

No synthesis helpers, no mocks. Phase 1 locked these values.

### Pattern E: `#[serial_test::serial]` for HOME-mutating tests

**Source:** `src/session/container_config.rs:2300, 2394, 2542, 2657, 2870` (every `build_container_config` test in the file)

```rust
#[test]
#[serial_test::serial]
fn test_build_container_config_... () { ... }
```

**Apply to:** Every Phase 3 test that calls `build_container_config` directly (SC-1a..d, SC-2, SC-3 + companions). The two pure-helper tests for `conform_workspace_paths` do NOT need `#[serial]` (no env mutation).

### Pattern F: TempDir fixture + canonicalize comparison

**Source:** `src/session/container_config.rs:1259-1260, 1377, 1499`

```rust
let mount_path_canon = Path::new(&volumes[0].host_path).canonicalize().unwrap();
let main_repo_canon = main_repo_path.canonicalize().unwrap();
assert_eq!(mount_path_canon, main_repo_canon, ...);
```

**Apply to:** Every Phase 3 path-comparison assertion. On macOS, `/tmp` symlinks to `/private/tmp`; bare path equality fails. Canonicalize both sides before `assert_eq!`. RESEARCH Pitfall #4 covers this in detail.

---

## No Analog Found

| File / Surface | Reason |
|----------------|--------|
| (none) | All Phase 3 surfaces have strong in-tree analogs. The closest "no analog" surface is the `container_workdir_now(&self)` helper RESEARCH proposes — but it's optional planner discretion, not a required new surface, and its shape is a trivial 3-line wrapper over the existing `container_workdir(caps)` + the `Pattern C` capability-access. No precedent needed. |

---

## Metadata

**Analog search scope:**
- `src/session/container_config.rs` (full file ~3000 lines; read targeted ranges 1-50, 820-1100, 1156-1275, 2300-2600)
- `src/session/instance.rs` (lines 60-115, 1240-1330)
- `src/containers/error.rs` (full, 53 lines)
- `src/containers/runtime_base.rs` (lines 30-220, 220-330)
- `src/containers/container_interface.rs` (lines 1-100)
- `src/session/projects.rs` (lines 1-50)
- `src/session/config.rs` (lines 720-745)

**Files scanned:** 7
**Pattern extraction date:** 2026-05-15

**Key design observations:**
1. The thiserror pattern in the codebase splits between **own-file** placement (`DockerError` in `src/containers/error.rs`) and **inline-at-top** placement (`RegistryError` in `src/session/projects.rs:18-33`). Both work; CONTEXT D-06 picks inline as default. No deviation from precedent either way.
2. `RuntimeBase::build_create_args` is the exact downstream counterpart to Phase 3's `conform_for_capabilities`: build_create_args gates argv emission, conform_for_capabilities gates `ContainerConfig` content. Same capability flags drive both. Phase 3 is the upstream half of a symmetric design.
3. There is **no precedent** for hand-rolled `Display` + `Error` on a new typed enum in the session tier — `EnsureReadyError` is the only example, and it predates the codebase's thiserror adoption. Phase 3 follows thiserror.
4. The `extra_volumes` `splitn(3, ':')` parse appears exactly once in the codebase (`container_config.rs:1070`). The post-pass adds a second iteration over the same raw input; this is intentional (validate-only, no push). Do not factor into a helper — one consumer pre-Phase-3, two post-Phase-3, still below the threshold where extraction helps.
5. Test patterns in `container_config.rs::tests` are remarkably consistent (HOME mutation + serial + TempDir + canonicalize). Phase 3's 9 new tests inherit this shape verbatim; zero new test infrastructure required.
