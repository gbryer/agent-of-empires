# Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake - Research

**Researched:** 2026-05-16
**Domain:** Rust TUI settings wiring, macOS/Linux power event handling, cross-surface verification
**Confidence:** HIGH

## Summary

Phase 6 has three distinct workstreams: (1) capability-gating the settings TUI field list so sbx-incompatible fields are excluded from display, (2) verifying cross-surface transparency (web dashboard, cockpit, cross-machine TUI all work without sbx-specific code), and (3) implementing a sleep/wake handler in `src/process/` that clock-resyncs sbx sandboxes on host resume.

The settings TUI work is minimal because Phase 2 already completed the full SET-01 wiring (FieldKey, apply/clear, override merge). Phase 6 only adds a filter predicate in `build_sandbox_fields()` and a cross-product unit test. The cross-surface verification is primarily a code-trace exercise because all surfaces attach to tmux sessions (runtime-agnostic by design). The sleep/wake handler is the most novel piece: it requires raw FFI to IOKit on macOS and D-Bus integration on Linux.

**Primary recommendation:** Structure as two plans: Plan 1 covers the settings field gating + cross-product test + verification trace + e2e tests (low risk, well-understood patterns). Plan 2 covers the sleep/wake handler (platform-specific FFI, isolated in `src/process/`).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Filter-at-build-time approach. `build_sandbox_fields()` excludes fields irrelevant to the resolved runtime by checking the capability matrix. Fields unsupported by the current runtime simply don't appear in the settings list. No new rendering states (no greyed-out, no disabled input handling, no footer notes).
- **D-02:** Cross-product unit test (every `FieldKey` x every `ContainerRuntimeName`) asserts that fields excluded by the filter match the capability declaration. Test lives in-module in `src/tui/settings/fields.rs`.
- **D-03:** Register for host wake notifications: IOKit `kIOMessageSystemHasPoweredOn` on macOS, `org.freedesktop.login1.Manager` `PrepareForSleep(false)` D-Bus signal on Linux. Handler lives in `src/process/macos.rs` / `src/process/linux.rs`.
- **D-04:** On wake, query aoe's `sessions.json` for running sbx-backed sessions with `created_at` older than 5 minutes. Only resync sandboxes aoe owns.
- **D-05:** Resync command: `sbx exec <sandbox_name> date` to force clock interaction with the host. Fire-and-forget background task with `tracing::warn!` on failure.
- **D-06:** The 5-minute threshold avoids resyncing freshly-created sandboxes.
- **D-07:** INT-01 is a verification + fix-if-needed concern. No preemptive code changes in `src/server/` or `src/cockpit/`.
- **D-08:** Integration test (`cargo test --features serve --test web_session_create -- --ignored`) creates an sbx session via the API and confirms PTY relay attaches.
- **D-09:** Follow existing `tests/e2e/` `TuiTestHarness` pattern with `#[ignore]` gating.
- **D-10:** Integration tests requiring a real `sbx` binary are `#[ignore]`-gated and skip when unavailable.

### Claude's Discretion
- Exact IOKit/D-Bus registration boilerplate (planner picks the minimal working approach)
- Whether the wake handler spawns a `std::thread` or `tokio::spawn_blocking`
- Which sandbox fields are currently capability-gated vs not (planner audits the full FieldKey list against capabilities)
- Whether the web integration test lives in a new `tests/web_session_create.rs` or extends an existing test file

