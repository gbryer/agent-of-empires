# Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake - Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 7 (new/modified files)
**Analogs found:** 7 / 7

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/tui/settings/fields.rs` (modify) | component | transform | `src/tui/settings/fields.rs` (self: `build_cockpit_fields`) | exact |
| `src/process/mod.rs` (modify) | utility | event-driven | `src/process/mod.rs` (self: `#[cfg]` dispatch pattern) | exact |
| `src/process/macos.rs` (modify) | utility | event-driven | `src/process/macos.rs` (self: background thread + Command) | role-match |
| `src/process/linux.rs` (modify) | utility | event-driven | `src/process/linux.rs` (self: background thread + subprocess) | role-match |
| `tests/e2e/sandbox.rs` (modify) | test | request-response | `tests/e2e/sandbox.rs` + `tests/e2e/serve.rs` | exact |
| `tests/web_session_create.rs` (new) | test | request-response | `tests/sbx_integration.rs` | role-match |
| `src/tui/settings/fields.rs` tests (modify) | test | transform | `src/tui/settings/fields.rs` `#[cfg(test)] mod tests` | exact |

## Pattern Assignments

### `src/tui/settings/fields.rs` - Field Visibility Filter (component, transform)

**Analog:** `src/tui/settings/fields.rs` itself, specifically `build_sandbox_fields()` lines 851-1122

**Imports pattern** (lines 1-11):
```rust
use crate::session::{
    validate_check_interval, Config, ContainerRuntimeName, DefaultTerminalMode, ProfileConfig,
    TmuxClipboardMode, TmuxMouseMode, TmuxStatusBarMode,
};
// Phase 6 adds:
use crate::containers::container_interface::RuntimeCapabilities;
use crate::containers::runtime_base::RuntimeBase;
```

**Core pattern: dynamic field-list construction** (lines 958-1121):
The function returns a `Vec<SettingField>` built inline. The filter predicate is applied by wrapping the vec construction or using `.retain()`. Pattern from `build_cockpit_fields` shows the existing structure; the new predicate gates field inclusion based on the resolved runtime's capabilities.

```rust
// Existing: build_sandbox_fields returns a flat vec![]
// Phase 6 adds: filter before returning, using the resolved runtime
fn build_sandbox_fields(
    scope: SettingsScope,
    global: &Config,
    profile: &ProfileConfig,
) -> Vec<SettingField> {
    // ... existing resolve_value calls ...
    
    let (container_runtime, o_cr) = resolve_value(
        scope,
        global.sandbox.container_runtime,
        sb.and_then(|s| s.container_runtime),
    );
    
    // NEW: get capabilities for the resolved runtime
    let caps = match container_runtime {
        ContainerRuntimeName::Docker => RuntimeBase::DOCKER.capabilities,
        ContainerRuntimeName::Podman => RuntimeBase::PODMAN.capabilities,
        ContainerRuntimeName::AppleContainer => RuntimeBase::APPLE_CONTAINER.capabilities,
        ContainerRuntimeName::Sbx => RuntimeBase::SBX.capabilities,
    };
    
    let mut fields = vec![/* ... existing field structs ... */];
    fields.retain(|f| is_field_applicable(f.key, &caps));
    fields
}

fn is_field_applicable(key: FieldKey, caps: &RuntimeCapabilities) -> bool {
    match key {
        FieldKey::ExtraVolumes => caps.supports_arbitrary_volume_paths,
        FieldKey::MountSsh => caps.supports_arbitrary_volume_paths,
        FieldKey::VolumeIgnores => caps.supports_anonymous_volumes,
        _ => true,
    }
}
```

**Test pattern: in-module cross-product test** (lines 2318-2365):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Config, ProfileConfig};

    #[test]
    fn test_field_visibility_matches_capabilities() {
        let runtime_caps = [
            (ContainerRuntimeName::Docker, RuntimeBase::DOCKER.capabilities),
            (ContainerRuntimeName::Podman, RuntimeBase::PODMAN.capabilities),
            (ContainerRuntimeName::AppleContainer, RuntimeBase::APPLE_CONTAINER.capabilities),
            (ContainerRuntimeName::Sbx, RuntimeBase::SBX.capabilities),
        ];
        
        for (runtime_name, caps) in &runtime_caps {
            // Assert specific expectations per field x runtime
            // ...
        }
    }
}
```

---

### `src/process/mod.rs` - Wake Handler Public API (utility, event-driven)

**Analog:** `src/process/mod.rs` lines 40-56 (`get_foreground_pid`) for `#[cfg]` dispatch pattern

