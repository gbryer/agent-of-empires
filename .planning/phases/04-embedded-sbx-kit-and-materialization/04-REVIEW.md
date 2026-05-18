---
phase: 04-embedded-sbx-kit-and-materialization
reviewed: 2026-05-16T19:45:00Z
depth: standard
files_reviewed: 9
files_reviewed_list:
  - src/containers/sbx/kit.rs
  - src/containers/sbx_kit/spec.yaml
  - xtask/src/main.rs
  - tests/xtask_check_kit.rs
  - tests/fixtures/sbx_kits/install-array-command/spec.yaml
  - tests/fixtures/sbx_kits/ok/spec.yaml
  - tests/fixtures/sbx_kits/guarded-install-verb/spec.yaml
  - tests/fixtures/sbx_kits/missing-schema-version/spec.yaml
  - tests/fixtures/sbx_kits/naked-install-verb/spec.yaml
findings:
  critical: 2
  warning: 4
  info: 0
  total: 6
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-05-16T19:45:00Z
**Depth:** standard
**Files Reviewed:** 9
**Status:** issues_found

## Summary

The embedded sbx kit materializer (`kit.rs`) is well-designed: atomic rename for concurrency safety, content-addressed cache dirs, bounded GC with path-traversal guards, and thorough test coverage including concurrency and stale-dir cleanup. The xtask `check-kit` linter correctly validates spec.yaml structure and catches naked install verbs.

Two critical bugs were found in `spec.yaml`'s startup health-check command: it references the wrong binary name for kiro (`kiro` instead of `kiro-cli`) and omits `vibe` entirely. Both agents are supported (not in `UNSUPPORTED_AGENTS`), so kits ARE materialized for them, but the startup command will fail in a single-agent container because the binary presence OR-chain finds no matching binary. Four warnings cover a misleading error message in the xtask linter, dead materialized assets, and a missing executable permission.

## Critical Issues

### CR-01: Startup command uses wrong binary name for kiro agent

**File:** `src/containers/sbx_kit/spec.yaml:10`
**Issue:** The `commands.startup` health-check runs `command -v kiro` but kiro's actual binary is `kiro-cli` (defined in `src/agents.rs:396` as `binary: "kiro-cli"`). When a kit is materialized for agent `kiro`, the `install.sh` runs `curl -fsSL https://cli.kiro.dev/install | bash` which installs the `kiro-cli` binary. The startup OR-chain then checks `command -v kiro`, which will not find `kiro-cli`. In a kiro-only container where no other agent binary is present, the entire startup check fails with `"aoe-agent: no known agent binary on PATH"`, preventing the container from starting.

This is not caught by any existing test because `spec_yaml_hook_shim_uses_workspace_path` and friends validate the spec.yaml structure but never exercise the startup command against the actual binary names defined in `src/agents.rs`.
**Fix:** Replace `command -v kiro` with `command -v kiro-cli` in the startup command:
```yaml
  startup:
    - command: ["sh", "-c", "command -v claude >/dev/null 2>&1 || command -v codex >/dev/null 2>&1 || command -v opencode >/dev/null 2>&1 || command -v gemini >/dev/null 2>&1 || command -v cursor >/dev/null 2>&1 || command -v droid >/dev/null 2>&1 || command -v pi >/dev/null 2>&1 || command -v qwen >/dev/null 2>&1 || command -v kiro-cli >/dev/null 2>&1 || command -v hermes >/dev/null 2>&1 || command -v vibe >/dev/null 2>&1 || (printf 'aoe-agent: no known agent binary on PATH\\n' >&2; exit 1)"]
```

### CR-02: Startup command missing vibe agent binary check

**File:** `src/containers/sbx_kit/spec.yaml:10`
**Issue:** The `commands.startup` health-check OR-chain checks for 10 agent binaries (claude, codex, opencode, gemini, cursor, droid, pi, qwen, kiro, hermes) but does not include `vibe`. Agent `vibe` is not in `UNSUPPORTED_AGENTS` (kit.rs:32), has a valid `install_hint` (`"pip install mistral-vibe"`), and is exercised in `install_sh_contains_install_hint_per_agent` (kit.rs:347). When `ensure_with_app_dir` is called with `agent="vibe"`, it succeeds and creates a kit directory with `install.sh` containing `pip install mistral-vibe`. However, after successful installation, the startup command fails because `command -v vibe` is not in the check chain, and no other agent binary will be present in a vibe-only container.
**Fix:** Add `command -v vibe >/dev/null 2>&1 ||` to the startup check chain. See CR-01 fix for the complete corrected line.