### Deferred Ideas (OUT OF SCOPE)
- Richer diagnostic surface for capability-gated fields (tooltip, `aoe doctor` listing)
- Visual greyed-out rendering for gated fields
- Wake handler for Docker/Podman sessions (clock drift is an sbx/microVM concern)
- Gitconfig identity / agent-config dirs (deferred from Phase 4)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RT-01 | `Sbx` selectable in `ContainerRuntimeName` and threaded through global Settings, profile, and per-session override resolution | Already complete from Phase 2 (verified: dropdown index 3, apply/clear/merge all wired). Phase 6 adds capability-gating layer on top. |
| INT-01 | Cockpit, web dashboard PTY relay, cross-machine TUI function transparently when runtime is sbx | Verified: `src/server/ws.rs` attaches to tmux sessions (runtime-agnostic). Server only references `get_container_runtime()` in `api/system.rs` for status reporting. No sbx-specific code paths needed. |
| INT-02 | Sleep/wake handler scaffolds clock-resync inside running sbx sandboxes after host wake | IOKit on macOS (`kIOMessageSystemHasPoweredOn`), D-Bus on Linux (`PrepareForSleep(false)`). Implementation lives in `src/process/{macos,linux}.rs` per AGENTS.md. |
| SET-01 | `Sbx` runtime fully wired through settings TUI per AGENTS.md | Complete from Phase 2. All five wiring points confirmed in codebase (see "Existing Wiring Status" below). |
| SET-02 | Settings fields gate visibility on runtime capabilities; cross-product unit test asserts editability matches capability declaration | Filter predicate in `build_sandbox_fields()` + in-module test. Capability matrix from `RuntimeBase::SBX.capabilities` drives the gating logic. |
| TEST-04 | New TUI/CLI surface gets e2e coverage | TUI test: settings runtime selector shows Sbx, selects it, persists. Uses `TuiTestHarness`. Integration test: web session create with sbx runtime. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Field visibility gating | TUI layer (`src/tui/settings/fields.rs`) | -- | Pure presentation logic; `build_sandbox_fields()` filters based on resolved capabilities |
| Cross-product correctness test | TUI layer (in-module test) | Container layer (`runtime_base.rs`) | Test cross-references FieldKeys against RuntimeCapabilities constants |
| Sleep/wake detection | OS process layer (`src/process/`) | -- | Per AGENTS.md, all OS-specific logic is confined to `src/process/` |
| Clock resync execution | OS process layer (`src/process/`) | Session storage | Handler reads `sessions.json` for sandbox names, shells out to `sbx exec` |
| Cross-surface transparency | Server layer (`src/server/`) | -- | Verification only; no code changes expected |
| E2E test coverage | Test harness (`tests/e2e/`) | -- | Follows existing TuiTestHarness + `#[ignore]` pattern |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| nix | 0.31 | Signal handling, process utilities | Already in Cargo.toml with signal/process features [VERIFIED: Cargo.toml] |
| tokio | 1.50 | Async runtime, spawn_blocking for background tasks | Already the project's runtime [VERIFIED: Cargo.toml] |
| tracing | (existing) | Structured logging for wake handler | Already used throughout `src/process/` [VERIFIED: codebase] |

### Supporting (Sleep/Wake handler)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| core-foundation-sys | (transitive) | CFRunLoop types for IOKit on macOS | Already in Cargo.lock transitively [VERIFIED: Cargo.lock] |
| libc | (transitive) | Raw FFI types if needed for IOKit | Available if needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw IOKit FFI | `system-configuration` crate | Would add a dep; raw FFI is ~30 lines and matches the existing pattern in `src/process/` of using low-level system calls |
| Raw D-Bus FFI (sd-bus) | `zbus` crate | `zbus` is async-native and ergonomic but adds ~5 new deps; a simple `dbus-monitor` subprocess or `sd-bus` raw FFI keeps the dep surface minimal |
| `std::thread::spawn` for IOKit run loop | `tokio::spawn_blocking` | Both work; `std::thread` is simpler since the IOKit run loop blocks forever and existing `src/process/` uses `std::thread` patterns |

**No new crate dependencies recommended.** The IOKit approach uses raw `extern "C"` FFI (the framework is linked automatically on macOS). The Linux approach can use a subprocess (`busctl monitor`) rather than adding a D-Bus crate, keeping the dep surface unchanged. [ASSUMED]

## Architecture Patterns

### System Architecture Diagram

```
Settings TUI (build_sandbox_fields)
    |
    +--> reads resolved ContainerRuntimeName from Config
    +--> calls RuntimeBase::{variant}.capabilities
    +--> filter predicate excludes inapplicable FieldKeys
    +--> returns filtered Vec<SettingField>

Wake Handler Registration (main.rs startup / tui::run)
    |
    +--> std::thread::spawn (background, never returns)
    |       |
    |       +--> [macOS] IOKit IORegisterForSystemPower
    |       |       +--> CFRunLoop waits for kIOMessageSystemHasPoweredOn
    |       |       +--> on wake: call resync_sbx_clocks()
    |       |
    |       +--> [Linux] busctl monitor / D-Bus PrepareForSleep(false)
    |               +--> on wake: call resync_sbx_clocks()
    |
    +--> resync_sbx_clocks():
            +--> load sessions.json (all profiles)
            +--> filter: is_sandboxed() && created_at < 5min ago
            +--> cross-reference with `sbx ls --json` (confirms sandbox exists)
            +--> for each: Command::new("sbx").args(["exec", name, "date"]).output()
            +--> tracing::warn! on failure
```

