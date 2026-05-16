---
phase: 04-embedded-sbx-kit-and-materialization
plan: 03
subsystem: sbx-kit
tags: [gap-closure, schema-fix, spec-yaml, xtask-linter]
dependency_graph:
  requires: [04-01, 04-02]
  provides: [corrected-spec-yaml, kind-mixin-validation, install-string-lint]
  affects: [src/containers/sbx_kit/spec.yaml, src/containers/sbx/kit.rs, xtask/src/main.rs]
tech_stack:
  added: []
  patterns: [env-based-credential-forwarding, string-format-install-commands]
key_files:
  created:
    - tests/fixtures/sbx_kits/install-array-command/spec.yaml
  modified:
    - src/containers/sbx_kit/spec.yaml
    - src/containers/sbx/kit.rs
    - xtask/src/main.rs
    - tests/xtask_check_kit.rs
    - tests/fixtures/sbx_kits/ok/spec.yaml
    - tests/fixtures/sbx_kits/guarded-install-verb/spec.yaml
    - tests/fixtures/sbx_kits/missing-schema-version/spec.yaml
    - tests/fixtures/sbx_kits/naked-install-verb/spec.yaml
decisions:
  - "kind: mixin is correct for overlay kits; kind: agent is for standalone agent definitions"
  - "Install commands must be strings per sbx YAML unmarshal; startup commands stay as arrays"
  - "SSH credential forwarding uses env: [SSH_AUTH_SOCK] not agent: ssh-agent"
metrics:
  duration: 8m
  completed: 2026-05-16T08:51:00Z
  tasks_completed: 2
  tasks_total: 2
  files_modified: 9
---

# Phase 04 Plan 03: Gap Closure for spec.yaml Schema Fixes Summary

Corrected three schema errors in the embedded sbx kit spec.yaml that caused `sbx kit validate` to fail: kind: agent to kind: mixin, array install commands to string format, and agent: ssh-agent to env: [SSH_AUTH_SOCK].

## Completed Tasks

| # | Task | Commit | Key Files |
|---|------|--------|-----------|
| 1 | Fix spec.yaml and update all fixtures | 51c947d | src/containers/sbx_kit/spec.yaml, tests/fixtures/sbx_kits/*/spec.yaml |
| 2 | Update kit.rs unit tests and xtask check-kit linter | 9b9a0e4 | src/containers/sbx/kit.rs, xtask/src/main.rs, tests/xtask_check_kit.rs |

## What Changed

### spec.yaml corrections (Task 1)
- `kind: agent` changed to `kind: mixin` because the kit is used as a `--kit` overlay on top of `sbx create <agent>`, not as a standalone agent definition
- `commands.install` entries changed from array format (`["sh", "-c", "..."]`) to string format (`"..."`) because sbx parses install commands as shell strings and cannot unmarshal sequences
- `credentials.sources.ssh` changed from `agent: ssh-agent` to `env: [SSH_AUTH_SOCK]` matching the actual sbx credential forwarding schema
- `[ASSUMED]` comment removed since the schema is now validated
- All four test fixtures updated to use `kind: mixin` and string install commands

### Linter and test updates (Task 2)
- Renamed `spec_yaml_declares_credentials_sources_ssh_agent` test to `spec_yaml_declares_credentials_sources_ssh_env`, now asserts SSH_AUTH_SOCK exists in `credentials.sources.ssh.env` sequence
- Removed unused `leaf_strings` recursive helper from the old test
- Added `kind` value validation to check-kit linter (must be "agent" or "mixin")
- Added install command string-format lint that rejects array-format `commands.install` entries with a clear error message
- Created `install-array-command` fixture and `install_command_array_format_fails` integration test

## Verification Results

- `cargo test --lib containers::sbx::kit`: 17 passed, 0 failed
- `cargo test --test xtask_check_kit`: 6 passed, 0 failed (including new `install_command_array_format_fails`)
- `cargo xtask check-kit`: OK (embedded spec.yaml passes)
- `cargo clippy --lib -- -D warnings`: clean
- `cargo fmt --all -- --check`: clean
- Pre-existing clippy warning in `src/tui/dialogs/serve.rs` (duplicated_attributes) is out of scope

## Deviations from Plan

None; plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None; all changes are within the existing trust boundary (spec.yaml to sbx CLI, xtask linter to spec.yaml).
