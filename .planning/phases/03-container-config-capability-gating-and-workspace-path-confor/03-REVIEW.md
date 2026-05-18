---
phase: 03-container-config-capability-gating-and-workspace-path-confor
reviewed: 2026-05-16T00:00:00Z
depth: deep
files_reviewed: 6
files_reviewed_list:
  - src/cli/remove.rs
  - src/containers/runtime.rs
  - src/session/container_config.rs
  - src/session/deletion.rs
  - src/session/instance.rs
  - src/tui/creation_poller.rs
findings:
  critical: 1
  warning: 7
  info: 4
  total: 12
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-05-16
**Depth:** deep
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Phase 03 introduces a `RuntimeCapabilities`-driven post-pass in
`build_container_config`, an end-of-function workspace conformance helper, an
`Instance::container_workdir(caps)` pure form with a `container_workdir_now()`
shim, an `ensure_image` capability gate, and a `ContainerRuntime::name()`
accessor. The contract is sound for the simple/single-repo flows it tests
exhaustively, but the conformance pass leaks one provable bug (workspace
sessions silently get a wrong exec-time workdir under sbx) and a handful of
narrower defects that the test matrix did not catch. Pre-existing issues in the
in-scope files are flagged separately so the team can decide whether to fix
them now while the area is hot.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: `Instance::container_workdir` ignores `workspace_info`, producing a divergent workdir for multi-repo sessions under sbx

**File:** `src/session/instance.rs:1272-1281`
**Severity:** BLOCKER

`build_container_config` (the mount-time path) routes multi-repo sessions
through `compute_workspace_volume_paths` (container_config.rs:936-940), which
computes a `workspace_path` based on the common ancestor of the workspace dir
and every main repo. The container is created with that workdir, e.g.
`/workspace/<common-ancestor-relative>/<workspace-dir>`.

`Instance::container_workdir` (the exec-time path) unconditionally calls the
single-repo helper:

```rust
pub fn container_workdir(&self, caps: RuntimeCapabilities) -> String {
    let (volumes, working_dir) = container_config::compute_volume_paths(
        Path::new(&self.project_path),
        &self.project_path,
    )
    .unwrap_or_else(|_| (vec![], "/workspace".to_string()));
    let (_, conformed_wd) =
        container_config::conform_workspace_paths(volumes, working_dir, caps);
    conformed_wd
}
```

This call site never reads `self.workspace_info`. For workspace sessions:

- Under Docker caps, the function returns a `/workspace/<dir-name>` path that
  is **not** the path mounted by `build_container_config` (the multi-repo path
  uses the common-ancestor layout). Every `docker exec -w <workdir>` that
  flows through `container_workdir_now()` will land in the wrong directory or
  fail outright if the path doesn't exist inside the container.
- Under sbx caps, conformance rewrites the single-repo guess to its host
  path. That happens to coincide with `project_path`, which is the workspace
  root, so it may "look right" in trivial cases but does not match what
  `compute_workspace_volume_paths` actually produced for the volume set, and
  it diverges as soon as `workspace_info.workspace_dir` differs from
  `project_path` (which the persisted schema explicitly allows; `cli/add.rs`
  populates them separately for multi-repo sessions).

Affected consumers (every call site of `container_workdir_now()` in this
phase touches it):

- `src/cli/remove.rs:92` (on_destroy hook workdir for sandboxed workspace
  sessions runs in the wrong dir)
- `src/session/deletion.rs:361` (same, TUI/web path)
- `src/tui/creation_poller.rs:167, 208` (on_create / on_launch hooks)
- `src/session/instance.rs:608, 622, 635, 648, 661, 674` (retroactive session
  ID capture — silently fails to find session files under the wrong path)
- `src/session/instance.rs:816, 935` (sandboxed terminal pane and on_launch
  hook resolution)
- `src/session/instance.rs:1338-1436` (every poller construction for sandboxed
  workspace sessions)

