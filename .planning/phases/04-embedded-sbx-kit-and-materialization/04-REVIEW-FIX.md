---
phase: 04-embedded-sbx-kit-and-materialization
fixed_at: 2026-05-16T20:15:00Z
review_path: .planning/phases/04-embedded-sbx-kit-and-materialization/04-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 5
skipped: 1
status: partial
---

# Phase 04: Code Review Fix Report

**Fixed at:** 2026-05-16T20:15:00Z
**Source review:** .planning/phases/04-embedded-sbx-kit-and-materialization/04-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 6
- Fixed: 5 (CR-01, CR-02, WR-01, WR-02, WR-03)
- Skipped: 1 (WR-04, resolved by WR-02)

## Fixed Issues

### CR-01: Startup command uses wrong binary name for kiro agent

**Files modified:** `src/containers/sbx_kit/spec.yaml`
**Commit:** f7a809f
**Applied fix:** Replaced `command -v kiro` with `command -v kiro-cli` in the startup health-check OR-chain to match the actual binary name installed by kiro's installer.

### CR-02: Startup command missing vibe agent binary check

**Files modified:** `src/containers/sbx_kit/spec.yaml`
**Commit:** f7a809f
**Applied fix:** Added `command -v vibe >/dev/null 2>&1 ||` to the startup check chain so vibe-only containers can pass the health check.

### WR-01: Lint error message hardcodes "commands.startup" for all command sections

**Files modified:** `xtask/src/main.rs`
**Commit:** 2d3581d
**Applied fix:** Added a `section: &str` parameter to both `lint_command_for_naked_install_verbs` and `lint_commands`. Call sites now pass `"commands.install"` or `"commands.startup"` respectively, so the error message correctly identifies the violating section.

### WR-02: hook-shim.sh materialized as dead code

**Files modified:** `src/containers/sbx/kit.rs`, `src/containers/sbx_kit/hook-shim.sh`
**Commit:** fcf5260
**Applied fix:** Removed the `HOOK_SHIM` constant, its inclusion in `embedded_kit_bytes()`, its `fs::write` call in `ensure_with_app_dir`, and the test assertion for its existence. Deleted the source file `src/containers/sbx_kit/hook-shim.sh`.

### WR-03: gitignore-aoe-hooks file materialized but never referenced

**Files modified:** `src/containers/sbx/kit.rs`, `src/containers/sbx_kit/gitignore-aoe-hooks`
**Commit:** fcf5260
**Applied fix:** Removed the `GITIGNORE_AOE_HOOKS` constant, its inclusion in `embedded_kit_bytes()`, its `fs::write` call in `ensure_with_app_dir`, and the test assertion for its existence. Deleted the source file `src/containers/sbx_kit/gitignore-aoe-hooks`.

## Skipped Issues

### WR-04: hook-shim.sh not made executable on Unix

**File:** `src/containers/sbx/kit.rs:115`
**Reason:** Resolved by WR-02. The hook-shim.sh file was removed entirely, making the missing chmod moot.
**Original issue:** hook-shim.sh was written without executable permissions, so direct invocation would fail with EACCES.

---

_Fixed: 2026-05-16T20:15:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