**Core pattern: platform-conditional dispatch** (lines 40-56):
```rust
/// Get the foreground process group leader PID for a given shell PID
pub fn get_foreground_pid(shell_pid: u32) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        linux::get_foreground_pid(shell_pid)
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_foreground_pid(shell_pid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = shell_pid;
        None
    }
}
```

**Phase 6 adds** (same dispatch pattern):
```rust
/// Register a background thread that listens for host wake events and
/// clock-resyncs any aoe-owned sbx sandboxes. No-op on unsupported platforms.
pub fn register_wake_handler() {
    #[cfg(target_os = "macos")]
    macos::register_wake_handler();

    #[cfg(target_os = "linux")]
    linux::register_wake_handler();

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {}
}
```

**Imports pattern** (lines 1-12):
```rust
use std::process::Command;
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use nix::errno::Errno;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use nix::sys::signal::{kill, Signal};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use nix::unistd::Pid;
```

---

### `src/process/macos.rs` - IOKit Wake Handler (utility, event-driven)

**Analog:** `src/process/macos.rs` lines 1-104 (module structure, `use` block, `Command::new` subprocess pattern, in-module `#[cfg(test)]` tests)

**Module header pattern** (lines 1-3):
```rust
//! macOS-specific process utilities

use std::collections::HashMap;
use std::process::Command;
```

**Subprocess execution pattern** (lines 18, 57):
```rust
let Ok(output) = Command::new("ps").args(["-o", "pid=,ppid=", "-A"]).output() else {
    return children_map;
};

if !output.status.success() {
    return children_map;
}
```

**In-module test pattern** (lines 106-182):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_descendants_from_map_empty() {
        // ...
    }
}
```

**Phase 6 adds:** `pub fn register_wake_handler()` using IOKit FFI + `std::thread::spawn`. The function spawns a dedicated thread that runs a CFRunLoop forever. On `kIOMessageSystemHasPoweredOn`, it calls `resync_sbx_clocks()`.

---

### `src/process/linux.rs` - D-Bus Wake Handler (utility, event-driven)

**Analog:** `src/process/linux.rs` lines 1-124 (module structure, subprocess parsing, `#[cfg(test)]` tests)

**Module header pattern** (lines 1-5):
```rust
//! Linux-specific process utilities

use std::collections::HashMap;
use std::fs;
use std::path::Path;
```

**Subprocess + line-by-line parsing pattern** (lines 80-110):
```rust
fn find_process_in_group(pgrp: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "pid=,pgid=", "-A"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // ...
    }
    None
}
```

**Phase 6 adds:** `pub fn register_wake_handler()` using `busctl monitor` subprocess with stdout piped and lines read via `BufReader`. On detecting `PrepareForSleep(false)`, calls `resync_sbx_clocks()`.

---

### `tests/e2e/sandbox.rs` - Sbx Runtime Selector TUI Test (test, request-response)

**Analog:** `tests/e2e/sandbox.rs` (exact file) + `tests/e2e/serve.rs` (TUI navigation pattern)

**E2E test with `#[ignore]` gating** (sandbox.rs lines 1-38):
```rust
use serial_test::serial;

use crate::harness::TuiTestHarness;

#[test]
#[serial]
#[ignore = "requires Docker daemon"]
fn test_cli_add_with_sandbox() {
    let h = TuiTestHarness::new("cli_sandbox");
    let project = h.project_path();

    let output = h.run_cli(&[
        "add",
        project.to_str().unwrap(),
        "-t",
        "Sandbox E2E",
        "--sandbox",
    ]);
    assert!(
        output.status.success(),
        "aoe add --sandbox failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```

**TUI navigation pattern** (serve.rs lines 64-80):
```rust
#[test]
#[serial]
fn tui_serve_dialog_opens_to_mode_picker() {
    require_tmux!();

    let mut h = TuiTestHarness::new("serve_mode_picker");
    h.spawn_tui();

    h.wait_for(" aoe [");
    h.send_keys("R");

    h.wait_for("How should this be reachable?");
    h.assert_screen_contains("Local network");
}
```

**Phase 6 adds:** A TUI test that navigates to Settings > Sandbox > Container Runtime, selects Sbx, saves, restarts, and verifies persistence.

---

### `tests/web_session_create.rs` (new) - Web API Integration Test (test, request-response)

**Analog:** `tests/sbx_integration.rs` (ignore-gated integration test pattern)

**Integration test with availability guard** (lines 1-19):
```rust
//! Integration tests for sbx sandbox lifecycle. Gated behind #[ignore]
//! per D-17; CI skips when sbx is unavailable.

use agent_of_empires::containers::container_interface::ContainerConfig;
use agent_of_empires::containers::sbx::SbxRuntime;

fn sbx_available() -> bool {
    SbxRuntime::new().is_available()
}

#[test]
#[ignore = "requires sbx daemon"]
fn lifecycle_create_exec_stop_rm() {
    if !sbx_available() {
        eprintln!("Skipping: sbx not available");
        return;
    }
    // ... test body ...
}
```

