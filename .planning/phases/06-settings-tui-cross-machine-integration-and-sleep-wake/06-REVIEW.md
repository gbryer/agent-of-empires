---
phase: 06-settings-tui-cross-machine-integration-and-sleep-wake
reviewed: 2026-05-16T12:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - src/process/linux.rs
  - src/process/macos.rs
  - src/process/mod.rs
  - src/tui/settings/fields.rs
  - tests/e2e/sandbox.rs
  - tests/web_session_create.rs
findings:
  critical: 2
  warning: 4
  info: 2
  total: 8
status: fixed
---

# Phase 6: Code Review Report

**Reviewed:** 2026-05-16T12:00:00Z
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Reviewed the process management modules (linux, macos, mod), TUI settings field definitions, and two test files. The main concerns are: (1) a potential panic in the Linux `/proc/*/stat` parser when the stat content is truncated or malformed, (2) an overly broad string match in the Linux wake handler that can trigger spurious clock resyncs, (3) early-return-on-error semantics in a loop that silently aborts the entire search, and (4) an unchecked null pointer in the macOS IOKit FFI path.

## Critical Issues

### CR-01: Potential panic in `parse_stat_field` on truncated `/proc/*/stat` content

**File:** `src/process/linux.rs:117`
**Issue:** The expression `&content[close_paren + 2..]` can panic with an out-of-bounds index if the content string ends at or immediately after the closing parenthesis. For example, if `/proc/<pid>/stat` contains `"1 (x)"` (5 bytes), `rfind(')')` returns `Some(4)`, and `4 + 2 = 6 > 5`, causing a panic. While a fully-formed `/proc/pid/stat` line always has fields after the comm field, the file is read from `/proc` which races with process lifecycle; a process exiting mid-read or a malformed entry can produce truncated content. This function is called in hot paths including `build_children_map`, `get_foreground_pid`, and `find_process_in_group`.
**Fix:**
```rust
fn parse_stat_field(content: &str, field_idx: usize) -> Option<i64> {
    let close_paren = content.rfind(')')?;
    // Ensure there are at least 2 more bytes (") " separator) after the closing paren
    if close_paren + 2 > content.len() {
        return None;
    }
    let after_comm = &content[close_paren + 2..];

    let adjusted_idx = field_idx.checked_sub(2)?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    fields.get(adjusted_idx)?.parse().ok()
}
```

### CR-02: Overly broad "false" string match in Linux wake handler triggers spurious resyncs

**File:** `src/process/linux.rs:169`
**Issue:** The check `line.contains("false")` matches any line from `busctl monitor` output that contains the substring "false" anywhere, not just the `PrepareForSleep` signal's boolean argument. The `busctl monitor` output is multi-line structured text with metadata fields, type annotations, and property values. A line like `BOOLEAN false` is emitted for the signal argument, but the monitor could also emit lines containing "false" from other signals on the same bus match or from metadata text (e.g., if a process name or path contained "false"). Each spurious match triggers `resync_sbx_clocks`, which executes `sbx ls --json` and potentially `sbx exec <name> date` on every sandbox, causing unnecessary subprocess spawns.
**Fix:**
```rust
// Match the actual busctl monitor output format for the boolean argument
if line.trim() == "BOOLEAN false"
    || line.contains("PrepareForSleep(false)")
{
    tracing::info!(target: "process.wake", "host woke from sleep; resyncing sbx clocks");
    super::resync_sbx_clocks();
}
```
Alternatively, use `busctl --json monitor` if available and parse the structured JSON output.

## Warnings

### WR-01: `find_process_in_group` uses `?` inside loop, causing silent early return on transient errors

**File:** `src/process/linux.rs:87,96`
**Issue:** Lines 87 and 96 use the `?` operator on `Option` values inside a `for` loop. On line 87, `entry.ok()?` returns `None` from the entire function if a single directory entry fails to read (e.g., a `/proc/<pid>` disappears between readdir and stat). On line 96, `name_str.parse().ok()?` has the same problem, though in practice the all-digits check on line 92 makes this less likely. The effect is that a single transient `/proc` error aborts the search, returning `None` as if no process in the group was found, even when the target process still exists later in the iteration.
**Fix:**
```rust
for entry in fs::read_dir(proc_dir).ok()? {
    let Ok(entry) = entry else { continue };
    let name = entry.file_name();
    let name_str = name.to_string_lossy();

    if !name_str.chars().all(|c| c.is_ascii_digit()) {
        continue;
    }

    let Ok(pid) = name_str.parse::<u32>() else { continue };
    // ... rest of loop body
```

