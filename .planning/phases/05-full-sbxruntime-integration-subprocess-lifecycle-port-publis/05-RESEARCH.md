# Phase 5: Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish) - Research

**Researched:** 2026-05-16
**Domain:** Rust subprocess management, sbx CLI integration, container lifecycle orchestration
**Confidence:** HIGH

## Summary

Phase 5 wires real `sbx` subprocess calls into the `SbxRuntime` skeleton established in Phase 2. The skeleton (`src/containers/sbx/mod.rs`) currently has 92 lines containing only `is_available()` and `capabilities()`; the dispatch layer in `runtime.rs` has stub arms that return `Ok(false)` or empty `HashMap` for six action verbs. Phase 5 replaces all stubs with real `Command::new(binary).args(argv).output()` calls, threading through the existing `sbx::argv` pure builders and `KitMaterializer::ensure()` from Phase 4.

The sbx CLI is available on the development machine (v0.29.0), and real `sbx ls --json` output has been captured. The JSON shape includes `sandboxes[]` with fields `id`, `name`, `agent`, `status`, `socket_path`, and `workspaces[]`. All serde structs must use `#[serde(default)]` for forward-compat. The agent name for kit materialization threads via `Instance.tool` which is already available at the `get_container_for_instance` call site.

**Primary recommendation:** Implement in two plans: (1) core action verbs + parse module + ports module on `SbxRuntime`, (2) dispatch arm replacement + session layer `publish_ports` call + integration test + manual test doc.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: Port-publish lives OUTSIDE `create_container`. A new `pub fn publish_ports(&self, name: &str, port_mappings: &[String]) -> Vec<PortPublishResult>` method on `SbxRuntime`.
- D-02: `PortPublishResult` is `enum PortPublishResult { Ok(String), Failed { port: String, stderr: String } }`. Session layer logs failures via `tracing::warn!`.
- D-03: Dispatch site in `runtime.rs` does NOT add a new trait method. `publish_ports` is sbx-specific, called only when capabilities gate allows.
- D-04: `is_daemon_running` stays `fn is_daemon_running(&self) -> bool` on the trait. No signature change.
- D-05: Separate `pub fn daemon_health(&self) -> SbxDaemonHealth` method on `SbxRuntime` (NOT on the trait).
- D-06: Composite probe: run `sbx version` then `sbx ls --json`; both must succeed.
- D-07: Readiness probe is synchronous (`std::thread::sleep` + loop). Exponential backoff: 200ms, 400ms, 800ms, 1600ms, 3200ms, 6400ms, 12800ms.
- D-08: "Ready" means `sbx ls --json` shows sandbox with `status` containing `"running"` or `"created"` (case-insensitive).
- D-09: First-exec retry inside `SbxRuntime`'s exec methods. If stderr contains "not ready" or "not running", retry up to 3 times with 1s, 2s, 4s delays.
- D-10: Private helper `fn run(&self, args: &[&str]) -> Result<Output>` wrapping `Command::new(&self.binary)`.
- D-11: No new error enum. Reuse `DockerError::CommandFailed(String)`.
- D-12: Add `parse.rs` and `ports.rs` to `src/containers/sbx/`. Action verb implementations go on `SbxRuntime` in `mod.rs`.
- D-13: Replace all nine stub dispatch arms with `.sbx.as_ref().expect(...).method(...)` delegation.
- D-14: `start_container` is a no-op returning `Ok(())`.
- D-15: `create_container` calls `KitMaterializer::ensure(agent_name)` to get kit path, then builds argv, then runs subprocess. Agent name flows from Instance.
- D-16: Unit tests with mock shell scripts injected via `SbxRuntime { binary: path }`.
- D-17: Integration test `tests/sbx_integration.rs` with `#[ignore]` on every test.
- D-18: Manual test plan in `.planning/phases/05.../05-MANUAL-TESTS.md`.

