# Stack Research

**Domain:** New container runtime backend (sbx) for an existing Rust CLI/TUI tool
**Researched:** 2026-05-15
**Confidence:** HIGH

This is a brownfield, additive milestone. The aoe stack is already mapped at `.planning/codebase/STACK.md`; this document only covers what's needed to add the `sbx` backend. The default answer for nearly every question is "use what aoe already has" — new dependencies have to clear a real bar.

## Recommended Stack

### Core Technologies (all already in aoe)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `std::process::Command` | std | Shelling out to `sbx` for short-lived ops (`ls`, `create`, `stop`, `rm`, `version`, `ports --publish`, `ports --json`) | Exactly matches the existing `RuntimeBase` pattern in `src/containers/runtime_base.rs:62`. The `ContainerRuntimeInterface` is sync by design; introducing async only inside the sbx backend would either force ad-hoc `block_on` calls or fragment the trait. Sync `Command` is appropriate here because each invocation is sub-second and the trait is called from blocking contexts (TUI tick, supervisor loop). |
| `tokio::process::Command` | via `tokio 1.50` | Long-lived `sbx exec -it` PTY-style attachment, only if/when sbx exec is invoked from a non-PTY async path | Already used elsewhere in aoe (`src/cockpit/runner.rs`, `src/cockpit/acp_client.rs`, `src/server/tunnel.rs`). For `aoe`'s primary attachment path, the existing tmux pane runs `sbx exec -it ...` as a foreground command — the runtime layer only constructs the argv string via `exec_command()`, so no async process plumbing is needed in the sbx backend itself. |
| `serde` + `serde_json` | `1.0` | Deserializing `sbx ls --json`, `sbx ports --json`, `sbx template ls --json` outputs into typed structs | Already in deps; matches the AppleContainer JSON inspection pattern in `src/containers/runtime.rs:131`. Define narrow `#[derive(Deserialize)]` structs scoped to the sbx module — do NOT share with Docker/Podman types. The sbx schemas evolve independently. |
| `serde_yaml` | `0.9` | Authoring the embedded kit `spec.yaml` — read at build/test time only; the file ships as-is | Already in deps. The runtime never re-parses the spec; sbx does. We only use serde_yaml to validate the spec in unit tests. |
| `anyhow` + `thiserror` | `1.0` / `2.0` | sbx-specific errors slot into the existing `DockerError` enum (`src/containers/error.rs`) with new variants like `SbxKitMaterializationFailed`, `SbxPortPublishFailed` | Already the project pattern; no new error library needed. |
| `tracing` | `0.1` | Same `containers.runtime`/`containers.image` targets used by Docker/Podman/AppleContainer | Already in deps; consistent telemetry. |
| `dirs` | `6.0` | Locating the kit cache dir under aoe's existing app data dir (`~/.agent-of-empires/` on macOS, `~/.config/agent-of-empires/` on Linux) | Already used everywhere in aoe (`src/session/mod.rs:81`). Do NOT introduce `etcetera` — see "What NOT to Use." |
| `sha2` | `0.11` | Computing a build-time hash of the embedded kit bytes for cache invalidation when `aoe` upgrades ships a new kit | Already in deps for auth tokens. Reuse rather than introducing a new hasher. |
| `tempfile` | `3.14` | Atomic kit materialization (write to temp dir, rename into place) so a crash mid-write doesn't leave a half-written cache | Already in deps. |

### Embedding the Kit Assets

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **`include_dir` 0.7.4** | `0.7.4` | Embed the entire `kits/aoe/` directory tree (`spec.yaml` + `files/...`) into the `aoe` binary, walk it at runtime to materialize files into the cache dir | RECOMMENDED for the sbx kit. A kit is a directory tree with arbitrary nesting (`files/home/...`, `files/workspace/...`); `include_dir` preserves structure with `include_dir!("$CARGO_MANIFEST_DIR/kits/aoe")` and exposes `Dir::extract(&path)` plus per-file iteration in one line. Last published 2024-06-17 but stable; the API is essentially feature-complete for this use case. MSRV 1.64. |
| `rust-embed` 8.11.0 | `8.11.0` | (Already in deps under `serve`) Could embed the kit instead — `#[derive(Embed)] #[folder = "kits/aoe/"]` | Acceptable alternative but pulls more weight: `rust-embed` is designed around HTTP serving (mime guessing, axum/actix integration features) and the embed-by-derive-on-a-struct API is awkward for arbitrary tree extraction. The `kit` is a TUI/CLI concern that ships in the default (non-`serve`) feature set, and pulling `rust-embed` into the default feature set would expand the dependency surface for TUI-only builds. Keep `rust-embed` scoped to `serve` (web frontend). |
| `include_bytes!` / `include_str!` | std | Embed individual files | NOT recommended — a kit is a directory tree of unknown size (5-15 files across `spec.yaml`, status hooks for Claude/Codex/Gemini/Qwen/Cursor, install scripts). Maintaining an explicit per-file `include_bytes!` list duplicates filenames in two places (filesystem + Rust code) and breaks when a hook is added/renamed. |
| build.rs codegen | std | Walk the kit dir at build time, generate a Rust module with embedded bytes | NOT recommended — `include_dir` already does this with one macro. A bespoke build.rs only wins if you need transformation (templating, minification), which the kit does not. |