### Recommended Project Structure

No new files beyond what's described below:
```
src/process/
    mod.rs          # + pub fn register_wake_handler(...)
    macos.rs        # + wake notification via IOKit FFI
    linux.rs        # + wake notification via D-Bus subprocess
src/tui/settings/
    fields.rs       # + is_field_applicable() predicate + cross-product test
tests/e2e/
    sandbox.rs      # + sbx runtime selector e2e test (or new file)
tests/
    web_session_create.rs  # new, #[ignore]-gated, requires --features serve
```

### Pattern 1: Field Visibility Filter (D-01)

**What:** A predicate function that checks whether a given `FieldKey` is applicable to the current `RuntimeCapabilities`.
**When to use:** Called inside `build_sandbox_fields()` before adding each field to the output vec.
**Example:**
```rust
// Source: codebase pattern analysis
fn is_field_applicable(key: FieldKey, caps: &RuntimeCapabilities) -> bool {
    match key {
        // Fields requiring arbitrary volume paths
        FieldKey::ExtraVolumes => caps.supports_arbitrary_volume_paths,
        FieldKey::MountSsh => caps.supports_arbitrary_volume_paths,
        // Fields requiring anonymous volumes
        FieldKey::VolumeIgnores => caps.supports_anonymous_volumes,
        // All other sandbox fields are universally applicable
        _ => true,
    }
}
```
[VERIFIED: RuntimeCapabilities fields from runtime_base.rs; field semantics from FieldKey descriptions in fields.rs]

### Pattern 2: Cross-Product Unit Test (D-02)

**What:** Exhaustive test that iterates every sandbox `FieldKey` x every `ContainerRuntimeName` and asserts the filter matches the capability declaration.
**When to use:** In-module test in `src/tui/settings/fields.rs`.
**Example:**
```rust
// Source: existing test pattern in fields.rs
#[test]
fn test_field_visibility_matches_capabilities() {
    let runtime_caps = [
        (ContainerRuntimeName::Docker, RuntimeBase::DOCKER.capabilities),
        (ContainerRuntimeName::Podman, RuntimeBase::PODMAN.capabilities),
        (ContainerRuntimeName::AppleContainer, RuntimeBase::APPLE_CONTAINER.capabilities),
        (ContainerRuntimeName::Sbx, RuntimeBase::SBX.capabilities),
    ];
    
    let sandbox_field_keys = [
        FieldKey::SandboxEnabledByDefault, FieldKey::DefaultImage,
        FieldKey::Environment, FieldKey::SandboxAutoCleanup,
        FieldKey::CpuLimit, FieldKey::MemoryLimit,
        FieldKey::DefaultTerminalMode, FieldKey::ExtraVolumes,
        FieldKey::PortMappings, FieldKey::VolumeIgnores,
        FieldKey::MountSsh, FieldKey::CustomInstruction,
        FieldKey::ContainerRuntime,
    ];
    
    for (runtime_name, caps) in &runtime_caps {
        for key in &sandbox_field_keys {
            let applicable = is_field_applicable(*key, caps);
            // Assert specific expectations per runtime
            match (runtime_name, key) {
                (ContainerRuntimeName::Sbx, FieldKey::ExtraVolumes) => assert!(!applicable),
                (ContainerRuntimeName::Sbx, FieldKey::MountSsh) => assert!(!applicable),
                (ContainerRuntimeName::Sbx, FieldKey::VolumeIgnores) => assert!(!applicable),
                (ContainerRuntimeName::Docker, _) => assert!(applicable),
                // ... etc
                _ => {}
            }
        }
    }
}
```
[VERIFIED: FieldKey enum and RuntimeBase constants from codebase]

### Pattern 3: IOKit Wake Handler (macOS)