### Claude's Discretion
- Exact backoff timing values (200ms/400ms/... is a suggestion; adjustable within 30s max budget)
- Whether `agent_name` threads through `ContainerConfig` as a new field or via a separate parameter on the sbx-specific dispatch path
- Exact stderr substrings for "not ready" detection in first-exec retry
- Whether `batch_running_states` filters by sandbox name prefix client-side or via sbx flag
- Fixture JSON shape for `tests/fixtures/sbx_ls.json`

### Deferred Ideas (OUT OF SCOPE)
- Gitconfig identity / agent-config dirs / GCP creds
- Richer `aoe doctor` sbx diagnostics
- Async subprocess calls
- Renaming `DockerError` to `ContainerError`
- Per-sandbox disk-usage tracking
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RT-02 | SbxRuntime implements ContainerRuntimeInterface end-to-end with serde(default) defensiveness | parse.rs serde structs derived from real sbx ls --json output; action verbs use run() helper |
| RT-05 | Post-create port-publish with per-port error capture | ports.rs module with PortPublishResult enum; session layer calls after create |
| RT-07 | Readiness probe + first-exec retry | wait_for_ready() with exponential backoff; exec retry loop on stderr match |
| LIFE-01 | Lifecycle mapping (create/start/stop/rm) | create->sbx create shell, start->no-op, stop->sbx stop, rm->sbx rm [-f] |
| TEST-01 | Unit tests for all new code | Mock binary injection pattern from Phase 2; parse fixture tests |
| TEST-02 | Integration test with real sbx | tests/sbx_integration.rs with #[ignore] guard |
| TEST-03 | Manual test plan | 05-MANUAL-TESTS.md documenting cockpit/web/cross-machine/sleep-wake walkthroughs |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Container CRUD (create/stop/rm) | SbxRuntime (containers layer) | -- | All subprocess orchestration is runtime-internal |
| Readiness probe | SbxRuntime (containers layer) | -- | Blocking poll before returning from create_container |
| Port publish orchestration | SbxRuntime (containers layer) | Session layer (instance.rs) | SbxRuntime owns the subprocess call; session layer owns the timing (post-create gate) |
| Daemon health detection | SbxRuntime (containers layer) | -- | Composite probe is runtime-internal; trait just returns bool |
| Batch state query | SbxRuntime (containers layer) | -- | Parses sbx ls --json, filters client-side |
| Agent name threading | Session layer (instance.rs) | Dispatch layer (runtime.rs) | Instance.tool provides agent_name; dispatch passes to SbxRuntime |
| Error surfacing | containers/error.rs | Session layer | Reuses existing DockerError::CommandFailed; session layer handles display |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde + serde_json | 1.x (project dep) | Parse sbx ls --json output | Already in Cargo.toml; project standard for all JSON |
| std::process::Command | stdlib | Subprocess invocation | Existing pattern throughout runtime_base.rs |
| std::thread::sleep | stdlib | Blocking backoff delays | Matches synchronous architecture (D-07) |
| tracing | 0.1.x (project dep) | Structured logging | Already used by all container runtime code |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile | (project dep) | Test temp dirs for mock scripts | Unit tests with injected binary path |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| std::thread::sleep | tokio::time::sleep | Would require async refactor of entire container layer (D-07 explicitly rejects) |
| DockerError::CommandFailed | New SbxError enum | Cosmetic; D-11 explicitly rejects this as churn |

## Architecture Patterns

### System Architecture Diagram

```
Instance::get_container_for_instance()
    |
    v
DockerContainer::create(config)
    |
    v
ContainerRuntime::create_container(name, image, config)
    |
    +-- [kind == Sbx] --> SbxRuntime::create_container(name, image, config)
    |                         |
    |                         +-- KitMaterializer::ensure(agent_name) --> kit_path
    |                         +-- argv::build_create_args(name, image, Some(kit_path), config) --> args
    |                         +-- self.run(&args) --> subprocess
    |                         +-- self.wait_for_ready(name) --> readiness poll
    |                         +-- return Ok(sandbox_name)
    |
    v
[Session layer: capability gate check]
    |
    +-- [supports_dynamic_port_publish && !supports_port_publish_at_create]
    |       |
    |       v
    |   SbxRuntime::publish_ports(name, port_mappings)
    |       |
    |       +-- for each port: argv::build_ports_args + self.run
    |       +-- return Vec<PortPublishResult>
    |
    v
Ok(container)
```

