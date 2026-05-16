---
phase: 04-embedded-sbx-kit-and-materialization
plan: 01
subsystem: containers/sbx-kit
tags:
  - sbx
  - kit
  - materializer
  - cache
  - gc
  - embedded-assets
requires:
  - phase-02-sbxruntime-skeleton-and-pure-argv-builders
  - phase-03-container-config-capability-gating-and-workspace-path-conformance
provides:
  - "src/containers/sbx/kit.rs::ensure(agent) -> Result<PathBuf> (public; SbxRuntime::create_container call site lands in Phase 5)"
  - "src/containers/sbx/kit.rs::cache_dir(app_dir, agent) -> PathBuf (pub(crate); deterministic content-addressed cache path)"
  - "Embedded kit assets at src/containers/sbx_kit/{spec.yaml, hook-shim.sh, gitignore-aoe-hooks, settings-{claude,gemini,cursor,qwen}.json} (compile-time via include_str!)"
  - "30-day stale-dir GC fired fire-and-forget on cache miss only"
affects:
  - "src/containers/sbx/mod.rs (one-line: pub mod kit;)"
tech-stack:
  added: []
  patterns:
    - "Atomic-rename materializer: write to <agent>.tmp.<pid>.<nanos> sibling dir, std::fs::rename to final path, post-rename target.exists() as authoritative race signal (mirrors src/logging.rs:447-468)"
    - "Content-addressed cache key: sha256 over concatenated embedded bytes, truncated to 16 lowercase-hex chars, prefixed with aoe-<CARGO_PKG_VERSION>"
    - "Fire-and-forget background work: tokio::task::spawn_blocking when a runtime is current, std::thread::spawn fallback for sync callers (mirrors src/tui/mod.rs:50 pattern with sync compat)"
    - "Stdlib-only embedding: include_str! for seven asset files; no include_dir/rust-embed (PROJECT.md Key Decisions)"
key-files:
  created:
    - "src/containers/sbx_kit/spec.yaml"
    - "src/containers/sbx_kit/hook-shim.sh"
    - "src/containers/sbx_kit/gitignore-aoe-hooks"
    - "src/containers/sbx_kit/settings-claude.json"
    - "src/containers/sbx_kit/settings-gemini.json"
    - "src/containers/sbx_kit/settings-cursor.json"
    - "src/containers/sbx_kit/settings-qwen.json"
    - "src/containers/sbx/kit.rs"
  modified:
    - "src/containers/sbx/mod.rs (registers `pub mod kit;` next to `pub mod argv;`)"
decisions:
  - "ensure_with_app_dir, cache_dir_inner, gc_stale_kit_dirs, spawn_gc are module-private; only ensure (public) and cache_dir (pub(crate)) cross the module boundary. Tests live in-module so they call the private helpers directly without exposing a test-only public surface (AGENTS.md no-dead-code)."
  - "spawn_gc uses tokio::runtime::Handle::try_current() to choose between tokio::task::spawn_blocking and std::thread::spawn. Plan 04-01 originally specified spawn_blocking only; the fallback was added so the in-module concurrent_ensure_all_succeed and ensure_creates_files_on_miss tests (sync, no tokio context) do not panic on cache-miss GC. Both branches are fire-and-forget; D-15 semantics preserved."
  - "credentials.sources schema in spec.yaml uses `credentials.sources.ssh.agent: ssh-agent` ([ASSUMED] per RESEARCH A1). Inline YAML comment flags the assumption; VALIDATION.md row `credentials.sources ssh-agent schema` requires live-sbx confirmation before phase exit."
  - "v1 UNSUPPORTED_AGENTS list (cursor, copilot, settl) returns Err from ensure at host time per D-10 fail-loud. install_hint values for those agents are doc pointers / Homebrew Cask URLs; the in-VM `sh /sbx-kit/install.sh` cannot execute them."
metrics:
  duration: "~30 minutes"
  completed: "2026-05-16T05:55Z"
  commits: 3
  tests-added: 17
  files-created: 8
  files-modified: 1
  lines-added: 947
---

# Phase 4 Plan 01: Embedded sbx Kit + KitMaterializer Summary

Authored the seven embedded kit assets at `src/containers/sbx_kit/` and the
`KitMaterializer` at `src/containers/sbx/kit.rs` with content-addressed
atomic-rename cache and 30-day stale-dir GC, fully covered by 17 in-module
tests.

