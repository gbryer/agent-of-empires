# Phase 2: SbxRuntime Skeleton and Pure Argv Builders - Pattern Map

**Mapped:** 2026-05-15
**Files analyzed:** 4 (2 new, 2 modified)
**Analogs found:** 4 / 4 (all in-tree, all in `src/containers/`)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/containers/sbx/mod.rs` | runtime-peer-struct (impl `ContainerRuntimeInterface`-aligned shape, but only `is_available` + `capabilities` in Phase 2) | request-response (binary probe), pure-data (capabilities) | `src/containers/runtime_base.rs` (`RuntimeBase` struct + `is_available` + `SBX` const) | role-match (peer struct vs shared const, but the probe + cap-accessor shape is identical) |
| `src/containers/sbx/argv.rs` | utility (pure argv composer) | transform (config -> `Vec<String>`) | `src/containers/runtime_base.rs::build_create_args` (lines 223-317) | exact (pure `Vec<String>` builder, same input shape, same env encoding pattern) |
| `src/containers/mod.rs` | config / module wiring | n/a (one-line module declaration) | `src/containers/mod.rs:1-4` (existing `pub mod container_interface;` / `mod runtime;` lines) | exact (literally the same pattern, one new line) |
| `src/containers/runtime.rs` | controller / facade dispatch | request-response (match-on-kind dispatch) | self — modifying three existing `RuntimeKind::Sbx` arms; struct-field addition mirrors existing `base: RuntimeBase` field pattern | exact (extending the same file) |

## Pattern Assignments

### `src/containers/sbx/mod.rs` (runtime-peer-struct, request-response + pure-data)

**Analog:** `src/containers/runtime_base.rs` (struct shape, `is_available` probe, capability const)

**Imports pattern** (mirrors `runtime_base.rs:1-3`):
```rust
// src/containers/runtime_base.rs:1-3
use super::container_interface::{ContainerConfig, EnvEntry, RuntimeCapabilities};
use super::error::{DockerError, Result};
use std::process::Command;
```

Sbx variant adds `PathBuf` for the injection seam (D-10); Phase 2 does not need `ContainerConfig` / `EnvEntry` / `DockerError` / `Result` in `mod.rs` because action verbs are deferred:
```rust
// New src/containers/sbx/mod.rs imports
use std::path::PathBuf;
use std::process::Command;

use crate::containers::container_interface::RuntimeCapabilities;
use crate::containers::runtime_base::RuntimeBase;
```

**Struct shape pattern** (mirrors `runtime_base.rs:11-26`):
```rust
// src/containers/runtime_base.rs:11-26 (analog)
pub(crate) struct RuntimeBase {
    pub binary: &'static str,
    pub name: &'static str,
    pub daemon_check_args: &'static [&'static str],
    pub pull_prefix: &'static [&'static str],
    pub remove_subcommand: &'static str,
    pub capabilities: RuntimeCapabilities,
}
```

Sbx peer struct deviates only on the binary field — `PathBuf` (not `&'static str`) so the test can inject a tempfile path per D-10:
```rust
// New SbxRuntime shape (Phase 2)
pub(crate) struct SbxRuntime {
    pub(crate) binary: PathBuf,
}
```