### Recommended Project Structure
```
src/containers/sbx/
  mod.rs          # SbxRuntime struct + action verb impls (create, exec, stop, rm, etc.)
  argv.rs         # Pure argv builders (unchanged from Phase 2)
  kit.rs          # KitMaterializer (unchanged from Phase 4)
  parse.rs        # [NEW] Serde structs for sbx ls --json and sbx version
  ports.rs        # [NEW] publish_ports() + PortPublishResult enum
```

### Pattern 1: Private `run()` helper (D-10)
**What:** Centralizes subprocess invocation, exit-code checking, and error mapping.
**When to use:** Every action verb that shells out to `sbx`.
**Example:**
```rust
// Source: CONTEXT.md D-10 / runtime_base.rs pattern
impl SbxRuntime {
    fn run(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new(&self.binary).args(args).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DockerError::CommandFailed(format!(
                "sbx {} failed: {}", args.first().unwrap_or(&""), stderr.trim()
            )).into());
        }
        Ok(output)
    }
}
```

### Pattern 2: Mock binary injection for tests (D-16)
**What:** Unit tests inject a shell script as the `binary` field to test error paths without real sbx.
**When to use:** All unit tests for subprocess-calling methods.
**Example:**
```rust
// Source: src/containers/sbx/mod.rs:69-82 (Phase 2 proven pattern)
#[test]
fn create_container_propagates_exit_code_error() {
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

### Pattern 3: Dispatch delegation (D-13)
**What:** `runtime.rs` Sbx arms delegate to `SbxRuntime` methods via `.sbx.as_ref().expect(...)`.
**When to use:** Every trait method implementation for RuntimeKind::Sbx.
**Example:**
```rust
// Source: runtime.rs:97-103 (already used for is_available and capabilities)
RuntimeKind::Sbx => self
    .sbx
    .as_ref()
    .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
    .does_container_exist(name),
```

### Anti-Patterns to Avoid
- **Changing the trait signature:** D-03/D-04 explicitly forbid adding new methods to `ContainerRuntimeInterface`. Port publish is sbx-specific.
- **Async subprocess:** D-07 explicitly rejects. The container layer is synchronous throughout; introducing async would cascade through dozens of call sites.
- **New error enum:** D-11 explicitly rejects. Reuse `DockerError::CommandFailed(String)`.
- **Checking `--version` flag:** sbx uses `sbx version` (positional subcommand), NOT `sbx --version`. The `--version` flag returns an error.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON deserialization | Manual parsing | serde_json with `#[serde(default)]` structs | Forward-compat against schema drift; catches missing fields gracefully |
| Argv construction | String formatting | Existing `sbx::argv::build_*_args()` pure builders | Already tested with 10+ shapes; no duplication |
| Kit materialization | Path resolution logic | `KitMaterializer::ensure(agent_name)` | Already handles cache hit/miss/stale/concurrency |
| Error mapping | Custom error types | `DockerError::CommandFailed(String)` | D-11; legacy name but saves dozens of match-arm changes |

**Key insight:** The Phase 2/3/4 groundwork means Phase 5 is pure glue code: call the existing pure builders, pipe through Command, parse the output. No new libraries, no new error types, no architectural innovation.

## Common Pitfalls

### Pitfall 1: sbx uses `version` subcommand, not `--version` flag
**What goes wrong:** `sbx --version` returns exit code 1 with "ERROR: unknown flag: --version"
**Why it happens:** sbx follows the Go cobra pattern where version is a subcommand, not a flag
**How to avoid:** Always use `["version"]` not `["--version"]` for the version probe
**Warning signs:** `is_available()` currently uses `--version` via RuntimeBase; the SbxRuntime override already uses `--version` but the real sbx binary responds to EITHER `--version` at the bare level OR `version` subcommand. Verified: `sbx --version` FAILS with exit 1. Must use `["version"]` for the daemon health probe (D-06). [VERIFIED: local sbx v0.29.0]