### WR-02: macOS IOKit FFI does not check for null return from `IONotificationPortGetRunLoopSource`

**File:** `src/process/macos.rs:180`
**Issue:** `IONotificationPortGetRunLoopSource(notify_port)` can return a null `CFRunLoopSourceRef` if the notification port is invalid or if there is an internal error. The code passes the result directly to `CFRunLoopAddSource` without a null check. Passing a null source to `CFRunLoopAddSource` is undefined behavior (it will typically crash with `EXC_BAD_ACCESS`).
**Fix:**
```rust
unsafe {
    let source = IONotificationPortGetRunLoopSource(notify_port);
    if source.is_null() {
        tracing::warn!(
            target: "process.wake",
            "IONotificationPortGetRunLoopSource returned null"
        );
        return;
    }
    let run_loop = CFRunLoopGetCurrent();
    CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode);
    CFRunLoopRun();
}
```

### WR-03: Linux wake handler spawns orphan `busctl` child process with no cleanup on exit

**File:** `src/process/linux.rs:134-174`
**Issue:** The `register_wake_handler` function spawns a `busctl monitor` subprocess and reads its stdout in a background thread, but the `child` handle is never stored or cleaned up. When the main application exits, the `busctl` process becomes orphaned (its parent changes to PID 1). While orphaned processes are cleaned up by init, this is unclean: the process keeps running and holding a D-Bus monitor connection until it is killed or the system reboots. Additionally, if the main process is killed with SIGKILL, the orphaned `busctl` process persists indefinitely.
**Fix:** Store the `Child` handle in a module-level `OnceLock<std::process::Child>` or similar, and kill it in an `atexit` handler or `Drop` guard. Alternatively, set the child to be in the same process group and ensure it is killed on parent exit.

### WR-04: `u32` to `i32` cast for PID can overflow for PIDs above `i32::MAX`

**File:** `src/process/mod.rs:93,106,115,167`
**Issue:** All calls to `Pid::from_raw(p as i32)` cast `u32` PIDs to `i32` without checking for overflow. While Linux limits PIDs to at most ~4 million by default (`/proc/sys/kernel/pid_max` max is 2^22), the kernel *can* be configured to allow PIDs up to 2^22 on 64-bit systems, and the data types involved (`u32` from `ps` output or `/proc` parsing) allow values up to 2^32-1. A PID above `i32::MAX` (2,147,483,647) would wrap to a negative value, and `Pid::from_raw` with a negative value represents a process group ID, not a single process. This would cause signals to be sent to the wrong target (a process group rather than a process).
**Fix:**
```rust
fn pid_to_nix(pid: u32) -> Option<Pid> {
    i32::try_from(pid).ok().map(Pid::from_raw)
}
```
Then use `if let Some(nix_pid) = pid_to_nix(p) { kill(nix_pid, signal) }` instead of bare casts.

## Info

### IN-01: Misleading comment grouping in `apply_field_to_global`

**File:** `src/tui/settings/fields.rs:1815-1823`
**Issue:** The `// Sandbox` comment on line 1815 groups `YoloModeDefault`, `StrictHotkeys`, and `AgentStatusHooks` under the Sandbox heading, but these fields all write to `config.session.*` (Session category). The match arms are functionally correct; only the comment is misleading.
**Fix:** Move these three arms below a `// Session` comment, after the Sandbox section ends at line 1853 (after `ContainerRuntime`).

### IN-02: `web_session_create_sbx` test body is a no-op placeholder

**File:** `tests/web_session_create.rs:27-46`
**Issue:** The test `web_session_create_sbx` performs the `sbx_available()` check twice (once redundantly at line 28 after the gate, and again at line 35-39 via `sbx.is_available()`), then prints a message and returns. It serves only as a compilation gate, not as an actual integration test. The function name and doc comment suggest it tests web session creation, which it does not. This could mislead developers into thinking the integration path is tested.
**Fix:** Either rename the test to `web_session_create_sbx_compiles` (or similar) to clarify its purpose, or implement the actual test body described in the comments. Additionally, remove the redundant `sbx_available()` check at line 28 since the `#[ignore]` attribute already prevents it from running in normal CI.

---

_Reviewed: 2026-05-16T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
