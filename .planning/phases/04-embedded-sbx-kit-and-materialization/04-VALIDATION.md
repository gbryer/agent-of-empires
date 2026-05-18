---
phase: 04
slug: embedded-sbx-kit-and-materialization
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-16
---

# Phase 04 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `04-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust 2021 edition, in-module `#[cfg(test)]` + `tests/*.rs`) |
| **Config file** | `Cargo.toml` (no separate test config) |
| **Quick run command** | `cargo test --lib containers::sbx::kit` |
| **Full suite command** | `cargo test --lib` |
| **Estimated runtime** | ~30s quick, ~120s full |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib containers::sbx::kit hooks::status_file`
- **After every plan wave:** Run `cargo test --lib` + `cargo xtask check-kit`
- **Before `/gsd:verify-work`:** Full suite must be green, `cargo clippy -- -D warnings` clean
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

> Filled by the planner per task. Each row binds a task to a requirement, threat ref (if any), and an automated command that proves the behavior.
>
> Plans are expected to follow the 2-plan split from RESEARCH.md:
> - **04-01** — Materialize + assets (KIT-01..KIT-05 except check-kit). Per Plan 04-01 revision, Task 2 is split into Task 2a (ensure + cache_dir + atomic-rename) and Task 2b (gc_stale_kit_dirs + spawn_blocking wire-in).
> - **04-02** — `cargo xtask check-kit` + CI wiring + StatusPoller sbx dispatch site (KIT-06 + remainder of KIT-04). Plan 04-02 adds a pure `resolve_status_dir(bool, &Path, &str) -> PathBuf` dispatcher with two unit tests pinning each capability-flag branch.

| Row Name | Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|----------|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| materializer_core | 04-01-2a | 01 | 1 | KIT-01..03, KIT-05 | T-04-01-01, T-04-01-03 | atomic-rename + cache key + fail-loud unsupported | unit | `cargo test --lib containers::sbx::kit` | ❌ W0 | ⬜ pending |
| gc_safety_rails | 04-01-2b | 01 | 1 | KIT-05 | T-04-01-02, T-04-01-06 | prefix + `.tmp.` guards; non-`aoe-` siblings untouched | unit | `cargo test --lib containers::sbx::kit::tests::gc_removes_only_stale_aoe_dirs containers::sbx::kit::tests::gc_refuses_to_delete_non_aoe_siblings containers::sbx::kit::tests::gc_skips_tmp_suffixed_dirs` | ❌ W0 | ⬜ pending |
| status_dir_dispatch_via_caps | 04-02-1 | 02 | 2 | KIT-04, D-03 | T-04-02-02 | pure `resolve_status_dir(bool, &Path, &str)` branches correctly per capability flag | unit | `cargo test --lib hooks::status_file::tests::resolve_status_dir_legacy_when_arbitrary_paths_supported hooks::status_file::tests::resolve_status_dir_sbx_when_arbitrary_paths_unsupported` | ❌ W0 | ⬜ pending |
| xtask_check_kit | 04-02-2 | 02 | 2 | KIT-06 | T-04-02-05 | install-verb lint + idempotency guards | xtask + integration | `cargo xtask check-kit` + `cargo test --test xtask_check_kit` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

> Test scaffolds and fixtures the planner must scaffold before execution can start sampling.

- [ ] `src/containers/sbx/kit.rs` — new module with in-module `#[cfg(test)] mod tests` block; covers cache-key determinism, atomic-rename race, GC selection, install.sh 0o755 chmod on unix, credentials.sources ssh-agent block declaration
- [ ] `src/hooks/status_file.rs` § `mod tests` — extend existing block to cover `sbx_hook_status_dir`, the pure `resolve_status_dir` dispatcher (two unit tests, one per capability-flag branch), and the extracted `parse_status_content` helper
- [ ] `xtask/src/main.rs` — new `check_kit` function + smoke test inputs (good spec, bad spec) under `tests/xtask_check_kit.rs`
- [ ] `tests/xtask_check_kit.rs` — table-driven integration test asserting:
  - `ok/spec.yaml` fixture passes
  - `missing-schema-version/spec.yaml` fixture fails with stderr containing `schemaVersion`
  - `naked-install-verb/spec.yaml` fixture fails with stderr containing `naked install verb`
  - `guarded-install-verb/spec.yaml` fixture passes
  - embedded `src/containers/sbx_kit/spec.yaml` (from 04-01) passes
- [ ] `tempfile` (already a dev-dep) — used for `<app_dir>` substitution in all materializer tests

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `sbx create shell --kit <materialized> --template <image>` survives a live `sbx` invocation against a real Docker Sandboxes install | KIT-02 (invocation shape) | Phase 4 does NOT wire the subprocess; Phase 5 does. Argv-builder unit test asserts the shape; real `sbx` execution is Phase 5 acceptance. | (Phase 5) |
| `credentials.sources` block in materialized kit grants ssh-agent forwarding inside sbx sandbox | KIT-03 (D-05) | Schema is [ASSUMED] per RESEARCH A1 (the `credentials.sources.<svc>.agent: ssh-agent` shape is not publicly documented). The kit declares the best-effort schema with an inline `[ASSUMED schema ...]` comment; a live-sbx smoke test is the only way to confirm or correct the shape. | Run `sbx create shell --kit <materialized> --template <image>` from the project root, then `ssh-add -L` inside the sandbox should return the host's loaded keys; if the schema is wrong, sbx will reject the kit at create time with a schema error, OR (worse) silently accept the kit and ship without forwarding. If the schema rejects: read the error, fix the field path in `src/containers/sbx_kit/spec.yaml`, re-run. If the schema accepts but `ssh-add -L` is empty: investigate per RESEARCH Open Question 1 (the alternative shapes `credentials.ssh-agent: true` or no-declaration / opt-in-by-daemon are the next two hypotheses). |
| Install-hook output retention on failure (PITFALLS.md N5) | KIT-03 | sbx must surface install-hook stderr/stdout when `commands.install` exits non-zero; cannot be auto-tested without inducing a real failure | Force a deliberate install failure (e.g., wrong package); confirm `sbx create` surfaces the script output |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter (planner / executor flips after Wave 0 lands)

**Approval:** pending
