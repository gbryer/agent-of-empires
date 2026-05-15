# Roadmap: Agent of Empires — sbx Container Runtime Backend

## Overview

This milestone adds Docker Sandboxes (`sbx`) as a fourth `ContainerRuntime` peer to the existing Docker / Podman / AppleContainer abstraction in `aoe`. The work is a brownfield insert with clear architectural boundaries: the trait surface and capability flags must land first (Phase 1), then `SbxRuntime` and the cross-runtime workspace-path conformance can land in parallel (Phases 2 and 3), the embedded kit develops independently (Phase 4), all three streams converge for full subprocess integration with real `sbx` (Phase 5), then the settings TUI wiring plus cross-machine integration verification land (Phase 6), and finally user docs are written against the now-stable UX (Phase 7). Every requirement is mapped to exactly one phase; the phase order is the dependency order, with each phase's exit gated by observable, verifiable behavior.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Capability Surface and Trait Foundation** - Extend `ContainerRuntimeInterface` with `RuntimeCapabilities`, migrate the existing two flags, add five new flags, register `Sbx` in `ContainerRuntimeName`, and lock platform support (completed 2026-05-15)
- [x] **Phase 2: SbxRuntime Skeleton and Pure Argv Builders** - Stand up `src/containers/sbx/` with capability constants, pure argv builders for `sbx create` / `exec` / `ports --publish`, and `is_available()`; fully unit-testable without sbx installed (completed 2026-05-15)
- [ ] **Phase 3: container_config Capability Gating and Workspace Path Conformance** - Rewrite `compute_volume_paths` and `build_container_config` to honor `!supports_arbitrary_volume_paths` (host_path == container_path) and `!supports_image_pull` (skip ensure_image), with a clear validation error for inconformable configs
- [ ] **Phase 4: Embedded sbx Kit and Materialization** - Author `src/containers/sbx_kit/` (`spec.yaml`, `install.sh`, status-hook shims), embed via stdlib `include_str!`/`include_bytes!`, materialize to a content-hashed cache dir on first use, and CI-validate the spec
- [ ] **Phase 5: Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish)** - Wire real `sbx` subprocess calls into `create_container` (with readiness probe + first-exec retry), `exec_command`, `stop_container`, `remove`, batch state via `sbx ls --json`, and the post-create `sbx ports --publish` per-port loop
- [ ] **Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake** - Wire `Sbx` through the settings TUI per AGENTS.md (FieldKey + apply/clear + override merge), gate field visibility on capabilities, verify cockpit/web/cross-machine transparency end-to-end, and scaffold the post-wake clock-resync handler in `src/process/`
- [ ] **Phase 7: User Documentation and CLI Reference Sync** - Ship `docs/sandbox/sbx.md` covering host-side prerequisites (`sbx login`, `sbx policy set-default`, `sbx secret set -g`), upgrade path, kit-format-experimental warning, and disk-usage note; regenerate `docs/cli/reference.md` via `cargo xtask gen-docs`

## Phase Details

### Phase 1: Capability Surface and Trait Foundation
**Goal**: The `ContainerRuntimeInterface` trait expresses runtime capabilities exhaustively, every existing backend declares honest values, and `Sbx` exists as a `ContainerRuntimeName` variant that compiles end-to-end (with no real impl behind it yet).
**Depends on**: Nothing (first phase)
**Requirements**: RT-03, RT-08, PLAT-01, TEST-05
**Success Criteria** (what must be TRUE):
  1. Adding a new capability flag forces every backend to declare a value or `cargo build` fails (compile-time exhaustiveness verified by attempting to add a fake flag in a test branch and observing the error)
  2. `cargo test` shows passing unit tests asserting Docker / Podman / AppleContainer report their honest capability matrices (existing two flags migrated, five new flags returned with their backend-correct values)
  3. `cargo build` succeeds on macOS arm64 and Linux x86_64 with `Sbx` added as a `ContainerRuntimeName` variant (no impl yet); `is_available()` for sbx returns false on every platform until Phase 2 lands
  4. No new `#[cfg(target_os = ...)]` branches appear in `src/containers/` or `src/session/`; any OS-specific scaffolding for the sleep/wake handler (Phase 6) lives only in `src/process/macos.rs` / `src/process/linux.rs`
  5. `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo deny check` all pass with the trait change applied
