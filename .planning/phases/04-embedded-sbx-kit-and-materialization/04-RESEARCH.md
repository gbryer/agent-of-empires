# Phase 4: Embedded sbx Kit and Materialization - Research

**Researched:** 2026-05-16
**Domain:** Stdlib-only kit embedding + content-addressed materialization + idempotent install-verb linter for sbx kit `spec.yaml`
**Confidence:** HIGH on the locked decisions in CONTEXT.md (verified against codebase + Docker Sandboxes docs); MEDIUM on the `credentials.sources` ssh-agent schema (undocumented publicly); HIGH on Rust atomic-rename semantics (verified against `std::fs::rename` docs + Linux/macOS man pages).

## Summary

CONTEXT.md locks 17 decisions (D-01..D-17) covering: stdlib `include_str!`/`include_bytes!` embedding, per-agent materialized layout `<app_dir>/sbx-kit/aoe-<ver>-<hash>/<agent>/`, workspace-relative `${WORKDIR}/.aoe-hooks/<id>/status` hook shim path, `RuntimeCapabilities::supports_arbitrary_volume_paths` dispatch (no new 6th flag), 30-day stale-dir GC via `tokio::task::spawn_blocking`, and a `cargo xtask check-kit` CI guard. This research validates those locks against authoritative sources, surfaces three implementation specifics that planners must internalize, and flags one `[ASSUMED]` schema claim that needs a smoke-test or doc check before commit.

The three key implementation specifics:
1. **`std::fs::rename` does NOT reliably return `ErrorKind::AlreadyExists`** when the target dir exists and is non-empty. Linux returns `ENOTEMPTY` (mapped to `ErrorKind::DirectoryNotEmpty` only since Rust 1.83 — and even then not as a guaranteed contract), macOS varies, and the stable `ErrorKind` mapping is in flux. **Use post-rename `target.exists()` as the authoritative signal**, exactly as CONTEXT.md `<specifics>` warns. The atomic-rename pattern is already in use at `src/logging.rs:447-468` (`persist_runtime_filter`) — Phase 4 follows that idiom.
2. **`commands.startup` in sbx kits is an array, not a shell string.** Verified against the live `code-server` and `openclaw` kits in `docker/sbx-kits-contrib`. Idempotency guards still work (`["sh", "-c", "[ -f marker ] || curl ..."]`) but the lint must understand this — naked-verb detection runs on `command[2]` when `command[0..2] == ["sh", "-c"]`, not on the whole array.
3. **`${WORKDIR}` substitution is documented inside `initFiles[*].content` only.** Verified via WebFetch of the live kit reference. The schema does not document `${WORKDIR}` inside `initFiles[*].path` nor inside `commands.startup[*].command`. The `code-server` kit relies on `${WORKDIR}` inside `content:` and on a separate `commands.startup` line that exec's the script written by `initFiles` — the hook shim must follow the same pattern: `initFiles` writes a small shim script with `${WORKDIR}` baked into its content, and the agent's settings.json points at the shim's absolute path (which is NOT `${WORKDIR}`-substituted).

**Primary recommendation:** Plan 04-01 ships the static kit assets (`src/containers/sbx_kit/{spec.yaml, hook-shim.sh, gitignore-aoe-hooks}`) + `KitMaterializer::ensure` with atomic-rename + cache key + `tokio::task::spawn_blocking` GC. Plan 04-02 ships `sbx_hook_status_dir(project_path, instance_id)` + the StatusPoller dispatch site + `cargo xtask check-kit` + CI wiring. Wave-0 has zero gaps (every test framework + tempfile pattern already in tree).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Stdlib embedding of kit bytes | Build-time (compiler) | — | `include_str!` / `include_bytes!` are compile-time intrinsics — no runtime ownership, no new crate. CONTEXT.md D-locked. |
| Cache key derivation (sha256 over embedded bytes + aoe version + agent name) | Library: `src/containers/sbx/kit.rs` | — | Pure function over compile-time constants — no I/O, no platform code. |
| Lazy materialization (write tmp dir, atomic rename) | Library: `src/containers/sbx/kit.rs` | Filesystem (POSIX rename / NTFS MoveFileEx) | Single-call-site contract per CONTEXT.md D-11; `SbxRuntime::create_container` (Phase 5) is the only invoker. |
| 30-day stale-dir GC | Async runtime: `tokio::task::spawn_blocking` | Library: `src/containers/sbx/kit.rs` | Background fire-and-forget per CONTEXT.md D-14/D-15; pattern matches `src/server/api/system.rs` and `src/tui/mod.rs:50` migrations. |
| Workspace-relative hook status path | Library: `src/hooks/status_file.rs` | — | New sibling free function per CONTEXT.md D-02; same module as the existing `hook_status_dir(instance_id)`. |
| Runtime-aware hook-path dispatch | Caller (StatusPoller) | Library: `src/containers/container_interface.rs::RuntimeCapabilities` | Single new dispatch site reading `caps.supports_arbitrary_volume_paths` per CONTEXT.md D-03; no method-on-Instance refactor. |
| Schema validation (xtask check-kit) | Build/CI: `xtask/src/main.rs` | Library: `serde_yaml` (already in deps) | Parallels existing `CheckSkill` xtask subcommand per CONTEXT.md D-16. |
| Status hook write at runtime (inside VM) | Embedded shim file (`src/containers/sbx_kit/hook-shim.sh`) | sbx kit `initFiles` substitution of `${WORKDIR}` | Writing happens inside the microVM; aoe never executes the shim directly. Host poller reads workspace-relative output. |

## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01 — Status-hook shim path inside VM:** `initFiles` writes per-agent settings.json with hook command of shape:
```
mkdir -p ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID && printf STATUS > ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status
```
`${WORKDIR}` is sbx's documented kit template substitution applied at sandbox-start. `$AOE_INSTANCE_ID` is an env var aoe passes per `sbx exec` (Phase 2's `build_exec_args` already emits `-e KEY=VAL` per `EnvEntry::Literal`).

**D-02 — Host-side new free function:** `sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf` in `src/hooks/status_file.rs` returning `<project_path>/.aoe-hooks/<instance_id>/`. Existing `hook_status_dir(instance_id)` stays unchanged for Docker/Podman/AppleContainer. NO rename, NO method-on-Instance refactor, NO new capability flag.

**D-03 — Single new dispatch site:** StatusPoller / status-detection consults the resolved runtime via `caps.supports_arbitrary_volume_paths` and calls `hook_status_dir(...)` or `sbx_hook_status_dir(...)`. Same dispatch shape for `cleanup_hook_status_dir`.

**D-04 — No new 6th capability flag.** The constraint coincides exactly with `!supports_arbitrary_volume_paths`.

**D-05 — `spec.yaml` declares `credentials.sources` for ssh-agent forwarding.** One block; restores the SSH parity that Phase 3 dropped.

**D-06 — `spec.yaml` declares `environment.proxyManaged`** listing known provider env vars (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, plus per-agent standards). Host-side `sbx secret set -g <service>` is documented in Phase 7.

**D-07 — Gitconfig identity, agent-config dirs beyond hook shim, GCP creds, home-seed files: DEFERRED to Phase 5/6.** Phase 3 already silently dropped these for sbx. Phase 4 does NOT attempt to restore them. Trade-off accepted.

**D-08 — Materialized cache key includes the agent name:** `<app_dir>/sbx-kit/aoe-<aoe_version>-<spec_hash>/<agent>/`. Composition: `env!("CARGO_PKG_VERSION")` + SHA256 over embedded kit bytes + agent name.

**D-09 — Per-agent dir layout:** static `spec.yaml` (identical across agents) plus a generated `install.sh` whose body is the chosen agent's `install_hint` from `src/agents.rs`. `KitMaterializer::ensure(agent_name) -> Result<PathBuf>` reads via `crate::agents::get_agent(agent_name).install_hint`, escapes/wraps it in a shell harness, writes to `<materialized>/install.sh`, returns the kit dir for `sbx create shell --kit <path>`.

**D-10 — Agents without a workable headless install path** (Kiro via Docker desktop, settl via Homebrew Cask) get a kit that fails-loud at install-hook time. v1 targets: Claude Code, Codex, OpenCode, Gemini, Cursor CLI, droid, pi, Qwen Code.

**D-11 — Lazy on first use.** `KitMaterializer::ensure(agent)` called from `SbxRuntime::create_container` (Phase 5 wires the call site; Phase 4 ships the materializer). Non-sbx users never hit this code path.

**D-12 — Concurrency-safe via stdlib only:** materialize into `<final_path>.tmp.<pid>.<nanos>/` and atomic-rename to `<final_path>/` once written. Concurrent racers either (a) hit cache-hit fast path on second-and-later calls, or (b) one wins the rename and the other discards its tmp dir. No `flock`, no global lock.

**D-13 — Materialization writes are idempotent per (aoe_version, spec_hash, agent).** Byte-for-byte determinism. No timestamps, PIDs, hostnames in the dir contents.

**D-14 — GC folded into `KitMaterializer::ensure` on cache miss.** Scan `<app_dir>/sbx-kit/` siblings; `rm_rf` any `aoe-*` with top-level dir mtime older than 30 days. `tracing::info!`; never propagate errors.

**D-15 — GC runs at most once per materialization** (not per startup, not per cache-hit). Wraps in `tokio::task::spawn_blocking` so the rmdir tree walk doesn't block session create.

**D-16 — `xtask check-kit` parses embedded `spec.yaml` via `serde_yaml`** (already in dep tree), asserts `schemaVersion == "1"`, asserts required fields (`kind`, `name`, `commands.install`, `commands.startup`), and lints `commands.startup[*].command` for naked install verbs without an idempotency guard. CI runs `cargo xtask check-kit` in the existing CI workflow.

