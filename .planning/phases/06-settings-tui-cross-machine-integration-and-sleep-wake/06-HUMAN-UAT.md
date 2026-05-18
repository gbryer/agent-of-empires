---
status: partial
phase: 06-settings-tui-cross-machine-integration-and-sleep-wake
source: [06-VERIFICATION.md]
started: 2026-05-16T12:55:00Z
updated: 2026-05-17T00:30:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Live TUI Sbx Runtime Selector
expected: Open Settings > Sandbox with Sbx runtime selected; ExtraVolumes, VolumeIgnores, and MountSsh fields are NOT shown; all other sandbox fields (10 total) are visible; switching to Docker shows all 13 fields
result: issue
reported: "Fields don't update dynamically when switching runtime — need to save, leave menu, and re-enter for field visibility to reflect the new runtime. Also Container Runtime field should be promoted to the top of the Sandbox settings list."
severity: major

### 2. Sbx Sandbox Toggle in Session Creation
expected: When sbx is the configured runtime and sbx binary is installed, the Sandbox checkbox appears in the new session creation dialog
result: pass

### 3. Web Session Creation with Sbx
expected: Create a sandboxed session with sbx runtime; session shows Running status; pressing 'c' switches to container terminal
result: blocked
blocked_by: third-party
reason: "sbx daemon Docker backend not responding — sbx create fails with 'failed to connect to docker API at docker.sock'. Unrelated to aoe code; sbx diagnose passes but create fails. Needs sbx daemon restart."

### 4. Sleep/Wake Clock Resync
expected: Put Mac to sleep with running sbx sessions older than 5 minutes; on wake, tracing logs show "clock resync successful" entries for each matching sandbox
result: blocked
blocked_by: third-party
reason: "Depends on sbx daemon running to have active sbx sessions. Daemon Docker backend is down. Code fix (register_wake_handler) was applied but can't verify end-to-end."

## Summary

total: 4
passed: 2
issues: 0
pending: 0
skipped: 0
blocked: 2
skipped: 0
blocked: 0

## Gaps

- truth: "Switching runtime dynamically updates visible fields without leaving the menu; Container Runtime is the first field in Sandbox settings"
  status: failed
  reason: "User reported: Fields don't update dynamically when switching runtime — need to save, leave menu, and re-enter. Container Runtime should be at the top of the list."
  severity: major
  test: 1
  root_cause: "apply_field_to_config for Select fields (input.rs:259) never calls rebuild_fields() after ContainerRuntime changes. Also ContainerRuntime is last in build_sandbox_fields vec (fields.rs:1125) instead of first."
  artifacts:
    - path: "src/tui/settings/input.rs"
      issue: "Line 259: after apply_field_to_config for Select, no rebuild_fields() call when key is ContainerRuntime"
    - path: "src/tui/settings/fields.rs"
      issue: "Line 1125: ContainerRuntime field is last in the vec; should be first (before SandboxEnabledByDefault)"
  missing:
    - "After apply_field_to_config at input.rs:259, add: if field.key == FieldKey::ContainerRuntime { self.rebuild_fields(); }"
    - "Move ContainerRuntime SettingField from position 13 to position 1 in the build_sandbox_fields vec"
  debug_session: ""

- truth: "Sbx sandbox checkbox appears in new session dialog when sbx is installed"
  status: failed
  reason: "SbxRuntime::is_available() runs 'sbx --version' which sbx doesn't support — exits non-zero, so is_available() returns false"
  severity: blocker
  test: 4
  root_cause: "sbx binary doesn't support --version flag; is_available() probe fails"
  artifacts:
    - path: "src/containers/sbx/mod.rs"
      issue: "Line 41: .arg(\"--version\") — sbx doesn't have this flag, use 'help' instead"
  missing:
    - "Change is_available() to use 'sbx help' (exits 0) instead of 'sbx --version'"
  debug_session: ""

- truth: "On wake from sleep, tracing logs show clock resync entries for sbx sessions older than 5 minutes"
  status: failed
  reason: "register_wake_handler() is defined in src/process/mod.rs but never called from application startup — handler never registered, resync never fires"
  severity: blocker
  test: 3
  root_cause: "register_wake_handler() not invoked at app startup"
  artifacts:
    - path: "src/process/mod.rs"
      issue: "pub fn register_wake_handler() exists but is dead code — no caller"
  missing:
    - "Call process::register_wake_handler() during TUI app initialization"
  debug_session: ""