**Plans**: 2 plans
Plans:
- [x] 01-01-PLAN.md — Add `RuntimeCapabilities` struct, migrate `RuntimeBase` to single `capabilities` field, declare honest 7-flag matrix on Docker/Podman/AppleContainer, add per-backend matrix tests
- [x] 01-02-PLAN.md — Add `ContainerRuntimeName::Sbx`, `RuntimeKind::Sbx`, `RuntimeBase::SBX`, `ContainerRuntime::sbx()`; add safe-stub dispatch arms; short-circuit `is_available` to `false`; wire factory functions

### Phase 2: SbxRuntime Skeleton and Pure Argv Builders
**Goal**: A `src/containers/sbx/` sub-module exists with the `SbxRuntime` skeleton implementing `ContainerRuntimeInterface`, exposing capability constants and pure-function argv builders for every `sbx` subcommand aoe will invoke. No real subprocess work yet; every method that would shell out either returns a stub error or is testable as a string-builder in isolation.
**Depends on**: Phase 1
**Requirements**: (Phase 2 stands up the structure that Phase 5 will wire to subprocess; no requirement formally completes here. Coverage moves to Phase 5 for RT-02 and LIFE-01.)
**Success Criteria** (what must be TRUE):
  1. `cargo test -p aoe sbx::argv` passes unit tests for `build_create_args` (positional workspace paths, `--kit`, `--template`, `--cpus`, `-m`, `:ro` mounts), `build_exec_args` (`-i` / `-t` / `-e` / `-w` permutations), and `build_ports_args` (per-port `--publish` shape) covering at least 10 distinct argv shapes
  2. `SbxRuntime::capabilities()` returns the five new flags with sbx-correct values (`supports_port_publish_at_create=false`, `supports_image_pull=false`, `supports_anonymous_volumes=false`, `supports_arbitrary_volume_paths=false`, `supports_dynamic_port_publish=true`) and is verified by unit test
  3. `SbxRuntime::is_available()` returns false when `sbx` is absent from `PATH` and true when present (validated via a unit test that injects a fake binary path)
  4. Trait dispatch through the new `RuntimeHandle::Sbx(SbxRuntime)` variant compiles and routes every `ContainerRuntimeInterface` method to `SbxRuntime` without a regression in existing `ContainerRuntime` (Docker/Podman/AppleContainer) behavior
**Plans**: 2 plans
Plans:
- [x] 02-01-PLAN.md — Stand up `src/containers/sbx/{mod.rs, argv.rs}`: SbxRuntime peer struct with injectable binary, three pure argv builders (build_create_args / build_exec_args / build_ports_args), >=10 in-module argv-shape tests covering SC-1/SC-2/SC-3
- [x] 02-02-PLAN.md — Update three `RuntimeKind::Sbx` dispatch arms in `src/containers/runtime.rs` (is_available, exec_command, build_create_args) to route through the new sbx module; add `sbx: Option<SbxRuntime>` storage field; strengthen sbx dispatch tests for SC-4