**D-17 — `check-kit` parses source files at `src/containers/sbx_kit/` directly,** not the materialized form. Generated `install.sh` content from `src/agents.rs` is covered by separate unit tests.

### Claude's Discretion

- Exact module location for `KitMaterializer`: new `src/containers/sbx/kit.rs` is the obvious slot.
- `include_str!` vs `include_bytes!` for `spec.yaml`. `include_str!` fine because spec.yaml is text; hash via `.as_bytes()` either way.
- Exact subset of agent install commands packaged into `install.sh` templates (which agents are v1, which fail-loud). REQUIREMENTS.md doesn't enumerate; planner picks based on `install_hint` shape per agent.
- Whether per-agent dir contains a single `install.sh` (current plan) vs `spec.yaml` with `commands.install` inlined.
- Hook-shim file format inside `initFiles` (one combined settings.json per hook-supporting agent vs per-agent files).
- Whether `ensure_kit_materialized` returns the per-agent dir or a tuple `(kit_dir, agent_install_marker)`.

### Deferred Ideas (OUT OF SCOPE)

- Gitconfig identity (`user.name` / `user.email`) for sbx sessions — Phase 5/6.
- Agent config dirs beyond hook shim (`~/.claude/` excluding settings.json, `~/.local/share/opencode/`, `~/.config/git/`) — Phase 5/6.
- GCP creds (`~/.config/gcloud/`) and home-seed files (`~/.claude.json`, `~/.sandbox-gitconfig`) — Phase 5/6.
- Hook-shim relocation for Docker/Podman/AppleContainer — out of phase scope.
- 6th `host_visible_tmpfs` capability flag — declined; covered by `!supports_arbitrary_volume_paths`.
- Kit-shipped `network.allowedDomains` — declined per C10; deny wins host-side, kit allow-lists give false confidence.
- `sbx template save` snapshotting — v2 TPL-01.
- Per-session `sbx ports --publish` orchestration — Phase 5 (RT-05).
- Real `pull_image Err(NotSupported)` semantics — Phase 5.

## Project Constraints (from CLAUDE.md / AGENTS.md)

- `cargo fmt` + `cargo clippy` clean. Address clippy warnings unless explicitly justified.
- **No dead code.** Never `#[allow(dead_code)]`; never add fields/functions nothing reads. If unused, remove.
- **No emdashes or `--` separators in docs/comments.** Use commas, semicolons, rephrase. (Applies to prose I author, not auto-generated content.)
- Rust naming: `snake_case` modules/functions, `CamelCase` types, `SCREAMING_SNAKE_CASE` constants.
- OS-specific logic in `src/process/{macos,linux}.rs` — Phase 4 adds **zero** `cfg(target_os = ...)` branches.
- Don't preserve backwards compatibility by default; this is a brownfield insert with an explicit Phase 1 capability-flag breaking change already accepted.
- Comments: explain non-obvious "why"; skip section headers and comments that restate code.
- Unit tests in-module (`#[cfg(test)]`) for pure logic; integration tests in `tests/*.rs`.
- Tests must be deterministic and clean up after themselves (use `tempfile` for temp dirs).
- Settings TUI: Phase 4 adds **no** new settings fields (SET-01 wiring is Phase 6).
- CLI docs (`docs/cli/reference.md`) auto-generated by `cargo xtask gen-docs`; Phase 4 adds no clap surface.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| KIT-01 | Source files at `src/containers/sbx_kit/` embedded via stdlib `include_str!`/`include_bytes!`; materialized to `<app_dir>/sbx-kit/aoe-<aoe_version>-<spec_hash>/` on first use. | Standard Stack table verifies `sha2 = "0.11"`, `serde_yaml = "0.9"`, `tempfile = "3.14"` already in `Cargo.toml`; `env!("CARGO_PKG_VERSION")` is a stdlib intrinsic; `include_str!` / `include_bytes!` are stdlib intrinsics. Cache-key composition pattern locked by CONTEXT.md `<specifics>`. |
| KIT-02 | Invocation shape: `sbx create shell --kit <materialized-path> --template <user's image>`. | Phase 2's `build_create_args(name, image, kit_path: Option<&Path>, &ContainerConfig)` already accepts `kit_path` — Phase 4 hands it `Some(materialized_agent_dir)`. Verified via `src/containers/sbx/argv.rs:24-60`. |
| KIT-03 | `install` hook installs the agent binary using each agent's documented install command from a generated mapping. | `src/agents.rs::AgentDef::install_hint` already populated for every agent (lines 188, 204, 220, 241, 281, 300, 316, 333, 349, 365, 391, 412, 434). Planner picks v1 subset per CONTEXT.md D-10. |
| KIT-04 | `initFiles` drops hook shims for hook-supporting agents (Claude Code, Cursor CLI, Gemini, Qwen Code); shims write to `<workspace>/.aoe-hooks/<id>/status`; `.gitignore` for `.aoe-hooks/` dropped. | `hook_config: Some(...)` set for: claude (line 178), gemini (line 253), cursor (line 293), qwen (line 424) in `src/agents.rs`. `${WORKDIR}` substitution confirmed in `initFiles[*].content` via WebFetch of live kit docs. |
| KIT-05 | Cache content-addressed by `<aoe_version>-<spec_hash>` (SHA256). 30-day stale-dir GC at startup (background, log-only). | `sha2 = "0.11"` already in deps; `tokio::task::spawn_blocking` pattern in `src/tui/mod.rs:50`, `src/server/api/system.rs:24+111+271+364`. |
| KIT-06 | `spec.yaml` pinned to `schemaVersion: "1"`; `cargo xtask check-kit` (or equivalent) CI guard. | `serde_yaml = "0.9"` already in deps. `CheckSkill` xtask precedent at `xtask/src/main.rs:21,28`; new `CheckKit` variant follows the same shape. CI `skill` job at `.github/workflows/ci.yml:82-93` is the template. |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sha2` | `0.11` | SHA256 over embedded kit bytes for content-addressed cache key | Already in `Cargo.toml:95` [VERIFIED: codebase grep]; used at `src/server/push.rs:748`, `src/cockpit/node.rs:286`, `src/session/repo_config.rs:9`, `src/session/capture.rs:1506`. **No new dep.** |
| `serde_yaml` | `0.9` | Parse embedded `spec.yaml` in `cargo xtask check-kit` | Already in `Cargo.toml:34` [VERIFIED: codebase grep]; used in `src/hooks/mod.rs` for Hermes config. **No new dep.** |
| `tempfile` | `3.14` | Dev-dep for unit-test temp `<app_dir>` substitutes | Already in `Cargo.toml:104` [VERIFIED: codebase grep]; used in `src/session/storage.rs`, `src/containers/sbx/mod.rs` tests. **No new dep.** |
| `tokio` (current dep) | (existing) | `tokio::task::spawn_blocking` for 30-day GC walk | Pattern confirmed at `src/tui/mod.rs:50` (`migrations::run_migrations`) and `src/server/api/system.rs:24,111,271,364` [VERIFIED: codebase grep]. Session create path is already on the tokio runtime. |
| `tracing` | `0.1` | `tracing::info!` for GC log; `tracing::warn!` for non-fatal errors during materialize | Already in `Cargo.toml:63` [VERIFIED: codebase grep]; universal logging throughout codebase. |
| `anyhow` | (existing) | Error propagation from `KitMaterializer::ensure` matching existing trait API | Used throughout `src/containers/`; matches Phase 3's `anyhow::Result<ContainerConfig>` return signature [CITED: 03-CONTEXT.md D-07]. |
| stdlib `std::fs::{create_dir_all, write, rename}` | std | Atomic-rename materializer recipe | Pattern in use at `src/logging.rs:447-468` (`persist_runtime_filter`), `src/update/install.rs:292`, `src/server/push.rs:171,306`, `src/cockpit/worker_registry.rs:226`, `src/cockpit/node.rs:279`, `src/session/mod.rs:245` [VERIFIED: codebase grep]. |
| stdlib `include_str!` / `include_bytes!` | std | Compile-time embedding of kit assets | Compile-time intrinsics; no runtime cost; no dep [CITED: PROJECT.md Key Decisions]. |
| stdlib `env!("CARGO_PKG_VERSION")` | std | Compile-time version string for cache key | Standard Cargo intrinsic [VERIFIED: docs.rs/cargo]. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` | (existing) | New `CheckKit` variant in `xtask/src/main.rs` `Commands` enum | When adding the xtask subcommand; mirror `CheckSkill` at `xtask/src/main.rs:21,59`. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| stdlib `include_str!`/`include_bytes!` | `include_dir` crate, `rust-embed` (already in `Cargo.toml:120` for `web/dist/`) | Rejected per PROJECT.md Key Decisions / REQUIREMENTS.md / `feedback_dependencies_and_gates` user preference. `rust-embed` exists ONLY for `web/dist/` (many files, directory walk); for a 2-3 file kit, `include_str!` is sufficient and zero-marginal-dep. |
| `flock` / `fd-lock` for materializer concurrency | `fs2`, `fd-lock`, advisory-lock crates | Rejected per CONTEXT.md D-12: atomic-rename plus post-rename `target.exists()` is sufficient. Locks add a global resource that doesn't survive crashes. |
| `tokio::task::spawn_blocking` for GC | `std::thread::spawn` | Spawn_blocking is the established codebase pattern (12+ call sites verified). Returns a `JoinHandle` we can drop, integrates with runtime back-pressure. Used at `src/tui/mod.rs:50` for migrations which is the closest analog. |
| Hand-rolled cache invalidation (version sentinel file) | Path-as-cache-key | Rejected per PITFALLS.md C5: version-string comparison bugs ("1.10.0" < "1.9.0") leak stale kits. Content-hash in the path itself = full invalidation with no comparison logic. |

