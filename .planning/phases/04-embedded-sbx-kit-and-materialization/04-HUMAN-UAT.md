---
status: resolved
phase: 04-embedded-sbx-kit-and-materialization
source: [04-VERIFICATION.md]
started: 2026-05-16T17:01:00Z
updated: 2026-05-16T09:25:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Live sbx install smoke-test: spec.yaml validation against sbx runtime
expected: Run `sbx kit validate src/containers/sbx_kit/` and confirm the embedded spec.yaml is accepted by the live sbx CLI. The credentials.sources ssh-agent schema (flagged `[ASSUMED]`), the `kind` field, and the `commands` format must all match what sbx actually expects.
result: issue
reported: "sbx kit validate fails with 3 errors: (1) kind: agent should be kind: mixin for kit overlays, (2) commands.install[].command uses array format [\"sh\", \"-c\", \"...\"] but sbx expects a string, (3) credentials.sources.ssh uses assumed `agent: ssh-agent` schema but sbx requires `env: [SSH_AUTH_SOCK]`"
severity: blocker
remediation: "All 3 errors fixed in gap closure plan 04-03"

### 2. Re-run sbx kit validate on corrected spec.yaml
expected: Run `sbx kit validate src/containers/sbx_kit/` on a host with sbx installed. It should exit 0 with no errors. All 3 UAT-discovered schema errors were fixed in 04-03: kind: mixin, string install commands, env: [SSH_AUTH_SOCK].
result: passed
output: "VALID: /Users/gbryer/projects/other/agent-of-empires/src/containers/sbx_kit/ (directory)"

## Summary

total: 2
passed: 1
issues: 1
pending: 0
skipped: 0
blocked: 0

## Gaps

- truth: "spec.yaml passes sbx kit validate and is accepted by the live sbx runtime"
  status: resolved
  reason: "All 3 errors fixed in gap closure plan 04-03. Re-validated: sbx kit validate returns VALID."
  severity: blocker
  test: 2