### Phase 3: container_config Capability Gating and Workspace Path Conformance
**Goal**: `build_container_config` and `compute_volume_paths` are rewritten to consult `runtime.capabilities()` so that for `!supports_arbitrary_volume_paths` runtimes (sbx) BOTH `working_dir` AND every `VolumeMount.container_path` are rewritten consistently to the host path before `ContainerConfig` leaves the function. Inconformable configs (e.g., a `volume_ignores` shadow that would clash) surface a clear validation error. `ensure_image` is skipped silently when `!supports_image_pull`. Existing Docker/Podman/AppleContainer behavior is unchanged.
**Depends on**: Phase 1
**Requirements**: RT-04, RT-06
**Success Criteria** (what must be TRUE):
  1. With sbx capability constants stubbed, `build_container_config` produces a `ContainerConfig` whose `working_dir == VolumeMount.container_path == VolumeMount.host_path` for the primary workspace mount, verified by unit tests covering at least three workspace-path shapes (plain path, path with spaces, path under `~/.agent-of-empires/worktrees/...`)
  2. With Docker capability constants, `build_container_config` produces output identical to the pre-Phase-3 baseline (verified by snapshot test against pre-existing fixtures); no regression to existing runtimes
  3. A workspace + `volume_ignores` combination that cannot be conformed to `host_path == container_path` returns `Err` with a typed variant naming the offending mount (verified by unit test)
  4. `instance.rs::get_container_for_instance` skips the `runtime.ensure_image()` call when `!supports_image_pull` (verified by unit test with a mock runtime), and the existing Docker path still calls `ensure_image` unchanged
**Plans**: 2 plans
Plans:
- [x] 03-01-PLAN.md; Add ContainerConfigError + conform_workspace_paths helper + conform_for_capabilities post-pass; thread capabilities + ContainerRuntimeName into build_container_config; add SC-1/SC-2/SC-3 tests
- [ ] 03-02-PLAN.md; Add ensure_image capability guard at instance.rs:1249-1250; thread capabilities through Instance::container_workdir + 19 call sites via container_workdir_now helper; add SC-4 source-substring + matrix-assertion + workdir-threading tests

### Phase 4: Embedded sbx Kit and Materialization
**Goal**: A single aoe-authored sbx kit ships embedded in the binary via stdlib `include_str!`/`include_bytes!` (no new crate dep). Source files live at `src/containers/sbx_kit/` (`spec.yaml`, `install.sh`, status-hook shims). The kit is materialized lazily to `<app_dir>/sbx-kit/aoe-<aoe_version>-<spec_hash>/` on first use; cache is content-addressed so `aoe` upgrades or kit content changes pick up new files automatically. Status-hook shims write to a workspace-relative path (`<workspace>/.aoe-hooks/<id>/status`) that crosses the microVM boundary, with a `.gitignore` for `.aoe-hooks/` dropped via the kit. A `cargo xtask check-kit` (or equivalent) CI guard parses and validates the embedded `spec.yaml`.
**Depends on**: Phase 1
**Requirements**: KIT-01, KIT-02, KIT-03, KIT-04, KIT-05, KIT-06
**Success Criteria** (what must be TRUE):
  1. The embedded kit materializes to the cache dir on first invocation; second invocation with the same `aoe_version + spec_hash` is a no-op (verified by unit test using `tempfile` for the app dir, asserting file mtimes are unchanged on the second call)
  2. Mutating one byte in any embedded kit file changes the cache path (different `spec_hash` produces a different dir) without affecting the previous dir on disk; stale dirs older than 30 days are GC'd at startup with log-only output (verified by unit test)
  3. `cargo xtask check-kit` parses the embedded `spec.yaml`, asserts `schemaVersion == "1"`, and fails the build if any required field is missing or any `commands.startup[*].command` contains an install verb (`apt-get`, `npm install -g`, etc.) without an idempotency guard
  4. The kit's status-hook shim writes to `<workspace>/.aoe-hooks/<id>/status` (NOT `/tmp/aoe-hooks/`), verified by inspecting the materialized shim file content; a `.gitignore` for `.aoe-hooks/` is dropped into the workspace by the kit
  5. The materialized kit invocation shape `sbx create shell --kit <materialized-path> --template <user's image>` passes argv-builder tests (kit path resolved from `KitMaterializer`, template from `default_sandbox_image()`)
**Plans**: TBD