**Recommendation:** Add `include_dir = "0.7"` to default deps. Use `static AOE_KIT: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/kits/aoe");` and call `AOE_KIT.extract(&kit_cache_dir)?` after a hash check determines the cache is stale.

### Cache Directory

| Approach | Why |
|----------|-----|
| **Subdirectory under `get_app_dir()`** — `<app-dir>/kits/aoe/<short-hash>/` | RECOMMENDED. Reuses aoe's existing `get_app_dir()` helper (`src/session/mod.rs:73`), inherits the dev/release namespace split (`agent-of-empires-dev` vs `agent-of-empires`) for free, and avoids introducing a parallel "cache vs config vs data" dir distinction the rest of aoe doesn't make. The `<short-hash>` segment (first 8 hex chars of SHA-256 over the embedded kit bytes) provides per-version isolation: a new aoe build writes to a new subdir, the old one becomes orphaned and can be GC'd lazily. |
| `dirs::cache_dir()` (`~/Library/Caches/...` on macOS, `$XDG_CACHE_HOME` on Linux) | NOT recommended. aoe deliberately keeps everything (config, profiles, sessions, sounds, tunnel state) under one app-data root. Splitting kit cache into `~/Library/Caches/` would break the dev/release isolation pattern, complicate `aoe reset`, and confuse users grepping `~/.agent-of-empires/`. |
| Per-sandbox materialization (write into the workspace tmp dir on each `sbx create`) | NOT recommended. Kits are immutable per aoe version; materializing per-create wastes IO and leaks aoe internals into user workspace tmp dirs. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo xtask check-skill` (existing) | Pattern to mimic | Existing `xtask` already validates the OpenClaw skill spec at CI time. Add a sibling `cargo xtask check-kit` that parses `kits/aoe/spec.yaml` with `serde_yaml` and asserts required fields are present. Cheap insurance against typos in the embedded kit. |
| Unit tests for argv builders | Match the `RuntimeBase` test style | `src/containers/runtime_base.rs:340+` shows the established pattern: build args for a `ContainerConfig`, assert the resulting `Vec<String>` contains/excludes the right tokens. Mirror it for the sbx argv builder (`sbx create shell --kit ... --template ... --name ... <workspace> [<workspace>:ro]...`) and the post-create `sbx ports --publish` orchestration. |
| `#[ignore]` integration tests | Match the Docker test style | `src/containers/runtime.rs:286+` shows the pattern: tests probe `is_available()` + `is_daemon_running()` and skip if absent. CI then never blocks on a missing runtime. Apply identically to a `SbxRuntime` test module. |

## Installation

No new build-time tooling needed. Cargo edits only:

```toml
# Cargo.toml — added under [dependencies], NOT gated behind serve
include_dir = "0.7"
```

