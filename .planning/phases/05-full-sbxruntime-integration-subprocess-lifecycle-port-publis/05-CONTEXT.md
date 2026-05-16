# Phase 5: Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish) - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire real `sbx` subprocess calls into every `SbxRuntime` action verb: `create_container` (with readiness probe + kit materialization), `exec_command` / `exec` (with first-exec retry), `stop_container`, `remove`, `batch_running_states` (via `sbx ls --json`), and `is_daemon_running` (composite probe). Add the post-create `sbx ports --publish` per-port loop with per-port error capture. Ship unit tests for all new code, a gated integration test, and a manual test plan doc.

Out of scope: Settings TUI wiring of `Sbx` dropdown (Phase 6); cross-machine/cockpit/web transparency verification (Phase 6); sleep/wake clock-resync handler (Phase 6); user-facing docs (Phase 7); gitconfig identity/agent-config-dirs restoration (deferred from Phase 4 D-07, still deferred).

</domain>

<decisions>
## Implementation Decisions

### Port-Publish Orchestration (RT-05)
- **D-01:** Port-publish lives OUTSIDE `create_container`. A new `pub fn publish_ports(&self, name: &str, port_mappings: &[String]) -> Vec<PortPublishResult>` method on `SbxRuntime` runs the per-port `sbx ports SANDBOX --publish <spec>` loop. Returns a vec of per-port results (success or captured stderr). The session layer calls it immediately after `create_container` returns Ok. Rationale: keeps `create_container`'s return type unchanged (`Result<String>`) and avoids changing the trait signature, which would be a large diff touching Docker/Podman/AppleContainer for no benefit.
- **D-02:** `PortPublishResult` is a simple enum: `enum PortPublishResult { Ok(String), Failed { port: String, stderr: String } }`. The session layer (likely `instance.rs::get_container_for_instance` or the builder) logs failures via `tracing::warn!` and stores them on the `Instance` for TUI/web to surface. Phase 6 handles the TUI rendering; Phase 5 only ensures the data is captured.
- **D-03:** The dispatch site in `runtime.rs` does NOT add a new trait method. `publish_ports` is sbx-specific, called only when `caps.supports_dynamic_port_publish && !caps.supports_port_publish_at_create`. The session layer checks the capability matrix before calling. This keeps the trait unchanged.

### Daemon Health / SbxNotConfigured (RT-02)
- **D-04:** `is_daemon_running` stays `fn is_daemon_running(&self) -> bool` on the trait. No signature change. Sbx's implementation runs `sbx version`; if stderr contains "not logged in" or "policy not configured", it logs `tracing::warn!("sbx not configured: {reason}")` and returns `false`. The existing TUI already surfaces "container runtime not available" when `!is_available() || !is_daemon_running()`.
- **D-05:** A separate `pub fn daemon_health(&self) -> SbxDaemonHealth` method on `SbxRuntime` (NOT on the trait) provides the typed reason (`Running`, `NotLoggedIn`, `PolicyNotConfigured`, `NotInstalled`, `Unknown(String)`). Phase 6's settings TUI or a future `aoe doctor` can call this for richer diagnostics. Phase 5 ships the method and uses it internally; the trait stays untouched.
- **D-06:** The composite probe shape: run `sbx version` (confirms binary works and auth is valid), then `sbx ls --json` (confirms the daemon responds to real queries). Both must succeed for `is_daemon_running` to return `true`. If `sbx version` fails, check stderr for the known substrings; if `sbx ls` fails, treat as daemon-not-running.

### Readiness Probe + First-Exec Retry (RT-07)
- **D-07:** The readiness probe is synchronous (`std::thread::sleep` + loop). The existing container layer uses blocking `Command` calls throughout (Docker/Podman do `command().output()?`); introducing async here would be a large refactor. Exponential backoff: 200ms, 400ms, 800ms, 1600ms, 3200ms, 6400ms, 12800ms (7 attempts, ~25s total). If the sandbox still isn't ready, return `Err` with a clear message including the last observed state.
- **D-08:** "Ready" means `sbx ls --json` shows the sandbox with `status` field containing `"running"`. The parser checks case-insensitively and also accepts `"created"` (sbx docs say first exec triggers start; so created-but-not-yet-running is acceptable for proceeding to the first exec which will trigger the start).
- **D-09:** First-exec retry is inside `SbxRuntime`'s exec methods. If `sbx exec` stderr contains "not ready" or "not running" (case-insensitive substring), retry up to 3 times with 1s, 2s, 4s delays. After 3 failures, propagate the error. This is transparent to callers.

