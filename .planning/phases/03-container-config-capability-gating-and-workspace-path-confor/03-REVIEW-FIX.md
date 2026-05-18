---
phase: 03-container-config-capability-gating-and-workspace-path-confor
fixed_at: 2026-05-16T00:00:00Z
review_path: .planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-REVIEW.md
iteration: 1
findings_in_scope: 8
fixed: 8
skipped: 0
status: all_fixed
---

# Phase 03: Code Review Fix Report

**Fixed at:** 2026-05-16T00:00:00Z
**Source review:** .planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 8 (1 Critical + 7 Warning)
- Fixed: 8
- Skipped: 0

The 4 Info-tier findings (IN-01..IN-04) were out of scope for this run
(`fix_scope: critical_warning`).

## Fixed Issues

### CR-01: `Instance::container_workdir` ignores `workspace_info`, producing a divergent workdir for multi-repo sessions under sbx

**Files modified:** `src/session/instance.rs`, `src/session/container_config.rs`
**Commit:** 66351a0
**Applied fix:** Mirror `build_container_config`'s branching in
`Instance::container_workdir`: dispatch to `compute_workspace_volume_paths`
when `self.workspace_info` is present, else `compute_volume_paths`.
Promoted `compute_workspace_volume_paths` from private to `pub(crate)`.
Added regression test
`test_instance_container_workdir_dispatches_on_workspace_info` that
constructs an `Instance` with `workspace_info` populated, calls
`compute_workspace_volume_paths` directly to capture the mount-time
workdir, and asserts the exec-time workdir under Docker caps equals it
and under sbx caps equals the matching volume's host_path.

### WR-01: `cli/remove.rs` retains the pre-#1023 worktree-before-container ordering and will still race the in-container agent

**Files modified:** `src/cli/remove.rs`, `src/session/deletion.rs`, `src/server/api/sessions.rs`, `src/tui/deletion_poller.rs`, `src/tui/home/operations.rs`
**Commit:** 972b8ed
**Applied fix:** Refactored `cli/remove.rs` to delegate the destructive
stages to `perform_deletion` (the post-#1023 ordering: hooks -> tmux
-> preclean -> container -> worktree -> branch). cli/remove still runs
on_destroy hooks attached to the terminal locally (so credential
prompts work), then calls `perform_deletion` with the new
`skip_hooks: true` flag to avoid double-running them. All other
`DeletionRequest` constructors default to `skip_hooks: false`. The
pre-existing perform_deletion test matrix (ordering, dirty-tree safety,
e2e worktree removal, 9 tests) continues to pass unchanged.

### WR-02: `Instance::container_workdir_now()` is called from background threads that fail to load `Config`, silently returning Docker-default workdir for sbx-configured users

**Files modified:** `src/containers/mod.rs`
**Commit:** 0033113
**Applied fix:** Stopgap per the review's recommendation. Replaced the
silent `if let Ok(cfg) = Config::load() { ... } else { default }` in
`get_container_runtime` with an explicit `match` that emits a
`tracing::warn!` on the `Err` branch including the error and a hint
about exec-time workdir misroute risk. The deeper cached-runtime-
identity threading fix is deferred (it requires an `Instance` field
migration + signature changes at every exec-time call site, which is
larger than the review's stopgap recommended).

### WR-03: `cli/remove.rs` does not clean up `workspace_info` repos at all

**Files modified:** Same commit as WR-01 (`src/cli/remove.rs` + 4 callers)
**Commit:** 972b8ed
**Applied fix:** Resolved by the same WR-01 refactor. `perform_deletion`
already walks `workspace_info.repos` (lines 192-239 of deletion.rs) and
removes the workspace parent directory; delegating cli/remove to it
fixes the divergence by construction. cli/remove now computes
`managed_workspace` and includes workspace cleanup in
`delete_worktree` when either a managed worktree OR a managed workspace
is present.

### WR-04: `extra_volumes` sbx conformance accepts a malformed `host:container:rw` entry as conformant when `parts[0] == parts[1]`

**Files modified:** `src/session/container_config.rs`
**Commit:** 496bae0
**Applied fix:** Validate the third segment at the push site in
`build_container_config`. Match `parts.get(2)` explicitly: `None` =>
rw, `Some("ro")` => ro, any other value => emit a `tracing::warn!`
including the malformed token and the entry, then treat as rw so the
user has a breadcrumb. Previously typos like `readonly`, `RO`,
`read-only` silently mounted read-write under Docker AND passed the
sbx conformance validator with no warning.

### WR-05: `compute_volume_paths` canonicalizes paths but returns a non-canonical fallback when `canonicalize` errors

**Files modified:** `src/session/container_config.rs`
**Commit:** dcdc110
**Applied fix:** Surgical fix targeted at the non-git fallback branch
(lines ~800-813), which used the raw `project_path_str` as the volume
host_path. Now canonicalize there too (falling back to raw only on
canonicalize failure) so the non-git branch's host_path stays
consistent with the git branches that already canonicalize. Updated
the existing test
`test_compute_volume_paths_non_git_directory` to assert against the
canonical form (so it's stable across macOS and Linux). The deeper
"persist canonicalized project_path at session creation" option from
the review is a larger Instance-schema change deferred for now; the
chosen fix addresses the symmetric mount-time/exec-time divergence
without touching session storage.

### WR-06: `rewrite_workdir` longest-prefix-match can return a wrong-sibling answer when two volumes share the same length

**Files modified:** `src/session/container_config.rs`
**Commit:** a146a0f
**Applied fix:** Replaced `sort_by_key(|p| Reverse(p.0.len()))` with
`sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)))`,
giving a deterministic tie-breaker on the container_path string itself.
Added regression test
`test_conform_workspace_paths_sbx_equal_length_prefix_deterministic_tie_break`
asserting symmetric resolution: `/workspace/a/src` -> `/host/repo-a/src`
and `/workspace/b` -> `/host/repo-b`, regardless of construction order.

### WR-07: `conform_for_capabilities` silently drops the gitconfig / SSH / GCP-cred / agent-config mounts under sbx

**Files modified:** `src/session/container_config.rs`
**Commit:** 91ab088
**Applied fix:** Per the review's recommendation, tally dropped
container_paths into a `Vec<String>` during the `retain()` pass and
emit one `tracing::warn!` summary after, including the dropped count
and an enumerated list of mounts (`dropped_mounts = ?dropped`). Users
who hit "git push asks for password" or "agent re-prompts for OAuth"
now have an actionable log line pointing at the runtime's
`!supports_arbitrary_volume_paths` behavior.

---

_Fixed: 2026-05-16T00:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