**Installation:** **No new dependencies.** Every library above is already in `Cargo.toml`.

**Version verification:** Confirmed via `grep -n "sha2\|serde_yaml\|tempfile\|tracing = " Cargo.toml` [VERIFIED: codebase grep on 2026-05-16].

## Package Legitimacy Audit

> Phase 4 installs **no new packages**. Every dependency listed in Standard Stack is already in the `Cargo.toml` of `agent-of-empires` and has been in use across the codebase for prior phases.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| (no new packages this phase) | — | — | — | — | — | — |

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

(slopcheck protocol not run because the phase adds no new install operations. The existing deps are inherited from the locked Phase 1-3 stack.)

## Architecture Patterns

### System Architecture Diagram

```
┌────────────────────────────────────────────────────────────────────────────┐
│ AOE PROCESS (host)                                                         │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ Compile-time:                                                    │      │
│  │   include_str!("../sbx_kit/spec.yaml")    ──┐                    │      │
│  │   include_str!("../sbx_kit/hook-shim.sh") ──┼─► EMBEDDED_KIT_BYTES│     │
│  │   include_str!("../sbx_kit/gitignore-aoe")──┘                    │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│                          │                                                 │
│                          │ sha2::Sha256::digest()                          │
│                          ▼                                                 │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ cache_dir(app_dir, agent) =                                      │      │
│  │   <app_dir>/sbx-kit/aoe-<CARGO_PKG_VERSION>-<hash16>/<agent>/    │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│                          │                                                 │
│  Phase 5 caller:         ▼                                                 │
│  SbxRuntime::create_container(name, image, agent, ...)                     │
│       │                                                                    │
│       ▼                                                                    │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ KitMaterializer::ensure(agent) ─► target = cache_dir(...)        │      │
│  │  ├─► target.exists() ─► YES ─► return Ok(target)  [fast path]    │      │
│  │  └─► NO:                                                         │      │
│  │     1. tmp = target.with_extension("tmp.<pid>.<nanos>")          │      │
│  │     2. create_dir_all(tmp)                                       │      │
│  │     3. write_embedded_files(tmp)                                 │      │
│  │     4. write_install_sh(tmp, agent.install_hint)                 │      │
│  │     5. fs::rename(tmp, target)                                   │      │
│  │     6. match outcome OR target.exists() ─► clean up tmp          │      │
│  │     7. tokio::task::spawn_blocking(|| gc_stale(parent, 30 days)) │      │
│  │     8. return Ok(target)                                         │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│                          │                                                 │
│                          ▼                                                 │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ build_create_args(name, image, Some(&target), &config) (Phase 2) │      │
│  │   └─► argv: [sbx, create, shell, --name, ..., --kit, <target>,   │      │
│  │              --template, <image>, <workspace>:ro, ...]           │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│                          │                                                 │
└──────────────────────────┼─────────────────────────────────────────────────┘
                           │ Phase 5 spawns
                           ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ SBX MICROVM (per sandbox)                                                  │
│                                                                            │
│  Sandbox start (sbx kit handler):                                          │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ commands.install (once, root):                                   │      │
│  │   sh -c "<install_hint from src/agents.rs>"                      │      │
│  │                                                                  │      │
│  │ initFiles (template-substituted at start):                       │      │
│  │   /home/agent/.claude/settings.json                              │      │
│  │     content: { hooks: { PreToolUse: [{                           │      │
│  │       command: "sh -c '...mkdir ${WORKDIR}/.aoe-hooks/...'"      │      │
│  │     }] } }                                                       │      │
│  │   <WORKDIR>/.gitignore.aoe-hooks                                 │      │
│  │     content: ".aoe-hooks/\n"                                     │      │
│  │                                                                  │      │
│  │ commands.startup (every start, agent user uid 1000):             │      │
│  │   ["sh","-c","command -v <agent> >/dev/null || \                 │      │
│  │                printf 'agent install incomplete\\n'>&2; exit 1"] │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│                                                                            │
│  Agent runtime: hook fires ─► writes ${WORKDIR}/.aoe-hooks/<id>/status     │
│  (Workspace is mounted at SAME absolute path host-side and VM-side,        │
│   so the host StatusPoller reads the same file path that the VM wrote.)   │
└──────────────────────────┬─────────────────────────────────────────────────┘
                           │ filesystem passthrough (same abs path)
                           ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ AOE PROCESS (host) — StatusPoller                                          │
│                                                                            │
│  for instance in active_instances:                                         │
│    caps = get_container_runtime().capabilities()                           │
│    status_dir = if caps.supports_arbitrary_volume_paths {                  │
│        hook_status_dir(&instance.id)            // /tmp/aoe-hooks/<id>     │
│    } else {                                                                │
│        sbx_hook_status_dir(                                                │
│            Path::new(&instance.project_path),                              │
│            &instance.id)                        // <project>/.aoe-hooks/<id>│
│    };                                                                      │
│    let status = read_status_file(status_dir.join("status"))                │
└────────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
src/
├── containers/
│   ├── sbx/
│   │   ├── mod.rs              # (Phase 2) SbxRuntime peer struct
│   │   ├── argv.rs             # (Phase 2) pure builders, kit_path already plumbed
│   │   └── kit.rs              # (Phase 4 NEW) KitMaterializer + cache_dir + GC
│   └── sbx_kit/                # (Phase 4 NEW) embedded asset source
│       ├── spec.yaml           # schemaVersion: "1" + base kit (sourced via include_str!)
│       ├── hook-shim.sh        # workspace-rooted hook command body (sourced via include_str!)
│       └── gitignore-aoe-hooks # one-liner ".aoe-hooks/" (sourced via include_str!)
├── hooks/
│   └── status_file.rs          # (Phase 4 EXTEND) add sbx_hook_status_dir() sibling fn
├── session/
│   └── poller.rs               # (Phase 4 EXTEND) single new dispatch site via caps
├── agents.rs                   # (Phase 4 READ ONLY) install_hint per agent → install.sh body
xtask/
└── src/
    └── main.rs                 # (Phase 4 EXTEND) add CheckKit variant + check_kit() fn
.github/
└── workflows/
    └── ci.yml                  # (Phase 4 EXTEND) add `cargo xtask check-kit` job after skill check
```

### Pattern 1: Atomic-rename materializer (stdlib only)

**What:** Write to a temp dir, then `std::fs::rename` to the final path. On `EEXIST`/`ENOTEMPTY`, discard the temp dir — the racer that won is authoritative.

**When to use:** Any cache-write where multiple processes might race on the same target path and the contents are byte-identical across writers.

**Example (already in codebase):**
```rust
// Source: src/logging.rs:447-468 [VERIFIED: read directly]
pub fn persist_runtime_filter(directive: &str, app_dir: &std::path::Path) {
    if let Err(e) = std::fs::create_dir_all(app_dir) {
        tracing::warn!(target: "log.runtime", error = %e, "could not create app dir for runtime_filter");
        return;
    }
    let path = runtime_filter_path(app_dir);
    let tmp = app_dir.join("runtime_filter.tmp");
    if let Err(e) = std::fs::write(&tmp, directive) {
        tracing::warn!(target: "log.runtime", error = %e, "could not write runtime_filter.tmp");
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        tracing::warn!(target: "log.runtime", error = %e, "could not rename runtime_filter");
    }
}
```

**Adaptation for `KitMaterializer::ensure`:** Same shape, but renaming a **directory** rather than a file, AND on error must check `target.exists()` because `ErrorKind::AlreadyExists` is not a reliable signal across platforms (see Pitfall 1).

### Pattern 2: `tokio::task::spawn_blocking` for fire-and-forget I/O

**What:** Offload a blocking filesystem walk to the tokio blocking-thread pool so the calling async task isn't held up.

**When to use:** Any background sweep that doesn't need a result and shouldn't propagate errors. CONTEXT.md D-14/D-15 GC is the textbook case.

**Example (already in codebase):**
```rust
// Source: src/tui/mod.rs:50 [VERIFIED: read directly]
let migration_handle = tokio::task::spawn_blocking(migrations::run_migrations);
```

**Adaptation for GC:**
```rust
// In KitMaterializer::ensure, after successful rename:
let gc_root = app_dir.join("sbx-kit");
tokio::task::spawn_blocking(move || {
    if let Err(e) = gc_stale_kit_dirs(&gc_root, std::time::Duration::from_secs(30 * 24 * 3600)) {
        tracing::info!(target: "containers.sbx.kit",
            error = %e,
            "stale-kit GC sweep failed (non-fatal)");
    }
});
// Note: we DROP the JoinHandle. We never await it. This is intentional —
// session create returns to the caller while GC runs in the background.
```

The dropped JoinHandle means the task is detached. If it panics, tokio logs it via its default panic hook; the GC sweep itself never propagates anyhow up to `ensure`.

### Pattern 3: xtask subcommand for build-time invariant checking

**What:** New xtask `Commands` variant + `fn` whose body parses an embedded file and asserts invariants; CI runs `cargo xtask <name>`.

**When to use:** Any time the codebase has an artifact that must satisfy a structural contract that's cheaper to enforce at CI than at runtime.

**Example (already in codebase):**
```rust
// Source: xtask/src/main.rs:16-30 [VERIFIED: read directly]
#[derive(Subcommand)]
enum Commands {
    GenDocs,
    CheckSkill,
}

fn main() {
    let args = Xtask::parse();
    match args.command {
        Commands::GenDocs => generate_cli_docs(),
        Commands::CheckSkill => check_skill(),
    }
}
```

