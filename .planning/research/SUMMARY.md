# Project Research Summary

**Project:** Agent of Empires — sbx Container Runtime Backend
**Domain:** Brownfield extension to an existing Rust CLI/TUI tool — adds a 4th `ContainerRuntime` peer (Docker Sandboxes / microVM) to an established 3-runtime abstraction
**Researched:** 2026-05-15
**Confidence:** HIGH (codebase patterns verified directly; sbx CLI surface verified against official docs; kit format flagged experimental by upstream)

## Executive Summary

This is an additive milestone on a well-mapped codebase. The `ContainerRuntimeInterface` trait, `RuntimeBase` capability-flag precedent, settings-TUI wiring conventions, and `rust-embed`-based asset embedding (already used for `web/dist/` under the `serve` feature) all exist and apply cleanly. The single biggest design choice is whether the new `RuntimeCapabilities` live as **trait method + struct** (ARCHITECTURE recommendation, more mockable) or as **plain const fields on `RuntimeBase` + sbx-side const** (PITFALLS C6/C7 recommendation, exhaustive-update enforcement at compile time). Both work; pick before P1 lands.

The hardest mechanical problem is **status-hook delivery across the microVM boundary**. The existing pattern bind-mounts `/tmp/aoe-hooks/<id>/` from host into the container at the same path; sbx microVMs have their own `/tmp` tmpfs, so writes from inside the VM are invisible to the host poller and every hook-driven agent (Claude, Cursor, Gemini, Qwen, Hermes, Kiro) silently degrades to `Idle`. Three of four researchers flagged this independently. The cleanest fix is writing status under a workspace-relative path (the workspace IS shared at the same absolute path) plus a new `host_visible_tmpfs` capability flag the hook installer reads to pick the destination.

The largest risk to "drop-in 4th backend" is **scope creep into host-side global state**: sbx's `policy`, `secret`, and `login` subcommands all manage user-owned, persistent, machine-global state that aoe must never write to (rejected as anti-features AF-01..AF-03). Read-only diagnostics (DF-08, DF-09) are fine. The `sbx create → sbx exec` cold-start race (PITFALLS C4), the `host_path == container_path` mount conformance work (which is bigger than it looks; `compute_volume_paths` currently remaps everything to `/workspace/<name>`), and the post-create port-publish loop with per-port error capture (C3) are the next-largest concrete risks. Two PROJECT.md contradictions were surfaced and need resolution at `/gsd-transition`: the platform list overstates what sbx supports, and the `bundled_sounds/` precedent for embedding is wrong (those are downloaded, not embedded).

## Key Findings

### Recommended Stack

The default answer for nearly every question is "use what aoe already has." New deps must clear a real bar; only one new dep is recommended. STACK and ARCHITECTURE disagreed on which embedding crate to use; surfaced in **Open Questions** below.

**Core technologies (all already in deps):**
- `std::process::Command` (sync) — shelling out to `sbx` for `ls`, `create`, `stop`, `rm`, `version`, `ports --publish`, `ports --json`. Matches the existing `RuntimeBase` pattern; the trait is sync by design and called from blocking contexts (TUI tick, supervisor loop).
- `serde` + `serde_json` — deserializing `sbx ls --json` and `sbx ports --json` into narrow per-sbx structs (do NOT share types with Docker). Use `#[serde(default)]` on every field; sbx kit format is officially experimental, JSON schemas are undocumented.
- `serde_yaml 0.9` — read-once test-time validation of the embedded `spec.yaml`. Already in deps.
- `anyhow` + `thiserror` — extend the existing `DockerError` enum with `SbxKitMaterializationFailed`, `SbxPortPublishFailed`, `SbxNotInstalled`, `SbxNotConfigured` variants. No new error crate.
- `dirs 6.0` + `get_app_dir()` helper — kit cache lives under `<app_dir>/sbx-kit/<version>/` (or `<aoe_version>-<spec_hash>` per PITFALLS C5). Inherits debug/release namespace split for free.
- `sha2` (already in deps) — content-hash of embedded kit bytes for cache key.

