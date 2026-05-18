---
phase: 6
slug: settings-tui-cross-machine-integration-and-sleep-wake
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-16
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | none |
| **Quick run command** | `cargo test -p aoe settings::fields -- --include-ignored` |
| **Full suite command** | `cargo test && cargo test --test e2e sbx -- --ignored` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p aoe settings::fields`
- **After every plan wave:** Run `cargo test && cargo test --test e2e sbx -- --ignored`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | 01 | 1 | SET-02 | — | N/A | unit | `cargo test settings::fields::tests::cross_product` | ❌ W0 | ⬜ pending |
| TBD | 01 | 1 | RT-01 | — | N/A | unit | `cargo test settings::fields` | ✅ | ⬜ pending |
| TBD | 01 | 1 | INT-01 | — | N/A | integration | `cargo test --features serve --test web_session_create -- --ignored` | ❌ W0 | ⬜ pending |
| TBD | 02 | 2 | INT-02 | — | N/A | unit | `cargo test process::macos::tests::wake_handler` | ❌ W0 | ⬜ pending |
| TBD | 02 | 2 | TEST-04 | — | N/A | e2e | `cargo test --test e2e sbx_runtime_selector -- --ignored` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/tui/settings/fields.rs` — cross-product test stub for SET-02
- [ ] `tests/e2e/sbx.rs` — e2e test file for runtime selector TUI test (TEST-04)
- [ ] `src/process/macos.rs` — wake handler unit test stub (INT-02)
- [ ] `src/process/linux.rs` — wake handler unit test stub (INT-02)
