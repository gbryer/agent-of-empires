---
phase: 04-embedded-sbx-kit-and-materialization
plan: 02
subsystem: hooks/dispatch + xtask/kit-lint
tags:
  - sbx
  - kit
  - hooks
  - dispatch
  - xtask
  - ci
requires:
  - phase-04-01-embedded-sbx-kit-and-materializer
provides:
  - "src/hooks/status_file.rs::sbx_hook_status_dir(project_path, instance_id) -> PathBuf"
  - "src/hooks/status_file.rs::resolve_status_dir(supports_arbitrary_volume_paths, project_path, instance_id) -> PathBuf"
  - "src/hooks/status_file.rs::read_hook_status_for_instance(&Instance) -> Option<Status>"
  - "src/hooks/status_file.rs::cleanup_hook_status_dir_for_instance(&Instance)"
  - "cargo xtask check-kit (--spec-path <path>) subcommand"
  - "CI kit: job validating the embedded spec.yaml on every push"
affects:
  - "src/hooks/mod.rs (re-exports four new symbols)"
  - "src/tui/app.rs:1055 (call site swap)"
  - "src/server/api/sessions.rs:1095 (call site swap)"
  - "src/cli/session.rs:535 (call site swap)"
  - "src/session/instance.rs:1603 (call site swap)"
  - "src/session/instance.rs:1642 (cleanup call site swap)"
  - "src/session/instance.rs:1761 (call site swap)"
  - "src/session/deletion.rs:300 (cleanup call site swap)"
tech-stack:
  added:
    - "serde_yaml = 0.9 added to xtask/Cargo.toml as a direct dep (was already a transitive dep via agent-of-empires)"
  patterns:
    - "Pure capability-dispatcher pinned in both branches by dedicated unit tests so the seam does not require stubbing the global runtime factory"
    - "xtask subcommand pattern (variant + match arm + body) mirroring the existing check_skill verb; eprintln/has_error accumulator and a single std::process::exit(1) at the end"
    - "Fixture-driven integration tests that spawn the xtask binary via `cargo run -p xtask -- check-kit --spec-path` and assert exit code + stderr substring"
key-files:
  created:
    - "tests/xtask_check_kit.rs"
    - "tests/fixtures/sbx_kits/ok/spec.yaml"
    - "tests/fixtures/sbx_kits/missing-schema-version/spec.yaml"
    - "tests/fixtures/sbx_kits/naked-install-verb/spec.yaml"
    - "tests/fixtures/sbx_kits/guarded-install-verb/spec.yaml"
  modified:
    - "src/hooks/status_file.rs"
    - "src/hooks/mod.rs"
    - "src/tui/app.rs"
    - "src/server/api/sessions.rs"
    - "src/cli/session.rs"
    - "src/session/instance.rs"
    - "src/session/deletion.rs"
    - "xtask/Cargo.toml"
    - "xtask/src/main.rs"
    - ".github/workflows/ci.yml"
    - "Cargo.lock"
decisions:
  - "Extract `parse_status_content` so the legacy `read_hook_status` and the new `read_hook_status_for_instance` share one parse implementation. This removes silent drift risk (`running`/`waiting`/`idle` semantics) and lets the parse table be unit-tested directly."
  - "The two `_for_instance` shims read the runtime capability via `crate::containers::get_container_runtime().capabilities()` once and pass the bool flag into the pure `resolve_status_dir`. The shims themselves remain trivially correct by inspection; the pure dispatcher carries the two unit tests that pin both branches (legacy /tmp and sbx workspace) without stubbing the global factory. VALIDATION.md row `status_dir_dispatch_via_caps` satisfied with automated tests, not a phase-5 placeholder."
  - "Combined RED+GREEN test authoring into single per-task commits because the project's pre-commit hook runs `cargo clippy -- -D warnings`. A separate RED commit with `todo!()` bodies fails the gate. Same precedent as 04-01."
  - "`xtask/Cargo.toml` gets an explicit `serde_yaml = 0.9` direct dep even though the workspace already pulls it in transitively, so the xtask build is self-contained and does not bit-rot if a future agent-of-empires refactor removes its serde_yaml dep."
  - "`extract_command_string` collapses `command: [\"sh\", \"-c\", \"...\"]` to the joined remainder so the install-verb lint operates on what the shell sees, not the argv vector. Without this collapse, `apt-get install` literals nested inside `sh -c` arrays would not match the substring search."
metrics:
  duration: "~25 minutes"
  completed: "2026-05-16T16:25Z"
  commits: 2
  tests-added: 10
  files-created: 5
  files-modified: 11
---

# Phase 4 Plan 02: Hook Status Dispatch + Kit Lint Summary