**`is_available` probe pattern** (copy from `runtime_base.rs:121-131`, swap `self.binary: &'static str` for `&self.binary: &PathBuf` — both are accepted by `Command::new<S: AsRef<OsStr>>`):
```rust
// src/containers/runtime_base.rs:121-131
pub fn command(&self) -> Command {
    Command::new(self.binary)
}

pub fn is_available(&self) -> bool {
    self.command()
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

Sbx variant inlines the `Command::new` (no helper needed; one call site):
```rust
// SbxRuntime::is_available (Phase 2)
pub fn is_available(&self) -> bool {
    Command::new(&self.binary)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

**Capability accessor pattern** (D-12 planner discretion; research recommends routing through the existing const, see Pitfall 6 in RESEARCH.md):
```rust
// Recommended: route through RuntimeBase::SBX.capabilities (already declared
// at runtime_base.rs:99-119) — single source of truth, zero drift risk.
pub fn capabilities(&self) -> RuntimeCapabilities {
    RuntimeBase::SBX.capabilities
}
```

Alternative (re-declare the literal) is acceptable per D-12 but adds drift risk.

**In-module test pattern** (mirrors every `#[cfg(test)] mod tests` block in `src/containers/*.rs`; for `tempfile` usage in tests, precedent is `src/migrations/v003_yolo_mode_config.rs:104, 141, 167` and `src/migrations/v004_unified_environment.rs:118, 154`):
```rust
// Pattern: in-module #[cfg(test)] block per AGENTS.md "Use unit tests
// in-module (#[cfg(test)]) for pure logic."
//
// tempfile precedent (top-level dep at Cargo.toml:104):
//   src/migrations/v003_yolo_mode_config.rs:104:
//     let dir = tempfile::TempDir::new().unwrap();
//
// Use NamedTempFile for the fake-binary recipe (CONTEXT.md <specifics>):
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
        drop(tmp); // keep tempfile alive past the assertion
    }
}
```

---

### `src/containers/sbx/argv.rs` (utility, pure transform)

**Analog:** `src/containers/runtime_base.rs::build_create_args` (lines 223-317) — pure `Vec<String>` builder, same input shape (`name`, `image`, `&ContainerConfig`), returns identically-typed result.

**Imports pattern** — bring in `ContainerConfig` and `EnvEntry` from the existing data-shape module:
```rust
// src/containers/sbx/argv.rs imports
use std::path::Path;

use crate::containers::container_interface::{ContainerConfig, EnvEntry};
```

**Core pure-builder pattern** (copy structural shape from `runtime_base.rs:223-317`; differ on output verbs/flags only):
```rust
// src/containers/runtime_base.rs:223-237 (analog skeleton)
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
        "-w".to_string(),
        config.working_dir.clone(),
    ];
    // ... capability-gated -v/-e/-p emission ...
    args.push(image.to_string());
    args.push("sleep".to_string());
    args.push("infinity".to_string());
    args
}
```

Sbx variant is a free function (D-07, no `&self`); unconditional emission (sbx caps are constants, no in-builder gating per RESEARCH.md "Anti-Patterns"):
```rust
// Phase 2 sbx::argv::build_create_args
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

    // sbx positional mounts: PATH[:ro] (single path; host_path == container_path
    // is Phase 3's invariant; Phase 2 emits host_path verbatim).
    for vol in &config.volumes {
        let suffix = if vol.read_only { ":ro" } else { "" };
        args.push(format!("{}{}", vol.host_path, suffix));
    }

    args
}
```

**EnvEntry encoding pattern** (COPY VERBATIM from `runtime_base.rs:276-287`; RESEARCH.md "Anti-Patterns" forbids extracting a shared helper):
```rust
// src/containers/runtime_base.rs:276-287 (CANONICAL EnvEntry encoding)
for entry in &config.environment {
    args.push("-e".to_string());
    match entry {
        EnvEntry::Inherit { key, .. } => {
            // Only the key in argv; value stays in process env
            args.push(key.clone());
        }
        EnvEntry::Literal { key, value } => {
            args.push(format!("{}={}", key, value));
        }
    }
}
```

Sbx `build_exec_args` uses the identical pattern (D-07 signature):
```rust
// Phase 2 sbx::argv::build_exec_args
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

    // Mirrors runtime_base.rs:276-287 exactly.
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

**Single-port builder pattern** (per-port loop is Phase 5's `ports.rs`):
```rust
// Phase 2 sbx::argv::build_ports_args
pub fn build_ports_args(name: &str, port_spec: &str) -> Vec<String> {
    vec![
        "ports".to_string(),
        name.to_string(),
        "--publish".to_string(),
        port_spec.to_string(),
    ]
}
```

**In-module test pattern** (mirrors `runtime.rs::tests` at lines 315-552). Reference shapes pulled from RESEARCH.md Example 3, which already enumerates ≥15 argv-shape tests covering the SC-1 minimum:
- 6 `build_create_args` tests (minimal / kit / volume no-ro / volume ro / multiple volumes / cpus+mem / anonymous-volumes-skipped / ports-not-emitted)
- 10 `build_exec_args` tests (covering the 16 combinations of {interactive,tty}×{workdir}×{env})
- 3 `build_ports_args` tests (simple / host-ip / ephemeral)

---

### `src/containers/mod.rs` (config / module wiring, n/a flow)

**Analog:** self — `src/containers/mod.rs:1-4` already declares three modules.

**Existing pattern** (lines 1-4):
```rust
pub mod container_interface;
pub mod error;
mod runtime;
pub(crate) mod runtime_base;
```

**Phase 2 change** — one new line, slotted alphabetically with the other private modules. `pub(crate)` matches `runtime_base`'s visibility (the SbxRuntime peer struct is reached via `ContainerRuntime`, never directly by callers outside `containers`):
```rust
pub mod container_interface;
pub mod error;
mod runtime;
pub(crate) mod runtime_base;
pub(crate) mod sbx;  // NEW
```

No other change in this file. The existing `get_container_runtime()` factory at `mod.rs:30-41` already pairs `ContainerRuntimeName::Sbx` with `ContainerRuntime::sbx()` — D-01 guarantees zero churn here.

---

### `src/containers/runtime.rs` (controller / facade dispatch, request-response)

**Analog:** self — extending three existing `RuntimeKind::Sbx` dispatch arms and (per D-03 / RESEARCH.md Option A recommendation) adding one field to `ContainerRuntime`.

**Storage shape pattern** (D-03 — RESEARCH.md recommends Option A "optional field" for smallest diff):
```rust
// Existing ContainerRuntime struct at runtime.rs:22-25:
pub struct ContainerRuntime {
    pub(crate) base: RuntimeBase,
    pub(crate) kind: RuntimeKind,
}

// Phase 2 (Option A — RESEARCH.md recommended; D-03 leaves planner discretion):
pub struct ContainerRuntime {
    pub(crate) base: RuntimeBase,
    pub(crate) kind: RuntimeKind,
    pub(crate) sbx: Option<sbx::SbxRuntime>,  // Some only when kind == Sbx
}
```

**Constructor pattern** (mirrors `runtime.rs:53-58` `ContainerRuntime::sbx()`; existing constructors at `runtime.rs:28-47` for docker / apple_container / podman gain `sbx: None`):
```rust
// runtime.rs:53-58 (existing — Phase 1 stub constructor)
pub fn sbx() -> Self {
    Self {
        base: RuntimeBase::SBX,
        kind: RuntimeKind::Sbx,
    }
}

// Phase 2 — extend to construct and hold the peer struct
pub fn sbx() -> Self {
    Self {
        base: RuntimeBase::SBX,
        kind: RuntimeKind::Sbx,
        sbx: Some(sbx::SbxRuntime::new()),
    }
}

// Other constructors (docker, apple_container, podman) gain `sbx: None`.
```

**Dispatch arm 1 — `is_available`** (replace the short-circuit at `runtime.rs:74-79`):

Before (Phase 1, current code at `runtime.rs:74-79`):
```rust
fn is_available(&self) -> bool {
    match self.kind {
        RuntimeKind::Sbx => false,
        _ => self.base.is_available(),
    }
}
```

After (Phase 2):
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

**Dispatch arm 2 — `exec_command`** (replace the sentinel-string body at `runtime.rs:240-254`):

Before (Phase 1, current code):
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

After (Phase 2 — adapter at dispatch site, builder stays pure; exact adapter is planner discretion per RESEARCH.md Open Question 4):
```rust
RuntimeKind::Sbx => {
    // Adapter: parse the Docker-style `options: Option<&str>` passthrough
    // (e.g. "-w /workspace") into discrete fields for the sbx builder.
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

**Dispatch arm 3 — `build_create_args`** (gate `RuntimeKind::Sbx` away from the unconditional `self.base.build_create_args` fall-through at `runtime.rs:181-183`):

Before (Phase 1, current code at `runtime.rs:181-183`):
```rust
fn build_create_args(&self, name: &str, image: &str, config: &ContainerConfig) -> Vec<String> {
    self.base.build_create_args(name, image, config)
}
```

After (Phase 2):
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

**Test strengthening pattern** (modify the existing `test_sbx_dispatch_arms_return_safe_stubs` test at `runtime.rs:534-542`; add a new test for `build_create_args`).

Current (loose, from `runtime.rs:534-542`):
```rust
#[test]
fn test_sbx_dispatch_arms_return_safe_stubs() {
    let rt = ContainerRuntime::sbx();
    assert!(!rt.does_container_exist("foo").unwrap());
    assert!(!rt.is_container_running("foo").unwrap());
    assert!(rt.batch_running_states("aoe-sandbox-").is_empty());
    let cmd = rt.exec_command("foo", None, "bar");
    assert!(cmd.contains("sbx"));
    assert!(cmd.contains("foo"));
}
```

Phase 2 strengthening — assert the new builder shape (`exec` verb instead of the Phase 1 sentinel-comment marker), and add a fresh test that `build_create_args` emits the sbx shape (`create shell --name ...`) and **not** the Docker shape (`run -d ...`) that `RuntimeBase::build_create_args` would produce. RESEARCH.md "Wave 0 Gaps" lists this strengthening as required.

```rust
#[test]
fn test_sbx_dispatch_arms_return_safe_stubs() {
    let rt = ContainerRuntime::sbx();
    assert!(!rt.does_container_exist("foo").unwrap());
    assert!(!rt.is_container_running("foo").unwrap());
    assert!(rt.batch_running_states("aoe-sandbox-").is_empty());
    let cmd = rt.exec_command("foo", None, "bar");
    assert!(cmd.contains("exec"));      // NEW: proves new builder, not Phase 1 sentinel
    assert!(cmd.contains("foo"));       // existing
    assert!(cmd.contains("bar"));       // NEW: proves cmd is no longer dropped
    assert!(!cmd.contains("Phase 1"));  // NEW: proves sentinel comment gone
}

#[test]
fn test_sbx_build_create_args_emits_sbx_shape_not_docker_shape() {
    let rt = ContainerRuntime::sbx();
    let cfg = ContainerConfig {
        working_dir: "/workspace".to_string(),
        volumes: vec![],
        anonymous_volumes: vec![],
        environment: vec![],
        cpu_limit: None,
        memory_limit: None,
        port_mappings: vec![],
    };
    let args = rt.build_create_args("aoe-sandbox-test", "alpine:latest", &cfg);
    // Sbx shape leads with "create shell --name", not Docker's "run -d --name"
    assert_eq!(args[0], "create");
    assert_eq!(args[1], "shell");
    assert!(!args.contains(&"run".to_string()));
    assert!(!args.contains(&"-d".to_string()));
}
```

**Test-skip-pattern note** — argv-builder tests need NO skip guard (they're pure, no `sbx` binary required). The `docker_if_available()` / `podman_if_available()` / `apple_container_if_available()` helpers at `runtime.rs:319-344` are for tests that hit real subprocesses; Phase 2's sbx unit tests do not need an equivalent helper.

---

## Shared Patterns

### Pattern A — Pure `Vec<String>` argv builders (no shell, no subprocess)

**Source:** `src/containers/runtime_base.rs::build_create_args` (lines 223-317)
**Apply to:** All three free functions in `src/containers/sbx/argv.rs`

Concrete idiom (the entire body is just `Vec::push` calls; no shell interpolation, no `String::format` of multi-token shell strings):
```rust
let mut args: Vec<String> = vec![
    "subcommand".to_string(),
    "-flag".to_string(),
    value.to_string(),
];
if let Some(opt) = &maybe_opt {
    args.push("--opt".to_string());
    args.push(opt.clone());
}
args
```

Why this works as a Phase-2 invariant: Phase 5 will pass these `Vec<String>` results to `Command::args(&args)`, which `execve`s each element as a discrete argv slot — no shell, no quoting, no injection surface (RESEARCH.md "Security Domain" pattern table).

### Pattern B — `EnvEntry` encoding (verbatim copy, no shared helper)

**Source:** `src/containers/runtime_base.rs:276-287` (the `for entry in &config.environment { ... match ... }` block)
**Apply to:** `src/containers/sbx/argv.rs::build_exec_args` only

`EnvEntry::Inherit` emits ONLY the key in argv (value travels via process env to keep secrets out of `ps`); `EnvEntry::Literal` emits `KEY=VALUE` directly. Phase 2 copies these ~10 lines verbatim into `build_exec_args`. RESEARCH.md "Anti-Patterns" explicitly forbids extracting a shared helper across Docker's `-v` argv and sbx's `exec` argv at this stage.

### Pattern C — Binary `--version` probe (Command + status check)

**Source:** `src/containers/runtime_base.rs:125-131`
**Apply to:** `src/containers/sbx/mod.rs::SbxRuntime::is_available`

The exact 5-line `Command::new(...).arg("--version").output().map(...).unwrap_or(false)` shape. Phase 2 swaps the source of the binary path (`&'static str` const -> `&PathBuf` field) but keeps every other token identical.

### Pattern D — Capability matrix declared at the backend's home file

**Source:** `src/containers/runtime_base.rs:99-119` (`RuntimeBase::SBX` const)
**Apply to:** `src/containers/sbx/mod.rs::SbxRuntime::capabilities` (D-12 — research recommends routing through `RuntimeBase::SBX.capabilities` rather than re-declaring; see RESEARCH.md Pitfall 6)

The const literal `RuntimeBase::SBX.capabilities` is already locked from Phase 1. SbxRuntime's `capabilities()` is a one-liner that returns it.

### Pattern E — In-module `#[cfg(test)] mod tests` for pure logic

**Source:** every file in `src/containers/*.rs` (e.g. `runtime.rs:315-552`); precedent for `tempfile` use is `src/migrations/v003_yolo_mode_config.rs:104` and `v004_unified_environment.rs:118`
**Apply to:** both new files (`sbx/mod.rs` and `sbx/argv.rs`)

AGENTS.md "Testing Guidelines" mandates this. No separate `tests/sbx_argv.rs` file; everything in-module.

### Pattern F — `match self.kind` dispatch with `_` fall-through for the CLI runtimes

**Source:** `src/containers/runtime.rs` — every existing arm that has a sbx specialization (e.g. `is_available` at lines 74-79, `does_container_exist` at lines 113-137, `exec_command` at lines 209-256)
**Apply to:** the three Phase 2-updated arms (`is_available`, `exec_command`, `build_create_args`)

The fall-through preserves `RuntimeBase` semantics for Docker / Podman / AppleContainer (D-04 minimum-surface guarantee, RESEARCH.md SC-4).

---

## No Analog Found

None — every Phase 2 file maps to an existing analog in `src/containers/`. The `SbxRuntime` peer struct is novel in that it does not extend `RuntimeBase`, but its individual methods (`is_available`, `capabilities`, and the future `argv` free functions) each map to existing patterns in `runtime_base.rs`.

---

## Metadata

**Analog search scope:**
- `src/containers/` (full tree: `mod.rs`, `runtime.rs`, `runtime_base.rs`, `container_interface.rs`, `error.rs`)
- `src/migrations/` (precedent for `tempfile` usage in `#[cfg(test)]` blocks)

**Files scanned:** 5 source files; targeted Grep over migrations directory for `tempfile`/`NamedTempFile`/`PermissionsExt` usage

**Key citations used in pattern assignments:**
- `src/containers/runtime_base.rs:1-3` (imports)
- `src/containers/runtime_base.rs:11-26` (struct shape)
- `src/containers/runtime_base.rs:99-119` (sbx capability const)
- `src/containers/runtime_base.rs:121-131` (Command + --version probe)
- `src/containers/runtime_base.rs:223-317` (build_create_args pure-builder pattern)
- `src/containers/runtime_base.rs:276-287` (EnvEntry encoding — verbatim-copy target)
- `src/containers/runtime.rs:14-25` (RuntimeKind + ContainerRuntime struct)
- `src/containers/runtime.rs:28-58` (constructors)
- `src/containers/runtime.rs:74-79` (current `is_available` arm to swap)
- `src/containers/runtime.rs:181-183` (current `build_create_args` fall-through to swap)
- `src/containers/runtime.rs:209-256` (current `exec_command` dispatch + Phase 1 sbx sentinel to swap)
- `src/containers/runtime.rs:534-542` (test to strengthen)
- `src/containers/mod.rs:1-4` (module declaration pattern)
- `src/containers/container_interface.rs:5-50` (`VolumeMount` / `EnvEntry` / `ContainerConfig` shapes — argv builder inputs)
- `src/migrations/v003_yolo_mode_config.rs:104` (`tempfile::TempDir::new()` precedent in `#[cfg(test)]`)

**Pattern extraction date:** 2026-05-15