### Subprocess Execution Pattern
- **D-10:** `SbxRuntime` gains a private helper `fn run(&self, args: &[&str]) -> Result<Output>` that wraps `Command::new(&self.binary).args(args).output()` with standard error mapping (non-zero exit → `DockerError::CommandFailed` with stderr text). All action verbs call `self.run(...)`. Keeps subprocess boilerplate DRY without adding a new public API surface.
- **D-11:** No new error enum for Phase 5. Reuse existing `DockerError::CommandFailed(String)` for subprocess failures (the variant name is legacy but changing it is cosmetic churn across the codebase). `SbxNotConfigured` is NOT a separate error type used in Results; it's the `SbxDaemonHealth` enum returned by the diagnostic method (D-05).

### Module Layout
- **D-12:** Add two new files to `src/containers/sbx/`:
  - `parse.rs` — serde structs for `sbx ls --json` and `sbx version` output parsing. Every field annotated `#[serde(default)]`. Committed test fixture at `tests/fixtures/sbx_ls.json`.
  - `ports.rs` — `publish_ports` function + `PortPublishResult` enum.
  Action verb implementations (create, exec, stop, rm, batch_running_states, is_daemon_running) go directly on `SbxRuntime` in `mod.rs`. The file is already small (92 lines); action verbs add ~150-200 lines which is reasonable for one file. If it grows unwieldy, refactor later.
- **D-13:** The `runtime.rs` dispatch arms that currently stub (`does_container_exist`, `is_container_running`, `batch_running_states`, `create_container`, `start_container`, `stop_container`, `remove`, `exec`, `is_daemon_running`) are replaced with delegation to `SbxRuntime` methods. Same pattern as `is_available` and `capabilities` already use (`.sbx.as_ref().expect(...).method(...)`).

### Lifecycle Mapping (LIFE-01)
- **D-14:** `start_container` → no-op returning `Ok(())`. sbx auto-starts on first exec; there's no `sbx start` command. The capability system doesn't currently have a `supports_explicit_start` flag, and adding one would be scope creep. A no-op is the right answer.
- **D-15:** `create_container` calls `KitMaterializer::ensure(agent_name)` to get the kit path, then builds argv via `build_create_args(name, image, Some(&kit_path), config)`, then runs the subprocess. The agent name flows from the `Instance` through the session layer. For Phase 5, the dispatch site in `runtime.rs` will need the agent name threaded through; the simplest path is adding it to `ContainerConfig` (which already carries everything else the runtime needs) rather than changing the trait signature.

### Testing Strategy (TEST-01, TEST-02, TEST-03)
- **D-16:** Unit tests (TEST-01): in-module `#[cfg(test)]` in `parse.rs` (fixture-based JSON parsing), `ports.rs` (mock sbx script for success/failure paths), and `mod.rs` (readiness probe with mock `sbx ls` script, first-exec retry with mock failing then succeeding). Mock scripts follow the Phase 2 pattern: tempfile executable shell scripts injected via `SbxRuntime { binary: path }`.
- **D-17:** Integration test (TEST-02): new `tests/sbx_integration.rs` with `#[ignore]` on every test. Single lifecycle test: create → exec → ports → stop → rm. Guarded by `SbxRuntime::new().is_available()` check at top. CI skips via the `#[ignore]` attribute (same as Docker tests).
- **D-18:** Manual test plan (TEST-03): `.planning/phases/05-full-sbxruntime-integration-subprocess-lifecycle-port-publis/05-MANUAL-TESTS.md`. Documents cockpit/web/cross-machine walkthroughs, sleep/wake observation, disk-usage growth. Written as part of Phase 5 delivery but the actual testing is Phase 6's verification scope.