The Phase 03 plan's own VERIFICATION note (`Phase 3 RT-04 (D-10): exec-time
consumers ... must see the same conformed workdir that build_container_config's
post-pass produced, otherwise the path inside the sandbox is wrong`) is exactly
the contract being violated for workspace sessions. The unit test
`test_instance_container_workdir_threads_capabilities`
(container_config.rs:3841) only exercises a single-repo path, so the gap was
not caught.

**Fix:**

Mirror `build_container_config`'s branching in `container_workdir`:

```rust
pub fn container_workdir(&self, caps: RuntimeCapabilities) -> String {
    let project_path = Path::new(&self.project_path);
    let (volumes, working_dir) = if let Some(ws_info) = self.workspace_info.as_ref() {
        container_config::compute_workspace_volume_paths(project_path, ws_info)
    } else {
        container_config::compute_volume_paths(project_path, &self.project_path)
    }
    .unwrap_or_else(|_| (vec![], "/workspace".to_string()));
    let (_, conformed_wd) =
        container_config::conform_workspace_paths(volumes, working_dir, caps);
    conformed_wd
}
```

This requires `compute_workspace_volume_paths` to be `pub(crate)` (currently
private to the module); promote its visibility along with the fix. Add a
regression test that builds an `Instance` with `workspace_info` populated and
asserts the returned workdir equals the workspace mount produced by
`build_container_config` under both Docker and sbx caps.

## Warnings

### WR-01: `cli/remove.rs` retains the pre-#1023 worktree-before-container ordering and will still race the in-container agent

**File:** `src/cli/remove.rs:116-231`

PR #1023 reordered `perform_deletion` to: hooks → tmux kill → preclean →
container remove → host worktree remove → branch delete. The TUI/web shared
path now matches `Instance::stop`.

`cli/remove.rs` was not updated. Its order is still:

1. on_destroy hooks (line 65-113)
2. worktree cleanup via `remove_managed_worktree` (line 130-167)
3. branch delete (line 178-196)
4. tmux kill (line 199-208)
5. container remove (line 211-231)

For sandboxed sessions with a worktree, `aoe remove --delete-worktree`
reproduces the exact failure mode #1023 was filed against ("validation failed,
cannot remove working tree: '<path>/.git' does not exist" etc.). The git
history confirms #1023 only added the `allow_container_removal` flag to this
file, not the reorder. This phase did not touch the ordering either, but the
file was opened for review and the divergence between the two deletion paths
is a real, reproducible bug surface.

**Fix:**

Factor the deletion stages into a shared helper used by both `aoe remove` and
`perform_deletion`, or at minimum reorder this file to match `perform_deletion`
(kill tmux + preclean + remove container before worktree). Add an integration
test that runs `aoe remove --delete-worktree` against a sandboxed worktree
session and asserts no race.

### WR-02: `Instance::container_workdir_now()` is called from background threads that fail to load `Config`, silently returning Docker-default workdir for sbx-configured users

**File:** `src/session/instance.rs:1287-1290`

```rust
pub fn container_workdir_now(&self) -> String {
    let caps = containers::get_container_runtime().capabilities();
    self.container_workdir(caps)
}
```

`get_container_runtime()` calls `Config::load()`, and on failure
(`Err(_)`) falls back to `ContainerRuntime::default()` which is **Docker**
(see containers/mod.rs:39 and runtime.rs:86). The 19 call sites include three
that run on a background creation thread (`creation_poller.rs:167, 208`) and
the deletion path (`deletion.rs:361`, `cli/remove.rs:92`) where `Config::load`
can fail because:

- `HOME`/`XDG_CONFIG_HOME` is unset or transiently unreadable
- the config file is being concurrently rewritten by another aoe process
- the user is mid-migration

When `Config::load` returns Err on a host configured for sbx, every exec-time
workdir resolves to Docker's `/workspace/<dir>` shape, which is not a valid
path inside a sbx sandbox (sbx requires `host_path == container_path`). The
hook will run in the wrong directory, the retroactive session-ID capture will
fail silently, and the user sees no error.

The behavior is also non-deterministic across calls within a single deletion:
if `Config::load` succeeds on the first call and fails on the second, the
sandbox path will silently change between stages.

**Fix:**

Cache the resolved runtime identity on the `Instance` at construction time
(it's already persisted via `source_profile`, so a parallel `runtime_kind:
Option<ContainerRuntimeName>` field on `Instance` with `#[serde(default)]` is
a small migration), or thread the `ContainerRuntime` explicitly down to the
call sites instead of re-resolving on every call. As a stopgap, change the
fallback in `get_container_runtime` to surface a `tracing::warn!` so the
silent Docker fallback is at least observable.

### WR-03: `cli/remove.rs` does not clean up `workspace_info` repos at all