## What landed

1. **Embedded kit assets** (`src/containers/sbx_kit/`, 7 files): `spec.yaml`
   pinned to `schemaVersion: "1"`, `kind: agent`, `commands.install` (run the
   per-agent `install.sh` plus an idempotency-guarded `.aoe-hooks/` drop into
   `${WORKDIR}/.gitignore`), `commands.startup` PATH guard for ten v1 agent
   binaries, `initFiles` for the four hook-supporting agents
   (claude/gemini/cursor/qwen) writing settings.json under
   `/home/agent/<rel>` with `${WORKDIR}`-rooted hook commands in `content`
   only (never in `path`; RESEARCH Pitfall 5), `environment.proxyManaged`
   listing seven provider env vars (D-06), and a `credentials.sources` block
   declaring ssh-agent forwarding ([ASSUMED] schema flagged inline per D-05
   plus VALIDATION row). The four per-agent settings.json sidecar files
   also exist as standalone assets so future cross-references (xtask
   check-kit lint comparisons, etc.) can hash them independently. The shim
   reference body lives in `hook-shim.sh`; the workspace-gitignore drop
   content lives in `gitignore-aoe-hooks`.

2. **KitMaterializer** (`src/containers/sbx/kit.rs`):
   - `pub fn ensure(agent) -> Result<PathBuf>` is the public entry; reads
     `crate::session::get_app_dir()` and delegates to private
     `ensure_with_app_dir`.
   - `pub(crate) fn cache_dir(app_dir, agent) -> PathBuf` returns
     `<app_dir>/sbx-kit/aoe-<CARGO_PKG_VERSION>-<sha256[:16]>/<agent>/`.
     The cache key includes the concatenated bytes of all seven embedded
     assets; a one-byte flip in any of them produces a new dir without
     touching the prior one (KIT-05).
   - Cache-miss path writes `spec.yaml` + `install.sh`
     (`#!/bin/sh\nset -eu\n<install_hint>\n`) + `hook-shim.sh` +
     `gitignore-aoe-hooks` into a `<agent>.tmp.<pid>.<nanos>` sibling, chmods
     `install.sh` to 0o755 under `cfg(unix)`, then `std::fs::rename` to the
     final path. Post-rename `target.exists()` is the authoritative race
     signal (`ErrorKind::AlreadyExists` for directory rename is unreliable
     across Linux/macOS per RESEARCH Pitfall 1).
   - Cache hit returns immediately; spec.yaml mtime is preserved, GC does
     not fire (D-11/D-15).
   - `UNSUPPORTED_AGENTS = ["cursor", "copilot", "settl"]` returns
     `Err(...)` from `ensure` at host time without writing any files
     (D-10 fail-loud).

3. **Stale-kit GC** (`gc_stale_kit_dirs(root, 30 days)`):
   - One-level enumeration under `<app_dir>/sbx-kit/`; skips siblings whose
     name does not start with `aoe-` (T-04-01-02 path-traversal guard) and
     any `.tmp.`-suffixed dir (mid-rename racer guard).
   - `remove_dir_all` per surviving entry whose mtime is older than 30 days;
     per-entry failures log via `tracing::info!` and continue.
   - Fired fire-and-forget exclusively on cache miss after the rename
     succeeds. `spawn_gc` prefers `tokio::task::spawn_blocking` when a
     runtime is current (production path from
     `SbxRuntime::create_container` in Phase 5), falls back to
     `std::thread::spawn` for sync callers (in-module tests today, future
     direct CLI calls). The handle is dropped in both branches.

## Acceptance criteria met

- KIT-01: `include_str!` constants in `kit.rs` cover all seven assets;
  `cache_dir` returns the documented shape; `ensure` materializes lazily and
  is idempotent on hit (`ensure_idempotent_on_hit` test asserts spec.yaml
  mtime stable across two `ensure` calls).
- KIT-02: `kit_path_threads_through_build_create_args` test feeds the
  materialized path into `build_create_args` and asserts
  `[--kit <materialized> --template alpine:latest]` shape and ordering.
