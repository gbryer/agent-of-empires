# Phase 5: Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish) - Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 7 new/modified files
**Analogs found:** 7 / 7

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/containers/sbx/parse.rs` | model | transform | `src/containers/sbx/kit.rs` (serde structs) + `src/containers/container_interface.rs` (struct style) | role-match |
| `src/containers/sbx/ports.rs` | service | request-response | `src/containers/runtime_base.rs` lines 381-405 (`remove` method) | exact |
| `src/containers/sbx/mod.rs` (action verbs) | service | request-response | `src/containers/runtime_base.rs` lines 319-422 (run_create, start, stop, remove, exec) | exact |
| `src/containers/runtime.rs` (dispatch arm replacement) | controller | request-response | `src/containers/runtime.rs` lines 96-103 (existing Sbx is_available delegation) | exact |
| `src/session/instance.rs` (publish_ports call site) | controller | request-response | `src/session/instance.rs` lines 1248-1265 (existing create + capability gate) | exact |
| `tests/sbx_integration.rs` | test | CRUD | `tests/sandbox_integration.rs` | exact |
| `tests/fixtures/sbx_ls.json` | config | transform | `tests/fixtures/sbx_kits/ok/` | role-match |

## Pattern Assignments

### `src/containers/sbx/parse.rs` (model, transform)

**Analog:** `src/containers/container_interface.rs` (struct declaration style) + `src/containers/sbx/kit.rs` (serde usage)

**Imports pattern** (container_interface.rs lines 1-4, kit.rs lines 1-15):
```rust
use serde::Deserialize;
```

**Core pattern: serde struct with `#[serde(default)]`** (derived from project convention in RESEARCH.md):
```rust
#[derive(Debug, Deserialize)]
pub(crate) struct SbxListOutput {
    #[serde(default)]
    pub sandboxes: Vec<SbxSandboxInfo>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SbxSandboxInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub socket_path: String,
    #[serde(default)]
    pub workspaces: Vec<String>,
}
```

**Testing pattern** (kit.rs lines 222-240, in-module `#[cfg(test)]`):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_output_shape() {
        let json = include_str!("../../../tests/fixtures/sbx_ls.json");
        let output: SbxListOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.sandboxes.len(), 1);
        assert_eq!(output.sandboxes[0].name, "claude-Lymow-HA");
    }
}
```

---

### `src/containers/sbx/ports.rs` (service, request-response)

**Analog:** `src/containers/runtime_base.rs` (remove method, lines 381-405)

**Imports pattern** (mod.rs lines 1-5 + runtime_base.rs pattern):
```rust
use super::argv;
use super::SbxRuntime;
```

**Core pattern: enum result + per-item subprocess loop** (runtime_base.rs remove pattern adapted):
```rust
#[derive(Debug, Clone)]
pub enum PortPublishResult {
    Ok(String),
    Failed { port: String, stderr: String },
}

impl SbxRuntime {
    pub fn publish_ports(&self, name: &str, port_mappings: &[String]) -> Vec<PortPublishResult> {
        port_mappings.iter().map(|spec| {
            let args = argv::build_ports_args(name, spec);
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match self.run(&arg_refs) {
                Ok(_) => PortPublishResult::Ok(spec.clone()),
                Err(e) => PortPublishResult::Failed {
                    port: spec.clone(),
                    stderr: e.to_string(),
                },
            }
        }).collect()
    }
}
```

**Testing pattern** (mod.rs lines 46-91, mock binary injection):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn publish_ports_captures_failure_stderr() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fake-sbx");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"#!/bin/sh\necho 'port already in use' >&2\nexit 1\n").unwrap();
        }
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();

        let sbx = SbxRuntime { binary: path };
        let results = sbx.publish_ports("test-sandbox", &["3000:3000".to_string()]);
        assert!(matches!(&results[0], PortPublishResult::Failed { .. }));
    }
}
```

---

### `src/containers/sbx/mod.rs` - Action Verbs (service, request-response)

**Analog:** `src/containers/runtime_base.rs` lines 319-422 (run_create, start_container, stop_container, remove, exec)

**Imports pattern** (add to existing mod.rs lines 1-5):
```rust
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

use crate::containers::container_interface::{ContainerConfig, RuntimeCapabilities};
use crate::containers::error::{DockerError, Result};
use crate::containers::runtime_base::RuntimeBase;
```

**Core pattern: private `run` helper** (adapted from runtime_base.rs lines 319-352 `run_create`):
```rust
impl SbxRuntime {
    fn run(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new(&self.binary).args(args).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DockerError::CommandFailed(format!(
                "sbx {} failed: {}", args.first().unwrap_or(&""), stderr.trim()
            )));
        }
        Ok(output)
    }
}
```