### Phase 5: Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish)
**Goal**: `SbxRuntime` calls real `sbx` subprocesses end-to-end. `create_container` invokes `sbx create shell --kit ... --template ... [PATH ...]`, polls `sbx ls --json` for readiness with exponential backoff (max 30s), then runs the per-port `sbx ports SANDBOX --publish` loop with per-port error capture (failures aggregate into `Vec<(port, error)>`, sandbox is NOT rolled back). `exec_command` builds `sbx exec [-i] [-t] [-e] [-w]` with up-to-3-attempt retry on first-exec "sandbox not ready" stderr. `stop_container` runs `sbx stop`; `remove` runs `sbx rm [-f]`; batch state parses `sbx ls --json` with `#[serde(default)]` defensiveness. `is_daemon_running` is a composite probe (`sbx version` + `sbx ls --json` parseable) returning typed `SbxNotConfigured` on auth/policy stderr substrings.
**Depends on**: Phase 2, Phase 3, Phase 4
**Requirements**: RT-02, RT-05, RT-07, LIFE-01, TEST-01, TEST-02, TEST-03
**Success Criteria** (what must be TRUE):
  1. With `sbx` installed locally, `cargo test --test sbx_integration -- --ignored` creates a sandbox, execs a command, publishes ports, stops, and removes the sandbox end-to-end (gated `#[ignore]` like existing Docker tests; CI skips when sbx is unavailable)
  2. `SbxRuntime::create_container` returns `Ok` only after the readiness probe confirms the sandbox is `running` or `created` per `sbx ls --json` (verified by integration test that asserts the first `sbx exec` after `create_container` returns succeeds without "sandbox not ready" errors across at least 5 back-to-back creates)
  3. Port-publish failure surfaces via `Vec<(port, error)>` aggregated on the runtime result (verified by unit test using a mock `sbx ports` script that fails on the second port; assertion: sandbox stays up, the first port's success is recorded, the second port's error is captured with stderr text)
  4. `is_daemon_running` returns typed `SbxNotConfigured(reason)` when stderr matches "not logged in" / "policy not configured" substrings (verified by unit test with mocked stderr); never spawns `sbx login` from inside aoe
  5. The manual-test plan (`.planning/manual-tests/sbx.md` or equivalent) documents the cockpit/web/cross-machine session-creation walkthroughs, sleep/wake clock-drift behavior to validate by hand, and host-side disk-usage growth observation across many sandboxes
  6. `cargo test sbx::parse` passes against a committed `tests/fixtures/sbx_ls.json` snapshot, with every deserializer field annotated `#[serde(default)]`
**Plans**: TBD

### Phase 6: Settings TUI, Cross-Machine Integration, and Sleep/Wake
**Goal**: Users can pick `Sbx` from the runtime dropdown in global Settings, set it as a profile override, and override it per-session; every layer of the existing precedence chain works. Settings fields the resolved runtime cannot honor are visibly gated (greyed out with a footer note), never silently ignored. A cross-product unit test (every FieldKey × every ContainerRuntimeName) asserts editability matches capability declaration. End-to-end TUI / cockpit / web dashboard / cross-machine session creation against an sbx-backed runtime works transparently; no surface gets sbx-specific code paths beyond the runtime layer. Sleep/wake handler scaffolding lives in `src/process/macos.rs` / `src/process/linux.rs` (per AGENTS.md), wired to mark sbx sessions as "potentially clock-drifted" and to attempt a clock-resync via `sbx exec` on resume so HTTPS handshakes don't silently break after macOS sleep.
**Depends on**: Phase 5
**Requirements**: RT-01, INT-01, INT-02, SET-01, SET-02, TEST-04
**Success Criteria** (what must be TRUE):
  1. In a live `aoe` TUI run, picking `Sbx` from global Settings persists to disk and survives a restart; setting it as a profile override and per-session override both resolve correctly through the existing `merge_configs()` precedence chain (verified by e2e test using `tests/e2e/` `TuiTestHarness` with unique `aoe_e2e_*` tmux names)
  2. The Settings panel renders sbx-incompatible fields (e.g., a hypothetical "Read-only mount toggle" if added later) as greyed-out with a footer note "Not supported by current runtime: sbx"; key input on greyed fields is no-op (verified by snapshot test)
  3. The cross-product unit test (every `FieldKey` x every `ContainerRuntimeName`) passes, asserting that no field is editable for a runtime whose capability declaration says it isn't honored
  4. Creating a sandboxed session through the web dashboard's new-session API with `container_runtime=sbx` succeeds, returns a `Running` status, and the WebSocket PTY relay attaches to the running `sbx exec` process; no sbx-specific branch appears in `src/server/` or `src/cockpit/` (verified by inspection plus a `cargo test --features serve --test web_session_create -- --ignored` integration test)
  5. After a simulated macOS sleep/wake event (or the Linux `org.freedesktop.login1.Manager` `PrepareForSleep` signal in test), the sleep/wake handler in `src/process/macos.rs` / `src/process/linux.rs` invokes a clock-resync `sbx exec sandbox` call against every sbx-backed running session that was alive longer than 5min before sleep (verified by unit test with mocked sleep events)
  6. `cargo test --test e2e sbx_runtime_selector -- --ignored` passes the new TUI / CLI surface coverage: the runtime dropdown shows `Sbx` and `aoe sandbox runtime sbx` (or whatever clap subcommand surfaces) succeeds
