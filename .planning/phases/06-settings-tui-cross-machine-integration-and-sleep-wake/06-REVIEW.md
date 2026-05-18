---
phase: 06-settings-tui-cross-machine-integration-and-sleep-wake
reviewed: 2026-05-17T14:30:00Z
depth: standard
files_reviewed: 11
files_reviewed_list:
  - src/containers/runtime.rs
  - src/containers/sbx_kit/spec.yaml
  - src/containers/sbx/argv.rs
  - src/containers/sbx/kit.rs
  - src/containers/sbx/mod.rs
  - src/process/linux.rs
  - src/process/macos.rs
  - src/process/mod.rs
  - src/tui/settings/fields.rs
  - tests/e2e/sandbox.rs
  - tests/web_session_create.rs
findings:
  critical: 1
  warning: 3
  info: 1
  total: 5
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-05-17T14:30:00Z
**Depth:** standard
**Files Reviewed:** 11
**Status:** issues_found

## Summary

Reviewed the sbx runtime integration (container lifecycle, argv builders, kit materializer, port publishing), OS-specific process management (Linux D-Bus wake handler, macOS IOKit wake handler, clock resync), TUI settings field gating by runtime capabilities, and two test files. The implementation is generally solid with good test coverage and correct dispatch routing. Key concerns: a path input used in filesystem operations without sanitization (mitigated but fragile), incorrect fallback logic for agent name parsing in container creation, and a platform portability issue in test code.

## Critical Issues

### CR-01: Unsanitized `agent` parameter in `cache_dir` path construction

**File:** `src/containers/sbx/kit.rs:52`
**Issue:** The `cache_dir` and `cache_dir_inner` functions use `.join(agent)` directly with an arbitrary `&str` input. On Unix, if `agent` contains `/` or `..` components (e.g., `../../etc/malicious`), `PathBuf::join` would resolve the traversal. The `pub(crate)` function `cache_dir` is exposed within the crate. While the `ensure_with_app_dir` function currently bails before writing files for unrecognized agents (because `install_hint()` returns `None` and `UNSUPPORTED_AGENTS` rejects them), the `cache_dir` function itself is called from the test at line 316 with arbitrary agent names and could be called from new code paths in the future without the protective gate. Additionally, `ensure_with_app_dir` still calls `target.exists()` at line 64 with the unsanitized path before the `install_hint` check, performing a filesystem probe at an attacker-controlled location.

**Fix:**
```rust
fn cache_dir_inner(app_dir: &Path, agent: &str, bytes: &[u8]) -> PathBuf {
    // Reject agent names containing path separators or traversal components
    assert!(
        !agent.contains('/') && !agent.contains('\\') && agent != ".." && agent != ".",
        "agent name must not contain path separators or traversal: {:?}",
        agent
    );
    let digest = Sha256::digest(bytes);
    let hash_hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    let segment = format!("aoe-{}-{}", env!("CARGO_PKG_VERSION"), &hash_hex[..16]);
    app_dir.join("sbx-kit").join(segment).join(agent)
}
```

## Warnings

### WR-01: Agent name fallback parsing will never produce a valid agent name

**File:** `src/containers/runtime.rs:248-250`
**Issue:** The fallback logic attempts to extract the agent name from the container name by stripping `aoe-sandbox-` and taking the first `-`-delimited segment. However, the actual container naming convention (from `src/containers/mod.rs:80`) is `format!("aoe-sandbox-{}", truncate_id(session_id, 8))` where `session_id` is a UUID fragment, not an agent name. The `split('-').next()` call will return a UUID prefix (e.g., `"a1b2c3d4"`), which will never match a valid agent name, causing `kit::ensure` to fail with "no install_hint". The `.unwrap_or("claude")` default means only Claude sessions will work when `config.agent_name` is None.

**Fix:** Either always require `config.agent_name` to be populated by the caller (make it non-optional when `kind == Sbx`), or fix the fallback to resolve the agent name from session storage rather than parsing the container name:
```rust
let agent_name = config.agent_name.as_deref().unwrap_or("claude");
```
This explicit "claude" default is more honest than the parsing that pretends to extract a name but always fails to.

### WR-02: Test module uses Unix-only imports without platform gate

**File:** `src/containers/sbx/mod.rs:367-368`
**Issue:** The `#[cfg(test)] mod tests` block at line 363 imports `std::os::unix::fs::PermissionsExt` unconditionally. This causes a compile failure when running `cargo test` on Windows. While the project primarily targets macOS and Linux, the tests in `sbx/argv.rs` and `sbx/kit.rs` (which test pure logic) could run on any platform.

**Fix:** Gate the test module that requires Unix APIs:
```rust
#[cfg(test)]
#[cfg(unix)]
mod tests {
    // ...existing test code...
}
```

### WR-03: Unchecked `i64 as u32` cast for PID values from `/proc/*/stat`

**File:** `src/process/linux.rs:39`
**Issue:** `parse_stat_field` returns `Option<i64>`, and the result is cast to `u32` with `ppid as u32` at line 39, `tpgid as u32` at line 76, and `proc_pgrp as u32` at line 101. While real PIDs on Linux are always positive and fit in u32, the `i64 as u32` cast silently truncates. If a malformed `/proc/*/stat` file contains a negative number or a value exceeding u32::MAX (e.g., from a crafted proc filesystem), the silent truncation could cause the wrong PID to be signaled. The tpgid check at line 70 (`tpgid <= 0`) only guards one of the three cast sites.

**Fix:** Use `u32::try_from()` with a fallback:
```rust
if let Some(ppid) = parse_stat_field(&content, 3) {
    if let Ok(ppid_u32) = u32::try_from(ppid) {
        children_map.entry(ppid_u32).or_default().push(child_pid);
    }
}
```

## Info

### IN-01: Placeholder test with no meaningful assertions

**File:** `tests/web_session_create.rs:27-46`
**Issue:** The `web_session_create_sbx` test is marked `#[ignore]` and its body only checks `sbx.is_available()` (duplicating the guard at line 28-31), then prints to stderr and returns. It does not exercise the web session create flow. The test provides compile-gate value only, which could be achieved with a simpler approach that does not appear to be a real test.

**Fix:** Either implement the actual web session create flow (POST to the API endpoint, verify response), or rename to `compile_gate_web_session_create_sbx` to communicate its limited purpose and prevent confusion during test triage.

---

_Reviewed: 2026-05-17T14:30:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