**What:** Register for system power notifications using IOKit's `IORegisterForSystemPower` and run a CFRunLoop to receive `kIOMessageSystemHasPoweredOn` messages.
**When to use:** Called once at startup, spawns a dedicated background thread.
**Example:**
```rust
// Source: Apple IOKit documentation (training knowledge)
// src/process/macos.rs

use std::ffi::c_void;

// IOKit FFI declarations
extern "C" {
    fn IORegisterForSystemPower(
        refcon: *mut c_void,
        notify_port_ref: *mut *mut c_void,
        callback: extern "C" fn(*mut c_void, u32, u32, *mut c_void),
        notifier: *mut u32,
    ) -> u32;
    fn IONotificationPortGetRunLoopSource(port: *mut c_void) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *mut c_void);
    fn CFRunLoopRun();
    fn IOAllowPowerChange(root_port: u32, notification_id: isize);
}

extern "C" {
    static kCFRunLoopDefaultMode: *mut c_void;
}

const KIO_MESSAGE_SYSTEM_HAS_POWERED_ON: u32 = 0xe0000300;

extern "C" fn power_callback(
    _refcon: *mut c_void,
    _service: u32,
    message_type: u32,
    _message_argument: *mut c_void,
) {
    if message_type == KIO_MESSAGE_SYSTEM_HAS_POWERED_ON {
        resync_sbx_clocks();
    }
}

pub fn register_wake_handler() {
    std::thread::spawn(|| {
        unsafe {
            let mut notify_port: *mut c_void = std::ptr::null_mut();
            let mut notifier: u32 = 0;
            let root_port = IORegisterForSystemPower(
                std::ptr::null_mut(),
                &mut notify_port,
                power_callback,
                &mut notifier,
            );
            if root_port == 0 {
                tracing::warn!("Failed to register for IOKit power notifications");
                return;
            }
            let source = IONotificationPortGetRunLoopSource(notify_port);
            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, kCFRunLoopDefaultMode);
            CFRunLoopRun(); // blocks forever
        }
    });
}
```
[ASSUMED: IOKit FFI signatures from Apple developer documentation. The exact function signatures and constants need verification against IOKit headers. The pattern is well-established in system software.]

### Pattern 4: D-Bus Wake Handler (Linux)

**What:** Monitor the `org.freedesktop.login1.Manager` interface for `PrepareForSleep(false)` signals via a subprocess.
**When to use:** Called once at startup, spawns a dedicated background thread.
**Example:**
```rust
// Source: systemd login1 interface documentation (training knowledge)
// src/process/linux.rs

use std::io::BufRead;
use std::process::{Command, Stdio};

pub fn register_wake_handler() {
    std::thread::spawn(|| {
        // Use busctl or dbus-monitor to watch for PrepareForSleep signals
        let child = Command::new("busctl")
            .args([
                "monitor",
                "--system",
                "--match",
                "type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let Ok(mut child) = child else {
            tracing::warn!("Failed to spawn busctl for wake monitoring");
            return;
        };

        let stdout = child.stdout.take().unwrap();
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            // PrepareForSleep(false) means "waking up"
            if line.contains("false") {
                resync_sbx_clocks();
            }
        }
    });
}
```
[ASSUMED: busctl monitor output format needs verification on a Linux system. Alternative: use `gdbus monitor` which has a slightly different output format. The subprocess approach avoids adding a D-Bus crate dependency.]

### Anti-Patterns to Avoid
- **Inline `#[cfg(target_os)]` outside `src/process/`:** Per AGENTS.md, all OS-specific logic is confined to `src/process/`. The public API in `mod.rs` uses `#[cfg]` dispatch internally.
- **Greyed-out / disabled rendering:** D-01 explicitly rejects this. Fields are simply excluded from the list.
- **Checking `Instance.sandbox_info.runtime` field:** This field does not exist. The wake handler must cross-reference sbx-owned sandboxes via `sbx ls --json` or the current config.
- **Adding sbx-specific code to `src/server/` or `src/cockpit/`:** D-07 explicitly forbids this. These surfaces are runtime-agnostic by design.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cross-platform power events | A generic "power event" abstraction layer | Platform-specific implementations in `src/process/{macos,linux}.rs` | Only two platforms; abstraction adds complexity with no reuse benefit |
| Identifying sbx sessions for resync | A new `runtime` field on `SandboxInfo` | Cross-reference `sbx ls --json` sandbox names with `sessions.json` container names | Avoids schema change; `sbx ls` is authoritative about what sbx manages |
| D-Bus communication | Hand-rolling the D-Bus wire protocol | `busctl monitor` subprocess | Avoids adding `zbus` or `dbus` crate; the subprocess pattern is reliable and self-contained |
| Settings field filtering | Custom disabled/greyed state rendering | Exclude fields from the `Vec<SettingField>` entirely | D-01 mandates filter-at-build-time; the list is already dynamically constructed |

