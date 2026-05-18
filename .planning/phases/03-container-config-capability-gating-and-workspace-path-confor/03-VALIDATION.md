---
phase: 3
slug: container-config-capability-gating-and-workspace-path-confor
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-15
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in `#[cfg(test)]`) |
| **Config file** | `Cargo.toml` (no separate test config) |
| **Quick run command** | `cargo test -p agent-of-empires --lib session::container_config` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30s (quick) / ~3-5 min (full workspace) |

---

## Sampling Rate

- **After every task commit:** Run quick command (`cargo test --lib session::container_config`)
- **After every plan wave:** Run full suite (`cargo test --workspace`)
- **Before `/gsd:verify-work`:** Full suite must be green, plus `cargo fmt --check` and `cargo clippy -- -D warnings`
- **Max feedback latency:** 30 seconds (quick); 5 min (full)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (populated by planner) | — | — | RT-04 / RT-06 | — | — | unit | `cargo test --lib` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*Planner: fill this table once tasks are decomposed in PLAN.md. Map each new test added to `container_config.rs::tests` (or `instance.rs::tests`) to its SC and REQ-ID. SC-1 = three workspace-shape tests under sbx caps. SC-2 = Docker baseline-unchanged test. SC-3 = `InconformableExtraVolume` downcast test. SC-4 = `ensure_image` skip test (source-substring + matrix per RESEARCH.md §SC-4).*

---

## Wave 0 Requirements

- [x] `src/session/container_config.rs::tests` — existing test module at ~line 1156+ (8+ tests for `compute_volume_paths`; integration-style tests at lines 2301, 2395, 2543, 2658, 2871). Phase 3 adds new tests into the same module.
- [x] `cargo test` — already configured via `Cargo.toml`; no install needed.
- [ ] Test helper for synthetic `RuntimeCapabilities` literals — `RuntimeBase::DOCKER.capabilities` and `RuntimeBase::SBX.capabilities` already exist (Phase 1); tests use them directly. No new fixture file required.

*Existing infrastructure covers all phase requirements. Wave 0 is effectively a no-op — Phase 1 already locked the capability literals and Phase 3's tests slot into the existing `mod tests` block.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | — | — |

*All phase behaviors have automated verification. SC-1, SC-2, SC-3, SC-4 each have unit-test coverage per RESEARCH.md Validation Architecture. No manual reproduction required.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (N/A here — none missing)
- [ ] No watch-mode flags (cargo test runs once and exits)
- [ ] Feedback latency < 30s (quick) / < 5 min (full)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