### Pitfall 2: sbx error messages differ from Docker
**What goes wrong:** Error detection logic tuned for Docker's stderr format misses sbx errors
**Why it happens:** sbx uses "Error: sandbox 'X' not found" while Docker uses "No such container"
**How to avoid:** The `run()` helper checks `!output.status.success()` (exit code), not stderr parsing. Only `daemon_health()` and first-exec retry need stderr substring matching.
**Warning signs:** Tests passing with mock scripts but failing with real sbx
**Real sbx error messages:** [VERIFIED: local sbx v0.29.0]
  - Missing sandbox: `Error: sandbox 'nonexistent-sandbox' not found`
  - Ports on missing sandbox: `ERROR: list published ports: runtime "nonexistent-sandbox" not found`
  - Stop missing: `Error: sandbox 'nonexistent-sandbox' not found`
  - Rm missing: `Error: sandbox 'nonexistent-sandbox' not found`

### Pitfall 3: sbx ls --json shape has `sandboxes` wrapper array
**What goes wrong:** Parser expects flat array `[{...}]` but gets `{"sandboxes": [{...}]}`
**Why it happens:** sbx wraps the list in a top-level object with a `sandboxes` key
**How to avoid:** Define a `SbxListOutput { sandboxes: Vec<SbxSandboxInfo> }` wrapper struct
**Warning signs:** Deserialization errors in `batch_running_states` or readiness probe
**Real output shape:** [VERIFIED: local sbx v0.29.0]
```json
{
  "sandboxes": [
    {
      "id": "uuid-string",
      "name": "claude-agent-of-empires",
      "agent": "claude",
      "status": "stopped",
      "socket_path": "/tmp/sboxd-501-sandboxes/docker.sock",
      "workspaces": ["/path/to/workspace"]
    }
  ]
}
```

### Pitfall 4: `SbxRuntime.is_available()` currently uses `--version` arg
**What goes wrong:** The existing `is_available()` in `mod.rs:31` runs `Command::new(&self.binary).arg("--version")` which exits 0 on the current sbx v0.29.0 binary BUT the flag is undocumented and sbx CLI reference only shows `sbx version` subcommand.
**Why it happens:** Phase 2 chose `--version` because Docker/Podman use that pattern.
**How to avoid:** For Phase 5's daemon health probe (D-06), use `["version"]` not `["--version"]`. Leave `is_available()` as-is (it works currently since `sbx` does respond to `--version` even though docs don't document it). The daemon_health composite probe uses `["version"]`.
**Warning signs:** A future sbx version could remove the undocumented `--version` flag.

### Pitfall 5: Readiness probe must parse sandbox by name, not assume position
**What goes wrong:** Parsing `sbx ls --json` assumes the just-created sandbox is the last entry
**Why it happens:** Other sandboxes may already exist; list order is not guaranteed
**How to avoid:** Filter the `sandboxes` array by `name == sandbox_name`
**Warning signs:** Readiness probe reports wrong status when multiple sandboxes exist

### Pitfall 6: agent_name comes from Instance.tool, not a new ContainerConfig field
**What goes wrong:** Planner adds `agent_name: Option<String>` to `ContainerConfig`, changing the struct everywhere
**Why it happens:** D-15 says "simplest path is adding it to ContainerConfig" but this changes the struct for all runtimes
**How to avoid:** Since `create_container` on the trait takes `(name, image, config)` and the kit path is sbx-internal, the agent_name should flow via a separate path. Options: (a) add to ContainerConfig (it already has no `#[derive(PartialEq)]` so adding a field is cheap), or (b) store it on `SbxRuntime` as mutable state before calling, or (c) look it up from the sandbox name prefix. Option (a) is cleanest per D-15.
**Warning signs:** Clippy dead_code warning if Docker/Podman never read the field

### Pitfall 7: `batch_running_states` prefix filtering
**What goes wrong:** sbx ls --json does not support a `--filter name=` flag; returns ALL sandboxes
**Why it happens:** Docker has `--filter name=` but sbx does not
**How to avoid:** Parse full `sbx ls --json` output then filter client-side by `name.starts_with(prefix)`. This is fine because sandbox count is typically < 20.
**Warning signs:** Returning non-aoe sandboxes in the health map