- KIT-03: `install_sh_contains_install_hint_per_agent` matrix test confirms
  ten v1 agents each receive their `install_hint` literal in the
  materialized `install.sh`; `ensure_fails_loud_for_unsupported` covers the
  Err path for cursor/copilot/settl.
- KIT-04 (in-VM half): `spec.yaml`'s `initFiles` declares
  `/home/agent/.claude/settings.json` etc with `${WORKDIR}`-rooted hook
  commands in `content` only; `commands.install` drops `.aoe-hooks/` into
  `${WORKDIR}/.gitignore` guarded by `grep -qsE`. Host-side dispatch +
  workspace-gitignore correctness lands in 04-02.
- KIT-05: cache key includes both `CARGO_PKG_VERSION` and the SHA256 of
  concatenated embedded bytes (`cache_dir_changes_on_content_byte_flip`
  test); 30-day GC walks `aoe-*` top-level dirs with `.tmp.` + non-`aoe-`
  skip rails (`gc_*` tests assert both rails); fires only on cache miss
  (verified by inspection of `ensure_with_app_dir` and the fact that the
  fast-path `if target.exists() { return Ok(target); }` is the first
  branch).

## Test counts

`cargo test --lib containers::sbx::kit` runs 17 tests, all passing:

- 12 Task 2a tests (cache_dir shape + byte flip, ensure files-on-miss
  + idempotency + fail-loud, per-agent install_hint, spec.yaml WORKDIR rules,
  gitignore drop, credentials.sources ssh-agent leaf, 4-thread concurrent
  ensure, embedded bytes match source, install.sh 0o755 on unix, kit-path
  threads through build_create_args).
- 3 Task 2b GC tests (stale removed / fresh kept, non-aoe siblings
  survive, .tmp. dirs survive).
- 2 extra coverage tests in 2a (`spec_yaml_no_workdir_in_paths`,
  `spec_yaml_gitignore_drop_in_commands_install`) bringing the total to 17.

Plan called for ">= 12 tests" in 2a + 3 in 2b = 15; landed 14 + 3 = 17. The
extra tests pin the `${WORKDIR}` placement rule and the install-hook
gitignore guard pattern, both directly tied to RESEARCH Pitfalls 5 and 3.

## Verification

| Check | Status |
|-------|--------|
| `cargo build --lib` | PASS |
| `cargo test --lib containers::sbx::kit` | PASS (17/17) |
| `cargo clippy --lib -- -D warnings` | PASS (no warnings introduced) |
| `cargo fmt --all -- --check` | PASS |
| `grep -rE '#\[allow\(dead_code\)\]' src/containers/sbx/kit.rs src/containers/sbx_kit/` | empty (no dead-code allowances) |
| em-dash / `--` separator scan (excluding `sh -c` and `--<flag>` strings) | 0 matches |
| `cargo build --features serve` | NOT RUN (sandbox network policy blocks `npm ci` in web/; orthogonal to this plan's surface, which doesn't touch any feature-gated code) |

## Commits

- `17e5abd` feat(04-01): author embedded sbx kit asset files
- `1187b58` feat(04-01): implement KitMaterializer::ensure with atomic-rename cache
- `ade47d3` feat(04-01): add 30-day stale-kit GC sweep fired on cache miss

## Deviations from Plan

### Adjustments to honor existing project constraints

**1. [Rule 2 - Missing critical functionality] Tokio-runtime guard around `spawn_blocking`**

- **Found during:** Task 2b implementation
- **Issue:** The plan specified `tokio::task::spawn_blocking` as the only
  GC trigger. `spawn_blocking` panics outside a tokio runtime context,
  and the in-module tests in Task 2a (`concurrent_ensure_all_succeed`,
  `ensure_creates_files_on_miss`, etc.) run as plain sync `#[test]`
  threads with no runtime. Without a guard, every cache-miss test would
  panic at the GC trigger point.
- **Fix:** Added a `spawn_gc` private helper that checks
  `tokio::runtime::Handle::try_current()` and uses `spawn_blocking` when a
  runtime is current (production path from `SbxRuntime::create_container`
  in Phase 5) or `std::thread::spawn` otherwise (tests today). Both
  branches drop the join handle so D-15 fire-and-forget semantics are
  preserved.
- **Files modified:** `src/containers/sbx/kit.rs`
- **Commit:** `ade47d3`