**Plans**: TBD
**UI hint**: yes

### Phase 7: User Documentation and CLI Reference Sync
**Goal**: A user-facing guide at `docs/sandbox/sbx.md` (or equivalent path consistent with the existing `docs/` structure) explains every host-side prerequisite aoe does NOT manage: `sbx login`, `sbx policy set-default <allow-all|balanced|deny-all>`, and `sbx secret set -g <service>` per agent. The doc covers the upgrade path (do NOT recommend `sbx reset`), kit-format-experimental warning, disk-usage warning ("each sbx sandbox is a full microVM with its own image cache; expect ~Image Size x Active Sessions of host disk usage"), and debug-log location. `cargo xtask gen-docs` regenerates `docs/cli/reference.md` to reflect any new clap help (e.g., new `aoe sandbox runtime` choices) and CI enforces the regeneration.
**Depends on**: Phase 5, Phase 6
**Requirements**: DOC-01, DOC-02
**Success Criteria** (what must be TRUE):
  1. `docs/sandbox/sbx.md` (or chosen path) exists and includes sections for: `sbx login` setup, default network policy choice with a per-agent provider-domain table, `sbx secret set -g` per agent (Claude maps to `anthropic`, Codex maps to `openai`, etc.), upgrade path that explicitly avoids `sbx reset`, disk-usage warning, debug-log location, and kit-format-experimental warning
  2. The doc is wired into the website per the AGENTS.md "Adding a new page to the website" recipe: entry in `website/scripts/sync-docs.mjs` `PAGES` and `URL_MAP`, nav entry in `website/src/data/docsNav.ts`
  3. `cargo xtask gen-docs` produces no diff in `docs/cli/reference.md` when run after merging Phase 6 (clap help reflects the new `Sbx` runtime choice); CI's docs job passes
  4. A first-time sbx user following only the new doc can go from "fresh aoe install" to "`Sbx` selected and a session running" without consulting source code

## Progress

**Execution Order:**
Phases execute in numeric order: 1, 2, 3, 4, 5, 6, 7

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Capability Surface and Trait Foundation | 2/2 | Complete   | 2026-05-15 |
| 2. SbxRuntime Skeleton and Pure Argv Builders | 2/2 | Complete   | 2026-05-15 |
| 3. container_config Capability Gating and Workspace Path Conformance | 1/2 | In Progress|  |
| 4. Embedded sbx Kit and Materialization | 0/TBD | Not started | - |
| 5. Full SbxRuntime Integration (Subprocess, Lifecycle, Port Publish) | 0/TBD | Not started | - |
| 6. Settings TUI, Cross-Machine Integration, and Sleep/Wake | 0/TBD | Not started | - |
| 7. User Documentation and CLI Reference Sync | 0/TBD | Not started | - |