## Code Examples

### Verified `sbx ls --json` output shape (captured from local sbx v0.29.0)
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
        "/Users/gbryer/projects/other/lymow/Lymow-HA",
        "/Users/gbryer/projects/other/lymow/apk"
      ]
    }
  ]
}
```
[VERIFIED: local sbx v0.29.0 output]

### Verified `sbx version` output shape
```
Client Version:  v0.29.0 7055fecde6b84aeb963d1680879e5620af15c119
Server Version:  v0.29.0 7055fecde6b84aeb963d1680879e5620af15c119
```
[VERIFIED: local sbx v0.29.0 output]

### Serde parse struct (parse.rs)
```rust
// Source: derived from verified sbx ls --json output
use serde::Deserialize;

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

### SbxDaemonHealth enum (D-05)
```rust
// Source: CONTEXT.md D-05
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SbxDaemonHealth {
    Running,
    NotLoggedIn,
    PolicyNotConfigured,
    NotInstalled,
    Unknown(String),
}
```

### Integration point: get_container_for_instance publish_ports call
```rust
// Source: CONTEXT.md D-01/D-03; src/session/instance.rs:1252-1264
// After the existing container.create(&config) call:
let container_id = container.create(&config)?;

// Phase 5 addition: publish ports out-of-band for sbx
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

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Phase 1 stub: `Ok(false)` for all container queries | Phase 5: real subprocess calls | This phase | Sbx becomes functional |
| No kit path in `build_create_args` (None) | Phase 4/5: kit materialized and passed as `Some(path)` | Phase 4 built kit; Phase 5 wires it | create_container gets `--kit` flag |
| `is_daemon_running` via RuntimeBase (sbx version subcommand) | Phase 5: composite probe (version + ls --json parseable) | This phase | Typed health diagnostics |
| Trait dispatch falls through to RuntimeBase methods | Phase 5: Sbx arms delegate to SbxRuntime methods | This phase | All 9 stub arms replaced |

**Deprecated/outdated:**
- `sbx --version` flag: undocumented, may be removed. Use `sbx version` subcommand.
- Direct RuntimeBase method calls for Sbx kind: replaced by SbxRuntime delegation.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `sbx exec` stderr contains "not ready" or "not running" when sandbox is in cold-start | Code Examples / first-exec retry | Retry logic won't trigger; may need different substring |
| A2 | `sbx version` stderr contains "not logged in" when auth is missing | Daemon health detection | SbxDaemonHealth enum won't correctly classify the reason |
| A3 | `sbx version` stderr contains "policy not configured" when no default policy set | Daemon health detection | Same as A2 |

**Note on A1-A3:** These stderr substrings are from CONTEXT.md decisions. They should be tested against a real sbx install with deliberate auth/policy misconfiguration. The mock-script unit tests cover the detection logic; the integration test will validate against real sbx. D-09 and CONTEXT.md Claude's Discretion explicitly note these are adjustable after testing against real output.

## Open Questions

1. **Exact stderr for "not logged in" / "policy not configured"**
   - What we know: D-04/D-05 specify these substrings for `SbxDaemonHealth` classification
   - What's unclear: The exact error messages sbx emits (would need to log out to capture)
   - Recommendation: Implement with the documented substrings; mark as LOW confidence; integration test will validate or the planner adjusts

2. **Whether `agent_name` goes on ContainerConfig or flows separately**
   - What we know: D-15 says "simplest path is adding it to ContainerConfig"; CONTEXT Claude's Discretion says "pick whichever is less churn"
   - What's unclear: CLAUDE.md says "No dead code" -- Docker/Podman/AppleContainer won't read an agent_name field
   - Recommendation: Add `agent_name: Option<String>` to ContainerConfig with `#[serde(skip)]`. Only SbxRuntime reads it; other runtimes ignore it. The field is populated in `build_container_config` from the `tool` param already present. No dead code since the field IS read by the sbx path.

