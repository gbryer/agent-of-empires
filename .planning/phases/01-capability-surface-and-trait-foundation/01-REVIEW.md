---
phase: 01-capability-surface-and-trait-foundation
reviewed: 2026-05-15T00:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - src/containers/container_interface.rs
  - src/containers/mod.rs
  - src/containers/runtime.rs
  - src/containers/runtime_base.rs
  - src/session/config.rs
  - src/tui/settings/fields.rs
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-05-15
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Phase 1 introduces the `RuntimeCapabilities` struct, migrates two existing capability flags into it, adds five new flags, wires `ContainerRuntimeName::Sbx` through all dispatch sites, and stubs out the Sbx backend with `is_available()` returning false unconditionally.

The safety model is sound: `builder.rs` gates sandbox session creation on `is_available()`, which short-circuits every live code path for Sbx. All five `match self.kind` arms in `runtime.rs` have explicit Sbx stubs that return safe values. The `fields.rs` reverse-mapping fix (changing `_ => AppleContainer` to `2 => AppleContainer, _ => Sbx`) is correct.

Three issues require attention before Phase 2 ships, because they are load-bearing assumptions that Phase 2 will rely on:

1. Five of seven capability flags are declared but never consumed as behavioral gates — the struct doc comment promises more than the implementation delivers.
2. The `exec_command` Sbx stub silently discards both `options` and `cmd`, producing an incomplete tmux exec string that would break container terminal mode if anything reaches it.
3. A test comment misidentifies the count of `match self.kind` dispatch sites.

---

## Warnings

### WR-01: Five `RuntimeCapabilities` flags declared but never used as behavioral gates

**File:** `src/containers/runtime_base.rs:248-269` (anonymous volumes loop), `src/containers/runtime_base.rs:266-269` (port mappings loop)

**Issue:** The `RuntimeCapabilities` struct doc comment (`container_interface.rs:53-58`) states the flags exist to "let new flags fan out without silently producing broken output for any backend that forgot to opt in." Only `supports_read_only_volumes` (line 232) and `supports_remove_volumes` (line 355) are actually checked in `build_create_args` and `remove`. The five new flags (`supports_port_publish_at_create`, `supports_image_pull`, `supports_anonymous_volumes`, `supports_arbitrary_volume_paths`, `supports_dynamic_port_publish`) have no consumers anywhere outside test assertions.

Practical consequence for Phase 2: when Sbx is enabled, `build_create_args` will emit `-v PATH` anonymous volume flags and `-p HOST:CONTAINER` port mapping flags for any Sbx container — both are explicitly `false` in `RuntimeBase::SBX.capabilities`. The capability matrix will have promised false but the args builder will have silently emitted them anyway.

**Fix:** Gate the corresponding loops in `build_create_args` on their flags, matching the existing pattern for `supports_read_only_volumes`:

```rust
// Anonymous volumes
if self.capabilities.supports_anonymous_volumes {
    for path in &config.anonymous_volumes {
        args.push("-v".to_string());
        args.push(path.clone());
    }
} else if !config.anonymous_volumes.is_empty() {
    tracing::warn!(
        "{} does not support anonymous volumes; {} path(s) skipped",
        self.name,
        config.anonymous_volumes.len()
    );
}

// Port mappings
if self.capabilities.supports_port_publish_at_create {
    for port in &config.port_mappings {
        args.push("-p".to_string());
        args.push(port.clone());
    }
} else if !config.port_mappings.is_empty() {
    tracing::warn!(
        "{} does not support port publish at create; {} mapping(s) skipped",
        self.name,
        config.port_mappings.len()
    );
}
```

`supports_arbitrary_volume_paths` should gate the bind-mount loop (`for vol in &config.volumes`), and `supports_image_pull` should gate the `pull_image` call in `ensure_image`. `supports_dynamic_port_publish` is metadata for callers, not for `build_create_args`, but it still needs at least one consumer or it belongs in a comment, not a public flag.

---

### WR-02: `exec_command` Sbx stub silently drops `options` and `cmd` arguments

**File:** `src/containers/runtime.rs:240-249`

**Issue:** The Sbx arm of `exec_command` discards both arguments with `let _ = options; let _ = cmd;` and returns `format!("sbx exec {}", name)`. The string returned by `exec_command` is used in `session/instance.rs:818-820` to build the tmux exec string that opens a terminal into the container. For Docker/Podman the returned string includes the working directory option and the agent command (e.g., `docker exec -it -w /workspace ... /bin/bash`). The Sbx stub would produce `sbx exec aoe-sandbox-abc12345`, silently omitting the working directory, environment, and agent command.

While `is_available()` prevents any Sbx session from being created in Phase 1, Phase 2's real implementation must replace this stub. The risk is that a Phase 2 developer assuming the stub is "close enough" ships broken container terminal mode before noticing.