**File:** `src/cli/remove.rs:130-196`

`perform_deletion` walks `ws_info.repos`, runs `remove_managed_worktree` on
each, and removes the workspace parent directory (deletion.rs:193-238).
`cli/remove.rs` only ever reads `inst.worktree_info`. For multi-repo
workspace sessions, `aoe remove --delete-worktree` leaves every per-repo
worktree on disk and never deletes the workspace parent directory, while the
session entry is removed from `sessions.json`. The user is then in a
broken-state situation: session gone, dangling worktrees + branches left
behind.

This is pre-existing (the divergence predates this phase), but it lives in
an in-scope file and is one of the larger gaps between the two deletion code
paths.

**Fix:**

Same as WR-01: factor a single deletion routine that both `aoe remove` and
`perform_deletion` call.

### WR-04: `extra_volumes` sbx conformance accepts a malformed `host:container:rw` entry as conformant when `parts[0] == parts[1]`, but the original push site dropped the `:rw` and treated it as read-write — silent shape divergence

**File:** `src/session/container_config.rs:1163-1180` and `1269-1278`

The push site treats `parts.get(2) == Some(&"ro")` as the only read-only
indicator (rw is implied by absence). The validator in
`conform_for_capabilities` checks only `parts[0] != parts[1]`. Any third token
that is not literally `"ro"` is silently treated as `:rw`. So a user typo like
`/host:/host:read-only`, `/host:/host:readonly`, or `/host:/host:RO` would
mount read-write under Docker AND pass the sbx conformance validator. There is
no warning emitted.

Adjacent: `splitn(3, ':')` will silently truncate a Windows path or any value
containing extra colons (e.g. an IPv6-shaped host path). Pre-existing, but
the push site doesn't validate that there's no fourth segment after `:ro`.

**Fix:**

Validate the third segment at the push site:

```rust
match parts.get(2) {
    None => { /* rw, fine */ }
    Some(&"ro") => { /* read-only */ }
    Some(other) => {
        tracing::warn!("Ignoring unknown extra_volume mode '{}' in {}, treating as rw", other, entry);
        // optionally: continue; to skip the mount
    }
}
```

### WR-05: `compute_volume_paths` canonicalizes paths but returns a non-canonical fallback when `canonicalize` errors, breaking the conformance comparison in `conform_workspace_paths`

**File:** `src/session/container_config.rs:724-731, 825-831`

```rust
let main_repo_canonical = main_repo
    .canonicalize()
    .unwrap_or_else(|_| main_repo.clone());
let project_canonical = project_path
    .canonicalize()
    .unwrap_or_else(|_| project_path.to_path_buf());
```

If only one of the two `canonicalize` calls succeeds (e.g. a temp dir where
the worktree path resolves but the main repo path doesn't because the parent
is a tombstoned mount), the comparison `main_repo_canonical != project_canonical`
will see one canonical and one non-canonical string. Under sbx, the
conformance pass then rewrites with mismatched prefixes and the resulting
mount has `host_path == "/private/var/folders/...." ` vs
`container_path == "/var/folders/..."` — both look conformant individually
(`v.host_path == v.container_path` is true) but `working_dir` may still
contain the un-canonicalized prefix because `rewrite_workdir` is called with
the canonicalized prefix pairs and the working_dir built from a mix.

The exec-time `container_workdir` path canonicalizes nothing
(`Instance::container_workdir` reads `self.project_path` raw), so on macOS
where `/tmp` symlinks to `/private/tmp`, the mount-time and exec-time paths
will diverge even when canonicalize succeeds.

**Fix:**

Either canonicalize both sides consistently at exec time, or persist the
canonicalized `project_path` once at session creation and use it everywhere
downstream. Add an integration test on macOS that mounts a path containing a
symlink in the canonical chain and asserts the exec-time workdir matches the
mount-time workdir.

### WR-06: `rewrite_workdir` longest-prefix-match can return a wrong-sibling answer when two volumes share the same length

**File:** `src/session/container_config.rs:65, 84-95`

```rust
prefix_pairs.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
```