3. **`sbx ls --json` status value for "running"**
   - What we know: Verified "stopped" status from local output; D-08 says check for "running" or "created"
   - What's unclear: Whether status is exactly `"running"` or something like `"Running"` (D-08 says case-insensitive)
   - Recommendation: Use `.to_lowercase()` comparison as D-08 specifies

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| sbx | All action verbs | Yes | v0.29.0 | Tests skip via `#[ignore]` and `is_available()` guard |
| cargo / rustc | Build | Yes | (project toolchain) | -- |
| tempfile crate | Unit tests | Yes | (project dep) | -- |

**Missing dependencies with no fallback:** None

**Missing dependencies with fallback:** None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml `[[test]]` sections |
| Quick run command | `cargo test sbx` |
| Full suite command | `cargo test` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RT-02 | Full trait impl with serde(default) | unit | `cargo test sbx::parse` | Wave 0 |
| RT-05 | Port-publish per-port error capture | unit | `cargo test sbx::ports` | Wave 0 |
| RT-07 | Readiness probe + first-exec retry | unit | `cargo test sbx::mod::tests::wait_for_ready` | Wave 0 |
| LIFE-01 | Lifecycle mapping (stop/rm/start-noop) | unit + integration | `cargo test sbx` | Wave 0 |
| TEST-01 | Unit tests for all new code | unit | `cargo test sbx` | Wave 0 |
| TEST-02 | Integration test with real sbx | integration | `cargo test --test sbx_integration -- --ignored` | Wave 0 |
| TEST-03 | Manual test plan | manual-only | N/A (document) | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test sbx`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green + `cargo clippy -- -D warnings` + `cargo fmt --check`

### Wave 0 Gaps
- [ ] `src/containers/sbx/parse.rs` -- serde structs + fixture tests (REQ RT-02)
- [ ] `src/containers/sbx/ports.rs` -- publish_ports + PortPublishResult (REQ RT-05)
- [ ] `tests/sbx_integration.rs` -- lifecycle end-to-end (REQ TEST-02)
- [ ] `tests/fixtures/sbx_ls.json` -- committed fixture for parse tests (REQ RT-02)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | -- (sbx auth is user's responsibility; aoe never calls `sbx login`) |
| V3 Session Management | no | -- |
| V4 Access Control | no | -- |
| V5 Input Validation | yes | Subprocess args built via pure typed builders (no string interpolation of user input into shell) |
| V6 Cryptography | no | -- |

### Known Threat Patterns for subprocess invocation

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Command injection via container name | Tampering | Container names are generated by `DockerContainer::generate_name` (alphanumeric + truncated UUID); not user-controlled strings passed to shell |
| Secrets in process argv | Information Disclosure | EnvEntry::Inherit passes only KEY in argv; value stays in process env (existing pattern from runtime_base.rs:279-282) |
| Subprocess stderr leaking to user | Information Disclosure | stderr captured in `DockerError::CommandFailed`; surfaced via tracing::warn, not directly shown to end-user in raw form |

## Sources

### Primary (HIGH confidence)
- Local sbx v0.29.0 binary output: `sbx ls --json`, `sbx version`, `sbx stop/rm/exec/ports` error messages (captured in this session)
- Source code: `src/containers/sbx/mod.rs`, `argv.rs`, `kit.rs`, `runtime.rs`, `runtime_base.rs`, `container_interface.rs`, `error.rs`, `mod.rs`, `session/instance.rs`
- `.planning/sbx-cli-reference.md` (project-committed CLI reference from official Docker docs)

### Secondary (MEDIUM confidence)
- CONTEXT.md decisions (D-01 through D-18): user-locked implementation design

### Tertiary (LOW confidence)
- stderr substrings for "not logged in" / "policy not configured" (documented in CONTEXT but not verified against real unauthenticated sbx session)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries are project deps; no new deps needed
- Architecture: HIGH - pattern is pure glue code following established Phase 2/3/4 conventions; all source code read
- Pitfalls: HIGH - verified against real sbx binary output on development machine; error shapes captured

**Research date:** 2026-05-16
**Valid until:** 2026-06-16 (stable; sbx CLI shape unlikely to change within 30 days)