**Key insight:** The settings wiring (SET-01) is already complete. Phase 6's novel contribution is purely the filter predicate, the cross-product test, and the wake handler.

## Common Pitfalls

### Pitfall 1: Resolving Runtime Capabilities at Wrong Time
**What goes wrong:** The filter uses the global config's `container_runtime` but the user is viewing profile settings where a different runtime is overridden.
**Why it happens:** `build_sandbox_fields()` receives `global: &Config` and `profile: &ProfileConfig`; the resolved runtime comes from `merge_configs()` which isn't called within the function.
**How to avoid:** The filter must use the resolved `container_runtime` value (after profile override merge), not just `global.sandbox.container_runtime`. Use the same `resolve_value()` result that builds the `ContainerRuntime` dropdown.
**Warning signs:** Fields disappear/appear incorrectly when switching between global and profile settings views.

### Pitfall 2: Wake Handler Identifying Docker Sessions as sbx
**What goes wrong:** The handler queries `sessions.json`, finds all sandboxed sessions, and tries to run `sbx exec` on Docker containers.
**Why it happens:** `SandboxInfo` has no `container_runtime` field. All sandboxed sessions look the same in storage.
**How to avoid:** The handler must verify each sandbox name appears in `sbx ls --json` output before attempting resync. This is cheap (one `sbx ls` call, filter client-side) and authoritative.
**Warning signs:** `tracing::warn!` noise from `sbx exec aoe-sandbox-xxx date` failing on Docker containers.

### Pitfall 3: IOKit Run Loop Blocking the Wrong Thread
**What goes wrong:** If `register_wake_handler()` is called without spawning a new thread, `CFRunLoopRun()` blocks the tokio runtime or TUI event loop.
**Why it happens:** IOKit requires a running CFRunLoop on the calling thread.
**How to avoid:** Always spawn a dedicated `std::thread` for the IOKit listener. Never call from a tokio context.
**Warning signs:** Application freezes at startup.

### Pitfall 4: busctl Not Available on Minimal Linux
**What goes wrong:** On systems without systemd (Alpine containers, WSL1), `busctl` does not exist and the wake handler silently fails.
**Why it happens:** D-Bus/systemd is not universal on Linux.
**How to avoid:** The handler spawns with `tracing::warn!` on failure and returns gracefully. No panic, no retry loop. The wake handler is "best effort" per D-05.
**Warning signs:** None visible to users (by design; clock drift is rare on non-systemd systems).