That's the entire dependency delta. `serde_json`, `serde_yaml`, `dirs`, `sha2`, `tempfile`, `tracing`, `anyhow`, `thiserror`, and `tokio` are already present.

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `std::process::Command` (sync) for sbx CLI calls | `tokio::process::Command` | If the `ContainerRuntimeInterface` trait is migrated to `async fn` end-to-end across all four backends. Out of scope for this milestone — would force changes to every TUI/cockpit/supervisor call site. Revisit only if a future milestone introduces long-running runtime ops (e.g., streaming `sbx ls` watch endpoints). |
| `std::process::Command` raw | `duct` `1.1.1` | Useful when you need pipelines (`cmd!("a") | cmd!("b")`), reliable signal handling for child trees, or want stderr captured by default. None of these apply to the sbx backend: each call is a single process, errors are surfaced via exit code + stderr (already handled by the existing `Output` parsing pattern), and there are no pipelines. Adding `duct` would create asymmetry with Docker/Podman/AppleContainer for no measurable benefit. |
| `std::process::Command` raw | `xshell` `0.2.7` (or `0.3.0-pre.2`) | Designed for build scripts and dev-tooling DSLs (`cmd!(sh, "...")`); appropriate for `xtask/` if it grows shell-out logic, but a poor fit for production library code. The pre-release status of 0.3.0 and the macro-heavy ergonomics push it out of scope for runtime use. |
| `include_dir` for kit assets | `rust-embed` | Use rust-embed if the kit ever needs to be served via HTTP from `aoe serve` (it doesn't — the kit is for sbx, not the dashboard) OR if we standardize all aoe binary embedding on rust-embed. Currently aoe only uses rust-embed for `web/dist/` under the `serve` feature, so adding kit assets there would require either pulling rust-embed into default deps (heavier) or duplicating embedding behavior across two crates. |
| Cache under `get_app_dir()` | `dirs::cache_dir()` (XDG cache) | Use cache_dir if/when aoe adopts the "config vs cache vs data" XDG split across the board. That would be a separate, opinionated migration affecting profiles, sessions, sounds, tunnel state — all of which currently colocate under one root. Don't introduce the split for kits alone. |
| `dirs` 6.0 | `etcetera` 0.11.0 | etcetera offers a more opinionated XDG-compliant API and is widely used (20M+ recent downloads). Switching aoe wholesale would require re-validating all 12+ existing `dirs::home_dir()` / `dirs::config_dir()` call sites. Not worth touching for this milestone. |
| Hand-written `Deserialize` structs for sbx JSON | `schemars`-generated types from a published JSON Schema | sbx does not publish a JSON Schema for `sbx ls --json` output. Hand-written types pinned to the observed shape are appropriate. Keep them narrow (only the fields aoe consumes) so unknown-field additions don't break parsing. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **`bollard` or any Docker SDK** | sbx is not Docker. The whole `containers/` abstraction is intentionally CLI-shelled, and bollard wouldn't help here regardless. | Continue shelling out to the `sbx` binary. |
| **A new error crate** (`color-eyre`, `miette`, etc.) | aoe already standardizes on `anyhow` + `thiserror`. Adding sbx-specific richer errors adds noise for the user. | Extend `DockerError` enum with `Sbx*` variants. |
| **`serde_yml`** (the soft-fork that replaced `serde_yaml`) | `serde_yaml` 0.9 is in deps and works for our read-once test-time validation of the embedded spec. Switching forks is a churn-only change. | `serde_yaml` 0.9. |
| **`xshell` for runtime sbx calls** | Shell-DSL ergonomics are wrong for production library code; argv-list construction is what we want, not a string-templating shell. | `std::process::Command` with the `Vec<String>` argv pattern already in `RuntimeBase::build_create_args`. |
| **`once_cell` / `lazy_static` for the embedded kit** | `include_dir!()` produces a `&'static Dir`, no extra wrapper needed. | The macro's output directly. |
| **`build.rs` for kit codegen** | `include_dir` already handles directory walks at compile time. A custom build script means more compile-time complexity, more chances for non-determinism in builds, and a maintenance surface for nothing. | `include_dir!()`. |
| **A new `cache` crate** (`cached`, `moka`) | Kit cache is a one-shot directory extraction with a hash check — not a key/value cache with eviction. | A simple `<dir>/<hash>/` layout + `Path::exists()` check. |
| **`async-process` / `command-group`** | aoe's runtime layer is sync; mixing async-process here would either deadlock or require runtime spawning from sync context. | `std::process::Command`. |
| **A wrapper crate around `sbx`** (none exists, but resist writing one) | The 4 existing runtimes intentionally each have their own ~200-line impl using shared `RuntimeBase`. A "sbx-rs" wrapper would introduce one more abstraction layer for a single consumer. | Inline `SbxRuntime` impl matching the Docker/Podman/AppleContainer pattern. |

## Stack Patterns by Variant

**If kit grows beyond ~50 files or 1MB:**
- Reconsider `include_dir` (it does a per-file `include_bytes!` under the hood, which can balloon compile times)
- At that point, switch to `rust-embed` with the `compression` feature, or extract kits to a separate sub-crate compiled less often
- Today's expected kit size (single `spec.yaml` + per-agent status hooks) is well within `include_dir`'s sweet spot (~5 KB total)

**If we ever need to ship multiple kits (e.g., a "minimal" and a "full" variant):**
- `include_dir!()` accepts any path — `include_dir!("$CARGO_MANIFEST_DIR/kits/minimal")` and `include_dir!("$CARGO_MANIFEST_DIR/kits/full")` side-by-side
- A `KitId` enum picks which `Dir<'_>` to materialize
- No tooling change needed

**If `sbx` adds a structured streaming API (e.g., a Unix socket protocol):**
- Out of scope; revisit only if the CLI shell-out becomes a real bottleneck
- Today's per-call overhead (~10-50ms cold) is dwarfed by sandbox startup time (multi-second microVM boot)

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `include_dir 0.7` | rustc ≥ 1.64; works with edition 2021 / rust-version 1.85 | No tokio interaction; pure compile-time embedding. Last release 2024-06-17 — old but stable; the crate is essentially feature-complete and used by 10K+ downstream crates. Single maintainer (Michael-F-Bryan); flag if maintenance signal degrades, but no active issues block our use. |
| `serde_json 1.0` + custom `Deserialize` | sbx 0.25.0 → 0.29.0 (tested transitively via release notes review) | sbx JSON output for `ls`, `ports`, `template ls`, `policy log` is documented as `--json` flag on each. Schema is not formally pinned by Docker, so use `#[serde(default)]` + narrow field selection so additive changes don't break parsing. **Verify the actual `sbx ls --json` and `sbx ports --json` schemas at implementation time** — official docs do not include a sample (verified by direct fetch); test-record one against a live sbx install during the implementation phase and pin the deserializers to that. |
| `dirs 6.0` (already in aoe) | All target platforms | Already cross-platform-validated in aoe; no change. |
| `tokio 1.50` (already in aoe) | n/a for sbx backend | Backend is sync; no version concern. |

### sbx CLI Version Baseline

| Item | Value | Source |
|------|-------|--------|
| **Latest stable** | `v0.29.0` (released 2026-05-13) | [docker/sbx-releases v0.29.0](https://github.com/docker/sbx-releases/releases/tag/v0.29.0) |
| **Recommended minimum** | `v0.28.0` (released 2026-04-27) | First release with first-class `kits` (`--kit` flag promoted out of hidden status; community kit repo `sbx-kits-contrib` opened). v0.28 also introduced `sbx cp`, host SSH agent forwarding, and the `tini` init shim — all of which downstream features (and likely future REQ items) depend on. |
| **Hard floor** | `v0.25.0` (released 2026-04-13) | First release with `--cpus` flag on `create`/`run`; needed if `SandboxConfig.cpu_limit` is ever surfaced. Also where reliable daemon stop via PID file landed. |
| **Detection** | Parse `sbx version` stdout | sbx's `version` command outputs a multi-line block including the semver. Parse the first `Version: ` line (or fall back to a regex `\bv?(\d+\.\d+\.\d+)\b`). Compare with the `semver` crate ONLY if you adopt it elsewhere; for now, a cheap string `split('.')` + integer compare is sufficient — keeps zero new deps. |
| **Behavior on too-old sbx** | Surface a one-line warning at runtime selection time (`"sbx 0.27.0 detected; aoe requires ≥ 0.28.0 for kit support"`) and prevent runtime selection in Settings | Mirrors the existing `is_daemon_running()` health check pattern. Do NOT silently fall back to a different runtime. |
| **Pinning** | None at build time | aoe shells out at runtime; sbx is the user's responsibility to install. Same posture as Docker/Podman/AppleContainer. |
| **Kits status caveat** | Docker explicitly marks kits as "experimental" with format subject to change | Document this in user-facing docs (REQ-DOCS-01). Pin the embedded `spec.yaml`'s `schemaVersion: "1"` and add a CI check that fails loudly if `sbx` changes the schema in a future release. |

### Cross-Platform Considerations for sbx

**This is the biggest risk surface for the milestone.** sbx's official platform matrix is narrower than aoe's.

| Platform | aoe supports? | sbx supports? | Action |
|----------|---------------|---------------|--------|
| **macOS arm64** (Apple Silicon) | Yes | Yes (only macOS arch) — requires macOS Tahoe (26)+ | Primary target. |
| **macOS x86_64** (Intel) | Yes | **NO** — release assets confirm: only `DockerSandboxes-darwin-arm64.*` exists in the v0.29.0 release; no `darwin-amd64` artifact | aoe's `is_available()` for sbx will return false on Intel Macs naturally (binary won't be installed). REQ-PLATFORM-01 needs to be amended: mark Intel macOS as unsupported for the sbx runtime specifically. Other runtimes (Docker/Podman) continue to work there. |
| **Linux x86_64** (amd64) | Yes | Yes — Ubuntu 24.04+ with KVM; release ships `.deb` (24.04, 25.10, 26.04) and `.rpm` (RockyLinux 8) | Primary target. |
| **Linux arm64** (aarch64) | Yes | **NO official support** as of v0.29.0 — release assets confirm: only `linux-amd64` artifacts; no `linux-arm64`. Community reports suggest some workarounds exist but break across upgrades | aoe's `is_available()` returns false → user picks a different runtime. Document this clearly. REQ-PLATFORM-01 needs amending. |
| **Windows** | aoe does NOT support Windows | sbx supports Windows 11 amd64 | Irrelevant — aoe's process module has only `macos.rs` + `linux.rs`. |

**Verified by:** direct fetch of the v0.29.0 release manifest (`gh api repos/docker/sbx-releases/releases/tags/v0.29.0`) and Docker's [install docs](https://docs.docker.com/ai/sandboxes/install/).

**Implication for the roadmap:** the sbx backend in practice covers (macOS arm64) ∪ (Linux x86_64). PROJECT.md REQ-PLATFORM-01 ("macOS arm64/x86_64, Linux x86_64/arm64") should be revised in a future requirements pass — flag this for `/gsd-transition` after Phase 0/1.

**Implication for code:** no new platform-specific shims are needed inside the sbx backend itself. The existing `is_available()` check handles "sbx not installed for your arch" cleanly; add a single `tracing::warn!` at startup if the configured runtime is sbx and `is_available()` returns false on a known-unsupported arch, so users get a hint to switch runtimes.

## Sources

- [docker/sbx-releases v0.29.0 GitHub release](https://github.com/docker/sbx-releases/releases/tag/v0.29.0) — verified release date, asset list (architectures shipped), changelog (HIGH)
- [docker/sbx-releases v0.28.0 release notes](https://github.com/docker/sbx-releases/releases/tag/v0.28.0) — confirmed `kits` first-class baseline (HIGH)
- [docker/sbx-releases v0.25.0 release notes](https://github.com/docker/sbx-releases/releases/tag/v0.25.0) — confirmed `--cpus` baseline (HIGH)
- [Docker Sandboxes install docs](https://docs.docker.com/ai/sandboxes/install/) — verified platform matrix (macOS arm64 only; Linux amd64 only Ubuntu 24.04+) (HIGH)
- [sbx CLI reference](https://docs.docker.com/reference/cli/sbx/) — full command surface; saved locally at `.planning/sbx-cli-reference.md` (HIGH)
- [Docker Sandboxes Kits docs](https://docs.docker.com/ai/sandboxes/customize/kits/) — kit `spec.yaml` schema (commands.install / startup / initFiles, network, environment, credentials sections; `files/home/`, `files/workspace/` layout); marked experimental (HIGH)
- crates.io API — verified current published versions and MSRVs:
  - `include_dir 0.7.4` (last published 2024-06-17, MSRV 1.64) (HIGH)
  - `rust-embed 8.11.0` (last published 2026-01-14, MSRV 1.70) (HIGH)
  - `duct 1.1.1` (last published 2025-11-09) (HIGH)
  - `xshell 0.2.7` stable / `0.3.0-pre.2` pre (last published 2024-12-23) (HIGH)
  - `etcetera 0.11.0` (last published 2025-10-28) (HIGH)
- aoe codebase (Cargo.toml, `src/containers/runtime_base.rs`, `src/containers/runtime.rs`, `src/session/mod.rs`, `src/server/mod.rs`, `src/sound/bundled.rs`) — verified existing patterns and dep set as of branch `sbx-sandbox-backend` (HIGH)
- `sbx ls --json` / `sbx ports --json` exact schemas — NOT formally documented by Docker; recommend recording a live sample during implementation Phase 1 and pinning the deserializers to it (LOW — flag for verification)

---
*Stack research for: aoe sbx container-runtime backend (additive milestone)*
*Researched: 2026-05-15*
