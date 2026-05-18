# Requirements: Agent of Empires — sbx Container Runtime Backend

**Defined:** 2026-05-15
**Core Value:** Picking `sbx` from the container runtime setting must be a true drop-in: every aoe flow that works on Docker today (TUI, web dashboard, cockpit, cross-machine, multi-agent) keeps working transparently on sbx, with sbx-specific quirks (out-of-band port publishing, microVM lifecycle) absorbed inside the runtime layer rather than leaked to the user.

## v1 Requirements

### Runtime — Trait + Capability Surface

- [ ] **RT-01**: `Sbx` is selectable in `ContainerRuntimeName` and threaded through global Settings, profile, and per-session override resolution
- [ ] **RT-02**: `SbxRuntime` implements `ContainerRuntimeInterface` end-to-end: availability (`which sbx`), daemon health (`sbx ls` + `sbx version`, with typed `SbxNotConfigured` on "not logged in" / "policy not configured" stderr), version (parse `sbx version` stdout), container CRUD (`sbx create`/`exec`/`stop`/`rm`), batch state (`sbx ls --json`). All JSON deserializers use `#[serde(default)]` on every field for forward-compat against undocumented sbx schema drift
- [ ] **RT-03**: Capability surface gains five flags so backends declare unsupported semantics: `supports_port_publish_at_create`, `supports_image_pull`, `supports_anonymous_volumes`, `supports_arbitrary_volume_paths`, `supports_dynamic_port_publish`. Existing `supports_read_only_volumes` and `supports_remove_volumes` migrate into the same place. Adding a flag must force every backend to declare a value (compile-time exhaustiveness)
- [ ] **RT-04**: `host_path == container_path` constraint for sbx is enforced in `build_container_config` by capability-gated rewrite of BOTH `working_dir` AND every `VolumeMount.container_path`. Configurations that cannot be conformed (e.g., a `volume_ignores` shadow that would clash) surface a clear validation error before reaching the runtime
- [ ] **RT-05**: Resolved `port_mappings` are auto-published via per-port `sbx ports SANDBOX --publish ...` calls immediately after `sbx create` succeeds. Per-port failures are captured (exit code + stderr), aggregated, and surfaced to the TUI/web with a warning glyph. The sandbox is NOT rolled back on port-publish failure
- [ ] **RT-06**: sbx-specific image semantics map cleanly: `pull_image` returns `Err(NotSupported)` (callers gate via capability flag), `ensure_image` skips silently when `!supports_image_pull`, `default_sandbox_image` returns the user's configured Image so `--template` reuses it
- [ ] **RT-07**: First-`exec` cold-start race is handled: `SbxRuntime::create_container` polls `sbx ls --json` for sandbox status with exponential backoff (max 30s) before returning Ok; subsequent first `exec` retries up to 3 times with 1s/2s/4s backoff on "sandbox not ready" stderr substring
- [ ] **RT-08**: Any OS-specific logic introduced (notably the sleep/wake handler in INT-02) lives in `src/process/macos.rs` / `src/process/linux.rs` per AGENTS.md, not as inline `#[cfg(target_os = ...)]` branches scattered through the runtime/integration layers

### Kit — Embedded Single Kit

- [ ] **KIT-01**: aoe ships a single sbx kit. Source files live at `src/containers/sbx_kit/` (`spec.yaml`, `install.sh`, status-hook shim files); embedded into the binary via stdlib `include_str!`/`include_bytes!` (no new crate dep); materialized to `<app_dir>/sbx-kit/aoe-<aoe_version>-<spec_hash>/` on first use
- [ ] **KIT-02**: The kit is invoked as `sbx create shell --kit <materialized-path> --template <user's image>` so the **same Docker image** that Docker/Podman/Apple Container already use is reused. No per-agent image proliferation; no per-agent native sbx mappings
- [ ] **KIT-03**: Kit `install` hook installs the chosen agent binary into the sandbox using each agent's documented install command (`npm install -g @anthropic-ai/claude-code`, `npm install -g @openai/codex`, etc.) selected from a generated mapping
- [ ] **KIT-04**: Kit `initFiles` drops in status-hook shims for agents that support hook-based status (Claude Code, Cursor, Gemini, Qwen Code). Shims write status to `<workspace>/.aoe-hooks/<id>/status` (workspace-relative path that crosses the microVM boundary), NOT `/tmp/aoe-hooks/`. Kit also drops `.gitignore` entry for `.aoe-hooks/`
- [ ] **KIT-05**: Kit cache is content-addressed by `<aoe_version>-<spec_hash>` (SHA256 over embedded bytes) so an aoe upgrade or kit content change automatically picks up new files. Stale kit dirs older than 30 days are GC'd at startup (background, log-only)
- [ ] **KIT-06**: Kit `spec.yaml` is pinned to `schemaVersion: "1"`. A `cargo xtask check-kit` (or equivalent) CI guard parses + validates the embedded spec.yaml so a sbx schema change surfaces as a build failure rather than runtime breakage

### Lifecycle

