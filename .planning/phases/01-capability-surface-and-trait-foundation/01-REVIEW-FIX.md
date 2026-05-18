---
phase: 01-capability-surface-and-trait-foundation
fixed_at: 2026-05-15T00:00:00Z
review_path: .planning/phases/01-capability-surface-and-trait-foundation/01-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-05-15
**Source review:** `.planning/phases/01-capability-surface-and-trait-foundation/01-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 3 (Warning)
- Fixed: 3
- Skipped: 0

Info-tier findings (IN-01, IN-02) are out of scope for the critical_warning fix pass and were not attempted.

## Fixed Issues

### WR-01: Five `RuntimeCapabilities` flags declared but never used as behavioral gates

**Files modified:** `src/containers/runtime_base.rs`, `src/containers/container_interface.rs`
**Commit:** `8a81208`
**Applied fix:** Gated the `build_create_args` loops on the corresponding capability flags so backends whose matrix declares a flag false no longer silently emit args for that feature:

- The `volumes` (bind mount) loop is now gated on `supports_arbitrary_volume_paths`; bind mounts on a backend that declares `false` are skipped with a `tracing::warn!` reporting the skipped count.
- The `anonymous_volumes` loop is now gated on `supports_anonymous_volumes` with the same warn-on-skip pattern.
- The `port_mappings` loop is now gated on `supports_port_publish_at_create` with the same warn-on-skip pattern.
- `ensure_image` now short-circuits with a clear `DockerError::CommandFailed` when called on a backend whose `supports_image_pull` is false, instead of falling through to a confusing `sbx IMAGE` command via an empty `pull_prefix`.
- The doc comment for `supports_dynamic_port_publish` now states it is caller-facing metadata (not consumed by `build_create_args`, which uses `supports_port_publish_at_create` for the at-create gate). The flag is asserted in capability-matrix tests, so it has a present consumer.

Verified with `cargo fmt`, `cargo build`, `cargo clippy -- -D warnings`, and the full `cargo test --lib` suite (1619 passed, 0 failed). All existing anonymous-volume and port-mapping tests pass without modification because they exercise Docker, whose flags are true.

### WR-02: `exec_command` Sbx stub silently drops `options` and `cmd` arguments

**Files modified:** `src/containers/runtime.rs`
**Commit:** `fda3601`
**Applied fix:** Replaced the stub return string `format!("sbx exec {}", name)` with `format!("sbx exec {} /* Phase 1 stub: cmd and options dropped */", name)` so that any accidental leak of the stub into a real tmux exec target fails visibly (the embedded `/* ... */` is not valid shell and produces an immediate, attributable error) rather than silently invoking `sbx exec NAME` without the required workdir, env, and command arguments. Also expanded the inline comment to call out exactly what the Phase 2 (RT-04) replacement must include: workdir option, env forwarding, name, and cmd. The existing `test_sbx_dispatch_arms_return_safe_stubs` assertions (`cmd.contains("sbx")` and `cmd.contains("foo")`) remain satisfied.

### WR-03: `match self.kind` dispatch site count is wrong in test comment

**Files modified:** `src/containers/runtime.rs`
**Commit:** `0254a41`
**Applied fix:** Updated the test-module comment from "The four match-self.kind dispatch sites must each have a safe Sbx arm" to "The five match-self.kind dispatch sites must each have a safe Sbx arm" and added a parenthetical noting that `is_available` is a sixth match site, tested separately in `test_sbx_is_available_returns_false_unconditionally`. The five sites are `is_available`, `does_container_exist`, `is_container_running`, `exec_command`, and `batch_running_states`; the surrounding test exercises four of them (excluding `is_available`, which has its own dedicated test).

---

_Fixed: 2026-05-15_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