Sort is unstable on ties. If two volumes have container_paths of identical
length but different host_paths (e.g. multi-repo workspace with `/workspace/a`
and `/workspace/b`), the order in which they are checked is implementation-
defined for ties. The loop short-circuits on the first match, so a `working_dir
== "/workspace/a"` lookup is safe (only one match), but if `working_dir` is
something unrelated that happens to match a prefix accidentally... actually
the equality branch is exact so this is fine. The risk is real only if a
caller constructs a working_dir that doesn't start with any prefix and
expects the function to return the input unchanged. It does, but the
test matrix doesn't cover that case.

**Fix:**

Switch to `sort_by` with a deterministic tie-breaker (e.g.
`then_with(|| pair.0.cmp(&other.0))`), and add a unit test asserting
that two equally-long prefixes resolve deterministically.

### WR-07: `conform_for_capabilities` silently drops the gitconfig / SSH / GCP-cred / agent-config mounts under sbx — no user-visible warning, only `tracing::debug!`

**File:** `src/session/container_config.rs:1286-1298`

```rust
config.volumes.retain(|v| {
    let keep = v.container_path == v.host_path;
    if !keep {
        tracing::debug!(
            target: "containers.config",
            ...
            "dropping mount (!supports_arbitrary_volume_paths)"
        );
    }
    keep
});
```

Every gitconfig, SSH, GCP credential, and per-agent config mount is dropped
when the runtime is sbx, but the log level is `debug` and there is no
user-facing summary. Users will hit "git push asks for password" or "Claude
re-prompts for OAuth" without any breadcrumb to follow. CONTEXT D-04 documents
the choice to silently drop, but for a feature that just landed, raising the
first occurrence to a single `tracing::warn!` ("sbx runtime: dropped N
convenience mounts; agent credentials may need to be re-authenticated") would
prevent a class of confused-user issues.

**Fix:**

Tally the dropped mounts and emit one `tracing::warn!` summary after the
retain pass, including the count and an enumerated list of mount kinds
dropped.

## Info

### IN-01: Stale doc comment in `instance.rs:1284` claims "19 exec-time call sites"

**File:** `src/session/instance.rs:1284-1286`

```rust
/// Convenience shim that fetches the live runtime's capabilities once and
/// delegates to container_workdir(caps). Keeps the 19 exec-time call sites
/// at a single uniform `_now` append rather than each fetching caps locally.
```

`grep -n container_workdir_now src/` reports 14 production call sites
(excluding the impl itself and three test references). The "19" appears to
come from the original phase scope estimate. Rounding the number into the
doc comment makes it more brittle than it needs to be. Prefer
"every exec-time call site" or drop the number.

### IN-02: `container_workdir_now()` does a fresh subprocess probe on every poller iteration

**File:** `src/session/instance.rs:1287-1290`, called from
`src/session/instance.rs:1338, 1356, 1376, 1391, 1406, 1421, 1436`

Each poller construction calls `container_workdir_now()`, which calls
`get_container_runtime()`, which calls `Config::load()`, which reads and
parses TOML from disk. For workspace sessions with multiple sandboxed pollers
this multiplies the disk reads on every new session. Not a correctness bug —
performance is out of scope for v1 — but worth noting that the live-probe
shim is genuinely expensive and the cached-runtime-identity fix proposed in
WR-02 would also resolve this.

### IN-03: `conform_workspace_paths` clones volume `host_path` twice in the rewrite path

**File:** `src/session/container_config.rs:61-74`

```rust
let mut prefix_pairs: Vec<(String, String)> = volumes
    .iter()
    .map(|v| (v.container_path.clone(), v.host_path.clone()))
    .collect();
...
let conformed_volumes: Vec<VolumeMount> = volumes
    .into_iter()
    .map(|v| VolumeMount {
        container_path: v.host_path.clone(),
        host_path: v.host_path,
        read_only: v.read_only,
    })
    .collect();
```

The second `.iter()` -> `.into_iter()` clone pattern allocates each host_path
string twice. Minor style point; cosmetic.

### IN-04: `rewrite_plugin_value_paths` recursion has no depth limit and would stack-overflow on a pathological JSON

**File:** `src/session/container_config.rs:487-514`

Recursive walk over arbitrarily nested `serde_json::Value` with no depth
guard. Practically unreachable from any agent-shipped plugin manifest, but
the function is fed user-controlled JSON. Replacing the recursion with an
explicit stack or `serde_json::to_value(...).pointer_mut(...)` for the two
fields of interest would harden it. Pre-existing, not introduced this phase.

---

_Reviewed: 2026-05-16_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