- [ ] **LIFE-01**: sbx lifecycle maps to existing trait shape: `create_container` → `sbx create shell --kit ... --template ... [PATH ...]` (no auto-attach), `start_container` → no-op (sbx auto-starts on first `exec`; capability-flagged), `exec` / `exec_command` → `sbx exec [-i] [-t] [-e KEY[=VAL]] [-w WORKDIR] SANDBOX CMD ...`, `stop_container` → `sbx stop SANDBOX`, `remove` → `sbx rm [-f] SANDBOX`

### Integration

- [ ] **INT-01**: Cockpit, web dashboard PTY relay, cross-machine TUI, and ACP cockpit clients all function transparently when the configured runtime is sbx. No surface gets sbx-specific code paths beyond the runtime layer
- [ ] **INT-02**: Sleep/wake handler scaffolds clock-resync inside running sbx sandboxes after host wake. Full implementation may be a follow-on; v1 must at least not silently break HTTPS handshakes after sleep on macOS

### Settings UI

- [ ] **SET-01**: `Sbx` runtime is fully wired through the settings TUI per AGENTS.md: new `FieldKey` if needed, `SettingField` entry in matching `build_*_fields()`, `apply_field_to_global()` + `apply_field_to_profile()` hookups, `clear_profile_override()` case, `*ConfigOverride` field with merge logic in `merge_configs()`
- [ ] **SET-02**: Settings fields gate visibility on runtime capabilities — fields the resolved runtime cannot honor are either hidden or marked unsupported, never silently ignored. Cross-product unit test (every FieldKey × every ContainerRuntimeName) asserts editability matches capability declaration

### Platform

- [ ] **PLAT-01**: aoe code compiles and runs on all of aoe's supported platforms (macOS arm64, macOS x86_64, Linux x86_64, Linux arm64). Whether the `sbx` CLI is actually installed on a given platform is the user's concern, handled by `is_available()` returning false. No platform-gating cfg branches added for sbx specifically

### Documentation

- [ ] **DOC-01**: User-facing docs explain host-side prerequisites that aoe does NOT manage: `sbx login`, `sbx policy set-default <allow-all|balanced|deny-all>`, and `sbx secret set -g <service>` per agent. Includes upgrade path (do NOT recommend `sbx reset`), kit-format-experimental warning, and disk-usage note
- [ ] **DOC-02**: `cargo xtask gen-docs` regenerates `docs/cli/reference.md` to reflect any new clap help (e.g., new `aoe sandbox runtime` choices) — CI-enforced

### Testing

- [ ] **TEST-01**: Unit tests cover sbx argv builders (every `sbx` subcommand shape), capability gating in `container_config.rs`, post-create port-publish orchestration with both success and per-port-failure paths, and kit materialization (cache hit / miss / stale paths)
- [ ] **TEST-02**: Integration tests exercise a real `sbx` install end-to-end (create + exec + stop + rm + ports), gated behind `#[ignore]` like existing Docker tests; CI gracefully skips when `sbx` is unavailable
- [ ] **TEST-03**: Manual-test plan documents what cannot be automated cheaply: cockpit/web/cross-machine session creation through sbx, sleep/wake clock-drift behavior, host-side disk-usage growth across many sandboxes
- [ ] **TEST-04**: New TUI surface (Sbx in container runtime selector) and CLI surface (any new clap choices for `aoe sandbox` / `aoe add`) get e2e coverage following the `tests/e2e/` `TuiTestHarness` pattern with unique `aoe_e2e_*` tmux names, per AGENTS.md "New features touching TUI rendering, CLI subcommands, or session lifecycle should consider adding an e2e test"
- [ ] **TEST-05**: All new and modified code passes `cargo fmt` (no diff), `cargo clippy` (no warnings; address all unless flagged with rationale), and `cargo deny check` (no new advisories; no new licenses outside the existing allow list). Pre-commit hook (cargo-husky) covers fmt+clippy locally; CI enforces deny

## v2 Requirements

Capabilities surfaced by research as worth doing eventually but explicitly out of scope for v1.

### Diagnostics

- **DIAG-01**: Read-only `sbx diagnose` passthrough inside aoe's diagnostics flow — `aoe doctor` shells out and surfaces output
- **DIAG-02**: First-run setup hint that reports the resolved default policy (read-only `sbx policy ls`) so the user knows whether their default is `allow-all`/`balanced`/`deny-all`

### Templates

- **TPL-01**: `sbx template save` integration so a session can be snapshotted into a reusable image via aoe UI

### Dynamic Port Publish

- **DYN-01**: Per-session "Publish port" runtime action on running sbx sessions (leverages `supports_dynamic_port_publish=true`), letting users open ports without editing settings

### Per-Profile Network Allow Lists

- **NET-01**: Profile-level `allowedDomains` declared via aoe and applied through `sbx policy allow network <sandbox> ...` at session create. Currently anti-feature (host-state pollution); could become v2 if a clean cleanup path exists

## Out of Scope

Explicit exclusions to prevent scope creep. Each entry includes the reason and stays Out of Scope unless the rationale changes.