**Phase 6 pattern:** Same structure with `#[ignore = "requires sbx daemon + serve feature"]`. Creates an sbx session via the web API and confirms PTY relay attaches.

---

## Shared Patterns

### Tracing for Background Task Failures
**Source:** `src/process/mod.rs` lines 81-101
**Apply to:** Wake handler in `macos.rs` and `linux.rs`, and `resync_sbx_clocks()`
```rust
tracing::debug!(
    target: "process.tree",
    descendants = ?pids,
    "killing process tree"
);

tracing::warn!(
    target: "process.reap",
    pid = p,
    "pid survived SIGTERM after 100ms; sending SIGKILL"
);
```
Convention: use `target: "process.wake"` for the new handler, with `tracing::warn!` for non-fatal failures and `tracing::info!` for success.

### Platform-Conditional Public API
**Source:** `src/process/mod.rs` lines 40-75
**Apply to:** `register_wake_handler()` in `mod.rs`
```rust
pub fn kill_process_tree(pid: u32) {
    #[cfg(target_os = "linux")]
    let pids = linux::collect_pid_tree(pid);

    #[cfg(target_os = "macos")]
    let pids = macos::collect_pid_tree(pid);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
    }
}
```
The wake handler follows this identical pattern with `#[cfg]` blocks dispatching to platform-specific implementations.

### Storage + Profile Iteration
**Source:** `src/session/mod.rs` lines 169-187 (`list_profiles`) + `src/session/storage.rs` lines 16-47 (`Storage::new`, `Storage::load`)
**Apply to:** `resync_sbx_clocks()` in the wake handler
```rust
pub fn list_profiles() -> Result<Vec<String>> {
    let base = get_app_dir()?;
    let profiles_dir = base.join("profiles");
    if !profiles_dir.exists() {
        return Ok(vec![]);
    }
    let mut profiles = Vec::new();
    for entry in fs::read_dir(&profiles_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                profiles.push(name.to_string());
            }
        }
    }
    profiles.sort();
    Ok(profiles)
}
```

### `#[ignore]`-Gated Integration Tests
**Source:** `tests/sbx_integration.rs` lines 8-19
**Apply to:** `tests/web_session_create.rs` and new e2e sandbox tests
```rust
fn sbx_available() -> bool {
    SbxRuntime::new().is_available()
}

#[test]
#[ignore = "requires sbx daemon"]
fn lifecycle_create_exec_stop_rm() {
    if !sbx_available() {
        eprintln!("Skipping: sbx not available");
        return;
    }
    // ...
}
```

### E2E TUI Test with Isolated Home
**Source:** `tests/e2e/harness.rs` lines 136-199 (`TuiTestHarness::new`)
**Apply to:** New sbx runtime selector e2e test
```rust
pub fn new(test_name: &str) -> Self {
    let home_dir = TempDir::new().expect("failed to create temp home");
    let session_name = format!("aoe_e2e_{}_{}", test_name, std::process::id());
    // ... pre-seed config.toml to skip welcome dialog ...
    let config_dir = app_dir_in(home_dir.path());
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    // ...
}
```

### RuntimeCapabilities Access Pattern
**Source:** `src/containers/runtime_base.rs` lines 28-119
**Apply to:** `is_field_applicable()` predicate, cross-product test
```rust
// Access as constants; no runtime lookup needed
RuntimeBase::DOCKER.capabilities    // RuntimeCapabilities { ... }
RuntimeBase::PODMAN.capabilities
RuntimeBase::APPLE_CONTAINER.capabilities
RuntimeBase::SBX.capabilities

// The struct is Copy + PartialEq + Eq
let caps: RuntimeCapabilities = RuntimeBase::SBX.capabilities;
assert!(!caps.supports_arbitrary_volume_paths);
```

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | -- | -- | All Phase 6 files have strong analogs in the existing codebase |

Note: The IOKit FFI registration (`IORegisterForSystemPower`) has no existing codebase analog since no FFI calls exist in `src/process/` today. However, the pattern (background thread + event loop) matches the existing `std::thread::spawn` usage in tests, and the FFI declarations follow standard Rust `extern "C"` patterns. RESEARCH.md Pattern 3 provides the concrete implementation reference.

## Metadata

**Analog search scope:** `src/tui/settings/`, `src/process/`, `src/containers/`, `src/session/`, `tests/e2e/`, `tests/`
**Files scanned:** 18
**Pattern extraction date:** 2026-05-16
