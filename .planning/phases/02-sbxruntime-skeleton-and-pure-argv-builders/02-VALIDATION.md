---
phase: 2
slug: sbxruntime-skeleton-and-pure-argv-builders
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-15
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `cargo test`; `tempfile 3.14` (dev-dep) for the fake-binary injection test (D-10) |
| **Config file** | `Cargo.toml` only — no `pytest.ini` / `jest.config` equivalent in Rust |
| **Quick run command** | `cargo test -p agent-of-empires sbx::` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds for `sbx::` quick run; ~60–90 seconds full suite |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p agent-of-empires sbx::`
- **After every plan wave:** Run `cargo test -p agent-of-empires --lib containers::`
- **Before `/gsd:verify-work`:** `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` must all be green
- **Max feedback latency:** ~5 seconds for the sbx-scoped quick run

---

## Per-Task Verification Map

> Task IDs will be filled in by the planner once PLAN.md files are produced. The map is keyed off Roadmap success criteria (Phase 2 has no formally-completing requirements; coverage moves to Phase 5).

| Task ID | Plan | Wave | Roadmap SC | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-XX-XX | XX | 1 | SC-1 (create) | — | Pure argv composition for `sbx create shell` | unit | `cargo test -p agent-of-empires sbx::argv::tests::create_args_` | ❌ W0 | ⬜ pending |
| 02-XX-XX | XX | 1 | SC-1 (exec) | — | Pure argv composition for `sbx exec` covering `-i`/`-t`/`-w`/`-e` permutations | unit | `cargo test -p agent-of-empires sbx::argv::tests::exec_args_` | ❌ W0 | ⬜ pending |
| 02-XX-XX | XX | 1 | SC-1 (ports) | — | Pure argv composition for `sbx ports <name> --publish <spec>` | unit | `cargo test -p agent-of-empires sbx::argv::tests::ports_args_` | ❌ W0 | ⬜ pending |
| 02-XX-XX | XX | 1 | SC-2 | — | `SbxRuntime::capabilities()` returns sbx-correct 7-flag matrix (matches `RuntimeBase::SBX.capabilities`) | unit | `cargo test -p agent-of-empires sbx::tests::capabilities_` | ❌ W0 | ⬜ pending |
| 02-XX-XX | XX | 1 | SC-3 | — | `is_available()` true for injected fake binary, false for nonexistent path | unit | `cargo test -p agent-of-empires sbx::tests::is_available_` | ❌ W0 | ⬜ pending |
| 02-XX-XX | XX | 2 | SC-4 (no regression) | — | All existing `runtime.rs` dispatch tests pass after three-arm edit | unit | `cargo test -p agent-of-empires --lib containers::runtime` | ✅ exists | ⬜ pending |
| 02-XX-XX | XX | 2 | SC-4 (route through) | — | `RuntimeKind::Sbx` dispatch arms for `is_available`, `exec_command`, `build_create_args` route to `SbxRuntime` (assert builder shape, not stub sentinel) | unit | Strengthen `runtime.rs::tests::test_sbx_dispatch_arms_*` | ✅ partial | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/containers/sbx/mod.rs` — new file; declares `SbxRuntime { binary: PathBuf }` and `#[cfg(test)] mod tests`
- [ ] `src/containers/sbx/argv.rs` — new file; `build_create_args` / `build_exec_args` / `build_ports_args` plus `#[cfg(test)] mod tests` covering ≥10 distinct argv shapes (research sketches 15)
- [ ] `src/containers/mod.rs` — add `mod sbx;` declaration
- [ ] `src/containers/runtime.rs` — modify three `RuntimeKind::Sbx` dispatch arms (`is_available`, `exec_command`, `build_create_args`), modify `ContainerRuntime` storage per D-03 planner choice, modify constructors accordingly
- [ ] `src/containers/runtime.rs::tests` — strengthen sbx dispatch test to assert new builder shape (not the Phase 1 sentinel)
- [ ] No framework install — Rust built-in test runner and existing `tempfile` dev-dep cover everything

---

## Manual-Only Verifications

| Behavior | Roadmap SC | Why Manual | Test Instructions |
|----------|-----------|------------|-------------------|
| *(none)* | — | All Phase 2 behaviors are pure argv composition or local subprocess probes (fake binary in tempfile). Every success criterion has an automated counterpart. | — |

*All Phase 2 behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s for `sbx::` quick run
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