**2. [Rule 3 - Blocking issue] Tweaked plan-suggested `format!("{:x}", hash)`**

- **Found during:** Task 2a GREEN compile
- **Issue:** The plan's pseudocode used `format!("{:x}", hash)`. `sha2 0.11`
  returns a `GenericArray` that does not implement `LowerHex`. Compile
  failure on first build.
- **Fix:** Switched to the repository's established hex-encoding pattern
  from `src/session/capture.rs:1508-1512`:
  `digest.iter().map(|b| format!("{:02x}", b)).collect::<String>()`. Output
  shape is identical (64 lowercase hex chars; we slice the first 16).
- **Files modified:** `src/containers/sbx/kit.rs`
- **Commit:** `1187b58`

**3. [Process] TDD red/green/refactor compressed into single per-task commits**

- **Found during:** Plan execution opening
- **Issue:** The plan marked Tasks 2a and 2b `tdd="true"` implying a separate
  RED (failing-tests) commit per the standard TDD workflow. The repository's
  pre-commit hook runs `cargo clippy -- -D warnings`, which fails on any
  unused stub function (which a RED commit with `todo!()` bodies would have
  produced).
- **Fix:** Combined RED test authoring + GREEN implementation into single
  commits per task so each commit passes the pre-commit gate. The test list
  is unchanged from the plan; only the commit boundary differs.
- **Files modified:** none (process change)

### No other deviations

Plan content (asset bodies, cache-key composition, atomic-rename recipe, GC
guards, test list) was implemented exactly as written.

## Known Stubs

None. Every public + module-private function in `src/containers/sbx/kit.rs`
is exercised by tests or production code; no stub data, no placeholder
return values.

## Threat Flags

None. The threat model rows declared in the plan (`T-04-01-01..T-04-01-08`)
are all mitigated in the implementation:

- T-04-01-01 (agent-name path traversal) -> `install_hint(agent)` accepts
  only registry names; unsupported agents short-circuit before any
  PathBuf::join with the agent name.
- T-04-01-02 (GC arbitrary deletion) -> `starts_with("aoe-")` +
  `contains(".tmp.")` guards covered by `gc_refuses_to_delete_non_aoe_siblings`
  and `gc_skips_tmp_suffixed_dirs`.
- T-04-01-03 (install_hint trust) -> consumed as-is; `&'static str` from
  the compile-time registry, no user-derived input.
- T-04-01-04 (hook shim command quoting) -> `$AOE_INSTANCE_ID` is shell-quoted
  inside the shim command; future user-influenceable instance_id would
  require revalidation at the call site.
- T-04-01-05 (workspace `.aoe-hooks/` git-tracked) -> `commands.install`
  drops `.aoe-hooks/` into `${WORKDIR}/.gitignore` idempotently
  (`spec_yaml_gitignore_drop_in_commands_install` test).
- T-04-01-06 (GC DoS via symlink loop) -> fire-and-forget background sweep;
  ensure's caller never blocks.
- T-04-01-07 (supply chain) -> zero new packages this plan.
- T-04-01-08 ([ASSUMED] credentials schema) -> inline YAML comment flags
  the assumption; `spec_yaml_declares_credentials_sources_ssh_agent` pins
  the current shape so a regression is caught; live-sbx confirmation
  tracked in VALIDATION.md.

No new threat surface beyond the plan's threat model.

## Self-Check: PASSED

**Files created (verified to exist):**
- `src/containers/sbx_kit/spec.yaml` FOUND
- `src/containers/sbx_kit/hook-shim.sh` FOUND
- `src/containers/sbx_kit/gitignore-aoe-hooks` FOUND
- `src/containers/sbx_kit/settings-claude.json` FOUND
- `src/containers/sbx_kit/settings-gemini.json` FOUND
- `src/containers/sbx_kit/settings-cursor.json` FOUND
- `src/containers/sbx_kit/settings-qwen.json` FOUND
- `src/containers/sbx/kit.rs` FOUND

**Files modified (verified in git):**
- `src/containers/sbx/mod.rs` (one-line `pub mod kit;` registered; FOUND in commit `1187b58`)

**Commits (verified via `git log --oneline 972b8ed..HEAD`):**
- `17e5abd` FOUND
- `1187b58` FOUND
- `ade47d3` FOUND