**Adaptation for `CheckKit`:**
```rust
#[derive(Subcommand)]
enum Commands {
    GenDocs,
    CheckSkill,
    CheckKit,  // NEW
}

fn check_kit() {
    let spec_src = include_str!("../../src/containers/sbx_kit/spec.yaml");
    let spec: serde_yaml::Value = serde_yaml::from_str(spec_src)
        .expect("src/containers/sbx_kit/spec.yaml: invalid YAML");
    assert_eq!(
        spec["schemaVersion"].as_str(),
        Some("1"),
        "schemaVersion must be the string \"1\""
    );
    assert!(spec["kind"].as_str().is_some(), "kind required");
    assert!(spec["name"].as_str().is_some(), "name required");
    assert!(
        spec["commands"]["install"].as_sequence().is_some(),
        "commands.install required"
    );
    let startup = spec["commands"]["startup"]
        .as_sequence()
        .expect("commands.startup required (may be empty array)");
    for cmd in startup {
        lint_startup_command_for_naked_install_verbs(cmd);
    }
    println!("check-kit: OK");
}
```

### Pattern 4: Workspace-rooted hook write via `${WORKDIR}` substitution

**What:** Embed `${WORKDIR}` in the `content:` of an `initFiles` block; sbx substitutes the workspace absolute host path at sandbox start. The VM and host see the same path.

**When to use:** Any time a file must be created at sandbox-start with the workspace path encoded.

**Example (verified live):**
```yaml
# Source: github.com/docker/sbx-kits-contrib/code-server/spec.yaml [VERIFIED: curl]
initFiles:
  - path: /home/agent/.local/bin/start-code-server.sh
    content: |
      exec code-server --bind-addr 0.0.0.0:8080 --auth none "${WORKDIR}"
    mode: "0755"
```

**Adaptation for hook shim:** Phase 4 uses the same shape, but the agent's `settings.json` is the file at a static absolute path (`/home/agent/.claude/settings.json`), and the **content** contains the `${WORKDIR}`-substituted command:

```yaml
initFiles:
  - path: /home/agent/.claude/settings.json
    mode: "0644"
    content: |
      {
        "hooks": {
          "PreToolUse": [{"hooks": [{"type":"command","command":"sh -c '[ -n \"$AOE_INSTANCE_ID\" ] || exit 0; mkdir -p ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID && printf running > ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status'"}]}],
          "Stop": [{"hooks": [{"type":"command","command":"sh -c '[ -n \"$AOE_INSTANCE_ID\" ] || exit 0; mkdir -p ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID && printf idle > ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status'"}]}]
        }
      }
```

Key constraints (verified):
- `${WORKDIR}` substitution **is documented inside `initFiles[*].content`** [VERIFIED: WebFetch of docs.docker.com/ai/sandboxes/customize/kits/].
- `${WORKDIR}` substitution **inside `initFiles[*].path` is NOT documented** — use static absolute paths there [ASSUMED: undocumented absence; if a future kit version supports this, we can simplify, but Phase 4 must not rely on it].
- `${WORKDIR}` substitution **inside `commands.startup[*].command` is NOT documented**, but the existing `code-server` example references the shim at a static path and the shim does the `${WORKDIR}` work — this is the right pattern for Phase 4.

### Anti-Patterns to Avoid

- **Using `ErrorKind::AlreadyExists` as the sole signal that the rename target already exists.** Use post-rename `target.exists()` as authoritative (see Pitfall 1).
- **Putting `${WORKDIR}` in `initFiles[*].path`.** Undocumented; may silently fail to substitute. Use a static absolute path for the file location; put `${WORKDIR}` only in `content:`.
- **Embedding install verbs in `commands.startup`** without an idempotency guard. The lint catches this (Pitfall 2). Install once in `commands.install`; in `startup` use existence checks.
- **Re-implementing kit caching with a version sentinel file.** Path-as-cache-key + content hash is simpler and correct (see Alternatives Considered).
- **Spawning the GC walk on `std::thread::spawn`.** Use `tokio::task::spawn_blocking` to integrate with the existing runtime back-pressure (matches `src/tui/mod.rs:50`).
- **Logging GC failures as `error!`.** Use `tracing::info!` — a stale kit dir that won't `rm -rf` is not a session-blocking event (CONTEXT.md D-14).
- **Calling `KitMaterializer::ensure` from `aoe` startup or from any non-sbx code path.** It's lazy-on-first-use; Phase 5 invokes it from `SbxRuntime::create_container` only.
- **Adding a 6th capability flag for "host-visible tmpfs."** Phase 1 already deferred this; CONTEXT.md D-04 explicitly declined. Use `!supports_arbitrary_volume_paths` for dispatch.
- **Hand-editing `docs/cli/reference.md`.** Phase 4 adds no clap surface; if a CLI flag changes, run `cargo xtask gen-docs`.
- **Using emdashes or `--` separators in source comments / doc strings.** Project rule.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Embed multiple small text files into the binary | A build script that reads files into a `static [u8]` array, or `rust-embed` for a 3-file kit | stdlib `include_str!` / `include_bytes!` | Zero deps, zero build-script, compile-time const, correct lifetimes; PROJECT.md decision. |
| Atomic-rename to a directory target | Lock files, advisory locks via `fcntl`, file-as-mutex | `std::fs::rename(tmp, target)` + post-rename `target.exists()` check | The POSIX rename + the cache-hit-on-second-call recheck is enough; locks add a global resource. CONTEXT.md D-12. |
| Hash content to derive a cache key | Hand-coded SHA, FNV, CRC | `sha2::Sha256::digest(bytes)` truncated to 16 hex chars (64 bits collision resistance) | Already in deps, audited, constant-time, deterministic. CONTEXT.md `<specifics>`. |
| Schedule a background sweep from an async task | `std::thread::spawn` and ignore the JoinHandle | `tokio::task::spawn_blocking(move || ...)` and drop the JoinHandle | Integrates with runtime's blocking thread pool, respects shutdown signals, matches `src/tui/mod.rs:50` precedent. |
| Parse YAML in xtask | Tokio + serde_yml + custom parser | `serde_yaml::from_str::<serde_yaml::Value>(s)` and index by string keys | Already in deps; `Value` indexing is enough for the assertions check-kit makes; no schema struct required for D-16's specific checks. |
| Lint shell commands for "did the author forget an idempotency guard around an install verb" | A full POSIX-shell parser | A small string-substring lint that recognizes the codebase's expected idiomatic guards (`[ -f`, `[ -x`, `command -v`, `which`, `test -e`) | Hadolint's DL3009/DL3008 do similar substring + AST work [CITED: github.com/hadolint/hadolint/wiki/DL3009]; for our embedded one-file kit the substring approach is sufficient and audit-friendly. |
| Walk a directory tree to delete stale dirs | Hand-roll `fs::read_dir` recursion with manual unlink | `std::fs::remove_dir_all` + a one-level `read_dir` of the parent to identify candidates | `remove_dir_all` handles the recursion correctly; we only need one-level enumeration to pick stale candidates by mtime. |

**Key insight:** Phase 4 is a discipline-test phase — every component has an obvious off-the-shelf solution, and the user's stated preference (per `feedback_dependencies_and_gates`) is to use stdlib over deps wherever stdlib suffices. Following that preference is the architectural value-add; resist the temptation to introduce `include_dir`, `fd-lock`, or any small "convenience" crate.

## Common Pitfalls

### Pitfall 1: `ErrorKind::AlreadyExists` from `fs::rename` is not the right signal

**What goes wrong:** Author writes `match fs::rename(tmp, target) { Err(e) if e.kind() == ErrorKind::AlreadyExists => ... }`. On Linux when the target is a non-empty directory, the actual error is `ENOTEMPTY` which Rust maps to `ErrorKind::DirectoryNotEmpty` (stable since Rust 1.83, but not guaranteed by the `fs::rename` contract — only a "more refined" mapping in current libstd). On older Rust or different platforms it may surface as `ErrorKind::Other`. macOS may use `ENOTEMPTY` or `EEXIST`. Windows uses `MoveFileEx` which behaves differently again. The `AlreadyExists` branch never fires, the code falls through to the generic `Err(e) => return Err(e.into())` arm, and the materializer surfaces a hard error instead of treating it as the cache-hit it actually is.

**Why it happens:** Rust's `ErrorKind` is intentionally not a stable contract for OS errors; the libstd docs explicitly warn that "the mapping is not exhaustive and may change between Rust versions." `rename` is a particularly tricky case because POSIX has three distinct error codes (`EEXIST`, `ENOTEMPTY`, `EBUSY` for already-mounted) that all mean "target unavailable."

**How to avoid:** Use post-rename `target.exists()` as the authoritative signal exactly as CONTEXT.md `<specifics>` recommends. Pattern:
```rust
match std::fs::rename(&tmp, &target) {
    Ok(()) => {}
    Err(_) if target.exists() => {
        // Another racer materialized the same content first. Use it.
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Err(e) => {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e.into());
    }
}
```

The `_` discard on the error kind is intentional — we don't want to take any policy decision from `ErrorKind` here; existence of the target is the only thing that matters.

**Warning signs:** Unit test "two concurrent materializers" intermittently fails with an `IO error: directory not empty` surfacing through `anyhow`; the same test on a single-threaded runner passes; CI green locally and flakes in concurrent CI matrix builds.

### Pitfall 2: `commands.startup` install verbs run on every restart

**What goes wrong:** Author puts `apt-get install foo` or `npm install -g bar` in `commands.startup` because "it worked the first time." Per kit docs, `startup` runs on **every sandbox start** as the agent user (uid 1000). On the second start, the package is already installed; `apt-get` either no-ops slowly, fails because the dpkg lock is held, or fails because there's no network (sbx policy may have changed). The sandbox starts in a degraded state.

**Why it happens:** The two hook phases look interchangeable in YAML. The constraint that startup must be idempotent is documented but easy to overlook.