**Lifecycle pattern: stop_container** (runtime_base.rs lines 366-379):
```rust
pub fn stop_container(&self, name: &str) -> Result<()> {
    tracing::info!(target: "containers.sbx", %name, "stopping sandbox");
    self.run(&["stop", name])?;
    Ok(())
}
```

**Lifecycle pattern: remove** (runtime_base.rs lines 381-405):
```rust
pub fn remove(&self, name: &str, force: bool) -> Result<()> {
    let mut args = vec!["rm"];
    if force {
        args.push("-f");
    }
    args.push(name);
    tracing::debug!(target: "containers.sbx", %name, %force, "removing sandbox");
    self.run(&args)?;
    Ok(())
}
```

**Lifecycle pattern: exec** (runtime_base.rs lines 415-422):
```rust
pub fn exec(&self, name: &str, cmd: &[&str]) -> Result<Output> {
    let args = argv::build_exec_args(name, None, &[], false, false, cmd);
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    // First-exec retry logic wraps this call
    self.run(&arg_refs)
}
```

**Readiness probe pattern** (synchronous sleep loop, same stdlib pattern as kit.rs's blocking approach):
```rust
fn wait_for_ready(&self, name: &str) -> Result<()> {
    let delays = [200, 400, 800, 1600, 3200, 6400, 12800];
    for delay_ms in &delays {
        match self.sandbox_status(name) {
            Ok(status) if status == "running" || status == "created" => return Ok(()),
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(*delay_ms));
    }
    Err(DockerError::CommandFailed(format!(
        "sbx sandbox '{}' not ready after ~25s", name
    )))
}
```

**Error handling pattern** (runtime_base.rs lines 335-348, stderr-based error classification):
```rust
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(DockerError::CommandFailed(format!(
        "sbx {} failed: {}", subcommand, stderr.trim()
    )));
}
```

**Mock binary unit test pattern** (mod.rs lines 65-81):
```rust
#[test]
fn run_propagates_exit_code_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("fake-sbx");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"#!/bin/sh\necho 'sandbox not ready' >&2\nexit 1\n").unwrap();
    }
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();

    let sbx = SbxRuntime { binary: path };
    let result = sbx.run(&["exec", "foo", "echo", "hi"]);
    assert!(result.is_err());
}
```

---

### `src/containers/runtime.rs` - Dispatch Arm Replacement (controller, request-response)

**Analog:** `src/containers/runtime.rs` lines 96-103 (existing Sbx is_available delegation)

**Core pattern: Sbx dispatch arm delegation** (lines 96-103):
```rust
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .does_container_exist(name),
```

**Pattern for each replaced stub** (9 arms, same shape):
```rust
// does_container_exist (replacing line 162-170 stub)
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .does_container_exist(name),

// is_container_running (replacing line 206-214 stub)
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .is_container_running(name),

// is_daemon_running (replacing line 107 base delegation)
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .is_daemon_running(),

// create_container (replacing lines 231-241 which use base.run_create)
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .create_container(name, image, config),

// start_container
RuntimeKind::Sbx => Ok(()), // no-op per D-14

// stop_container
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .stop_container(name),

// remove
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .remove(name, force),

// exec
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .exec(name, cmd),

// batch_running_states (replacing lines 352-359 stub)
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .batch_running_states(prefix),
```

---

### `src/session/instance.rs` - publish_ports Call Site (controller, request-response)

**Analog:** `src/session/instance.rs` lines 1248-1265 (existing create + capability gate pattern)

**Imports needed:** (add to existing instance.rs imports)
```rust
use crate::containers::sbx::ports::PortPublishResult;
```

**Core pattern: capability-gated post-create call** (lines 1252-1264):
```rust
// Existing pattern: capability-gated call after container creation
let runtime = containers::get_container_runtime();
if runtime.capabilities().supports_image_pull {
    runtime.ensure_image(image)?;
}

let config = self.build_container_config()?;
let container_id = container.create(&config)?;

// Phase 5 addition (same capability-gate pattern):
let caps = runtime.capabilities();
if !caps.supports_port_publish_at_create && caps.supports_dynamic_port_publish {
    if !config.port_mappings.is_empty() {
        if let Some(sbx_rt) = runtime.sbx.as_ref() {
            let results = sbx_rt.publish_ports(&container.name, &config.port_mappings);
            for result in &results {
                if let PortPublishResult::Failed { port, stderr } = result {
                    tracing::warn!(
                        target: "containers.sbx.ports",
                        %port, %stderr,
                        "port publish failed (sandbox kept running)"
                    );
                }
            }
        }
    }
}
```

---

### `tests/sbx_integration.rs` (test, CRUD)

**Analog:** `tests/sandbox_integration.rs` lines 1-14

**Imports pattern** (sandbox_integration.rs lines 8-9):
```rust
use agent_of_empires::containers::{self, ContainerRuntimeInterface};
```

**Guard pattern** (sandbox_integration.rs lines 11-14):
```rust
fn docker_available() -> bool {
    let rt = containers::get_container_runtime();
    rt.is_available() && rt.is_daemon_running()
}
```

**Phase 5 adaptation with `#[ignore]` per D-17:**
```rust
use agent_of_empires::containers::sbx::SbxRuntime;

fn sbx_available() -> bool {
    let sbx = SbxRuntime::new();
    sbx.is_available()
}

#[test]
#[ignore]
fn lifecycle_create_exec_stop_rm() {
    if !sbx_available() {
        return;
    }
    // create -> exec -> ports -> stop -> rm
}
```

---

### `tests/fixtures/sbx_ls.json` (config, transform)

**Analog:** `tests/fixtures/sbx_kits/ok/` directory (committed test fixture pattern)

**Content pattern** (verified from real sbx v0.29.0 output per RESEARCH.md):
```json
{
  "sandboxes": [
    {
      "id": "edacdd7d-61a6-442b-a9a6-d8ccea8903ed",
      "name": "claude-Lymow-HA",
      "agent": "claude",
      "status": "stopped",
      "socket_path": "/tmp/sboxd-501-sandboxes/docker.sock",
      "workspaces": [
        "/Users/gbryer/projects/other/lymow/Lymow-HA"
      ]
    }
  ]
}
```

---

## Shared Patterns

### Subprocess Execution (DRY helper)
**Source:** `src/containers/runtime_base.rs` lines 319-352 (run_create)
**Apply to:** All `SbxRuntime` action verb methods (`create_container`, `stop_container`, `remove`, `exec`, `does_container_exist`, `is_container_running`, `is_daemon_running`, `batch_running_states`)
```rust
// Pattern: Command::new(binary).args(args).output()? with non-zero exit -> DockerError::CommandFailed
let output = Command::new(&self.binary).args(args).output()?;
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(DockerError::CommandFailed(format!(
        "sbx {} failed: {}", args.first().unwrap_or(&""), stderr.trim()
    )));
}
Ok(output)
```

### Mock Binary Injection for Tests
**Source:** `src/containers/sbx/mod.rs` lines 65-81
**Apply to:** All unit tests for subprocess-calling methods in `mod.rs`, `ports.rs`
```rust
let dir = tempfile::TempDir::new().unwrap();
let path = dir.path().join("fake-sbx");
{
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
}
let mut perms = std::fs::metadata(&path).unwrap().permissions();
perms.set_mode(0o755);
std::fs::set_permissions(&path, perms).unwrap();

let sbx = SbxRuntime { binary: path };
```

### Capability-Gated Dispatch
**Source:** `src/session/instance.rs` lines 1252-1254
**Apply to:** The `publish_ports` call site and any future sbx-specific post-create logic
```rust
let caps = runtime.capabilities();
if !caps.supports_port_publish_at_create && caps.supports_dynamic_port_publish {
    // sbx-specific code
}
```

### Error Reuse Convention
**Source:** `src/containers/error.rs` line 47
**Apply to:** All new SbxRuntime methods
```rust
// Reuse existing variant; no new error types per D-11
Err(DockerError::CommandFailed(format!("sbx {} failed: {}", subcommand, stderr.trim())))
```

### Dispatch Delegation Shape
**Source:** `src/containers/runtime.rs` lines 96-103
**Apply to:** All 9 stub arms being replaced
```rust
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .method_name(args),
```

### Tracing Convention
**Source:** `src/containers/runtime_base.rs` lines 354-355, 366-367, 393
**Apply to:** All action verb methods on SbxRuntime
```rust
tracing::info!(target: "containers.sbx", %name, "stopping sandbox");
tracing::debug!(target: "containers.sbx", %name, %force, "removing sandbox");
tracing::warn!(target: "containers.sbx.ports", %port, %stderr, "port publish failed");
```

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `.planning/phases/05.../05-MANUAL-TESTS.md` | config | N/A | Documentation artifact; no code analog needed (markdown template) |

## Metadata

**Analog search scope:** `src/containers/`, `src/session/`, `tests/`
**Files scanned:** 9 source files read directly
**Pattern extraction date:** 2026-05-16
