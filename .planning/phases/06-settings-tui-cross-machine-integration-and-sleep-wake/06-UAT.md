---
status: complete
phase: 06-settings-tui-cross-machine-integration-and-sleep-wake
source: [06-01-SUMMARY.md, 06-02-SUMMARY.md, 06-03-SUMMARY.md]
started: 2026-05-17T17:15:00Z
updated: 2026-05-17T17:25:00Z
---

## Current Test

[testing complete]

## Auto-Verified (by CI/programmatic checks)

- [x] `sbx create` no longer emits invalid `-e` flag (the bug you hit)
- [x] Session creation via CLI succeeds (`aoe add --sandbox --cmd claude`)
- [x] Session starts and reaches "running" in sbx daemon
- [x] SSH relay socket exists at /run/ssh-agent.sock inside sandbox
- [x] SSH keys visible via ssh-add -l inside sandbox
- [x] ssh -T git@github.com authenticates as gbryer inside sandbox
- [x] Field visibility: Docker shows 13 sandbox fields, Sbx shows 10 (ExtraVolumes, VolumeIgnores, MountSsh hidden)
- [x] 81 sbx unit tests pass (argv, kit, lifecycle, ports, parsing)
- [x] 4 wake handler filter tests pass (correct candidate selection)
- [x] Clippy clean, cargo fmt clean

## Tests

### 1. Sbx Session Creation from TUI
expected: Open aoe TUI, press 'a' to add session with sandbox enabled. Session appears as Running within a few seconds.
result: pass

### 2. Settings Field Visibility
expected: Open Settings > Sandbox. With Sbx runtime: you see ~10 fields (no ExtraVolumes, VolumeIgnores, MountSsh). Switch to Docker: all 13 fields visible.
result: pass

### 3. Dynamic Field Refresh
expected: While in Settings > Sandbox, change Container Runtime between Docker and Sbx. Field list updates immediately without leaving the menu.
result: pass

### 4. Container Runtime Field Position
expected: In Settings > Sandbox, Container Runtime is the second field (after Enabled by Default).
result: pass

### 5. Sandbox Agent Runs Inside Container
expected: With a running sbx session, attach to it (Enter in TUI). You're inside the sandbox with agent running or ready to prompt.
result: pass

### 6. Git Operations Work in Sandbox
expected: Inside an attached sbx session, run git fetch/pull on a repo with SSH remote. Should succeed (no Permission denied).
result: pass

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none]