**How to avoid:** `cargo xtask check-kit` (CONTEXT.md D-16) detects naked install verbs in `commands.startup[*].command` and fails CI. Detection runs on:
- `command: ["sh","-c","..."]` — extract `command[2]` (the actual shell string) and substring-scan.
- `command: "string-form"` — substring-scan the string directly.
- `command: ["bin", "arg1", "arg2"]` — substring-scan `command[0]` against verb names.

Naked-verb list (mirror Hadolint DL3008/DL3009 + npm/curl idioms): `apt-get`, `apt `, `dpkg`, `pip install`, `pip3 install`, `npm install`, `yarn add`, `curl `, `wget `.

Acceptable idempotency guard patterns (regex-or string-prefix recognition):
- Starts with `[ -f`, `[ -x`, `[ -e`, `[ -d`.
- Starts with `test `.
- Contains ` command -v ` followed by `||`.
- Contains ` which ` followed by `||`.
- Contains `&& ` (i.e., the verb is downstream of a conditional that the author asserts is sufficient — accept this as the author's intent, but log it as a `tracing::debug!` in `check_kit` so a human notices novel patterns).

**Warning signs:** Sandbox cold-start time on restart is similar to first creation rather than near-instant; `sbx exec <sandbox> claude --version` returns "command not found" on a restarted sandbox while the same sandbox started fine.

### Pitfall 3: Hook status file unwritable because workspace is `:ro` or git-ignored upstream

**What goes wrong:** The kit's `initFiles` plants `<workspace>/.gitignore.aoe-hooks` containing `.aoe-hooks/`. But if the user's workspace is `<repo>:ro` (read-only convenience mount), the in-VM hook's `mkdir -p ${WORKDIR}/.aoe-hooks/...` fails with EROFS. Or if the user's `.gitignore` doesn't include `.aoe-hooks/` (because our drop was overridden, deleted, or written to a different path), `.aoe-hooks/` shows up as dirty in `git status` inside agent sessions.

**Why it happens:** The hook shim writes inside the workspace, which crosses two ownership boundaries: the user's repo (gitignore policy) and the sbx mount (`:ro` vs `:rw`).

**How to avoid:**
1. The kit's `commands.install` (which runs once as root) writes `<WORKDIR>/.gitignore.aoe-hooks` only if no existing `.aoe-hooks/` line is found in the workspace's `.gitignore`. Implementation: a small `grep -qsE '^\.aoe-hooks/?$' ${WORKDIR}/.gitignore || (cat >> ${WORKDIR}/.gitignore <<EOF...\n.aoe-hooks/\nEOF)`. This is idempotent (the grep guard handles re-runs).
2. The primary workspace MUST be `:rw`; Phase 3's `compute_volume_paths` already ensures this for the workspace mount specifically (Phase 3 D-04 only drops convenience mounts; the primary workspace stays writable). Phase 4 confirms via a unit test.
3. The hook command itself uses `mkdir -p` rather than failing on existing dirs.

**Warning signs:** `git status` inside an sbx agent session shows `.aoe-hooks/` as untracked; agents commit the directory by mistake (this is a UX bug, not a correctness bug, but should be caught in a test that asserts the post-install gitignore content is correct).

### Pitfall 4: `kit-shipped allowedDomains` overrides global policy in surprising ways

**What goes wrong:** Author adds `network.allowedDomains: ["api.anthropic.com", ...]` thinking "this makes the kit work everywhere." User has `sbx policy set-default deny-all`; the global deny wins host-side and the kit's request is blocked. User blames aoe.

**Why it happens:** Two policy layers; deny dominates. Documented by PITFALLS.md C10.

**How to avoid:** Per CONTEXT.md `<deferred>`: **Phase 4 does NOT declare `network.allowedDomains` in `spec.yaml`.** Phase 7 docs say: "If your global policy is `deny-all`, run `sbx policy allow network -g api.<provider>` for each agent." This trades v1 first-run friction for predictable behavior; CONTEXT.md `<deferred>` accepts the trade.

**Warning signs:** Provider API calls in an sbx-backed session return "request blocked"; `sbx policy log <sandbox>` shows the provider domain denied; user has not run `sbx policy allow network`.

### Pitfall 5: `${WORKDIR}` substitution may not fire in undocumented positions

**What goes wrong:** Author writes `initFiles: [{path: "${WORKDIR}/.gitignore.aoe-hooks", content: "..."}]` thinking the path will be substituted. Docs only confirm substitution in `content:`; substitution in `path:` is undocumented. The file might be written literally at `/sandbox/${WORKDIR}/.gitignore.aoe-hooks` or fail with an invalid-path error.

**Why it happens:** Sbx kit docs say `${WORKDIR}` "expands to the workspace path" in `content:` but do not enumerate positions where substitution applies.

**How to avoid:** Use static absolute paths in `initFiles[*].path`; do `${WORKDIR}` work in `content:`. For the workspace gitignore drop, do it from `commands.install` (which has shell-string semantics and can use `${WORKDIR}` as an env var substituted at runtime by sh).

**Warning signs:** A first-run `sbx create` succeeds but `<workspace>/.gitignore.aoe-hooks` is missing or shows up at a literal `${WORKDIR}` path; check-kit doesn't catch this because spec.yaml looks valid at parse time.

### Pitfall 6: GC walk runs during `aoe` startup and blocks the TUI

**What goes wrong:** Author wires GC into `aoe` startup ("eager") instead of `KitMaterializer::ensure` ("lazy on cache miss") thinking "I want GC to run frequently." Every `aoe` startup pays the disk-walk cost of every aoe-X-Y dir in the cache. On a developer who's had aoe installed for 12 months across 6 aoe versions, this is dozens of dir walks at every startup.

**Why it happens:** "Run GC at startup" sounds reasonable in isolation; the cost compounds.

**How to avoid:** CONTEXT.md D-14/D-15 explicit: GC runs **once per materialization on cache miss**, not at startup, not on cache hit. Non-sbx users never trigger it. Sbx users on a steady state (cache-hit path) never trigger it. Only when a new kit hash materializes (aoe upgrade, kit content change) does GC fire, and even then it's `spawn_blocking` so it doesn't block the create.

**Warning signs:** TUI startup is observably slower for users with many sbx sessions; flame graph shows time in `gc_stale_kit_dirs`; `tracing` logs show GC firing on `aoe doctor` or any other code path that's not a real session create.

## Code Examples

Verified patterns from official sources and existing codebase.

### Example 1: Cache-key composition

```rust
// Source: derived from CONTEXT.md <specifics> + verified against codebase
// (sha2 usage at src/server/push.rs:748; env! intrinsic at src/main.rs)
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// Compile-time-embedded kit bytes (deterministic across aoe versions
// because input files are static). Place at the top of kit.rs.
const SPEC_YAML: &str = include_str!("../sbx_kit/spec.yaml");
const HOOK_SHIM: &str = include_str!("../sbx_kit/hook-shim.sh");
const GITIGNORE: &str = include_str!("../sbx_kit/gitignore-aoe-hooks");

fn embedded_kit_bytes() -> Vec<u8> {
    // Concatenate in stable order so spec_hash is deterministic.
    let mut bytes = Vec::with_capacity(SPEC_YAML.len() + HOOK_SHIM.len() + GITIGNORE.len());
    bytes.extend_from_slice(SPEC_YAML.as_bytes());
    bytes.extend_from_slice(HOOK_SHIM.as_bytes());
    bytes.extend_from_slice(GITIGNORE.as_bytes());
    bytes
}

fn cache_dir(app_dir: &Path, agent: &str) -> PathBuf {
    let hash = Sha256::digest(embedded_kit_bytes());
    let hash_hex = format!("{:x}", hash);
    app_dir
        .join("sbx-kit")
        .join(format!(
            "aoe-{}-{}",
            env!("CARGO_PKG_VERSION"),
            &hash_hex[..16]  // 64 bits of collision resistance; short paths
        ))
        .join(agent)
}
```

### Example 2: Atomic-rename materializer with authoritative-signal fallback

```rust
// Source: derived from src/logging.rs:447-468 pattern, adapted for
// directory rename + post-rename target.exists() per CONTEXT.md <specifics>
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn ensure(agent: &str) -> Result<PathBuf> {
    let app_dir = crate::session::get_app_dir()?;
    let target = cache_dir(&app_dir, agent);
    if target.exists() {
        return Ok(target);
    }

    // Build a tmp dir path uniquely scoped by pid + nanos so concurrent
    // racers do not collide on tmp itself.
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = target.with_file_name(format!(
        "{}.tmp.{}.{}",
        target.file_name().unwrap_or_default().to_string_lossy(),
        pid,
        nanos
    ));

    std::fs::create_dir_all(&tmp)
        .with_context(|| format!("create_dir_all({})", tmp.display()))?;
    write_embedded_files(&tmp)
        .with_context(|| format!("write_embedded_files({})", tmp.display()))?;
    write_install_sh(&tmp, agent)
        .with_context(|| format!("write_install_sh({}, {})", tmp.display(), agent))?;

    // ErrorKind from fs::rename is NOT reliable for "target exists / non-empty"
    // (Linux: ENOTEMPTY; macOS: ENOTEMPTY/EEXIST; Windows: varies).
    // Use post-rename target.exists() as authoritative signal.
    match std::fs::rename(&tmp, &target) {
        Ok(()) => {}
        Err(_) if target.exists() => {
            // Concurrent racer materialized first. Their content is byte-equal
            // by D-13 so just discard ours.
            let _ = std::fs::remove_dir_all(&tmp);
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(anyhow::Error::from(e).context(format!(
                "rename({} -> {})",
                tmp.display(),
                target.display()
            )));
        }
    }

    // Background GC: drop the JoinHandle so it runs detached.
    let gc_root = app_dir.join("sbx-kit");
    tokio::task::spawn_blocking(move || {
        if let Err(e) = gc_stale_kit_dirs(&gc_root, std::time::Duration::from_secs(30 * 24 * 3600))
        {
            tracing::info!(
                target: "containers.sbx.kit",
                error = %e,
                "stale-kit GC sweep skipped (non-fatal)"
            );
        }
    });

    Ok(target)
}
```

### Example 3: GC walk (one-level enumeration + age filter)

```rust
// Source: derived from std::fs idioms; uses fs::remove_dir_all which
// handles the recursion correctly.
use std::path::Path;
use std::time::{Duration, SystemTime};

fn gc_stale_kit_dirs(root: &Path, max_age: Duration) -> std::io::Result<()> {
    let now = SystemTime::now();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("aoe-") {
            continue;
        }
        // Tmp dirs from concurrent racers should not survive past the rename
        // window; still defensively skip them so a never-completed materialize
        // does not chew the dir mid-walk.
        if name_str.contains(".tmp.") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = metadata.modified()?;
        let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
        if age > max_age {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                tracing::info!(
                    target: "containers.sbx.kit",
                    path = %path.display(),
                    error = %e,
                    "could not remove stale kit dir (will retry next sweep)"
                );
            } else {
                tracing::info!(
                    target: "containers.sbx.kit",
                    path = %path.display(),
                    "removed stale kit dir"
                );
            }
        }
    }
    Ok(())
}
```

### Example 4: `sbx_hook_status_dir` sibling free function

```rust
// Source: derived from src/hooks/status_file.rs:13 (hook_status_dir)
// per CONTEXT.md D-02. Same module, sibling fn, no rename of the existing.

use std::path::{Path, PathBuf};

pub fn sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf {
    project_path.join(".aoe-hooks").join(instance_id)
}

// Existing hook_status_dir stays unchanged. Per D-03 the single dispatch
// point in StatusPoller picks between them based on capabilities.
```

### Example 5: StatusPoller dispatch (illustrative)

```rust
// Source: derived from CONTEXT.md D-03 / <specifics> illustration.
// Planner picks the exact site (likely a thin _for_instance shim adjacent
// to read_hook_status); shown here as inline dispatch for clarity.

let caps = crate::containers::get_container_runtime().capabilities();
let status_dir = if caps.supports_arbitrary_volume_paths {
    crate::hooks::hook_status_dir(&instance.id)
} else {
    crate::hooks::sbx_hook_status_dir(
        std::path::Path::new(&instance.project_path),
        &instance.id,
    )
};
let status_file = status_dir.join("status");
let status = std::fs::read_to_string(&status_file).ok();
```

### Example 6: `cargo xtask check-kit` lint regex for install verbs

```rust
// Source: derived from CONTEXT.md <specifics> + Hadolint DL3008/DL3009 patterns
// (citation: github.com/hadolint/hadolint/wiki/DL3009)

const NAKED_INSTALL_VERBS: &[&str] = &[
    "apt-get install", "apt install", "apt-get update", "apt update",
    "dpkg -i", "dpkg --install",
    "pip install", "pip3 install", "pip2 install",
    "npm install -g", "npm i -g",
    "yarn global add", "yarn add",
    "curl ", "wget ",
    "gem install",
];

const IDEMPOTENCY_GUARD_PREFIXES: &[&str] = &[
    "[ -f", "[ -x", "[ -e", "[ -d",
    "test -f", "test -x", "test -e", "test -d",
    "command -v", "which ",
];

fn extract_command_string(cmd: &serde_yaml::Value) -> Option<String> {
    // Form 1: `command: "string"`
    if let Some(s) = cmd["command"].as_str() {
        return Some(s.to_string());
    }
    // Form 2: `command: [array]` — if first elements are sh -c, inspect the rest.
    if let Some(arr) = cmd["command"].as_sequence() {
        let strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        if strs.len() >= 3 && strs[0] == "sh" && strs[1] == "-c" {
            return Some(strs[2..].join(" "));
        }
        return Some(strs.join(" "));
    }
    None
}

fn has_idempotency_guard(cmd: &str) -> bool {
    let trimmed = cmd.trim_start();
    IDEMPOTENCY_GUARD_PREFIXES
        .iter()
        .any(|p| trimmed.starts_with(p))
        // Also accept `&&` chaining and `||` short-circuit chained guards.
        || cmd.contains(" || ")
}

fn lint_startup_command_for_naked_install_verbs(cmd: &serde_yaml::Value) {
    let Some(s) = extract_command_string(cmd) else {
        return;
    };
    for verb in NAKED_INSTALL_VERBS {
        if s.contains(verb) && !has_idempotency_guard(&s) {
            panic!(
                "check-kit: naked install verb `{}` in commands.startup without idempotency guard:\n  {}",
                verb, s
            );
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Match on `ErrorKind::AlreadyExists` for rename collision | Match on post-rename `target.exists()` | Rust 1.83 added `DirectoryNotEmpty` to libstd but did not promise the mapping; production code should not rely on the kind | Authoritative-signal fallback is more portable across Rust versions and platforms |
| Eagerly extract all kit assets at app startup | Lazy materialization on first sbx-runtime use | sbx integration is opt-in per session; non-sbx users (the majority) pay zero materialization cost | Faster `aoe` startup for non-sbx users; first sbx session pays one-time materialization |
| Version-sentinel file for cache invalidation | Path-as-cache-key (`aoe-<ver>-<hash>`) | Version comparison bugs ("1.10.0" < "1.9.0" via string compare) leak stale kits; path-as-key eliminates the comparison step | No comparison logic to get wrong; full invalidation is implicit |
| `static` mutex / `flock` for concurrent materialize | Atomic rename + post-rename `exists()` check | Stdlib provides enough; locks add a global resource that doesn't survive crash | Simpler code, no new dep |
| Per-agent native sbx mapping (`sbx create claude`, `sbx create codex`) | Single `sbx create shell --kit <ours>` | PROJECT.md decision; matches user's Docker mental model | One image, one kit; agent install becomes a layer above the image, not a per-agent product surface |

**Deprecated/outdated:**
- Bind-mounting host `/tmp/aoe-hooks/<id>` into the sandbox: works for Docker/Podman/AppleContainer but NOT sbx (separate microVM kernel). Sbx-specific path is workspace-rooted per KIT-04.
- Hand-rolled kit version comparison: see above; path-as-cache-key is the current pattern.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `credentials.sources.<service>.agent: ssh-agent` shape (or any specific schema for ssh-agent forwarding in spec.yaml) | Standard Stack / D-05 | If the schema is different in the live sbx version, the kit fails to declare ssh-agent forwarding at all and SSH commit signing fails inside sbx sessions. Mitigation: smoke-test the kit by running `sbx create shell --kit <path>` against a live sbx install during plan-phase or as an integration test, OR omit `credentials.sources` from v1 and defer SSH to Phase 5/6. Documentation gap: docs.docker.com/ai/sandboxes/security/credentials does not show YAML schema; existing kits (openclaw) use `credentials.sources.<svc>.env:` for API keys only. |
| A2 | `${WORKDIR}` substitution does NOT apply in `initFiles[*].path` | Pattern 4 / Pitfall 5 | If substitution DOES apply, a strict static-path-only rule is unnecessary but harmless (the kit still works). If substitution does NOT apply (current assumption), and the author uses `${WORKDIR}` in `path:`, the file is written at a literal path. Authoritative claim: docs only document substitution in `content:`. |
| A3 | `commands.startup[*].command` accepts both string-form ("`command: "foo bar"`") and array-form ("`command: [sh, -c, ...]`") | Pattern 4 / lint pattern | Verified against live kits (openclaw uses array; code-server uses array). String-form is documented in CONTEXT.md `<specifics>` as legacy support but not seen in production kits. If the live schema only accepts array-form, the lint's string-form branch is dead code (no harm) but our written spec.yaml should use array-form exclusively for portability. |
| A4 | sbx mounts workspace at the exact same absolute host path inside the VM (required for the workspace-rooted hook shim to work) | Pattern 4 / KIT-04 | VERIFIED via WebFetch of docs.docker.com/ai/sandboxes/architecture/ ("Your workspace is mounted at the same absolute path as on your host"). Low risk. |
| A5 | Truncating SHA256 to 16 hex chars (64 bits) is sufficient collision resistance for a per-developer cache key | Cache-key composition | At 64 bits, accidental collision requires ~2^32 distinct kit versions before a birthday-collision is plausible. For a single-developer cache with low double-digit aoe versions, this is overkill. If we ever ship dynamic kits or per-user kit content, revisit. |
| A6 | Phase 4's kit ships `agent: shell` (not a per-agent kit kind) | Standard Stack | Confirmed by PROJECT.md and CONTEXT.md D-09 / KIT-02. Live `code-server` example is `kind: mixin extends: claude`; Phase 4's choice between `kind: agent` and `kind: mixin` is planner discretion within CONTEXT.md's Claude's Discretion. Recommendation: `kind: agent` with `agent.entrypoint.run: [shell]` (simpler, doesn't require an `extends:` target). |

**If A1 turns out wrong, planner should:** add a `checkpoint:human-verify` task before plan-02 commits the `credentials.sources` block; the human verifies the live schema by reading `sbx kit validate` output or by running an end-to-end smoke test. Alternative: drop `credentials.sources` from v1 and ship Phase 4 with API-key-only credentials, deferring SSH to Phase 5/6.

## Open Questions

1. **What is the live shape of `credentials.sources.<service>.agent` for ssh-agent forwarding?**
   - What we know: openclaw kit uses `credentials.sources.<service>.env: [VAR_NAME]` for API keys. The architecture docs say SSH agent forwarding "is used for Git operations over SSH and SSH-based commit signing" and that the host SSH agent socket is forwarded into the VM.
   - What's unclear: whether the kit declares the forwarding via `credentials.sources.<service>.agent: ssh` (analog to `env:`), via a top-level `credentials.ssh-agent: true`, or whether it's purely an opt-in by the sbx daemon (no spec.yaml declaration needed).
   - Recommendation: plan-phase smoke-tests via `sbx create shell --kit <minimal-kit>` against a live sbx install with `SSH_AUTH_SOCK` set; observe whether `ssh-add -l` inside the sandbox sees the host's keys. If yes-out-of-the-box, no `spec.yaml` declaration needed. If no, find the schema by inspecting `sbx kit validate` output or by checking the `claude-safe` agent kit's spec.yaml. Until resolved, ship without `credentials.sources` in the embedded kit.

2. **Does `cargo xtask check-kit` need to run on every push, or only on changes under `src/containers/sbx_kit/`?**
   - What we know: existing `cargo xtask check-skill` runs on every push (no path filter in `.github/workflows/ci.yml`).
   - What's unclear: optimization opportunity vs. consistency with skill check.
   - Recommendation: run on every push for consistency. The xtask itself is fast (sub-second YAML parse + a few string scans). Optimization is premature.

3. **What does "Claude Code, Codex, OpenCode, Gemini, Cursor CLI, droid, pi, Qwen Code" install correctly in a sbx sandbox without further tweaking?**
   - What we know from `src/agents.rs`: claude=`npm install -g @anthropic-ai/claude-code`; opencode=`curl -fsSL https://opencode.ai/install | bash`; codex=`npm install -g @openai/codex`; gemini=`npm install -g @google/gemini-cli`; cursor=`see https://docs.cursor.com/cli` (no executable); droid=`npm install -g droid`; pi=`npm install -g @mariozechner/pi-coding-agent`; qwen=`npm install -g @qwen-code/qwen-code`.
   - What's unclear: cursor has no executable install hint; settl uses `brew install --cask` (host-only); kiro uses `curl -fsSL https://cli.kiro.dev/install | bash` (likely works); hermes uses a `curl | bash` install (likely works); vibe uses `pip install mistral-vibe` (works if Python+pip present).
   - Recommendation: planner picks v1 subset:
     - **Definitely include (npm install -g):** claude, codex, gemini, droid, pi, qwen
     - **Include with confidence (curl install):** opencode, kiro, hermes
     - **Include if Python in base image:** vibe (pip install)
     - **Exclude (no headless install):** cursor (no executable), copilot (no install path documented), settl (Homebrew Cask, host-only)
   - Plan 04-01 builds a small dispatch table; agents not in the table get a fail-loud `install.sh` body that prints "agent <name> requires manual install in the sandbox image; configure it via the user's `--template <image>` setting" and exits 1.

4. **Should the embedded spec.yaml be a single file or a directory with multiple `initFiles` shipped as separate source files?**
   - What we know: CONTEXT.md `<specifics>` shows a single `spec.yaml` plus inline `initFiles` blocks; the code-server live example also inlines `initFiles[*].content`.
   - What's unclear: maintainability of larger settings.json blobs inside YAML.
   - Recommendation: single `spec.yaml` for v1 with inline `initFiles.content`. If a settings.json grows beyond ~30 lines, factor it out to a separate `src/containers/sbx_kit/settings-<agent>.json` file via `include_str!` and inject it into the spec.yaml at xtask-build time.

5. **What if a user's project_path is not writable when the hook tries to `mkdir -p <project_path>/.aoe-hooks/`?**
   - What we know: Phase 3 already enforces the primary workspace mount is `:rw` (only convenience mounts are dropped); the hook's `mkdir` is workspace-rooted.
   - What's unclear: edge case where user-supplied `extra_volumes` shadow the project workspace.
   - Recommendation: trust Phase 3 D-04 to keep the primary workspace writable; document the failure mode as a UX bug in Phase 7 (user observation: agent status stays "Idle" forever, log shows `EROFS` from the in-VM shim). Outside Phase 4 scope to detect/fix.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | Build, test, xtask | (host concern) | (project pinned) | — |
| `sbx` CLI | Phase 5/6 runtime; **not Phase 4** (Phase 4 is pure code + embedded assets) | n/a for phase 4 | — | — |
| `serde_yaml` (already in deps) | `cargo xtask check-kit` | ✓ | 0.9 (Cargo.toml:34) | — |
| `sha2` (already in deps) | Cache-key derivation | ✓ | 0.11 (Cargo.toml:95) | — |
| `tempfile` (dev-dep) | Unit tests for materializer | ✓ | 3.14 (Cargo.toml:104) | — |
| tokio runtime | `spawn_blocking` GC | ✓ (in deps) | (project pinned) | — |

**Missing dependencies with no fallback:** none — Phase 4 adds no new external dependencies.

**Missing dependencies with fallback:** none.

Phase 4 is implementable end-to-end without requiring `sbx` to be installed on the developer machine; every Phase 4 test path uses `tempfile` to fake `<app_dir>` and uses string assertions on the materialized artifacts.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` (libtest) + `tempfile = "3.14"` for temp dirs |
| Config file | none (`cargo test` defaults are fine for unit tests) |
| Quick run command | `cargo test --lib containers::sbx::kit` and `cargo test --test e2e_kit` (planner picks integration test name) |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KIT-01 | Embedded kit bytes match the source files (`include_str!` produces the same content). | unit | `cargo test --lib containers::sbx::kit::tests::embedded_bytes_match_source -- --exact` | Wave 0 |
| KIT-01 | `cache_dir(app_dir, agent)` returns a deterministic path that includes `aoe-<CARGO_PKG_VERSION>-<16hex>`. | unit | `cargo test --lib containers::sbx::kit::tests::cache_dir_path_shape -- --exact` | Wave 0 |
| KIT-01 | `KitMaterializer::ensure(agent)` writes the expected files into a fresh tmp app_dir. | unit | `cargo test --lib containers::sbx::kit::tests::ensure_creates_files_on_miss -- --exact` | Wave 0 |
| KIT-01 | Second call to `ensure` with the same agent / version / hash is a no-op (target exists fast path; no rewrite). | unit | `cargo test --lib containers::sbx::kit::tests::ensure_idempotent_on_hit -- --exact` | Wave 0 |
| KIT-01 | Mutating one byte in the embedded spec changes the cache dir name. | unit (round-trip with patched hash input) | `cargo test --lib containers::sbx::kit::tests::cache_dir_changes_on_content_byte_flip -- --exact` | Wave 0 |
| KIT-02 | Phase 2's `build_create_args` accepts the materialized agent dir as `kit_path: Some(...)`. | unit | `cargo test --lib containers::sbx::argv::tests::create_args_with_kit_path -- --exact` (already exists from Phase 2; add a new test that uses a `KitMaterializer`-produced path). | already exists; add one new |
| KIT-02 | `sbx create shell --kit <path> --template <image>` argv shape, verified by string assertion on argv vec. | unit | `cargo test --lib containers::sbx::argv::tests::create_args_shape -- --exact` | already exists from Phase 2 |
| KIT-03 | For each v1 agent (claude, codex, ...), `install.sh` body contains the agent's `install_hint` from `src/agents.rs`. | unit (matrix; one assert per agent) | `cargo test --lib containers::sbx::kit::tests::install_sh_contains_install_hint -- --exact` | Wave 0 |
| KIT-03 | For agents not in the v1 subset (cursor, settl, copilot), `install.sh` body fails loud with a clear error message. | unit | `cargo test --lib containers::sbx::kit::tests::install_sh_fails_loud_for_unsupported -- --exact` | Wave 0 |
| KIT-04 | Embedded `spec.yaml` `initFiles` writes hook shim with `${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status` substring. | unit | `cargo test --lib containers::sbx::kit::tests::spec_yaml_hook_shim_uses_workspace_path -- --exact` | Wave 0 |
| KIT-04 | Embedded `spec.yaml` `commands.install` writes `.aoe-hooks/` line into `<workspace>/.gitignore` idempotently. | unit (substring scan + idempotency-guard pattern check) | `cargo test --lib containers::sbx::kit::tests::spec_yaml_gitignore_drop_is_idempotent -- --exact` | Wave 0 |
| KIT-04 | `sbx_hook_status_dir(project_path, instance_id)` returns the expected path shape. | unit | `cargo test --lib hooks::status_file::tests::sbx_hook_status_dir_path -- --exact` | Wave 0 |
| KIT-04 | StatusPoller dispatch picks `sbx_hook_status_dir` when `!supports_arbitrary_volume_paths`, else `hook_status_dir`. | unit | `cargo test --lib session::poller::tests::status_dir_dispatch_via_caps -- --exact` (planner picks exact name) | Wave 0 |
| KIT-05 | Mutating one byte in any embedded kit file changes the cache path; previous dir is not affected on disk. | unit | `cargo test --lib containers::sbx::kit::tests::content_byte_flip_yields_new_dir_old_dir_preserved -- --exact` | Wave 0 |
| KIT-05 | `gc_stale_kit_dirs(root, 30 days)` removes only `aoe-*` dirs older than the threshold; ignores fresh dirs and `.tmp.*` dirs. | unit (mtime-controlled fixture) | `cargo test --lib containers::sbx::kit::tests::gc_removes_only_stale -- --exact` | Wave 0 |
| KIT-05 | Concurrent `ensure` racers (N=4) all succeed; final target dir has byte-equal content; tmp dirs are all cleaned. | unit (`std::thread::scope` with shared `tempfile::TempDir`) | `cargo test --lib containers::sbx::kit::tests::concurrent_ensure_all_succeed -- --exact` | Wave 0 |
| KIT-06 | `cargo xtask check-kit` exits 0 when `src/containers/sbx_kit/spec.yaml` is valid. | integration (xtask invocation via `std::process::Command`) | `cargo test --test xtask_check_kit -- --exact ok` | Wave 0 |
| KIT-06 | `cargo xtask check-kit` exits non-zero when `schemaVersion` is missing or not `"1"`. | integration | `cargo test --test xtask_check_kit -- --exact missing_schema_version` | Wave 0 |
| KIT-06 | `cargo xtask check-kit` exits non-zero on a naked install verb in `commands.startup` without idempotency guard. | integration (uses fixture spec file, NOT the embedded one) | `cargo test --test xtask_check_kit -- --exact naked_install_verb_fails` | Wave 0 |
| KIT-06 | `cargo xtask check-kit` exits 0 when an install verb in `commands.startup` is guarded by `command -v ... ||` or `[ -f ... ] ||`. | integration | `cargo test --test xtask_check_kit -- --exact guarded_install_verb_passes` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib containers::sbx::kit` (sub-second for the unit tests)
- **Per wave merge:** `cargo test` (full suite)
- **Phase gate:** Full suite green before `/gsd:verify-work`. CI also runs `cargo xtask check-kit` per the new CI job.

### Wave 0 Gaps

- [ ] `src/containers/sbx_kit/spec.yaml` — embedded asset (does not exist yet; Phase 4 creates it)
- [ ] `src/containers/sbx_kit/hook-shim.sh` — embedded hook command body (Phase 4 creates)
- [ ] `src/containers/sbx_kit/gitignore-aoe-hooks` — embedded `.gitignore` content (Phase 4 creates)
- [ ] `src/containers/sbx/kit.rs` — `KitMaterializer` module (Phase 4 creates)
- [ ] `xtask/src/main.rs` (extend) — `CheckKit` variant + `fn check_kit()`
- [ ] `tests/xtask_check_kit.rs` — integration test invoking `cargo xtask check-kit` against fixture specs under `tests/fixtures/sbx_kits/`
- [ ] `tests/fixtures/sbx_kits/{ok,missing-schema-version,naked-install-verb,guarded-install-verb}/spec.yaml` — fixture specs for the four xtask integration tests
- [ ] `.github/workflows/ci.yml` (extend) — add a `kit` job mirroring the existing `skill` job
- [ ] No framework install needed; libtest + tempfile + serde_yaml are all already in tree.

## Sources

### Primary (HIGH confidence)
- `.planning/phases/04-embedded-sbx-kit-and-materialization/04-CONTEXT.md` (this phase's locked decisions)
- `.planning/REQUIREMENTS.md` (KIT-01..KIT-06 requirement text)
- `.planning/PROJECT.md` § Key Decisions (stdlib-only embedding; single embedded kit; user-owned host-side state)
- `.planning/ROADMAP.md` § Phase 4 (goal + dependencies + 5 success criteria)
- `.planning/research/FEATURES.md` § "sbx Kits Ecosystem Reference" (lines 189-205, schema fields confirmed)
- `.planning/research/PITFALLS.md` § C1, C5, C9, C10, M5, N5 (every Phase 4 implementation concern)
- `.planning/sbx-cli-reference.md` § `sbx create` (`--kit`, `--template`, positional workspace; no `-e` at create)
- `src/containers/sbx/argv.rs` (Phase 2 `build_create_args` signature with `kit_path: Option<&Path>` already in place)
- `src/containers/sbx/mod.rs` (Phase 2 SbxRuntime peer struct)
- `src/agents.rs` § AGENTS array + AgentDef::install_hint (lines 188-434) — per-agent install commands
- `src/hooks/status_file.rs` (existing `hook_status_dir`, `read_hook_status`, `cleanup_hook_status_dir`)
- `src/hooks/mod.rs` (`HOOK_STATUS_BASE = "/tmp/aoe-hooks"`, existing hook command shape)
- `src/logging.rs:447-468` (atomic-rename precedent)
- `src/tui/mod.rs:50` and `src/server/api/system.rs` (tokio::task::spawn_blocking precedents)
- `xtask/src/main.rs` (CheckSkill precedent for CheckKit shape)
- `.github/workflows/ci.yml` § `skill` job (template for new `kit` job)
- `Cargo.toml` (sha2 = 0.11; serde_yaml = 0.9; tempfile = 3.14; tracing = 0.1; rust-embed = 8 [used for web/dist only])
- https://docs.docker.com/ai/sandboxes/customize/kits/ — schemaVersion: "1" required, `${WORKDIR}` substitution in `initFiles[*].content`, commands.install runs once as root, commands.startup runs every start as agent user uid 1000
- https://docs.docker.com/ai/sandboxes/architecture/ — workspaces mounted at the same absolute path host-side and inside the VM
- github.com/docker/sbx-kits-contrib/openclaw/spec.yaml (live example: schemaVersion, kind: agent, agent block, network.serviceDomains, network.serviceAuth, credentials.sources.<svc>.env, environment.proxyManaged, commands.install with user: "0" and user: "1000")
- github.com/docker/sbx-kits-contrib/code-server/spec.yaml (live example: kind: mixin, initFiles with mode + ${WORKDIR} in content)

### Secondary (MEDIUM confidence)
- docs.rs/std/fs/fn.rename.html (Rust std::fs::rename portability) — confirms Unix requires empty dest dir; ErrorKind not stable
- github.com/rust-lang/libs-team/issues/131 (rename_noreplace proposal) — confirms RENAME_NOREPLACE pattern; ENOTEMPTY vs EEXIST distinction
- github.com/hadolint/hadolint/wiki/DL3009 (Hadolint install-verb lint precedent for check-kit)
- WebFetch of docs.docker.com/ai/sandboxes/customize/build-an-agent/ (agent kit example; `agent.entrypoint.run` array; `network.serviceAuth.<svc>.{headerName, valueFormat}`)
- WebFetch of docs.docker.com/ai/sandboxes/customize/kit-examples/ (initFiles + ${WORKDIR} pattern + onlyIfMissing field)

### Tertiary (LOW confidence)
- WebFetch of docs.docker.com/ai/sandboxes/security/credentials/ — only confirms `sbx secret set` exists; YAML schema for `credentials.sources.<svc>.agent` (ssh-agent) was NOT found. See Assumption A1.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dep already in tree and verified via codebase grep
- Architecture patterns: HIGH — atomic-rename, spawn_blocking, xtask all match existing codebase precedents
- Pitfalls: HIGH for 1 (verified against Rust libstd docs), HIGH for 2-3-4-6 (locked in CONTEXT.md), MEDIUM for 5 (relies on doc-omission inference; mitigated by static-path-only rule)
- Validation architecture: HIGH — every test target maps to a specific behavior + a deterministic automated command

**Research date:** 2026-05-16
**Valid until:** 2026-06-15 (kit format is explicitly experimental per docs; revisit if a sbx daemon version bump lands in the interim or if A1 is resolved)

## RESEARCH COMPLETE

**Phase:** 04 — Embedded sbx Kit and Materialization
**Confidence:** HIGH

### Key Findings
- All 17 locked decisions in CONTEXT.md (D-01..D-17) verified against the codebase + Docker Sandboxes docs. No re-litigation needed; this research focuses on de-risking implementation specifics.
- `std::fs::rename` does NOT return `ErrorKind::AlreadyExists` reliably for non-empty target dirs on any platform; post-rename `target.exists()` is the authoritative signal (Pitfall 1; CONTEXT.md `<specifics>` already flagged this — research confirms via std docs and rust-lang/libs-team issue 131).
- `${WORKDIR}` substitution is documented inside `initFiles[*].content` only; do NOT rely on it in `initFiles[*].path` or `commands.startup[*].command` (Pitfall 5).
- `commands.startup` accepts both array form (`["sh","-c","..."]`, used by openclaw + code-server live kits) and the legacy string form; the install-verb lint must handle both shapes.
- All 8 v1 agents identified in CONTEXT.md D-10 (Claude, Codex, OpenCode, Gemini, Cursor CLI, droid, pi, Qwen Code) have npm/curl install hints in `src/agents.rs`; Cursor CLI is the one exception (no executable install hint) — recommendation: drop Cursor from v1 install dispatch and ship a fail-loud handler.
- Phase 4 adds **zero** new dependencies, **zero** new `cfg(target_os = ...)` branches, **zero** new settings fields, and **zero** new clap surface.
- One `[ASSUMED]` claim (A1): the `credentials.sources.<svc>.agent: ssh-agent` schema is not publicly documented; mitigation is to defer SSH parity until Phase 5/6 OR to smoke-test the schema during plan-phase.

### File Created
`.planning/phases/04-embedded-sbx-kit-and-materialization/04-RESEARCH.md`

### Confidence Assessment
| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | Every dep already in tree; no new packages |
| Architecture | HIGH | Atomic-rename + spawn_blocking + xtask patterns all already in codebase |
| Pitfalls | HIGH | 5 of 6 verified against Rust libstd / live kit examples / existing codebase; only Pitfall 5 (substitution scope) relies on doc-omission inference |
| Validation Architecture | HIGH | Every KIT-XX requirement maps to a specific test with a deterministic command; Wave 0 gaps enumerated |
| Credentials.sources schema (A1) | MEDIUM | Schema for ssh-agent forwarding in spec.yaml is undocumented publicly; plan-phase should resolve via smoke-test or defer SSH to Phase 5/6 |

### Open Questions
1. `credentials.sources.<svc>.agent` shape for ssh-agent — resolve via plan-phase smoke-test OR defer SSH parity to Phase 5/6 per Assumption A1.
2. v1 agent install subset (which agents fail-loud, which work) — planner picks per CONTEXT.md D-10 / Open Question 3.
3. CI scoping for `cargo xtask check-kit` (every push vs path-filtered) — recommend every-push for parity with skill check.

### Ready for Planning
Research complete. Planner can now create PLAN.md files for plans 04-01 (materializer + embedded assets) and 04-02 (hook dispatch + xtask check-kit + CI).