| Feature | Reason |
|---------|--------|
| Per-agent native sbx mapping (`sbx create claude`, etc.) | Decided in PROJECT.md against per-agent image proliferation; one kit / one image is simpler and matches user's Docker model |
| Authoring or fetching kits at runtime | Embedded shipping is sufficient; runtime authoring would require kit DSL, validation, and update flows out of scope |
| aoe writing to `sbx secret set -g` on user's behalf | Anti-feature AF-02 — global host-side state aoe doesn't own. User configures secrets manually |
| aoe setting global sbx default network policy | Anti-feature AF-01 — same reason. User runs `sbx policy set-default` manually |
| Per-session sbx network policy from new-session dialog | Anti-feature AF-03 — leaks persistent host state aoe never cleans up |
| Auto-migrate Docker sessions to sbx on runtime change | Anti-feature AF-07 — destroys live work |
| Cleanup of sbx ports/policy/secret state on session removal | Anti-feature AF-08 — same host-state ownership concern |
| Pre-flight `sbx login` flow inside aoe | Anti-feature AF-09 — interactive browser OAuth doesn't belong inside aoe; surface clear error instead |
| Use `sbx run` (auto-attach) instead of `sbx create` + `sbx exec` | Anti-feature AF-10 — breaks the "create then exec" lifecycle aoe relies on |
| Adding port_mappings (or cpu/memory/mount_ssh) to the new-session TUI dialog | Out for this milestone — `SandboxConfigOverride` already supports per-profile/per-session via global Settings; promote later if user friction emerges |
| Migrating existing Docker-backed sessions to sbx | Users opt in by changing the runtime setting; no data migration story for in-flight sessions |
| Backwards-compatibility shims for the trait change | Per AGENTS.md: capability-flag addition is a breaking internal change; existing backends update in place |
| Windows support | aoe does not support Windows generally; sbx's Windows availability does not change that |
| Adding a new crate dep for kit embedding (e.g., `include_dir`, `rust-embed`) | Stdlib `include_str!`/`include_bytes!` is sufficient; user preference to keep the dep surface minimal |
| Code-level "sbx unsupported on platform X" gates | User preference: `is_available()` runtime check is sufficient; users install their own tooling |
| Data migration via `src/migrations/vNNN_*.rs` | Not required: adding `Sbx` to `ContainerRuntimeName` is a serde-backwards-compatible enum extension; no on-disk schema changes; kit cache is new state, not a migration target. Per AGENTS.md, migrations are reserved for breaking changes to stored data |

## Traceability

Each v1 requirement maps to exactly one roadmap phase. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| RT-01 | Phase 6 | Pending |
| RT-02 | Phase 5 | Pending |
| RT-03 | Phase 1 | Complete |
| RT-04 | Phase 3 | Pending |
| RT-05 | Phase 5 | Pending |
| RT-06 | Phase 3 | Pending |
| RT-07 | Phase 5 | Pending |
| RT-08 | Phase 1 | Complete |
| KIT-01 | Phase 4 | Pending |
| KIT-02 | Phase 4 | Pending |
| KIT-03 | Phase 4 | Pending |
| KIT-04 | Phase 4 | Pending |
| KIT-05 | Phase 4 | Pending |
| KIT-06 | Phase 4 | Pending |
| LIFE-01 | Phase 5 | Pending |
| INT-01 | Phase 6 | Pending |
| INT-02 | Phase 6 | Pending |
| SET-01 | Phase 6 | Pending |
| SET-02 | Phase 6 | Pending |
| PLAT-01 | Phase 1 | Complete |
| DOC-01 | Phase 7 | Pending |
| DOC-02 | Phase 7 | Pending |
| TEST-01 | Phase 5 | Pending |
| TEST-02 | Phase 5 | Pending |
| TEST-03 | Phase 5 | Pending |
| TEST-04 | Phase 6 | Pending |
| TEST-05 | Phase 1 | Complete |

**Coverage:**
- v1 requirements: 27 total
- Mapped to phases: 27
- Unmapped: 0

**Per-phase distribution:**
- Phase 1 (Capability Surface and Trait Foundation): 4 requirements (RT-03, RT-08, PLAT-01, TEST-05)
- Phase 2 (SbxRuntime Skeleton and Pure Argv Builders): 0 requirements (scaffolding phase; coverage realized in Phase 5)
- Phase 3 (container_config Capability Gating and Workspace Path Conformance): 2 requirements (RT-04, RT-06)
- Phase 4 (Embedded sbx Kit and Materialization): 6 requirements (KIT-01..06)
- Phase 5 (Full SbxRuntime Integration): 7 requirements (RT-02, RT-05, RT-07, LIFE-01, TEST-01, TEST-02, TEST-03)
- Phase 6 (Settings TUI, Cross-Machine Integration, and Sleep/Wake): 6 requirements (RT-01, INT-01, INT-02, SET-01, SET-02, TEST-04)
- Phase 7 (User Documentation and CLI Reference Sync): 2 requirements (DOC-01, DOC-02)

---
*Requirements defined: 2026-05-15*
*Last updated: 2026-05-15 after roadmap creation (traceability filled, phase distribution added)*