### Pitfall 5: Cross-Product Test Not Catching New Fields
**What goes wrong:** A new sandbox FieldKey is added in the future but the cross-product test doesn't include it.
**Why it happens:** The test enumerates sandbox FieldKeys manually.
**How to avoid:** Derive the list of sandbox FieldKeys programmatically by calling `build_sandbox_fields()` with a known config and extracting all keys. Or use a const array that must be updated alongside the enum (compiler won't enforce, but review catches).
**Warning signs:** The test stays green while a new field is silently shown for all runtimes.

## Code Examples

### Existing Wiring Status (SET-01 Verification)

All five wiring points for `ContainerRuntimeName::Sbx` are already implemented:

```rust
// 1. build_sandbox_fields() dropdown - line 936, 949, 951-956
ContainerRuntimeName::Sbx => 3,  // index in selector
"Docker Sandboxes".into(),        // display label

// 2. apply_field_to_global() - line 1822-1829
(FieldKey::ContainerRuntime, FieldValue::Select { selected, .. }) => {
    config.sandbox.container_runtime = match selected {
        ...
        _ => ContainerRuntimeName::Sbx,
    };
}

// 3. apply_field_to_profile() - line 2111-2121
(FieldKey::ContainerRuntime, FieldValue::Select { selected, .. }) => {
    let runtime = match selected { ... _ => ContainerRuntimeName::Sbx, };
    set_profile_override(runtime, &mut config.sandbox, |s, val| {
        s.container_runtime = val
    });
}

// 4. clear_profile_override() - line 724-728
FieldKey::ContainerRuntime => {
    if let Some(ref mut s) = config.sandbox {
        s.container_runtime = None;
    }
}

// 5. SandboxConfigOverride field + merge_configs() - profile_config.rs:193, 363-364
pub container_runtime: Option<ContainerRuntimeName>,
// merge: if let Some(container_runtime) = source.container_runtime { target.container_runtime = container_runtime; }
```
[VERIFIED: All line numbers confirmed by direct codebase inspection]

### Capability Matrix for Field Gating

```rust
// Source: src/containers/runtime_base.rs (verified)
// SBX capabilities that drive field gating:
RuntimeCapabilities {
    supports_read_only_volumes: false,      // no :ro on mounts
    supports_remove_volumes: false,         // no -v on rm
    supports_port_publish_at_create: false, // ports published post-create
    supports_image_pull: false,             // no pull verb
    supports_anonymous_volumes: false,      // no -v PATH
    supports_arbitrary_volume_paths: false, // host_path == container_path
    supports_dynamic_port_publish: true,    // sbx ports --publish
}
```
[VERIFIED: `RuntimeBase::SBX` const in runtime_base.rs line 99-118]

### Field-to-Capability Mapping (Planner Must Audit)

| FieldKey | Relevant Capability | Sbx Applicable? |
|----------|-------------------|-----------------|
| SandboxEnabledByDefault | (none, universal) | YES |
| DefaultImage | (none, used as --template) | YES |
| Environment | (none, -e works in sbx exec) | YES |
| SandboxAutoCleanup | (none, sbx rm works) | YES |
| CpuLimit | (none, sbx create accepts --cpus) | YES |
| MemoryLimit | (none, sbx create accepts -m) | YES |
| DefaultTerminalMode | (none, terminal mode is TUI concept) | YES |
| ExtraVolumes | supports_arbitrary_volume_paths | NO (sbx only allows workspace paths) |
| PortMappings | (none, published post-create) | YES (still configurable, just published differently) |
| VolumeIgnores | supports_anonymous_volumes | NO (sbx has no anonymous volumes for excludes) |
| MountSsh | supports_arbitrary_volume_paths | NO (can't mount arbitrary host paths) |
| CustomInstruction | (none, universal) | YES |
| ContainerRuntime | (none, always shown) | YES |

[VERIFIED: Capability flags from runtime_base.rs; field descriptions from fields.rs. The mapping is derived from the capability semantics.]

### resync_sbx_clocks() Implementation Pattern

```rust
// Source: CONTEXT.md D-04, D-05, D-06 + codebase patterns
fn resync_sbx_clocks() {
    // Step 1: Get list of sandboxes sbx currently manages
    let sbx_output = Command::new("sbx")
        .args(["ls", "--json"])
        .output();
    let Ok(output) = sbx_output else {
        tracing::warn!(target: "process.wake", "sbx ls failed; skipping clock resync");
        return;
    };
    if !output.status.success() {
        tracing::warn!(target: "process.wake", "sbx ls non-zero exit; skipping clock resync");
        return;
    }
    
    // Step 2: Parse sandbox names from sbx ls output
    let sbx_names: HashSet<String> = // parse from JSON .sandboxes[].name
    
    // Step 3: Load aoe sessions and find sbx-backed ones
    let five_min_ago = Utc::now() - chrono::Duration::minutes(5);
    // Load from all profiles, or just current profile
    let storage = Storage::new(&profile).ok();
    let instances = storage.and_then(|s| s.load().ok()).unwrap_or_default();
    
    for inst in instances.iter().filter(|i| {
        i.is_sandboxed() 
        && i.created_at < five_min_ago
        && sbx_names.contains(&i.sandbox_info.as_ref().unwrap().container_name)
    }) {
        let name = &inst.sandbox_info.as_ref().unwrap().container_name;
        let result = Command::new("sbx")
            .args(["exec", name, "date"])
            .output();
        match result {
            Ok(o) if o.status.success() => {
                tracing::info!(target: "process.wake", sandbox = %name, "clock resync successful");
            }
            Ok(o) => {
                tracing::warn!(
                    target: "process.wake",
                    sandbox = %name,
                    stderr = %String::from_utf8_lossy(&o.stderr).trim(),
                    "clock resync failed"
                );
            }
            Err(e) => {
                tracing::warn!(target: "process.wake", sandbox = %name, error = %e, "clock resync spawn failed");
            }
        }
    }
}
```
[VERIFIED: Storage API from session/storage.rs; Instance fields from session/instance.rs; sbx ls output format from local `sbx ls --json` which returns `{"sandboxes":[]}`]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Greyed-out / disabled fields (ROADMAP SC-2) | Filter-at-build-time (D-01) | Decided in discuss phase | Simpler implementation; no new rendering states needed |
| Store `container_runtime` on Instance | Cross-reference with `sbx ls --json` | N/A (never stored) | No schema migration; authoritative source for sbx ownership |
| `zbus` crate for D-Bus | `busctl monitor` subprocess | Design decision | Zero new dependencies; acceptable for a fire-and-forget listener |

**Deprecated/outdated:**
- ROADMAP SC-2 mentions "greyed out with a footer note": superseded by D-01's filter-at-build-time approach. The ROADMAP goal statement is aspirational; CONTEXT.md decisions override.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | IOKit `IORegisterForSystemPower` FFI signature matches the pattern shown | Code Examples (Pattern 3) | Would need to adjust FFI declarations; well-established API but exact Rust bindings need testing |
| A2 | `busctl monitor` output contains "false" on a line when PrepareForSleep(false) fires | Code Examples (Pattern 4) | Would need to switch to `gdbus monitor` or use `dbus-monitor` instead; parsing approach may differ |
| A3 | No new crate dependencies needed for IOKit/D-Bus integration | Standard Stack | If raw FFI proves too fragile, may need `core-foundation` (already transitive) or `zbus` |
| A4 | `sbx exec <name> date` is sufficient to resync the guest clock with the host | CONTEXT.md D-05 | If sbx has a dedicated clock-resync mechanism, that would be preferable; `date` as a trigger is a heuristic |
| A5 | `VolumeIgnores` maps to `supports_anonymous_volumes` capability | Field-to-Capability Mapping | If volume ignores work differently in sbx (e.g., via kit spec), the mapping may need revision |

## Open Questions

1. **How does the wake handler discover sandboxes across multiple profiles?**
   - What we know: `Storage::new(profile)` is profile-scoped. The wake handler fires globally.
   - What's unclear: Should it iterate all profiles (by listing profile dirs) or just the current/default profile?
   - Recommendation: Iterate all profile directories (they're just subdirs of the app dir). This catches all aoe-owned sbx sessions regardless of which profile created them.

2. **IOKit linking on macOS**
   - What we know: `core-foundation-sys` is already in Cargo.lock transitively. IOKit framework is automatically available on macOS.
   - What's unclear: Whether `#[link(name = "IOKit", kind = "framework")]` is needed explicitly or if it links automatically.
   - Recommendation: Add the `#[link]` attribute. It's harmless if already linked and necessary if not.

3. **Thread safety of resync_sbx_clocks()**
   - What we know: Called from a dedicated background thread. `Storage::load()` reads a file (no mutex needed). `Command::new` is thread-safe.
   - What's unclear: If the TUI is modifying sessions.json simultaneously, could we get a partial read?
   - Recommendation: Use a fresh `Storage::load()` each time (atomic file read via `read_to_string`). Worst case is stale data (missed a session or tried a gone one); both are harmless.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| sbx | Wake handler resync, integration tests | Yes | v0.29.0 | Wake handler is no-op if sbx unavailable; tests `#[ignore]`-gated |
| tmux | E2E TUI tests | Yes (implied by project) | -- | Tests skip via `require_tmux!` macro |
| busctl (Linux) | Wake handler on Linux | N/A (macOS host) | -- | `tracing::warn!` and return; non-fatal |
| IOKit framework (macOS) | Wake handler on macOS | Yes (system framework) | -- | -- |

**Missing dependencies with no fallback:**
- None

**Missing dependencies with fallback:**
- busctl on non-systemd Linux: handler logs warning and exits; clock drift is rare on such systems

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + serial_test + tempfile |
| Config file | Cargo.toml `[dev-dependencies]` |
| Quick run command | `cargo test -p agent-of-empires -- field_visibility` |
| Full suite command | `cargo test && cargo test --test e2e -- --ignored sbx` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RT-01 | Sbx selectable and threaded through settings/profile/session | e2e | `cargo test --test e2e -- --ignored sbx_runtime_selector` | No (Wave 0) |
| INT-01 | Cross-surface transparency (web/cockpit/cross-machine) | integration | `cargo test --features serve --test web_session_create -- --ignored` | No (Wave 0) |
| INT-02 | Wake handler invokes resync on sbx sessions | unit | `cargo test -p agent-of-empires wake` | No (Wave 0) |
| SET-01 | Full settings TUI wiring per AGENTS.md | (already passing) | `cargo test -p agent-of-empires settings` | Yes (existing) |
| SET-02 | Field visibility gated + cross-product test | unit | `cargo test -p agent-of-empires field_visibility_matches_capabilities` | No (Wave 0) |
| TEST-04 | E2E TUI + CLI coverage for sbx surface | e2e | `cargo test --test e2e -- --ignored sbx` | No (Wave 0) |

### Sampling Rate
- **Per task commit:** `cargo test -p agent-of-empires -- field_visibility wake`
- **Per wave merge:** `cargo test && cargo test --test e2e -- --ignored sbx_runtime_selector`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `src/tui/settings/fields.rs` test: `test_field_visibility_matches_capabilities` (covers SET-02)
- [ ] `src/process/macos.rs` or `src/process/mod.rs` test: wake handler unit test with mock (covers INT-02)
- [ ] `tests/e2e/sandbox.rs` or new file: sbx runtime selector TUI test (covers RT-01, TEST-04)
- [ ] `tests/web_session_create.rs`: sbx session via API (covers INT-01)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | N/A (no auth changes) |
| V3 Session Management | No | N/A (no session auth changes) |
| V4 Access Control | No | N/A (wake handler only touches aoe-owned sandboxes) |
| V5 Input Validation | Yes (minimal) | `sbx ls --json` output parsed with serde + `#[serde(default)]` |
| V6 Cryptography | No | N/A |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Wake handler executing arbitrary sandbox names | Tampering | Only process names from `sessions.json` that also appear in `sbx ls --json` output; names are generated by aoe (format: `aoe-sandbox-{8-char-id}`) |
| busctl subprocess injection on Linux | Tampering | Arguments are hardcoded constants, no user input flows into the command |

## Sources

### Primary (HIGH confidence)
- `src/tui/settings/fields.rs` - Complete FieldKey enum, build_sandbox_fields(), apply/clear wiring
- `src/containers/runtime_base.rs` - RuntimeBase::SBX capability matrix (7 flags, all verified)
- `src/containers/container_interface.rs` - RuntimeCapabilities struct definition
- `src/process/macos.rs` and `src/process/linux.rs` - Current process utilities (pid tree only)
- `src/process/mod.rs` - Public API and `#[cfg]` dispatch pattern
- `src/session/storage.rs` - Storage::load() API for reading sessions
- `src/session/instance.rs` - Instance struct, SandboxInfo (no container_runtime field)
- `src/server/ws.rs` - PTY relay attaches to tmux (runtime-agnostic, verified)
- Local `sbx` install - v0.29.0, `sbx ls --json` returns `{"sandboxes":[]}`

### Secondary (MEDIUM confidence)
- `06-CONTEXT.md` - All D-01 through D-10 decisions locked
- `05-CONTEXT.md` - D-15 agent_name threading via ContainerConfig confirmed
- Phase 2 verified wiring (all 5 SET-01 checkpoints confirmed in code)

### Tertiary (LOW confidence)
- IOKit FFI function signatures (from training knowledge; needs compilation test) [ASSUMED]
- busctl monitor output format for PrepareForSleep signal [ASSUMED]
- `sbx exec <name> date` as clock resync trigger [ASSUMED, per CONTEXT.md D-05]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies; all patterns verified in codebase
- Architecture: HIGH - settings filter is trivial; wake handler is isolated in `src/process/`
- Pitfalls: HIGH - all identified from direct codebase analysis
- IOKit/D-Bus specifics: MEDIUM - FFI signatures from training knowledge, not runtime-verified

**Research date:** 2026-05-16
**Valid until:** 2026-06-16 (stable domain; IOKit and D-Bus APIs don't change)