**Fix:** Add a note in the stub body calling out explicitly what the return string must include, not just that it will be replaced:

```rust
RuntimeKind::Sbx => {
    // Phase 1 stub. Phase 2 (RT-04) must replace with the real sbx exec
    // shape, which must include: workdir option, env forwarding, name, and cmd.
    // The current stub omits options and cmd; any use of this string as a
    // real exec target will produce an incomplete command.
    let _ = options;
    let _ = cmd;
    format!("sbx exec {}", name)
}
```

Or, stronger: return a clearly invalid string that will fail visibly rather than silently running `sbx exec <name>` without the required arguments:

```rust
RuntimeKind::Sbx => {
    // Phase 1 stub: Sbx exec shape is not yet known.
    // Sbx is not available (is_available returns false), so this is
    // unreachable in normal flows. Phase 2 (RT-04) replaces this.
    format!("sbx exec {} /* Phase 1 stub: cmd and options dropped */", name)
}
```

---

### WR-03: `match self.kind` dispatch site count is wrong in test comment

**File:** `src/containers/runtime.rs:521-522`

**Issue:** The comment reads "The four match-self.kind dispatch sites must each have a safe Sbx arm." There are five such sites: `is_available` (line 75), `does_container_exist` (line 114), `is_container_running` (line 140), `exec_command` (line 210), and `batch_running_states` (line 257). The test itself only exercises four (correctly excluding `is_available` since it has its own dedicated test), but the count in the comment is wrong. A reviewer or Phase 2 developer auditing dispatch sites would undercount by one.

**Fix:**

```rust
// The five match-self.kind dispatch sites must each have a safe Sbx
// arm. is_available short-circuits to false, so callers never reach
// these paths in normal flows; the safety here is defense-in-depth
// for any future caller that bypasses the availability gate.
// (is_available itself is a sixth match site, tested separately in
// test_sbx_is_available_returns_false_unconditionally.)
```

---

## Info

### IN-01: Select reverse-mapping uses `_ =>` wildcard instead of explicit index for Sbx

**File:** `src/tui/settings/fields.rs:1827`, `src/tui/settings/fields.rs:2116`

**Issue:** Both `apply_field_to_global` and `apply_field_to_profile` reverse-map the `ContainerRuntime` select index to a `ContainerRuntimeName` variant using `_ => ContainerRuntimeName::Sbx`. There are exactly four options (indices 0-3), so this is functionally correct today. However, if a fifth runtime is added in a future phase and the options list grows to five entries without updating the match, the new runtime at index 4 would silently be stored as `Sbx`.

The existing pattern in this file for other three-option selects (e.g., `TmuxStatusBarMode`, `TmuxMouseMode`, `TmuxClipboardMode`) also uses `_ =>` for the last variant, so this is consistent. Still, for a user-visible setting where a mismap would silently corrupt config, an explicit `3 =>` followed by an `unreachable!()` or log-and-default fallback would be safer.

**Fix:**
```rust
config.sandbox.container_runtime = match selected {
    0 => ContainerRuntimeName::Docker,
    1 => ContainerRuntimeName::Podman,
    2 => ContainerRuntimeName::AppleContainer,
    3 => ContainerRuntimeName::Sbx,
    _ => {
        tracing::warn!("unknown container_runtime index {}, defaulting to Docker", selected);
        ContainerRuntimeName::Docker
    }
};
```

---

### IN-02: `pull_image` on Sbx with empty `pull_prefix` produces a confusing command

**File:** `src/containers/runtime_base.rs:159-193`, `src/containers/runtime_base.rs:103`

**Issue:** `RuntimeBase::SBX` sets `pull_prefix: &[]` with the comment "pull_prefix is intentionally empty because supports_image_pull is false." The `pull_image` implementation appends `self.pull_prefix` args then appends the image name. With an empty prefix the resulting command is `sbx <image_name>` (e.g., `sbx ghcr.io/njbrake/aoe-sandbox:latest`). This is unreachable in Phase 1 because `is_available()` returns false, but if any code path calls `pull_image` or `ensure_image` on an Sbx runtime — directly or via a future test harness — the resulting command would interpret the full image URI as an `sbx` subcommand, producing a confusing error with no indication that image pull is unsupported.

**Fix:** Add a guard in `pull_image` (or `ensure_image`) against runtimes that declare `supports_image_pull: false`:

```rust
pub fn ensure_image(&self, image: &str) -> Result<()> {
    if !self.capabilities.supports_image_pull {
        return Err(DockerError::CommandFailed(format!(
            "{} does not support image pull; ensure the image '{}' is available through other means",
            self.name, image
        )));
    }
    // ... existing logic
}
```

This is consistent with WR-01's recommendation and closes the path where an empty `pull_prefix` produces a nonsense command.

---

_Reviewed: 2026-05-15_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