### Claude's Discretion
- Exact backoff timing values (the 200ms/400ms/... sequence is a suggestion; planner can adjust within the 30s max budget)
- Whether `agent_name` threads through `ContainerConfig` as a new field or via a separate parameter on the sbx-specific dispatch path (both are small changes; pick whichever is less churn)
- Exact stderr substrings for "not ready" detection in the first-exec retry (planner tests against real sbx output)
- Whether `batch_running_states` filters by sandbox name prefix (like Docker's `--filter name=`) or filters client-side after parsing the full `sbx ls --json` output (sbx may not have a prefix filter flag)
- Fixture JSON shape for `tests/fixtures/sbx_ls.json` (planner captures from real `sbx ls --json` output or constructs a representative sample from docs)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary and requirements
- `.planning/ROADMAP.md` § Phase 5 — phase goal, dependencies (Phase 2, 3, 4), six success criteria
- `.planning/REQUIREMENTS.md` — RT-02 (full trait impl with serde(default) defensiveness), RT-05 (post-create port-publish with per-port error capture), RT-07 (readiness probe + first-exec retry), LIFE-01 (lifecycle mapping), TEST-01 (unit tests), TEST-02 (integration test), TEST-03 (manual test plan)
- `.planning/PROJECT.md` § Key Decisions — auto-publish post-create via `sbx ports --publish`; failures surface but do NOT roll back the sandbox

### Prior phase context (carry-forward)
- `.planning/phases/02-sbxruntime-skeleton-and-pure-argv-builders/02-CONTEXT.md` — D-01 (facade dispatch), D-07 (free-function argv builders), D-08 (kit_path: Option<&Path> on build_create_args), D-10 (injectable binary path for tests)
- `.planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-CONTEXT.md` — D-09 (capabilities parameter threading), D-12 (ensure_image skip already wired)
- `.planning/phases/04-embedded-sbx-kit-and-materialization/04-CONTEXT.md` — D-08 (materialized cache key includes agent name), D-09 (per-agent dir layout), D-11 (lazy on first use from create_container), D-12 (concurrency-safe atomic-rename)

### Codebase (the surface being modified)
- `src/containers/sbx/mod.rs` — SbxRuntime skeleton (binary, is_available, capabilities); Phase 5 adds action verb methods here
- `src/containers/sbx/argv.rs` — `build_create_args`, `build_exec_args`, `build_ports_args` (pure functions; Phase 5 calls them from the new action verbs)
- `src/containers/sbx/kit.rs` — `KitMaterializer::ensure(agent_name) -> Result<PathBuf>` (Phase 5 calls from create_container)
- `src/containers/runtime.rs` — RuntimeKind::Sbx dispatch arms (lines 162, 206, 219, 286, 352 and the action verb fall-throughs); Phase 5 replaces stubs with delegation to SbxRuntime methods
- `src/containers/container_interface.rs` — `ContainerRuntimeInterface` trait; Phase 5 does NOT change any trait signatures
- `src/containers/error.rs` — `DockerError` enum; Phase 5 reuses `CommandFailed(String)` variant
- `src/session/instance.rs` § `get_container_for_instance` — call site for create_container; Phase 5 adds the publish_ports call here (gated by capability check)
- `src/session/builder.rs` — session builder; may be the site where agent_name is threaded to the container layer

### sbx CLI reference (the subprocess shapes)
- `.planning/sbx-cli-reference.md` § `sbx create` — `shell --kit <path> --template <image> [PATH ...]`
- `.planning/sbx-cli-reference.md` § `sbx exec` — `[-i] [-t] [-e KEY=VAL] [-w WORKDIR] SANDBOX CMD ...`
- `.planning/sbx-cli-reference.md` § `sbx ports` — `SANDBOX --publish [[HOST_IP:]HOST_PORT:]SANDBOX_PORT[/PROTOCOL]`
- `.planning/sbx-cli-reference.md` § `sbx stop` — `SANDBOX`
- `.planning/sbx-cli-reference.md` § `sbx rm` — `[-f] SANDBOX`
- `.planning/sbx-cli-reference.md` § `sbx ls` — `--json` flag for machine-readable output
- `.planning/sbx-cli-reference.md` § `sbx version` — stdout version string

### Repo policy
- `AGENTS.md` § Coding Style — no dead code; no em-dashes; no inline `#[cfg(target_os)]` outside `src/process/`
- `AGENTS.md` § Testing Guidelines — unit tests in-module for pure logic; integration tests in `tests/`; tests must be deterministic

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SbxRuntime { binary: PathBuf }` (`src/containers/sbx/mod.rs:17-19`) — injectable binary path enables mock-script-based unit tests for all new action verbs (proven pattern from Phase 2's is_available tests)
- `sbx::argv::build_create_args`, `build_exec_args`, `build_ports_args` (`src/containers/sbx/argv.rs`) — pure argv builders already produce correct Vec<String> shapes; Phase 5 methods just pipe these into `Command::new(binary).args(argv)`
- `KitMaterializer::ensure(agent_name)` (`src/containers/sbx/kit.rs`) — returns the materialized kit dir path; Phase 5's create_container calls this before building argv
- `DockerError::CommandFailed(String)` (`src/containers/error.rs`) — existing error variant for subprocess failures; reuse avoids adding new error types
- `RuntimeBase::SBX.capabilities` (`src/containers/runtime_base.rs`) — locked capability matrix; callers use `supports_dynamic_port_publish` to gate the publish_ports call

### Established Patterns
- **Blocking Command calls**: every existing runtime method (`does_container_exist`, `is_container_running`, etc.) uses `Command::new(binary).args(...).output()?` synchronously. Phase 5 follows the same pattern.
- **Mock binary injection**: Phase 2's `is_available` test creates a tempfile shell script and injects it via `SbxRuntime { binary: path }`. Phase 5 unit tests follow the same approach for testing subprocess error paths.
- **`#[ignore]` gated integration tests**: Docker tests in `src/containers/runtime.rs:319-326` gate on `is_available() && is_daemon_running()`. Phase 5's integration test mirrors this in `tests/sbx_integration.rs`.
- **Serde defensiveness**: `#[serde(default)]` on every field of external JSON structs (REQ-RT-02). Prior art: `src/session/storage.rs` storage format; `src/cockpit/protocol.rs` ACP messages.

### Integration Points
- `src/containers/runtime.rs` dispatch arms — the ONLY files outside `src/containers/sbx/` that Phase 5 modifies. All stubs replaced with `.sbx.as_ref().expect(...).method(...)` delegation.
- `src/session/instance.rs::get_container_for_instance` — the call site that invokes `runtime.create_container(...)`. Phase 5 adds a `publish_ports` call immediately after, gated by `if !caps.supports_port_publish_at_create && caps.supports_dynamic_port_publish`.
- `src/containers/sbx/kit.rs::KitMaterializer::ensure` — called inside `SbxRuntime::create_container` before `run(build_create_args(...))`.

</code_context>

<specifics>
## Specific Ideas

- **`run` helper shape:**
  ```rust
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

- **Readiness probe skeleton:**
  ```rust
  fn wait_for_ready(&self, name: &str) -> Result<()> {
      let delays = [200, 400, 800, 1600, 3200, 6400, 12800]; // ms
      for delay in &delays {
          let status = self.sandbox_status(name)?;
          if status == "running" || status == "created" {
              return Ok(());
          }
          std::thread::sleep(Duration::from_millis(*delay));
      }
      Err(DockerError::CommandFailed(format!(
          "sbx sandbox '{}' not ready after ~25s", name
      )).into())
  }
  ```

- **publish_ports shape:**
  ```rust
  pub fn publish_ports(&self, name: &str, port_mappings: &[String]) -> Vec<PortPublishResult> {
      port_mappings.iter().map(|spec| {
          let args = argv::build_ports_args(name, spec);
          let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
          match self.run(&arg_refs) {
              Ok(_) => PortPublishResult::Ok(spec.clone()),
              Err(e) => PortPublishResult::Failed { port: spec.clone(), stderr: e.to_string() },
          }
      }).collect()
  }
  ```

</specifics>

<deferred>
## Deferred Ideas

- **Gitconfig identity / agent-config dirs / GCP creds** — Deferred from Phase 4 D-07, still deferred. sbx v1 ships without auto-injected git identity. Users configure inside template image or via agent first-run.
- **Richer `aoe doctor` sbx diagnostics** — `SbxDaemonHealth` enum (D-05) is shipped in Phase 5 but only consumed by internal `is_daemon_running`. A user-facing diagnostic surface belongs in a future `aoe doctor` enhancement.
- **Async subprocess calls** — The container layer is synchronous throughout. Converting to async (tokio::process::Command) would be a large refactor with no user-facing benefit for this milestone.
- **Renaming `DockerError` to `ContainerError`** — Cosmetic; would touch many files for no functional gain. Leave for a future cleanup if desired.
- **Per-sandbox disk-usage tracking** — TEST-03 manual plan documents observation; automated tracking is out of scope.

</deferred>

---

*Phase: 05-full-sbxruntime-integration-subprocess-lifecycle-port-publis*
*Context gathered: 2026-05-16*