Closed the host side of KIT-04 with a capability-driven status-dir
dispatcher, and shipped KIT-06 as a peer xtask subcommand + CI job that
keeps the embedded sbx kit spec.yaml honest on every push.

## What landed

1. **Sibling free function** in `src/hooks/status_file.rs`:
   `sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf`
   returns `<project_path>/.aoe-hooks/<instance_id>`. Doc-comment
   explains why the sbx microVM uses the workspace-relative path: it
   cannot mount the host's `/tmp/aoe-hooks` at a different path under
   `!supports_arbitrary_volume_paths`, so the in-VM hook writes through
   the workspace mount that the sandbox provides natively, and the host
   poller reads through the same workspace path with no host-side
   hook-volume mount.

2. **Pure dispatcher**: `resolve_status_dir(supports_arbitrary_volume_paths,
   project_path, instance_id) -> PathBuf` is the single site for
   selecting between `hook_status_dir` (legacy /tmp) and
   `sbx_hook_status_dir` (workspace-relative). Two dedicated unit tests
   pin both branches without stubbing `get_container_runtime`.

3. **Dispatch shims**: `read_hook_status_for_instance(&Instance)` and
   `cleanup_hook_status_dir_for_instance(&Instance)` read the runtime
   capability once and pass the flag into `resolve_status_dir`. The
   shims are one-liner consumers of the dispatcher.

4. **Parse extraction**: `parse_status_content(&str) -> Option<Status>`
   is shared by `read_hook_status` and `read_hook_status_for_instance`,
   so the legacy and dispatching paths cannot drift on what counts as a
   valid value.

5. **Call-site routing**: all five `read_hook_status` consumers
   (`tui/app.rs:1055`, `server/api/sessions.rs:1095`, `cli/session.rs:535`,
   `session/instance.rs:1603`, `session/instance.rs:1761`) and both
   `cleanup_hook_status_dir` consumers (`session/instance.rs:1642`,
   `session/deletion.rs:300`) now use the `_for_instance` shims. The
   legacy `hook_status_dir`, `read_hook_status`, `cleanup_hook_status_dir`
   public functions stay unchanged in shape; the call site in
   `session/container_config.rs:1108` (which builds the legacy
   `/tmp/aoe-hooks` bind mount for Docker/Podman/AppleContainer) keeps
   the legacy path because Phase 3's conform-for-capabilities post-pass
   already drops that mount under sbx.

6. **xtask check-kit** (`xtask/src/main.rs`): a new `CheckKit { --spec-path }`
   variant parses the embedded `src/containers/sbx_kit/spec.yaml`
   (or any fixture passed via `--spec-path`) and asserts:
   - `schemaVersion` is the **string** "1" (not the int 1).
   - `kind`, `name`, `commands.install`, `commands.startup` exist with
     the expected shapes.
   - No `commands.{install,startup}` entry contains a naked install
     verb (apt-get install, npm install -g, curl, wget, gem install,
     ...) without an idempotency guard (`[ -f`, `command -v`, `||`,
     `&& `, ...). `extract_command_string` collapses
     `["sh", "-c", "..."]` arrays so the lint operates on the shell
     command, not the argv list.
   On any violation: `eprintln!` per failure, `std::process::exit(1)`
   at the end. On success: `println!("check-kit: OK")` and exit 0.

7. **Fixtures + integration test** (`tests/xtask_check_kit.rs` plus four
   `tests/fixtures/sbx_kits/<name>/spec.yaml` files): five tests spawn
   `cargo run -p xtask -- check-kit ...` and assert the expected exit
   code + stderr substring. The fifth test runs `cargo xtask check-kit`
   with no args against the embedded spec.yaml authored in 04-01,
   pinning that the project's own kit is lint-clean.

8. **CI `kit:` job** (`.github/workflows/ci.yml`): mirrors the existing
   `skill:` job exactly, using the same pinned action SHAs
   (`actions/checkout@de0fac2e...`, `dtolnay/rust-toolchain@stable`,
   `Swatinem/rust-cache@c1937114...`). The job runs
   `cargo xtask check-kit` on every push.

## Acceptance criteria met

- KIT-04 (host side): `read_hook_status_for_instance` resolves to
  `/tmp/aoe-hooks/<id>/status` for Docker/Podman/AppleContainer (when
  `supports_arbitrary_volume_paths=true`), and to
  `<project_path>/.aoe-hooks/<id>/status` for sbx (when the flag is
  false). Two unit tests pin both branches. All five `read_hook_status`
  call sites and both `cleanup_hook_status_dir` call sites consume the
  new shims. The legacy public API stays unchanged for backwards
  compatibility within the codebase (no rename).
- KIT-06: `cargo xtask check-kit` is a real subcommand, parses the
  embedded spec.yaml directly via `include_str!`, asserts
  `schemaVersion == "1"` and the required field shape, runs the
  install-verb lint with idempotency-guard recognition, exits 0 on the
  embedded spec, and runs in CI as a peer `kit:` job to `skill:`.