## Warnings

### WR-01: Lint error message hardcodes "commands.startup" for all command sections

**File:** `xtask/src/main.rs:260`
**Issue:** `lint_command_for_naked_install_verbs` always prints `"in commands.startup without idempotency guard"` in its error message, even when the function is linting `commands.install` entries. `lint_commands` (line 319) is called for both `install` entries (line 380) and `startup` entries (line 383), so a naked install verb in `commands.install` produces an error that incorrectly blames `commands.startup`. This could mislead a contributor into fixing the wrong section.
**Fix:** Add a `section` parameter:
```rust
fn lint_command_for_naked_install_verbs(command: &str, section: &str, has_error: &mut bool) {
    if is_idempotency_guarded(command) {
        return;
    }
    for verb in NAKED_INSTALL_VERBS {
        if command.contains(verb) {
            eprintln!(
                "check-kit: naked install verb `{}` in {} without idempotency guard:\n  {}",
                verb, section, command
            );
            *has_error = true;
        }
    }
}

fn lint_commands(entries: &[serde_yaml::Value], section: &str, has_error: &mut bool) {
    for entry in entries {
        if let Some(cmd_value) = entry.get("command") {
            if let Some(cmd_str) = extract_command_string(cmd_value) {
                lint_command_for_naked_install_verbs(&cmd_str, section, has_error);
            }
        }
    }
}
```

### WR-02: hook-shim.sh materialized as dead code

**File:** `src/containers/sbx/kit.rs:21,115-116`
**Issue:** `hook-shim.sh` is embedded via `include_str!` (line 21), included in the cache hash via `embedded_kit_bytes()` (line 38), written to the materialized kit directory (line 115), and asserted on in tests (line 293), but is never referenced from `spec.yaml` or any runtime code. The spec.yaml hooks all use inline `sh -c '...'` commands that duplicate the hook-shim logic. Per project CLAUDE.md: "No dead code. Never add fields/functions that nothing reads. If a field isn't used yet, don't add it; if it stops being used, remove it."

Additionally, the file lacks a shebang line and is not made executable (chmod is only applied to install.sh at lines 108-113), so even a future attempt to invoke it directly would fail.
**Fix:** Either remove the `HOOK_SHIM` constant, the `embedded_kit_bytes()` inclusion, and the `fs::write(tmp.join("hook-shim.sh"), ...)` call; or refactor `spec.yaml` initFiles hook commands to call `/sbx-kit/hook-shim.sh <status>` (with shebang and chmod added).

### WR-03: gitignore-aoe-hooks file materialized but never referenced

**File:** `src/containers/sbx/kit.rs:22,117-121`
**Issue:** Same pattern as WR-02. The `gitignore-aoe-hooks` file is embedded (line 22), hashed (line 39), materialized (lines 117-121), and asserted-on (line 295), but never referenced from spec.yaml or any other code. The gitignore append logic is handled inline in `spec.yaml` `commands.install[1]` via `grep -qsE ... || printf ...`. This is dead code per project guidelines.
**Fix:** Remove the `GITIGNORE_AOE_HOOKS` constant, the `embedded_kit_bytes()` inclusion, and the `fs::write(tmp.join("gitignore-aoe-hooks"), ...)` call.

### WR-04: hook-shim.sh not made executable on Unix

**File:** `src/containers/sbx/kit.rs:115`
**Issue:** `hook-shim.sh` is written to the kit directory at line 115, but `set_permissions(..., 0o755)` (lines 108-113) is only applied to `install.sh`. If WR-02 is resolved by wiring up `hook-shim.sh` rather than removing it, the file needs the same executable permission. As currently written, `sh /sbx-kit/hook-shim.sh` would work (sh reads the file regardless of execute bit), but direct invocation (`/sbx-kit/hook-shim.sh`) would fail with EACCES/ENOEXEC.
**Fix:** If keeping hook-shim.sh, add chmod after writing it:
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let hook_shim_path = tmp.join("hook-shim.sh");
    std::fs::set_permissions(&hook_shim_path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("chmod 0o755 {}", hook_shim_path.display()))?;
}
```

---

_Reviewed: 2026-05-16T19:45:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
