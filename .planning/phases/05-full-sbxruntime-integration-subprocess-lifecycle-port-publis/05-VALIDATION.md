---
phase: 5
slug: full-sbxruntime-integration-subprocess-lifecycle-port-publis
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-16
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml `[[test]]` sections |
| **Quick run command** | `cargo test sbx` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test sbx`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | RT-02 | T-05-01 | serde(default) prevents panics on missing fields | unit | `cargo test sbx::parse` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | RT-05 | — | N/A | unit | `cargo test sbx::ports` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | RT-07 | — | N/A | unit | `cargo test sbx::mod::tests::wait_for_ready` | ❌ W0 | ⬜ pending |
| 05-01-04 | 01 | 1 | LIFE-01 | — | N/A | unit | `cargo test sbx` | ❌ W0 | ⬜ pending |
| 05-02-01 | 02 | 2 | TEST-01 | — | N/A | unit | `cargo test sbx` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 2 | TEST-02 | — | N/A | integration | `cargo test --test sbx_integration -- --ignored` | ❌ W0 | ⬜ pending |
| 05-02-03 | 02 | 2 | TEST-03 | — | N/A | manual-only | N/A (document) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/containers/sbx/parse.rs` — serde structs + fixture tests (RT-02)
- [ ] `src/containers/sbx/ports.rs` — publish_ports + PortPublishResult (RT-05)
- [ ] `tests/sbx_integration.rs` — lifecycle end-to-end (TEST-02)
- [ ] `tests/fixtures/sbx_ls.json` — committed fixture for parse tests (RT-02)

*Existing infrastructure covers framework installation; no new deps needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Cockpit/web session creation with sbx | TEST-03 | Requires real sbx daemon + web dashboard | Start `aoe serve`, create session with `container_runtime=sbx`, verify PTY attaches |
| Sleep/wake clock-drift observation | TEST-03 | Requires physical macOS sleep | Sleep laptop with running sbx session, wake, verify session recovers |
| Host disk-usage growth | TEST-03 | Requires multiple sandboxes over time | Create 5+ sandboxes, observe disk usage at `~/.sbx/` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