**One new dep — STACK and ARCHITECTURE disagree (see Open Questions):**
- **Option A: `include_dir 0.7.4`** (STACK pick) — single-line `include_dir!("$CARGO_MANIFEST_DIR/kits/aoe")` with `Dir::extract(&path)`. MSRV 1.64. Last published 2024-06-17 but stable; single maintainer.
- **Option B: `rust-embed 8.11.0`** (ARCHITECTURE pick) — already in deps under the `serve` feature for `web/dist/`. Promote out of feature-gate. Pulls more weight (mime guessing, axum/actix integration features) but reuses an existing, in-repo precedent (`StaticAssets` in `src/server/mod.rs:48-50`).

**sbx CLI version baseline:** recommended minimum v0.28.0 (first release with first-class `--kit`); hard floor v0.25.0 (first `--cpus`). Detect via `sbx version` parse; warn (don't crash) on too-old. Pin embedded `spec.yaml` to `schemaVersion: "1"` and CI-check that sbx hasn't changed it.

See `.planning/research/STACK.md` for the full version table, alternatives considered, and the explicit "what NOT to use" list.

### Expected Features

The `ContainerRuntimeInterface` trait gives a precise checklist; FEATURES.md splits it into 28 table-stakes items, 9 sbx-only differentiators, and 10 explicit anti-features.

**Must have (table stakes; gated by `is_available()`):**
- All 28 `ContainerRuntimeInterface` methods backed by an sbx call OR an honest no-op behind a capability flag (TS-01..TS-12)
- Workspace mounts respecting `host_path == container_path` (TS-13) and `:ro` (TS-14)
- Env injection at exec time, not create time (TS-15, TS-16)
- CPU/memory passthrough via `--cpus`/`-m` (TS-17, TS-18)
- Post-create port publish via `sbx ports --publish` loop (TS-19); failures surface but don't roll back per REQ-RT-05
- `ensure_image` / `pull_image` as no-ops behind `supports_image_pull = false` (TS-20, TS-21)
- Default image hand-off to kit `--template` flag (TS-22)
- Embedded sbx kit, materialized to cache, version-checked (TS-23, REQ-KIT-01..04)
- Status hooks reachable across the microVM boundary (TS-24); see Critical Pitfalls
- Settings TUI + profile-override wiring per AGENTS.md (TS-26, TS-27)
- Cockpit / web / cross-machine non-regression (TS-28)

**Should have (sbx-only differentiators worth surfacing):**
- Single embedded kit installs agents on top of the user's existing image (DF-01)
- Status hooks delivered via kit `initFiles` (DF-02)
- microVM isolation per session (DF-03; emergent)
- Read-only `sbx diagnose` passthrough (DF-09); small, P2

**Defer (post-MVP):**
- First-run diagnostic that reports the resolved default policy (DF-08)
- Per-profile network allow lists (DF-04)
- `sbx template save` integration (DF-05)
- Dynamic port re-publish UI (DF-06)

**Anti-features (DO NOT BUILD):**
- aoe sets global sbx default network policy (AF-01)
- aoe writes to `sbx secret set -g` on user's behalf (AF-02)
- Per-session sbx network policy from new-session dialog (AF-03)
- Per-agent native sbx mappings (AF-04; already rejected in PROJECT.md)
- Runtime kit fetching/authoring (AF-05)
- Auto-migrate Docker sessions to sbx on runtime change (AF-07)
- Cleanup of sbx ports/policy/secret state on session removal (AF-08)
- Pre-flight `sbx login` flow inside aoe (AF-09)
- Use `sbx run` (auto-attach) instead of `sbx create` + `sbx exec` (AF-10)

See `.planning/research/FEATURES.md` for the full matrix.

### Architecture Approach

The wider system (CLI/TUI/server, session layer, tmux, ACP cockpit) is unchanged. All work fits into one slot — the `Container Layer` in `src/containers/` — and threads through three small surfaces around it (settings TUI fields, `container_config.rs` validation, the `ContainerRuntimeName` enum).

**Major components:**
1. **`src/containers/sbx/` (new sub-module)** — peer to `runtime.rs` / `runtime_base.rs`, NOT a single `sbx.rs`. Sub-divided into `mod.rs` (façade impl ~250 LOC), `argv.rs` (pure builders ~200 LOC), `kit.rs` (materializer ~80 LOC), `ports.rs` (publish loop ~80 LOC), `parse.rs` (sbx ls JSON parser ~100 LOC). The Apple Container precedent (single file with match arms) doesn't scale here because sbx breaks four `RuntimeBase` assumptions: image semantics inverted, argv shape diverges, port lifecycle out-of-band, kit materialization is a brand-new persistent on-disk concern.
2. **`SbxRuntime` as a peer struct** to `ContainerRuntime`, joined via `enum RuntimeHandle { Cli(ContainerRuntime), Sbx(SbxRuntime) }` returned from `get_container_runtime()`.
3. **`RuntimeCapabilities`** — single struct returned by `fn capabilities() -> RuntimeCapabilities` on `ContainerRuntimeInterface` (ARCHITECTURE) OR plain const fields on `RuntimeBase` + sbx-side const (PITFALLS). Five new flags: `supports_port_publish_at_create`, `supports_image_pull`, `supports_anonymous_volumes`, `supports_arbitrary_volume_paths`, `supports_dynamic_port_publish`. Existing `supports_read_only_volumes` / `supports_remove_volumes` migrate into the same place. **A sixth flag `host_visible_tmpfs` is needed to fix the hook status problem (PITFALLS C1)**; not in PROJECT.md REQ-RT-03 yet.
4. **Port publish orchestration inside `SbxRuntime::create_container`** — NOT a separate trait method. Trait contract stays "after `create_container` returns Ok, the container is fully usable." Per-port error capture; never roll back the sandbox per REQ-RT-05.
5. **Lazy kit materialization on first use** — `<app_dir>/sbx-kit/<version>/` (or content-hashed key per PITFALLS C5). Self-healing if interrupted (version stamp gate).

**Files touched:** ~8 new files (~800 LOC), ~12 modified files (small additive edits each). Files explicitly unchanged: `src/server/`, `src/cockpit/`, `src/tmux/`, `src/git/`, `src/process/`, `src/hooks/`, `src/tui/dialogs/new_session/`, `src/tui/home/`, `src/cli/add.rs`. All consume the runtime via `DockerContainer` opaquely.

**Adding `Sbx` to `ContainerRuntimeName` is a serde-backwards-compatible enum extension.** No `src/migrations/vNNN_*.rs` needed.

See `.planning/research/ARCHITECTURE.md` for the full file-by-file delta and explicit anti-patterns to avoid.

### Critical Pitfalls

PITFALLS.md identifies 12 critical, 6 moderate, and 6 minor pitfalls. The top issues that shape phase structure:

1. **C1 — Hook status file invisible across microVM boundary.** The existing host bind-mount of `/tmp/aoe-hooks/<id>/` is dead on arrival for sbx (microVM has its own `/tmp` tmpfs). Every hook-driven agent (Claude/Cursor/Gemini/Qwen/Hermes/Kiro) silently degrades to `Idle`. **Fix:** add `host_visible_tmpfs` capability flag (false for sbx); hook installer rewrites destination to a workspace-relative path (`<workspace>/.aoe-hooks/<id>/status`). Drop a `.gitignore` for `.aoe-hooks/` via the kit's `files/workspace/` block.

2. **C2 — `host_path != container_path` mounts silently mismount or fail.** `compute_volume_paths` in `src/session/container_config.rs` remaps everything to `/workspace/<name>`. For sbx, this needs to be auto-conformed in `build_container_config` so BOTH `working_dir` AND `VolumeMount.container_path` are rewritten consistently to the host path before `ContainerConfig` leaves the function. ARCHITECTURE flags this as "bigger than it looks"; deserves its own dedicated phase work.

3. **C3 — Post-create port publish failures swallowed.** Naive impl runs `sbx ports SANDBOX --publish` after `sbx create` succeeds, ignores stderr, reports success because the sandbox itself was created. **Fix:** per-port (not batched) execution with exit code + stderr capture; aggregate failures into `Vec<(port, error)>`; surface in TUI/web with warning glyph. Do NOT roll back the sandbox per REQ-RT-05.

4. **C4 — Cold-start race between `sbx create` returning and `sbx exec` working.** microVM boot is seconds-class. **Fix:** readiness probe in `SbxRuntime::create_container` (poll `sbx ls --json` for status with exponential backoff, max 30s); retry-with-backoff on first exec only.

5. **C5 — Kit cache staleness after aoe upgrade.** **Fix:** materialize to `~/.agent-of-empires/kits/aoe-<aoe_version>-<spec_hash>/` where `spec_hash` is SHA256 over embedded bytes. Path itself is the cache key. GC old `aoe-*` dirs older than 30 days at startup.

6. **C6/C7 — Capability-flag placement and UI honesty.** See Open Questions. Each `SettingField` should also gain `requires_capability: Option<fn(&RuntimeCapabilities) -> bool>` so the settings panel never shows fields the runtime won't honor.

7. **C8 — `sbx login` is mandatory, interactive, NOT validated up front.** **Fix:** composite `is_daemon_running` that runs `sbx version` AND `sbx ls --json`, returning typed `SbxNotConfigured(reason)`. Never spawn `sbx login` from inside aoe.

8. **C12 — Clock drift after laptop sleep breaks long-running sandboxes.** **Fix:** sleep/wake handler; auto-resync via `sbx exec sandbox sudo date -s @$(date +%s)` post-wake.

See `.planning/research/PITFALLS.md` for the full enumeration with phase tags.

## Implications for Roadmap

The dependency graph in ARCHITECTURE.md is the right starting point. Adding C1's `host_visible_tmpfs` work and C2's `compute_volume_paths` rewrite shifts the phase boundaries slightly:

### Phase 1: Capability struct + trait extension (foundation)
**Rationale:** Architectural keystone. Until the trait/RuntimeBase has the new capability surface (including `host_visible_tmpfs`), nothing else can opt into capability-gated behavior. Small (~50 LOC + tests), uncontroversial once placement is decided.
**Delivers:** `RuntimeCapabilities` (struct or const fields; see Open Questions); five new flags + `host_visible_tmpfs`; existing two flags migrated; honest capabilities for Docker/Podman/AppleContainer; Sbx variant added to `ContainerRuntimeName` enum (no impl yet).
**Addresses:** REQ-RT-01 (partial), REQ-RT-03; PROJECT.md scope correction for REQ-PLATFORM-01.

### Phase 2: SbxRuntime skeleton + pure argv builders (sbx-runtime)
**Rationale:** Argv builders are pure functions, fully unit-testable without sbx installed. Validates command shape before any subprocess work. Can land in parallel with P3 and P5-cosmetic once P1 is in.
**Delivers:** `src/containers/sbx/mod.rs` skeleton implementing `ContainerRuntimeInterface` with capability constants and stub subprocess methods; `argv.rs` pure builders for create/exec/ports; `is_available` + `capabilities()` returning sbx constants; unit tests on argv builders.

### Phase 3: container_config.rs capability gating + workspace path conformance
**Rationale:** The "bigger than it looks" phase. `compute_volume_paths` currently remaps everything to `/workspace/<name>`. When `!supports_arbitrary_volume_paths`, BOTH `working_dir` AND every `VolumeMount.container_path` need consistent rewriting before leaving `build_container_config`. Worth its own phase because it touches a cross-runtime function with the biggest regression surface.
**Delivers:** Capability-gated rewrites in `container_config.rs` for path conformance, anonymous-volume drop with warning, `ensure_image` skip in `instance.rs`; Settings UI fields gain `requires_capability` gate per PITFALLS C7; cross-product unit test (every FieldKey × every ContainerRuntimeName) asserts editability matches capability declaration.
**Addresses:** REQ-RT-04, REQ-RT-06.

### Phase 4: Embedded kit + materialization (kit)
**Rationale:** Independent of P2/P3 once the trait is stable. Develops in parallel.
**Delivers:** `containers/sbx-kit/spec.yaml` (`kind: mixin`, `schemaVersion: "1"`); `install.sh` agent installer; `files/` status-hook shims (writing to workspace-relative path per C1 fix); `.gitignore` for `.aoe-hooks/`; `network.allowedDomains` declaring required provider domains; `src/containers/sbx/kit.rs` materializer; content-hashed cache key; kit-lint unit test.
**Addresses:** REQ-KIT-01..04, DF-01, DF-02.

### Phase 5: SbxRuntime full integration — subprocess, port publish, parsing, lifecycle
**Rationale:** Critical-path phase. Joins P2's argv builders, P3's gated config, and P4's materialized kit. First place real `sbx` is exercised. Allocate the most validation budget here.
**Delivers:** Real subprocess in `create_container` (with readiness probe per C4 + first-exec retry), `publish_ports` per-port loop with typed errors per C3, `parse_sbx_ls_json` with `#[serde(default)]` defensiveness, `start_container` no-op + capability flag, `stop_container`/`remove`, `exec_command` argv with proper TTY flags, `get_version` parser, composite `is_daemon_running` per C8, version min-check; `#[ignore]`-gated integration tests; manual-test plan for C12 (sleep/wake) and disk usage.
**Addresses:** REQ-RT-02, REQ-RT-05, REQ-LIFE-01.

### Phase 6: TUI settings + cross-machine integration
**Rationale:** Mostly mechanical TUI wiring per AGENTS.md, plus end-to-end verification that cockpit/web/cross-machine work transparently with sbx.
**Delivers:** `Sbx` variant in TUI runtime select per AGENTS.md (FieldKey, build_*_fields, apply/clear, profile override merge); end-to-end test creating an sbx session through web/cockpit/cross-machine paths; sleep/wake handler scaffolding per C12; blocked-requests cockpit pane per C10.
**Addresses:** REQ-RT-01, REQ-INTEGRATION-01.

### Phase 7: Documentation + xtask gen-docs
**Rationale:** Independent of code; needs P5/P6 for screenshots and accurate UX flow.
**Delivers:** `docs/sandbox/sbx.md` user guide covering `sbx login`, `sbx policy set-default`, `sbx secret set -g <service>` per agent; platform support matrix; disk-usage warning; debug log location; upgrade path (do NOT recommend `sbx reset`); kit format experimental warning. `xtask gen-docs` keeps `docs/cli/reference.md` in sync.
**Addresses:** REQ-DOCS-01.

### Phase Ordering Rationale

- **P1 first, blocking everything else** — capability surface is the architectural keystone.
- **P3 is its own phase** (not folded into P2) because workspace-path conformance has the biggest cross-runtime regression surface.
- **P5 is the longest single phase** — first place real `sbx` is exercised. More validation budget than its LOC implies.
- **P7 last** — docs need actual UX to describe and screenshot.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 4 (kit):** sbx kit spec is officially experimental; recommend `/gsd-research-phase` to lock in `spec.yaml` schema fields, declare provider-domain allowlists per agent, validate `proxyManaged` semantics. Record `sbx ls --json` and `sbx ports --json` samples against a live install.
- **Phase 5 (sbx-runtime full integration):** Multiple unverified behaviors; TTY flag semantics, `sbx ports` lifecycle on stop/restart, exact stderr substrings for typed errors. Recommend `/gsd-research-phase` early.

Phases with standard patterns (skip research-phase):
- **Phase 1, 2, 6, 7** — established codebase patterns / mechanical wiring.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All deps verified; sbx CLI surface verified; only uncertainty is `sbx ls --json` / `sbx ports --json` exact schemas |
| Features | HIGH | Trait surface is in-repo; sbx capabilities cross-checked; anti-feature rationale grounded in PROJECT.md |
| Architecture | HIGH | Every claim traced to source files with line numbers |
| Pitfalls | MEDIUM-HIGH | sbx is officially experimental; CLI surface and policy/login behavior documented; kit cache invalidation behavior not |

**Overall confidence:** HIGH for design decisions; MEDIUM for the specific behaviors at the sbx CLI boundary.

### Gaps to Address

These need to be flagged at `/gsd-transition` because they touch PROJECT.md as currently written:

1. **PROJECT.md REQ-PLATFORM-01 is overstated.** Lists "macOS arm64/x86_64, Linux x86_64/arm64". sbx ships binaries only for `darwin-arm64` and `linux-amd64`. **User decision needed:** amend the requirement OR commit to graceful degradation (sbx option not selectable on unsupported platforms; other runtimes continue to work). Recommend the former.
2. **PROJECT.md cites `bundled_sounds/` as the embedding precedent; it is not.** `src/sound/bundled.rs` confirms sounds are downloaded from a hardcoded GitHub raw URL on first use, not embedded in the binary. The actual embedding precedent is `rust-embed` (`StaticAssets` in `src/server/mod.rs:48-50`). Update the Key Decision in PROJECT.md.
3. **`host_visible_tmpfs` capability flag is not in PROJECT.md REQ-RT-03.** REQ-RT-03 lists five capability flags; PITFALLS C1's fix requires a sixth.

### Open Questions (researchers disagreed; roadmapper to pick)

1. **Capability-flag placement.** ARCHITECTURE recommends `fn capabilities(&self) -> RuntimeCapabilities` on the trait, returning a struct (mockable; cohesive; future-proof). PITFALLS C6/C7 recommend plain const fields on `RuntimeBase` plus a sbx-side const (compile-time exhaustive-update enforcement; matches existing precedent exactly). Both real trade-offs:
   - Trait method wins on **mockability** and **uniformity** across `RuntimeHandle::Cli` and `RuntimeHandle::Sbx`.
   - Const fields win on **compile-time enforcement** (adding a flag forces every backend to declare a value).
   - Hybrid: trait method that returns the struct, but the struct fields are populated from `RuntimeBase` consts.
2. **Embedding crate.** STACK picks `include_dir 0.7.4` (single-line API, no extra weight). ARCHITECTURE picks `rust-embed 8.11.0` (already in deps under `serve` feature; reuses existing `StaticAssets` precedent). Either works.

## Sources

### Primary (HIGH confidence; verified directly)

**Codebase:**
- `src/containers/container_interface.rs`, `runtime.rs`, `runtime_base.rs:22-25`, `mod.rs`
- `src/session/config.rs:679-741`, `profile_config.rs:138-194`, `instance.rs:1228-1283`
- `src/session/container_config.rs:630-735` (the C2 rewrite target), `:993-1006` (the C1 pattern)
- `src/sound/bundled.rs` — NEGATIVE evidence: bundled_sounds is downloaded, not embedded
- `src/server/mod.rs:25-50` — `rust-embed` precedent
- `src/tui/settings/fields.rs:921-1112`, `input.rs:1816-1820, 2104-2108, 724-727`
- `src/hooks/status_file.rs`, `src/agents.rs`
- `Cargo.toml:120` — `rust-embed` currently feature-gated under `serve`
- `.planning/PROJECT.md`, `.planning/sbx-cli-reference.md`, `.planning/codebase/*.md`

**External (HIGH):**
- docker/sbx-releases v0.29.0, v0.28.0, v0.25.0 GitHub releases
- docs.docker.com/ai/sandboxes/{install, get-started, customize/kits, architecture, troubleshooting, usage}
- docs.docker.com/reference/cli/sbx/
- github.com/docker/sbx-kits-contrib
- crates.io API for `include_dir`, `rust-embed`, `duct`, `xshell`

### Secondary (MEDIUM)
- containerd/nerdctl command reference (prior art)
- Firecracker design docs
- Andrew Lock practitioner blog (cold-start performance, worktree-creation conflict)

### Tertiary (LOW; needs validation at implementation time)
- `sbx ls --json` and `sbx ports --json` exact schemas — NOT formally documented; record live samples at start of Phase 5
- Exact stderr substrings for typed error matching — capture during Phase 5
- `sbx ports` lifecycle behavior on stop/daemon restart — undocumented; design for reconcile-on-attach
- TTY/PTY semantic differences between `sbx exec` and `docker exec` — capture in Phase 5

---
*Research completed: 2026-05-15*
*Ready for roadmap: yes (after PROJECT.md gaps are addressed)*
