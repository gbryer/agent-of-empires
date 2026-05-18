---
phase: 05-full-sbxruntime-integration-subprocess-lifecycle-port-publis
reviewed: 2026-05-16T12:00:00Z
depth: deep
files_reviewed: 12
files_reviewed_list:
  - src/containers/container_interface.rs
  - src/containers/mod.rs
  - src/containers/runtime.rs
  - src/containers/runtime_base.rs
  - src/containers/sbx/argv.rs
  - src/containers/sbx/kit.rs
  - src/containers/sbx/mod.rs
  - src/containers/sbx/parse.rs
  - src/containers/sbx/ports.rs
  - src/session/container_config.rs
  - tests/sandbox_integration.rs
  - tests/sbx_integration.rs
findings:
  critical: 0
  warning: 0
  info: 1
  total: 1
status: fixed
fixed:
  - CR-01
  - WR-01
  - WR-02
---

# Phase 5: Code Review Report

**Reviewed:** 2026-05-16T12:00:00Z
**Depth:** deep
**Files Reviewed:** 12
**Status:** issues_found

## Summary

The sbx runtime integration is well-structured with clear separation of concerns (argv builders, parse module, ports module, kit materializer). The capability matrix pattern is sound and the test coverage is thorough. However, the `exec_command` dispatch arm for sbx has a critical parsing bug that will break container terminal sessions and agent launches when environment variables are configured. Two additional warnings address a dead-code fallback parser and a missing env-forwarding pathway.

## Critical Issues

### CR-01: exec_command sbx dispatch incorrectly parses options string, corrupting workdir and dropping env vars [FIXED]

**File:** `src/containers/runtime.rs:357-361`
**Issue:** The sbx `exec_command` dispatch arm parses the `options` parameter by calling `strip_prefix("-w ")` and treating the remainder as a workdir. However, callers (e.g., `src/session/instance.rs:819`) pass compound strings like `"-w /workspace/project -e KEY1 -e KEY2=val "`. The `strip_prefix` returns the entire suffix including `-e` flags, so the "workdir" passed to `build_exec_args` becomes `"/workspace/project -e KEY1 -e KEY2=val"` (a garbage path). Additionally, the `&[]` empty slice passed as env means all environment variables are silently dropped.

A second call site (`instance.rs:994`) passes `options` as just the env part (e.g., `"-e AOE_INSTANCE_ID=abc -e KEY1 "`), without a `-w` prefix. Here `strip_prefix("-w ")` returns `None`, so workdir is lost entirely and env vars are again discarded.

This means: (1) any sbx container terminal launched with environment config will receive a malformed `-w` path causing exec failure; (2) environment variables like `AOE_INSTANCE_ID` needed for hook status detection are never forwarded into the sandbox exec.

**Fix:**
```rust
RuntimeKind::Sbx => {
    // Parse the compound options string into structured components.
    // The caller passes a space-joined string of docker-style flags:
    //   "-w /path -e KEY1 -e KEY2=val"
    // Split into individual tokens and extract -w and -e pairs.
    let mut workdir: Option<&str> = None;
    let mut env_entries: Vec<crate::containers::container_interface::EnvEntry> = Vec::new();

    if let Some(opts) = options {
        let tokens: Vec<&str> = opts.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            match tokens[i] {
                "-w" if i + 1 < tokens.len() => {
                    workdir = Some(tokens[i + 1]);
                    i += 2;
                }
                "-e" if i + 1 < tokens.len() => {
                    let entry = tokens[i + 1];
                    if let Some((k, v)) = entry.split_once('=') {
                        env_entries.push(EnvEntry::Literal {
                            key: k.to_string(),
                            value: v.to_string(),
                        });
                    } else {
                        env_entries.push(EnvEntry::Inherit {
                            key: entry.to_string(),
                            value: String::new(),
                        });
                    }
                    i += 2;
                }
                _ => { i += 1; }
            }
        }
    }

    let cmd_parts = [cmd];
    let args = sbx::argv::build_exec_args(
        name, workdir, &env_entries, true, true, &cmd_parts,
    );
    std::iter::once("sbx".to_string())
        .chain(args)
        .collect::<Vec<_>>()
        .join(" ")
}
```

## Warnings

### WR-01: Fallback agent_name parser uses wrong prefix, always defaults to "claude" [FIXED]

**File:** `src/containers/runtime.rs:245-249`
**Issue:** The fallback parser attempts `name.strip_prefix("aoe_")` (underscore), but `DockerContainer::generate_name` produces names with hyphens: `"aoe-sandbox-{id}"`. The strip_prefix will never match, so the fallback always yields `"claude"`. While the primary path always provides `config.agent_name` (set by `build_container_config`), any future caller that omits `agent_name` will silently get the wrong kit materialized.

**Fix:**
```rust
let agent_name = config.agent_name.as_deref().unwrap_or_else(|| {
    name.strip_prefix("aoe-sandbox-")
        .and_then(|rest| rest.split('-').next())
        .unwrap_or("claude")
});
```

Note: Even with the corrected prefix, the container name `aoe-sandbox-{truncated_id}` does not encode the agent name, so this fallback will still not resolve correctly. The real fix is to ensure `config.agent_name` is always populated (which it currently is through `build_container_config`). Consider removing the dead fallback or adding a `tracing::warn!` when it fires.

### WR-02: SbxRuntime::run does not forward inherited env vars to the subprocess [FIXED]

**File:** `src/containers/sbx/mod.rs:53-73`
**Issue:** The `run` helper spawns `sbx` via `Command::new(&self.binary).args(args).output()` without explicitly setting environment variables via `.env(key, value)`. For the `exec` path, `EnvEntry::Inherit` entries are encoded as bare keys in argv (`-e KEY`), relying on sbx reading the value from its own process environment. Since Rust's `Command` inherits the parent process environment by default, this works when the secret is already exported in the aoe process. However, the Docker path (`runtime_base.rs:328-331`) explicitly calls `.env(key, value)` for each Inherit entry during container creation, providing an explicit value even if the aoe process was launched differently. For `SbxRuntime::create_container`, inherited env vars configured via `config.environment` (e.g., API keys forwarded via `sandbox.environment` config) are never set on the child Command, meaning sbx won't have them unless they happen to be in aoe's ambient environment.

**Fix:** In `SbxRuntime::create_container` (and potentially `exec`), thread `config.environment` Inherit entries through to the Command builder:
```rust
let mut cmd = Command::new(&self.binary);
cmd.args(&arg_refs);
for entry in &config.environment {
    if let EnvEntry::Inherit { key, value } = entry {
        cmd.env(key, value);
    }
}
let output = cmd.output().map_err(|e| ...)?;
```

## Info

### IN-01: Trait-level build_create_args for sbx always passes kit_path=None

**File:** `src/containers/runtime.rs:225`
**Issue:** The `ContainerRuntimeInterface::build_create_args` implementation for sbx passes `None` for `kit_path`, meaning the trait method's output never includes `--kit`. The actual creation path (`create_container`) materializes the kit and passes it correctly. This makes the trait method's output misleading for dry-run display or debugging purposes (it omits `--kit` even though the real invocation always includes it).

**Fix:** Document in a comment that the trait method is preview-only and will not include `--kit` (which requires agent context not available at this call site). Alternatively, accept `agent_name` on the trait method to enable kit resolution, though this changes the interface for all backends.

---

_Reviewed: 2026-05-16T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