- Per the plan's `success_criteria` row, no file owned by 04-01
  (`src/containers/sbx/kit.rs`, `src/containers/sbx_kit/*`) was
  modified; the wave-parallelism contract is honored.

## Test counts

`cargo test --lib hooks::status_file` runs **15** tests (10 existing +
5 new), all passing.

`cargo test --test xtask_check_kit` runs **5** tests
(`ok_fixture_passes`, `missing_schema_version_fails`,
`naked_install_verb_fails`, `guarded_install_verb_passes`,
`embedded_spec_passes`), all passing.

Plan called for 5 new unit tests in `hooks::status_file` plus 5
integration tests; landed 5 + 5 = 10 new tests total.

## Verification

| Check | Status |
|-------|--------|
| `cargo build --lib` | PASS |
| `cargo build --features serve` | PASS (took 2m25s; web dashboard rebuilt cleanly) |
| `cargo test --lib hooks::status_file` | PASS (15/15) |
| `cargo test --test xtask_check_kit` | PASS (5/5) |
| `cargo xtask check-kit` (no args; embedded spec) | PASS (exit 0; "check-kit: OK") |
| `cargo xtask check-kit --spec-path .../missing-schema-version/spec.yaml` | FAIL with exit 1, stderr mentions `schemaVersion` |
| `cargo xtask check-kit --spec-path .../naked-install-verb/spec.yaml` | FAIL with exit 1, stderr mentions `naked install verb` |
| `cargo clippy --lib -- -D warnings` | PASS |
| `cargo clippy -p xtask -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| em-dash / `--` separator scan (excluding CLI flag literals) | 0 prose matches |
| `grep -c 'pub fn sbx_hook_status_dir' src/hooks/status_file.rs` | 1 |
| `grep -c 'pub fn resolve_status_dir' src/hooks/status_file.rs` | 1 |
| `grep -c 'pub fn read_hook_status_for_instance' src/hooks/status_file.rs` | 1 |
| `grep -c 'pub fn cleanup_hook_status_dir_for_instance' src/hooks/status_file.rs` | 1 |
| `grep -c 'pub fn hook_status_dir' src/hooks/status_file.rs` | 1 (legacy preserved) |
| `grep -cE 'pub fn read_hook_status\b' src/hooks/status_file.rs` | 1 (legacy preserved; no rename) |
| `grep -cE 'pub fn cleanup_hook_status_dir\b' src/hooks/status_file.rs` | 1 (legacy preserved) |
| `grep -rEc 'read_hook_status\(' src/tui/app.rs src/server/api/sessions.rs src/cli/session.rs` | 0 (all switched to `_for_instance`) |
| `grep -cE '^  kit:' .github/workflows/ci.yml` | 1 |
| `grep -c 'cargo xtask check-kit' .github/workflows/ci.yml` | 1 |
| `grep -c 'de0fac2e4500dabe0009e67214ff5f5447ce83dd' .github/workflows/ci.yml` increment | 10 -> 11 (verified via stash) |

## Commits

- `c59cad4` feat(04-02): capability-dispatched hook status dir for sbx runtime
- `81d8e90` feat(04-02): cargo xtask check-kit lints the embedded sbx kit spec.yaml

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 3 - Blocking issue] Missing trait import for `capabilities()` method**

- **Found during:** Task 1 first `cargo build --lib`.
- **Issue:** `crate::containers::get_container_runtime().capabilities()`
  failed to compile because `ContainerRuntimeInterface` was not in
  scope at the call site in `src/hooks/status_file.rs`. The plan's
  pseudocode assumed the trait would resolve via the existing
  re-exports, but `pub use container_interface::{..., ContainerRuntimeInterface, ...}`
  in `src/containers/mod.rs` makes the trait name importable, not
  automatically in scope.
- **Fix:** Added `use crate::containers::ContainerRuntimeInterface;`
  alongside the existing imports.
- **Files modified:** `src/hooks/status_file.rs`
- **Commit:** `c59cad4`

**2. [Rule 3 - Blocking issue] `inst` is already `&Instance`, not owned**

- **Found during:** Task 1 first `cargo clippy --lib -- -D warnings`.
- **Issue:** Clippy `needless_borrow` flagged
  `read_hook_status_for_instance(&inst)` at `src/cli/session.rs:535`.
  The plan said `&inst.id` -> `&inst`, but `resolve_session` already
  returns a `&Instance`, so taking another reference is a needless
  borrow.
- **Fix:** Pass `inst` directly to the shim. Same outcome; one extra
  `&` removed.
- **Files modified:** `src/cli/session.rs`
- **Commit:** `c59cad4`

**3. [Process] RED and GREEN folded into single per-task commits**

- **Found during:** Task 1 RED-phase planning.
- **Issue:** Plan marked Task 1 and Task 2 `tdd="true"` implying
  separate RED (failing-tests) commit per the standard TDD workflow.
  The project's pre-commit hook runs `cargo clippy -- -D warnings`,
  which fails on any unused stub function (a RED commit with
  `todo!()` bodies produces dead-code warnings).
- **Fix:** Combined RED test authoring + GREEN implementation into
  one commit per task so each commit passes the pre-commit gate.
  Test list unchanged from plan; only commit boundary differs.
  Identical to 04-01's deviation #3.
- **Files modified:** none (process change)

### No other deviations

Plan content (function names, signatures, call-site list, fixture
content, integration-test shape, CI job structure, threat-model
mitigations) was implemented as written.

## Known Stubs

None. Every new public + module-private function in
`src/hooks/status_file.rs` is exercised by tests or production call
sites. Every new arm of `xtask::Commands` is exercised by the
integration test suite. No placeholder return values.

## Threat Flags

None. The threat model rows declared in the plan
(`T-04-02-01..T-04-02-07`) are all mitigated or accepted in the
implementation:

- T-04-02-01 (`read_hook_status_for_instance` reading
  `<project_path>/.aoe-hooks/<id>/status`): accepted as documented;
  the workspace path is already under user control.
- T-04-02-02 (`sbx_hook_status_dir(&Path, &str)` path injection):
  mitigated by upstream invariants - `project_path` from
  `Instance::project_path` (validated by Phase 3's `compute_volume_paths`)
  and `instance_id` from `Instance::new` (UUID-shaped, no `..` possible).
  The dispatch shim's doc-comment documents the invariant.
- T-04-02-03 (`cleanup_hook_status_dir_for_instance`, `remove_dir_all`):
  same UUID-shape gate; instance ids match `[a-f0-9-]+` and cannot
  contain `..`.
- T-04-02-04 (xtask runtime DoS): accepted; the parser runs on a
  single < 10 KB embedded file with sub-second wall time.
- T-04-02-05 (install-verb lint bypass): mitigated by the strict
  guard-prefix allowlist (`GUARD_PREFIXES`) plus the `||` / `&& `
  operator recognition. Custom guards not on the list fall through
  to "naked verb" and fail the lint, forcing the author to use a
  recognized guard. `guarded-install-verb` fixture passes;
  `naked-install-verb` fixture fails.
- T-04-02-06 (embedded spec.yaml path resolution): accepted; the
  `include_str!("../../src/containers/sbx_kit/spec.yaml")` path is
  compile-time-resolved and a directory rename would surface as a
  build error.
- T-04-02-07 (supply chain): `serde_yaml = "0.9"` added as a direct
  dep to `xtask/Cargo.toml` is the same version already in the
  root `Cargo.toml:34` and the workspace lock; no new transitive
  packages were pulled in. Verified via `Cargo.lock` diff in commit
  `81d8e90`.

No new threat surface beyond the plan's threat model.

## Self-Check: PASSED

**Files created (verified to exist via `[ -f path ]`):**

- `tests/xtask_check_kit.rs` FOUND
- `tests/fixtures/sbx_kits/ok/spec.yaml` FOUND
- `tests/fixtures/sbx_kits/missing-schema-version/spec.yaml` FOUND
- `tests/fixtures/sbx_kits/naked-install-verb/spec.yaml` FOUND
- `tests/fixtures/sbx_kits/guarded-install-verb/spec.yaml` FOUND

**Files modified (verified in git diff):**

- `src/hooks/status_file.rs` MODIFIED (sbx_hook_status_dir,
  resolve_status_dir, read_hook_status_for_instance,
  cleanup_hook_status_dir_for_instance, parse_status_content + 5
  new unit tests)
- `src/hooks/mod.rs` MODIFIED (re-export the four new symbols)
- `src/tui/app.rs` MODIFIED (one call-site swap)
- `src/server/api/sessions.rs` MODIFIED (one call-site swap)
- `src/cli/session.rs` MODIFIED (one call-site swap)
- `src/session/instance.rs` MODIFIED (three call-site swaps)
- `src/session/deletion.rs` MODIFIED (one call-site swap)
- `xtask/Cargo.toml` MODIFIED (serde_yaml = "0.9")
- `xtask/src/main.rs` MODIFIED (CheckKit variant, check_kit body,
  helpers)
- `.github/workflows/ci.yml` MODIFIED (kit: job appended)
- `Cargo.lock` MODIFIED (workspace lock for the new dep)

**Commits (verified via `git log --oneline`):**

- `c59cad4` FOUND
- `81d8e90` FOUND
