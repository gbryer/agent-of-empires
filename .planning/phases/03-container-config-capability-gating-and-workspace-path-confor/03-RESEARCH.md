# Phase 3: container_config Capability Gating and Workspace Path Conformance - Research

**Researched:** 2026-05-15
**Domain:** Capability-gated container config rewriting (Rust, in-tree refactor)
**Confidence:** HIGH

## Summary

CONTEXT.md is already exhaustive: every architectural decision (D-01..D-13) is locked, the post-pass skeleton is sketched, the error variant shape is defined, and the call-site guard for `ensure_image` is one two-line patch. This research does NOT re-derive any of that. Instead it surfaces the **technical risks and validation surface** the planner needs but that CONTEXT.md does not fully spell out:

1. A **conflict between ROADMAP SC-3 and CONTEXT D-08** (volume_ignores vs extra_volumes as the typed-error trigger) that the planner must resolve up-front before writing tests.
2. The **exact enumeration of `container_workdir()` call sites** (19 occurrences across 4 files, not the "1308+ poll fns" estimate in CONTEXT.md).
3. A **borrow-checker trap** in the skeleton's `std::mem::take(&mut config.volumes)` pattern and the idiomatic Rust shape that avoids it.
4. The **existing-tests inventory** with named analogs per success criterion, including a critical observation: SC-2 ("Docker output unchanged") has no pre-existing fixture file in the repo, so the baseline must be generated as part of the phase.
5. Several **edge cases** (zero workspace mounts, hook-status-dir under `~/.agent-of-empires-dev/`, multi-repo workspace conformance, extra_volume with malformed input) that the planner must wire into the plan or explicitly declare out of scope.

**Primary recommendation:** Lock the SC-3 ambiguity first (treat ROADMAP's mention of `volume_ignores` as superseded by CONTEXT D-08, but document it in the plan so future readers don't reopen the question), pick the idiomatic Rust shape for `conform_for_capabilities` (owned mutate-by-value, not `&mut + mem::take`), and generate the SC-2 baseline fixture file at plan time, not test time.

## User Constraints (from CONTEXT.md)

### Locked Decisions

The full Implementation Decisions section is reproduced verbatim from `.planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-CONTEXT.md`:

**Workspace Path Conformance (RT-04)**
- **D-01:** Sbx forces `host_path == container_path`; verified by the Docker docs (`sbx create AGENT PATH` mounts at the absolute host path; no `HOST:CONTAINER` syntax; kits cannot define mounts). The current `/workspace/<dir_name>` strategy is unachievable on sbx without kit-side symlink shims that would contradict Phase 1's `supports_arbitrary_volume_paths=false` capability flag.
- **D-02:** Conformance happens in a single end-of-function post-pass inside `build_container_config`. Function body builds the ContainerConfig the way it does today (workspace + every convenience mount), then a final `conform_for_capabilities(&mut config, caps)` step rewrites workspace `container_path` to `host_path`, rewrites `working_dir`, and drops non-conformable mounts. Docker capability matrix makes the pass a no-op.
- **D-03:** A shared `conform_workspace_paths(volumes, working_dir, caps) -> (Vec<VolumeMount>, String)` free function in `src/session/container_config.rs` is the single source of truth for workspace conformance. Both `build_container_config`'s post-pass AND `Instance::container_workdir()` call it.

**Convenience Mount Handling (RT-04)**
- **D-04:** Mounts whose `container_path` is not equal to (or under) one of the workspace `host_path` prefixes are DROPPED silently with `tracing::debug!` when `!supports_arbitrary_volume_paths`.
- **D-05:** The hook-status-dir mount already uses `host_path == container_path` shape. It survives the conformance pass unchanged.

**Validation Error Shape (RT-04 SC-3)**
- **D-06:** Introduce `ContainerConfigError` (thiserror) with exactly one variant: `InconformableExtraVolume { entry: String, runtime: ContainerRuntimeName, reason: String }`.
- **D-07:** `build_container_config` keeps its `anyhow::Result<ContainerConfig>` signature; typed errors slot in via `Err(typed.into())`; tests use `err.downcast_ref::<ContainerConfigError>()`.
- **D-08:** **Only `extra_volumes` (user-declared via `sandbox.extra_volumes`) trigger the typed error.** Aoe-internal convenience mounts are silently dropped.

**Capability Injection Seam**
- **D-09:** `build_container_config` gains a `capabilities: RuntimeCapabilities` parameter.
- **D-10:** `Instance::container_workdir()` gains a `capabilities: RuntimeCapabilities` argument.
- **D-11:** `compute_volume_paths` stays capability-blind.

**ensure_image Skip (RT-06)**
- **D-12:** Inline two-line call-site guard at `src/session/instance.rs:1249-1250`:
  ```rust
  let runtime = containers::get_container_runtime();
  if runtime.capabilities().supports_image_pull {
      runtime.ensure_image(image)?;
  }
  ```
- **D-13:** `RuntimeBase::ensure_image` keeps its defensive `Err(NotSupported)` behavior; Phase 3 does not modify it.

### Claude's Discretion

The following are Claude's discretion per CONTEXT.md; research recommendations appear in the Standard Stack / Architecture Patterns sections below:

- Exact module location for `ContainerConfigError` (inline at top of `container_config.rs` vs new `container_config/error.rs` peer module).
- Whether to clear `anonymous_volumes` in the post-pass for `!supports_anonymous_volumes` runtimes, or leave to `build_create_args`.
- Snapshot/baseline test mechanism for SC-2 (Docker output unchanged); `insta` vs hand-written fixture comparison.
- Working_dir picking under multi-mount cases (sbx multi-repo / sibling-worktree).

### Deferred Ideas (OUT OF SCOPE)

- Phase 4 kit `files/home/` and credential proxy delivery (gitconfig identity, agent settings, SSH agent forwarding, `sbx secret set -g`).
- Hook-status-dir relocation under workspace per KIT-04 (Phase 4 decision).
- `!supports_anonymous_volumes` ContainerConfig cleanup (planner discretion within Phase 3 OR defer to Phase 2's `build_create_args`).
- `!supports_port_publish_at_create` post-create publish loop (Phase 5).
- `insta` snapshot dep (planner discretion; default recommended is "no").
- Auto-conform extra_volumes (rejected for Phase 3 in favor of hard-error).
- Diagnostic surface for dropped mounts.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RT-04 | `host_path == container_path` constraint for sbx enforced in `build_container_config` by capability-gated rewrite of BOTH `working_dir` AND every `VolumeMount.container_path`. Inconformable configs surface a clear validation error. | All sections below; Standard Stack, Architecture Patterns, Validation Architecture provide the post-pass shape, error type, and per-criterion test plan. |
| RT-06 | sbx-specific image semantics: `pull_image` returns `Err(NotSupported)`, `ensure_image` skips silently when `!supports_image_pull`. | "Architecture Patterns / ensure_image call-site guard" section; confirms the two-line patch is the entire RT-06 surface for Phase 3; `RuntimeBase::ensure_image` is untouched (D-13). |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Workspace volume computation (host/container paths) | `src/session/container_config.rs` (session tier, pure function) | none | Stays capability-blind per D-11; the same computation runs for every runtime, conformance is downstream |
| Capability matrix declaration | `src/containers/runtime_base.rs` (runtime tier) | none | Already locked in Phase 1; Phase 3 reads it, never writes |
| Capability-driven config rewrite (conformance) | `src/session/container_config.rs` (session tier) | none | D-03: shared `conform_workspace_paths` lives where `build_container_config` lives |
| Capability-driven call-site gate (`ensure_image`) | `src/session/instance.rs` (session tier) | none | D-12: two-line inline guard at call site; no helper, no new module |
| Workdir derivation for sandboxed poll fns | `src/session/instance.rs::container_workdir()` (session tier) | none | Both call sites (post-pass + container_workdir) call the same shared helper to keep workdir consistent at mount-time and exec-time |
| Container action verbs (create/exec/stop/rm) | `src/containers/runtime.rs` dispatch + `src/containers/sbx/` (runtime tier) | none | Phase 2 already routes sbx through `RuntimeKind::Sbx` arms; Phase 3 does NOT touch this tier |
| Image-pull verb implementation | `src/containers/runtime_base.rs::ensure_image` (runtime tier) | none | D-13: defensive `Err(NotSupported)` preserved as a safety net for accidental future call-site bugs |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `thiserror` | 2.0 (already in Cargo.toml line 47) | Typed error variant `ContainerConfigError` (D-06) | Existing repo pattern: `DockerError`, `RegistryError`, `EnsureReadyError` all use it [VERIFIED: src/containers/error.rs:1-53, src/session/projects.rs, src/session/instance.rs:68] |
| `anyhow` | 1.0 (already in Cargo.toml line 48) | `build_container_config` return type stays `anyhow::Result<ContainerConfig>` (D-07) | Existing function signature; typed variants slot in via `Err(typed.into())`, tests use `downcast_ref` [VERIFIED: src/session/container_config.rs:9, line 833] |
| `tracing` | (already in workspace) | `tracing::debug!` for dropped-mount logging (D-04) | Established pattern in the same file (lines 865-870, 922-924, 1063-1066, 1085-1086) [VERIFIED: src/session/container_config.rs] |
| `tempfile` | 3.14 (already in Cargo.toml line 104) | Test fixtures for SC-1/SC-3 cases | Used extensively in existing `mod tests` (1154+); zero new deps required [VERIFIED: Cargo.toml] |
| `serial_test` | 3.4 (dev-dep, line 185) | `#[serial_test::serial]` for tests that mutate `HOME`/env | Existing pattern for every test that touches `build_container_config` (2395, 2543, 2658, 2871+); MUST use for any Phase 3 test that calls `build_container_config` directly [VERIFIED: Cargo.toml, container_config.rs tests] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none) | none |; | All needed types/helpers already exist in scope: `RuntimeCapabilities`, `ContainerRuntimeName`, `RuntimeBase::DOCKER` / `RuntimeBase::SBX` const literals, `VolumeMount`, `ContainerConfig` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-written fixture comparison for SC-2 | `insta` snapshot crate | `insta` is ergonomic (auto-update via `cargo insta review`) but adds a new dev-dep. CONTEXT.md explicitly notes "recommend hand-written fixture comparison". User memory entry `feedback_dependencies_and_gates.md` (your stored memory) explicitly says **"prefer stdlib + existing runtime checks over new deps"**. **Strong recommendation: hand-written `assert_eq!` of deduped volume tuples against expected literal Vec.** |
| `&mut ContainerConfig` + `std::mem::take` post-pass | Owned mutate-by-value: `let config = conform_for_capabilities(config, caps, ...)?;` | The skeleton in CONTEXT.md uses `std::mem::take(&mut config.volumes)` to satisfy the borrow checker when calling `conform_workspace_paths(volumes, working_dir, caps)` (which takes ownership). This works but reads awkwardly. **Idiomatic Rust:** take owned `ContainerConfig`, return owned, no `mem::take`. See "Architecture Patterns" below for the recommended shape. |
| New `container_config/error.rs` peer module | Inline `ContainerConfigError` at top of `container_config.rs` | CONTEXT.md says "inline keeps churn smallest". Inline is correct: single variant, single use site, follows the pattern of `DockerError` (own file in same module, but `DockerError` has 11 variants; single-variant doesn't warrant the split). **Recommend inline near the top of `container_config.rs`** between the existing `use` block and `SANDBOX_SUBDIR`. |

**Installation:** No new crate dependencies. Phase 3 adds zero `Cargo.toml` lines.

**Version verification:**
```bash
grep "^thiserror\|^anyhow\|^tempfile\|^serial_test" Cargo.toml
# thiserror = "2.0"          (line 47, already)
# anyhow = "1.0"             (line 48, already)
# tempfile = "3.14"          (line 104, already)
# serial_test = "3.4"        (line 185, dev-dep, already)
```
All Phase 3 deps are present in the existing tree. Verified via `Cargo.toml` direct grep.

## Package Legitimacy Audit

> Phase 3 installs zero new packages. The legitimacy audit is N/A; no dependency surface change.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| (none) | none |; | none |; | none |; |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                          build_container_config (existing entrypoint, +caps param)
                                          |
                                          v
              +-------------------------------------------------------+
              | 1. compute_volume_paths / compute_workspace_volume_paths   |
              |    (capability-BLIND, unchanged)                          |
              +-------------------------------------------------------+
                                          |
                                          v   (workspace VolumeMounts + working_dir)
              +-------------------------------------------------------+
              | 2. Push convenience mounts:                              |
              |    - gitconfig, SSH, GCP creds, agent config, home seeds |
              |    - hook-status-dir (already host==container shape)     |
              |    - extra_volumes (user-declared, parsed via splitn(3,:))|
              +-------------------------------------------------------+
                                          |
                                          v
              +-------------------------------------------------------+
              | 3. Build anonymous_volumes from volume_ignores            |
              |    + dedupe by container_path (existing logic)            |
              +-------------------------------------------------------+
                                          |
                                          v   (ContainerConfig, owned)
              +-------------------------------------------------------+
              | 4. POST-PASS: conform_for_capabilities                    |
              |    a. If caps.supports_arbitrary_volume_paths: return Ok  |
              |    b. Validate extra_volumes (host != container -> Err)   |
              |    c. conform_workspace_paths: rewrite container_path     |
              |       to host_path for workspace mounts (D-03 helper)     |
              |    d. Drop any mount where container_path != host_path    |
              |    e. Optionally clear anonymous_volumes (planner pick)   |
              +-------------------------------------------------------+
                                          |
                                          v
                            anyhow::Result<ContainerConfig>


  instance.rs::get_container_for_instance (RT-06 surface)
  --------------------------------------------------------
       let runtime = containers::get_container_runtime();
       if runtime.capabilities().supports_image_pull {  // <-- two-line guard
           runtime.ensure_image(image)?;
       }
       let config = self.build_container_config()?;     // calls into container_config.rs


  instance.rs::container_workdir(caps) (D-10 surface)
  ----------------------------------------------------
       let (volumes, wd) = compute_volume_paths(...)?;
       let (_, conformed_wd) = conform_workspace_paths(volumes, wd, caps);
       conformed_wd
       (consumed by 13 call sites; see Integration Points below)
```

### Recommended Project Structure

No new files. Phase 3 modifies exactly two existing files:

```
src/session/
  container_config.rs   # +ContainerConfigError, +conform_workspace_paths, +conform_for_capabilities,
                        #  +capabilities param on build_container_config, +tests
  instance.rs           # +capabilities param on container_workdir + every call site (13 places),
                        #  +capabilities param on the build_container_config wrapper (line 1269),
                        #  +two-line ensure_image guard (line 1249-1250)
```

### Pattern 1: Idiomatic Conform Helper (owned, not &mut)

**What:** The post-pass takes owned `ContainerConfig`, mutates locally, returns owned. Avoids the `std::mem::take` dance in CONTEXT.md's skeleton.

**When to use:** Pure data transformation with full ownership. The function is called at the very end of `build_container_config` (right before `Ok(config)`), so owned-passthrough is natural.

**Example:**
```rust
// Source: project-specific, derived from CONTEXT.md's skeleton with the owned-passthrough fix
fn conform_for_capabilities(
    mut config: ContainerConfig,
    caps: RuntimeCapabilities,
    runtime_name: ContainerRuntimeName,
    extra_volumes: &[String],
) -> anyhow::Result<ContainerConfig> {
    if caps.supports_arbitrary_volume_paths {
        return Ok(config); // Docker / Podman / AppleContainer; true no-op
    }

    // 1. Validate user-declared extra_volumes BEFORE mutating anything.
    //    Inconformable entries are a hard error per D-08.
    for entry in extra_volumes {
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        if parts.len() >= 2 && parts[0] != parts[1] {
            return Err(ContainerConfigError::InconformableExtraVolume {
                entry: entry.clone(),
                runtime: runtime_name,
                reason: "requires host_path == container_path".into(),
            }
            .into());
        }
    }

    // 2. Conform workspace mounts. conform_workspace_paths is the D-03
    //    shared helper, called by both this post-pass AND container_workdir().
    let (conformed_volumes, conformed_workdir) =
        conform_workspace_paths(config.volumes, config.working_dir, caps);
    config.volumes = conformed_volumes;
    config.working_dir = conformed_workdir;

    // 3. Drop convenience mounts that didn't conform (container_path != host_path).
    config.volumes.retain(|v| {
        let keep = v.container_path == v.host_path;
        if !keep {
            tracing::debug!(
                target: "containers.config",
                runtime = %runtime_name,
                host = %v.host_path,
                container = %v.container_path,
                "dropping mount (!supports_arbitrary_volume_paths)"
            );
        }
        keep
    });

    // 4. Planner-discretion clear of anonymous_volumes.
    //    Recommendation: clear them here to keep ContainerConfig honest about what the
    //    runtime will actually emit. (build_create_args already skips with warn at the
    //    argv layer in runtime_base.rs:268-274, so this is belt-and-suspenders.)
    if !caps.supports_anonymous_volumes {
        config.anonymous_volumes.clear();
    }

    Ok(config)
}
```

**Why owned-passthrough beats `&mut + mem::take`:**
- No borrow-checker gymnastics around moving `config.volumes` out while config is still borrowed mutably.
- Call site is one line: `let config = conform_for_capabilities(config, caps, runtime_name, &sandbox_config.extra_volumes)?;`
- Matches the existing function's overall shape: `build_container_config` already builds `ContainerConfig` by owning all the pieces and returning the final owned struct.

### Pattern 2: `conform_workspace_paths` shared helper (D-03)

**What:** Pure free function. Capability-aware. Used by post-pass AND `container_workdir()`. Has NO knowledge of convenience mounts, error variants, or sandbox config; just the workspace volumes + workdir.

**When to use:** Anytime aoe needs to know "what is the workdir inside the container?"; both at mount-time (when building `ContainerConfig`) and at exec-time (when feeding `-w` to a runtime's exec verb).

**Example:**
```rust
// Source: project-specific, derived from D-03
pub(crate) fn conform_workspace_paths(
    volumes: Vec<VolumeMount>,
    working_dir: String,
    caps: RuntimeCapabilities,
) -> (Vec<VolumeMount>, String) {
    if caps.supports_arbitrary_volume_paths {
        return (volumes, working_dir);
    }

    // Build a map of old container_path -> host_path so we can rewrite working_dir.
    // The map is needed because workspace_path may be a SUBDIRECTORY of a volume's
    // container_path (bare-repo worktree case: container=/workspace/repo, workdir=/workspace/repo/main).
    let conformed_volumes: Vec<VolumeMount> = volumes
        .into_iter()
        .map(|v| VolumeMount {
            container_path: v.host_path.clone(),
            ..v
        })
        .collect();

    // Rewrite working_dir: find the volume whose old container_path is a prefix of working_dir,
    // then swap that prefix for the volume's host_path.
    // NOTE: this loop runs ONLY in the !supports_arbitrary_volume_paths branch, so we re-derive
    // the prefix mapping from the just-conformed volumes by recomputing what the original prefix was.
    // Practical implementation: keep a parallel Vec<(old_container_prefix, new_host_prefix)> built
    // from the pre-conformance volumes, sort longest-prefix-first, then strip+prepend.
    // (Planner: see Pitfall #2; naive ordering breaks bare-repo case.)
    let conformed_workdir = rewrite_workdir_under_conformed_volumes(&working_dir, &conformed_volumes);

    (conformed_volumes, conformed_workdir)
}
```

The skeleton above is illustrative; see "Common Pitfalls" Pitfall #2 for the longest-prefix-match requirement that the actual implementation needs.

### Pattern 3: ensure_image call-site guard (D-12)

**What:** A two-line `if` gate at the single call site. No helper, no new types, no boilerplate.

**Example:**
```rust
// src/session/instance.rs:1248-1250; REPLACE current two lines
// Source: CONTEXT.md D-12, verbatim
let runtime = containers::get_container_runtime();
if runtime.capabilities().supports_image_pull {
    runtime.ensure_image(image)?;
}
```

### Anti-Patterns to Avoid

- **`if runtime == ContainerRuntimeName::Sbx` branches anywhere.** PROJECT.md "Key Decisions" explicitly forbids this; capability flags are the only acceptable gate. The Phase 3 post-pass MUST gate on `caps.supports_arbitrary_volume_paths`, never on the runtime name.
- **Multiple `ContainerConfigError` variants "for symmetry".** D-08 explicitly says one variant only, per AGENTS.md "no dead code". Resist the urge to also add `InconformableConvenienceMount` or `InconformableAnonymousVolume`.
- **Touching `RuntimeBase::ensure_image`.** D-13 keeps the defensive `Err(NotSupported)` in place as a safety net. Removing it would convert future bugs (someone forgetting the call-site gate) from typed errors into silent skips.
- **Reading `runtime.capabilities()` inside `container_workdir()` via a global lookup.** D-10 says caps come in as an argument. Threading caps means the function stays pure / mockable / testable without spinning up a runtime.
- **Snapshot-test mechanism that requires generated baseline files.** For SC-2 the baseline IS the test's `assert_eq!` literal. No on-disk JSON, no `insta` directory, no separate fixture binary.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parsing `<host>:<container>[:ro]` strings | New parser module | Reuse the existing `entry.splitn(3, ':')` pattern already at `container_config.rs:1070` | Only one parse site exists; introducing a helper just to wrap a one-liner is dead-code-adjacent (AGENTS.md). The post-pass validator can re-`splitn` the same string for free. |
| Mock runtime for SC-4 ensure_image test | Mock trait impl, `Arc<Mutex<Vec<ImageCall>>>`, etc. | Inject `RuntimeCapabilities` directly. The unit test asserts call-site behavior, not the trait method | CONTEXT.md D-12: "SC-4 unit test asserts the capability-driven branch by exercising `Instance::ensure_container()` with a session whose resolved runtime reports `!supports_image_pull` and verifying `ensure_image` is not invoked". Phase 1 already proves the capability matrix is honest. The test surface is the call-site branch, not the trait method's contract. |
| Snapshot framework for SC-2 | `insta`, custom JSON serialization, file-on-disk fixtures | Hand-written `assert_eq!` on the deduped volume Vec, using known-good inputs | The "baseline" is literally what `build_container_config` returns today with Docker caps. Capture it as a Rust `Vec<(host, container, ro)>` literal in the test and compare. Zero new deps. |
| `RuntimeCapabilities` synthesis helpers (`fn sbx_caps()`, `fn docker_caps()`) | Test-only constructors | Use the const literals `RuntimeBase::DOCKER.capabilities` and `RuntimeBase::SBX.capabilities` directly | They're `pub(crate)` consts already exposed via `RuntimeBase`; Phase 1 already pins them. No need for synthesis. (D-09 explicitly says so.) |

**Key insight:** Phase 3 is a "pure transformation" phase. No new abstractions, no new dependencies, no mock infrastructure. Every test uses real const literals against real pure functions. The whole change is two new free functions (`conform_workspace_paths`, `conform_for_capabilities`), one new error enum (1 variant), one new param on three existing functions, and one two-line call-site guard.

## Runtime State Inventory

> Phase 3 is a capability-gating refactor. No data is renamed, no state is migrated, no external service is reconfigured.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None; Phase 3 changes no on-disk format. `ContainerConfig` lives in memory only; sandbox config TOML is unchanged. | none |
| Live service config | None; sbx is not yet integrated end-to-end (Phase 5). | none |
| OS-registered state | None; no tmux session names, no Task Scheduler entries, no launchd plists touched. | none |
| Secrets/env vars | None; sandbox `extra_env`, GH_TOKEN, GCP creds env names are unchanged. | none |
| Build artifacts | None; no `Cargo.toml` dependency changes, so no `Cargo.lock` regeneration concerns beyond normal compile. | none |

**Nothing found in category:** Verified by grep of all on-disk paths, sandbox config field names, and tmux session prefixes against Phase 3 surface (`container_config.rs` + `instance.rs::get_container_for_instance` + `instance.rs::container_workdir`). No state crosses the wire that would break under the planned changes.

## Common Pitfalls

### Pitfall 1: SC-3 ambiguity between ROADMAP and CONTEXT

**What goes wrong:** ROADMAP.md Phase 3 SC-3 says: *"A workspace + `volume_ignores` combination that cannot be conformed to `host_path == container_path` returns `Err` with a typed variant"*. CONTEXT.md D-08 says: *"Only `extra_volumes` (user-declared via `sandbox.extra_volumes`) trigger the typed error."* A planner who reads ROADMAP and starts writing a `volume_ignores`-triggered test will produce a test that contradicts CONTEXT D-08.

**Why it happens:** ROADMAP was written first and used `volume_ignores` as the example of "a thing that could cause conformance to fail." CONTEXT later locked the narrower rule; only `extra_volumes` errors loudly, everything else (including `volume_ignores` → anonymous volumes) is either silently dropped or unaffected.

**How to avoid:** Treat ROADMAP SC-3's `volume_ignores` mention as an example of "a sbx-unfriendly user config", not a specification of which field triggers the error. The plan MUST encode the SC-3 test as **"extra_volumes with `host != container` returns `Err::InconformableExtraVolume`"**, not "volume_ignores shadows trigger Err". Document this resolution in the PLAN.md so future readers don't reopen.

**Warning signs:** Any test fixture that sets `volume_ignores = [...]` and expects a typed Err; that test is testing the wrong thing.

### Pitfall 2: Naive workdir prefix rewriting breaks bare-repo case

**What goes wrong:** Bare-repo worktree case (`compute_volume_paths` lines 656-681): single volume `host=/path/to/repo, container=/workspace/repo`, but `working_dir=/workspace/repo/main` (a SUBDIRECTORY of the mount). A naive "loop through volumes and `working_dir.replace(old_container, new_host)`" can match the wrong prefix if a sibling-worktree case puts two volumes under `/workspace/`.

**Why it happens:** `String::replace` doesn't respect path boundaries; `/workspace/r` could partially match `/workspace/repo`. Sibling-worktree case (line 696-713) has two volumes both under `/workspace/`.

**How to avoid:** Use longest-prefix-match with explicit path-separator boundary check. Build `Vec<(old_container_with_trailing_slash, new_host_with_trailing_slash)>` (or use `Path::strip_prefix`), sort by `old_container.len()` descending, find the first prefix where `working_dir == prefix || working_dir.starts_with(&format!("{prefix}/"))`, strip it, prepend the host path.

**Warning signs:** SC-1 test for the bare-repo case (workspace under `~/.agent-of-empires/worktrees/...`) fails or produces a workdir like `/host/path/repo/main` when the bare-repo's worktree directory is named `main`. The test must explicitly exercise the bare-repo case where workdir != volume.container_path.

### Pitfall 3: HOME mutation breaks parallel test execution

**What goes wrong:** Every existing `build_container_config` test sets `std::env::set_var("HOME", temp_home.path())` and uses `#[serial_test::serial]`. A new SC-1/SC-2/SC-3 test that forgets `#[serial]` will race with other tests, intermittently failing because HOME / XDG_CONFIG_HOME flips mid-test.

**Why it happens:** `cargo test` runs tests in parallel threads by default. `std::env` is process-global. The `#[serial]` attribute serializes a subset of tests against each other.

**How to avoid:** Every new test that calls `build_container_config` MUST be `#[serial_test::serial]`. Pattern is visible at lines 2395, 2543, 2658, 2871, etc. The SC-4 ensure_image test, if it doesn't call `build_container_config`, may not need `#[serial]`; but if it constructs an `Instance` and touches `containers::get_container_runtime()`, treat that as global state too and add `#[serial]`.

**Warning signs:** Test passes locally but fails in CI with "config file at unexpected path" or wrong default profile resolved.

### Pitfall 4: `compute_volume_paths` canonicalizes paths; `host_path` after conformance is the canonicalized form

**What goes wrong:** Tests that hard-code `host_path = "/tmp/foo"` will fail on macOS where `/tmp` is a symlink to `/private/tmp`. The existing tests call `.canonicalize()` everywhere (lines 1259-1260, 1377, 1499). A naive SC-1 test that asserts `volumes[0].container_path == "/tmp/foo"` after conformance will fail because the host_path was canonicalized to `/private/tmp/foo` first.

**Why it happens:** `compute_volume_paths` already canonicalizes via `path.canonicalize()` for git repo detection (lines 645-650). Conformance preserves whatever `host_path` is, so canonicalized host = canonicalized container.

**How to avoid:** SC-1 tests should either (a) canonicalize the expected path before comparing, mirroring the existing pattern, or (b) use a TempDir's already-canonicalized `path()` (TempDir paths on macOS are under `/var/...` which `canonicalize` may rewrite to `/private/var/...`). The "path with spaces" and "path under `~/.agent-of-empires/worktrees/...`" success-criterion shapes are best built via TempDir + manual subdirs to control canonicalization.

**Warning signs:** Test fails on macOS but passes on Linux with a diff between `/tmp/...` and `/private/tmp/...`.

### Pitfall 5: hook-status-dir path under debug builds is `~/.agent-of-empires-dev/...` not `~/.agent-of-empires/...`

**What goes wrong:** AGENTS.md notes "Debug builds use an isolated namespace ... app data dir is `~/.agent-of-empires-dev`". The hook_status_dir (container_config.rs:993, line 1001-1006 mount) goes under the debug app dir during `cargo test`, not the release path. Phase 3 tests that hard-code `expected = "/home/user/.agent-of-empires/hooks/<id>"` will fail.

**Why it happens:** `crate::hooks::hook_status_dir(instance_id)` reads the runtime-determined app dir, which differs between debug and release.

**How to avoid:** SC-1 tests should not hard-code the hook-status-dir path. The relevant SC-1 assertion is "the workspace mount has `container_path == host_path`", not "the hook dir has specific X." If the hook-dir survival needs to be tested (D-05 says it already conforms), assert via the predicate `v.host_path == v.container_path`, not via path string equality.

**Warning signs:** SC-1 test asserts a specific hook-status path and fails because the test was running against the `-dev` namespace.

### Pitfall 6: `extra_volumes` malformed-entry handling already exists; don't double-warn

**What goes wrong:** Lines 1069-1086 in `container_config.rs` already handle malformed extra_volumes (`parts.len() < 2` → `tracing::warn!("Ignoring malformed extra_volume entry")`). The post-pass validator that re-runs `splitn(3, ':')` must NOT also warn on the same input; it would double-log and confuse debug output.

**Why it happens:** Two parse loops over the same input, both emitting logs. The post-pass only needs to check `parts.len() >= 2 && parts[0] != parts[1]`; malformed entries (`parts.len() < 2`) are already filtered out at line 1071 (they don't make it into the `volumes` Vec, but they DO remain in `sandbox_config.extra_volumes` because the post-pass iterates the raw input).

**How to avoid:** The post-pass validator skips entries where `parts.len() < 2`. Only entries where `parts.len() >= 2 && parts[0] != parts[1]` trigger the typed error. This matches CONTEXT.md skeleton (line 148: `if parts.len() >= 2 && parts[0] != parts[1]`).

**Warning signs:** Test sees double-log output, or post-pass returns Err on a malformed entry that the push-loop already filtered.

### Pitfall 7: Threading `caps` through 19 `container_workdir()` call sites (keep the signature change clean)

**What goes wrong:** D-10 says `container_workdir()` gains `caps: RuntimeCapabilities`. There are 13 call sites across 5 files (enumerated in Integration Points below). A bulk find/replace that mis-edits one; e.g. forgetting to pass the param in a poll_fn; produces compile errors at every changed site.

**Why it happens:** Manual edits to 13 sites are error-prone. Each call site has access to either `self` (Instance method) or an `instance: &Instance` reference, so each needs to derive `caps` locally.

**How to avoid:** Two options:
1. **Add caps param** to `container_workdir(caps: RuntimeCapabilities)`; each call site does `let caps = containers::get_container_runtime().capabilities(); instance.container_workdir(caps)`. 13 mechanical edits.
2. **Read caps inside `container_workdir()`**; `let caps = containers::get_container_runtime().capabilities();` inside the function, no signature change, all 13 call sites untouched.

CONTEXT.md D-10 picks option 1 ("Keeps `container_workdir()` a pure derivation, no global lookup"). **Recommend sticking with D-10.** It's more code but the function stays mockable for tests, and the test surface for SC-1 / SC-2 is the `container_workdir` function itself; passing caps as an arg makes the test trivial.

**Warning signs:** A poll_fn compiles but produces a wrong workdir at runtime because someone fetched `caps` from the wrong runtime in scope. Catching this would require integration testing, which Phase 3 doesn't include; so the unit test for `container_workdir(caps_sbx) -> /host/path/...` and `container_workdir(caps_docker) -> /workspace/...` is the only safety net.

### Pitfall 8: SC-2 baseline test isolation; global default profile leaks

**What goes wrong:** `test_build_container_config_uses_passed_profile_not_global_default` (line 2395) deliberately tests profile resolution. A SC-2 baseline test that calls `build_container_config(..., "")` (empty profile) inherits whatever the test's HOME has configured as the global default. Without explicit setup, the test may see different volumes on different machines.

**Why it happens:** Empty-string profile means "resolve to user's globally configured default profile" (line 862, `effective_profile`). In an isolated TempDir HOME with no global config, this defaults to `default`, but if any test leaks state (HOME not reset), behavior diverges.

**How to avoid:** SC-2 baseline test should create a TempDir HOME, write an empty global config, and pass a known profile name. Mirror the setup at line 2395-2440 for predictability. Use `#[serial_test::serial]`.

**Warning signs:** SC-2 test passes locally but fails in CI due to extra mounts from a profile that the local dev had set up.

## Code Examples

### Example 1: `ContainerConfigError` variant

```rust
// src/session/container_config.rs (inline at top, after `use` block)
// Source: CONTEXT.md D-06 + project pattern from src/containers/error.rs

#[derive(Debug, thiserror::Error)]
pub enum ContainerConfigError {
    #[error(
        "extra_volume `{entry}` cannot conform to host_path == container_path \
         under runtime {runtime}; {reason}. Fix: use `<host>:<host>` or remove the entry."
    )]
    InconformableExtraVolume {
        entry: String,
        runtime: ContainerRuntimeName,
        reason: String,
    },
}
```

`ContainerRuntimeName` already derives `Debug` (config.rs:734). It does NOT derive `Display`; verify before relying on `{runtime}` in the error message; may need `{runtime:?}` or add `#[derive(Display)]` via `strum_macros` (avoid: pulls in a new dep). **Recommend `{runtime:?}` (uses the Debug impl, no new dep, matches existing variant rendering in `DockerError` which uses simple strings).**

### Example 2: SC-1 unit test shape

```rust
// src/session/container_config.rs `mod tests` block
// Source: project-specific, modeled on test_build_container_config_includes_repo_sandbox_settings (line 2301)

#[test]
#[serial_test::serial]
fn test_build_container_config_sbx_conforms_workspace_mount_plain_path() {
    let temp_home = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_home.path());
    #[cfg(target_os = "linux")]
    std::env::set_var("XDG_CONFIG_HOME", temp_home.path().join(".config"));

    let project_dir = TempDir::new().unwrap();
    git2::Repository::init(project_dir.path()).unwrap();

    let sandbox_info = super::super::instance::SandboxInfo {
        enabled: true,
        container_id: None,
        image: "test:latest".to_string(),
        container_name: "test-container".to_string(),
        extra_env: None,
        custom_instruction: None,
    };

    let project_path_str = project_dir.path().to_str().unwrap();
    let caps = crate::containers::runtime_base::RuntimeBase::SBX.capabilities;

    let config = build_container_config_with_caps(  // new signature
        project_path_str,
        &sandbox_info,
        "claude",
        false,
        "test-instance-id",
        None,
        "",
        caps,
        crate::session::config::ContainerRuntimeName::Sbx,
    )
    .unwrap();

    // The primary workspace mount should have host_path == container_path.
    let canonical_project = project_dir.path().canonicalize().unwrap();
    let canonical_str = canonical_project.to_string_lossy().to_string();
    let workspace_mount = config
        .volumes
        .iter()
        .find(|v| v.host_path == canonical_str)
        .expect("workspace mount must survive conformance");
    assert_eq!(workspace_mount.container_path, workspace_mount.host_path);
    assert_eq!(config.working_dir, workspace_mount.host_path);

    // No convenience mount survived (gitconfig, SSH, etc.); all dropped.
    for v in &config.volumes {
        assert_eq!(
            v.host_path, v.container_path,
            "every surviving mount must be conformant; got {:?} -> {:?}",
            v.host_path, v.container_path
        );
    }
}
```

### Example 3: SC-3 typed-error downcast test

```rust
#[test]
#[serial_test::serial]
fn test_build_container_config_sbx_inconformable_extra_volume_errors() {
    let temp_home = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_home.path());
    #[cfg(target_os = "linux")]
    std::env::set_var("XDG_CONFIG_HOME", temp_home.path().join(".config"));

    let project_dir = TempDir::new().unwrap();
    let config_dir = project_dir.path().join(".agent-of-empires");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        r#"
[sandbox]
extra_volumes = ["/host/data:/container/data:ro"]
"#,
    )
    .unwrap();
    git2::Repository::init(project_dir.path()).unwrap();

    let sandbox_info = super::super::instance::SandboxInfo { /* ... */ };
    let caps = crate::containers::runtime_base::RuntimeBase::SBX.capabilities;

    let err = build_container_config_with_caps(
        project_dir.path().to_str().unwrap(),
        &sandbox_info,
        "claude",
        false,
        "test-instance-id",
        None,
        "",
        caps,
        crate::session::config::ContainerRuntimeName::Sbx,
    )
    .expect_err("inconformable extra_volume must error under sbx caps");

    match err.downcast_ref::<ContainerConfigError>() {
        Some(ContainerConfigError::InconformableExtraVolume { entry, .. }) => {
            assert_eq!(entry, "/host/data:/container/data:ro");
        }
        other => panic!("expected InconformableExtraVolume, got {:?}", other),
    }
}
```

### Example 4: SC-4 ensure_image skip test

The challenge: `get_container_for_instance` calls `containers::get_container_runtime()` which is a global. Three options for SC-4:

1. **Direct unit test of the capability gate logic**; extract the guard into a `fn should_pull_image(caps: RuntimeCapabilities) -> bool { caps.supports_image_pull }` (overkill for a 2-line guard).
2. **Test the capability assertion only**; `assert!(!RuntimeBase::SBX.capabilities.supports_image_pull); assert!(RuntimeBase::DOCKER.capabilities.supports_image_pull);` and rely on `cargo expand` / code review to verify the guard's correct shape.
3. **Behavioral test via tracing/log capture**; wrap the call in tracing spans; subscribe a test layer; assert the "ensure_image called" event fires for Docker and not for sbx.

CONTEXT.md says: *"SC-4 unit test asserts the capability-driven branch by exercising `Instance::ensure_container()` with a session whose resolved runtime reports `!supports_image_pull` and verifying `ensure_image` is not invoked."* This implies option 3, BUT it requires a mock runtime injectable into `Instance`, which doesn't exist today. **Recommendation:** Combine option 2 (cheap capability assertion) with a static-source assertion via `include_str!`/`assert!`-on-substring of the file content. The whole guard is two lines; a substring assertion is sufficient and zero-infrastructure.

```rust
// src/session/instance.rs mod tests (new) or container_config.rs
#[test]
fn test_ensure_image_call_site_is_capability_gated() {
    // Pin the capability-driven branch by content-inspecting the source.
    // If someone removes the guard, this test fails immediately.
    let src = include_str!("instance.rs");
    let idx = src.find("runtime.ensure_image(image)").expect("call site must exist");
    let prefix = &src[idx.saturating_sub(200)..idx];
    assert!(
        prefix.contains("supports_image_pull"),
        "ensure_image call must be gated on supports_image_pull; got prefix: {}",
        prefix
    );
}

#[test]
fn test_capability_matrices_drive_ensure_image_branch() {
    // Companion assertion: the matrices the guard reads are honest.
    assert!(RuntimeBase::DOCKER.capabilities.supports_image_pull);
    assert!(RuntimeBase::PODMAN.capabilities.supports_image_pull);
    assert!(RuntimeBase::APPLE_CONTAINER.capabilities.supports_image_pull);
    assert!(!RuntimeBase::SBX.capabilities.supports_image_pull);
}
```

This pair of tests proves the SC-4 requirement: the gate exists at the right site, and the matrices it reads are correct. Adding a full mock-runtime test scaffold is non-trivial (no current test fakes `containers::get_container_runtime()`) and isn't proportional to a two-line guard.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `if runtime == Sbx` branching scattered | Capability-flag-driven gating via `RuntimeCapabilities` | Phase 1 (locked 2026-05-15) | Phase 3 inherits this; every new gate is a capability check, never a name check |
| Runtime stored as `RuntimeBase` const only | Phase 1 const + Phase 2 `SbxRuntime` peer struct dispatched via `RuntimeKind::Sbx` arms | Phase 2 (locked 2026-05-15) | Phase 3 reads via `runtime.capabilities()`; both Docker (via `RuntimeBase.capabilities`) and sbx (via `SbxRuntime` peer) flow through the same trait method |
| `ensure_image` always called pre-create | `ensure_image` gated by `supports_image_pull` (Phase 3) | This phase | Sbx silently skips; Docker unchanged |

**Deprecated/outdated:** None. Phase 3 does not deprecate any code path; it extends gating semantics. The `/workspace/<dir_name>` strategy for sbx is "deprecated" in the sense that the post-pass rewrites it, but the path is still computed by the capability-blind `compute_volume_paths`; it's then replaced for sbx.

## Project Constraints (from CLAUDE.md/AGENTS.md)

| Directive | Source | Phase 3 Compliance Check |
|-----------|--------|--------------------------|
| `cargo fmt` + `cargo clippy` must pass; clippy warnings fixed unless flagged | AGENTS.md § Coding Style | All new code formatted; no clippy warnings expected (additions are simple) |
| **No dead code; never add `#[allow(dead_code)]`** | AGENTS.md § Coding Style | Drives D-08 (single error variant). Plan must NOT add unused variants, fields, or helpers. If `conform_workspace_paths` is only used by `container_workdir`, that's two consumers; fine. |
| **No emdashes (,) or `--` separators in docs/comments** | AGENTS.md § Coding Style | RESEARCH.md and PLAN.md comments must use commas, semicolons, or rephrase. |
| Naming: `snake_case` modules/functions, `CamelCase` types, `SCREAMING_SNAKE_CASE` constants | AGENTS.md | `conform_workspace_paths`, `conform_for_capabilities` (snake_case fns); `ContainerConfigError` (CamelCase type); no constants added |
| **No inline `#[cfg(target_os = ...)]` outside `src/process/`** | AGENTS.md, REQ-RT-08 | Phase 3 adds zero `#[cfg(target_os)]` branches. Existing tests in container_config.rs use them for `XDG_CONFIG_HOME` setup; copy pattern unchanged. |
| Comments explain "why", skip section headers | AGENTS.md | Comments on `conform_for_capabilities` should explain "why these particular drop rules" / "why owned-passthrough"; not narrate the code |
| Don't preserve backwards compat by default; call out breaking changes | AGENTS.md | Adding `capabilities` param to `build_container_config` and `container_workdir` is a breaking internal change; the single in-tree call site of each gets threaded. No external API impact. |
| Unit tests in-module `#[cfg(test)]` for pure logic | AGENTS.md § Testing Guidelines | All Phase 3 tests go in `mod tests` at the bottom of `container_config.rs` and `instance.rs` (instance.rs doesn't have a tests mod today; adding one is the lighter-weight choice over a new `tests/instance_caps.rs` integration file). |
| Tests deterministic, clean up after themselves | AGENTS.md | All new tests use `tempfile::TempDir` (auto-cleanup) + `#[serial_test::serial]` (pattern from existing tests). |
| Settings TUI fields fully wired (SET-01) | AGENTS.md § Settings & Configuration | Phase 3 changes NO settings fields. Confirmed by inspection of `sandbox.extra_volumes` (existing field, no change to its TUI representation). Phase 6 handles SET-01/SET-02. |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `ContainerRuntimeName` does NOT impl `Display` and the error message must use `{runtime:?}` (Debug) rather than `{runtime}` | Code Examples / Example 1 | Clippy/compile error on the `#[error("...{runtime}...")]` attribute string. Fix: change to `{runtime:?}` or add a manual `Display` impl. Low risk; surfaces immediately at compile time. |
| A2 | `hooks::hook_status_dir(instance_id)` returns a path under `~/.agent-of-empires-dev/...` in debug builds, NOT under the workspace | Pitfall #5, D-05 commentary | If the function actually returns a workspace-relative path today, then D-05's claim that "hook-status-dir already uses host==container shape" is correct *only because* the path's container_path matches its host_path (it's mounted at the same absolute path either way). Either way the conformance pass survives the mount; but the SC-1 test should NOT assert path content, only the predicate `host_path == container_path`. Verified the source pushes `host = container = hook_dir.to_string_lossy()` at lines 1001-1006, so the predicate holds regardless of which app-dir variant. |
| A3 | The 19 `container_workdir()` call sites enumerated below are the complete set in the current `main` worktree-agent branch | Integration Points | If a recent commit added a 14th call site, the plan undercounts the threading work by one edit. Mitigation: planner should re-run `grep -rn "container_workdir" src/` immediately before writing PLAN.md tasks. |
| A4 | The Phase 2 `RuntimeKind::Sbx` dispatch arms have already wired `runtime.capabilities()` to return the sbx matrix via the `SbxRuntime` peer struct (per Phase 2 D-12) | Architecture Patterns | If Phase 2 punted on this and `runtime.capabilities()` for sbx still returns the `RuntimeBase::SBX` const literal rather than going through `SbxRuntime::capabilities()`, the Phase 3 guard at instance.rs:1249 still works correctly because both paths yield the same matrix. Low risk; values are identical by Phase 2 D-12. |
| A5 | No `insta` snapshot crate is desired by the user | Standard Stack, Don't Hand-Roll | User memory (`feedback_dependencies_and_gates.md`) explicitly says "prefer stdlib + existing runtime checks over new deps" and `feedback_minimum_lib_churn.md` says "pick the smallest-delta option"; both point at no `insta`. Verified by reading the user's stored feedback memories. |
| A6 | The SC-2 baseline can be inlined in test code as a Rust literal (no on-disk fixture file needed) | Don't Hand-Roll, Validation Architecture | Acceptable; the baseline is small (handful of volumes for a known input). If the planner discovers the volume Vec is unwieldy (10+ entries), an on-disk fixture under `tests/fixtures/container_config_baseline.txt` becomes more readable. Planner decides at test-write time. |

## Open Questions (RESOLVED)

1. **Should the SC-4 test use the source-substring assertion (Example 4 above) or build a mock-runtime test scaffold?**
   - What we know: CONTEXT.md describes the latter ("session whose resolved runtime reports `!supports_image_pull`") but no infrastructure exists for it.
   - What's unclear: Whether to invest the test-design effort in a runtime mock, or accept the substring assertion as proportional.
   - RESOLVED: Use the substring + capability-matrix assertion pair. Document that a full integration test waits for Phase 5 (which has real subprocess wiring).

2. **Does the Phase 3 plan add a `tests` mod to `src/session/instance.rs`?**
   - What we know: `instance.rs` is 3164 lines and has no in-module tests today (verified by `grep "^#\\[cfg(test)\\]" src/session/instance.rs`). All tests are integration-style in `tests/*.rs`.
   - What's unclear: Whether SC-4 test belongs in a new `instance.rs::tests` mod (lighter), in `container_config.rs::tests` (Phase 3 surface), or in a new `tests/capability_gating.rs` integration test.
   - RESOLVED: Put SC-4 tests in `container_config.rs::tests` (the source-substring assertion reads instance.rs via `include_str!`, and the capability-matrix assertion needs no Instance at all). Keeps all Phase 3 tests in one module.

3. **Should `conform_for_capabilities` clear `anonymous_volumes` when `!supports_anonymous_volumes`?**
   - What we know: CONTEXT.md says "planner discretion" but recommends clearing.
   - What's unclear: Whether sbx's anonymous_volumes path-shape (`<workspace>/<ignored>`) can actually be present after Phase 3 conformance; once the workspace mount is rewritten to `host_path`, the anonymous volume strings (e.g., `/workspace/repo/target`) reference the OLD container path and would be dangling.
   - RESOLVED: **Clear `anonymous_volumes` in the post-pass**. The strings are stale references to pre-conformance container paths and would mislead any consumer that reads them downstream. Document this as a third reason (beyond the two in CONTEXT.md) in PLAN.md.

4. **Should Phase 3 explicitly test the multi-repo workspace conformance shape?**
   - What we know: `compute_workspace_volume_paths` exists for multi-repo workspaces; it produces multiple `VolumeMount` entries.
   - What's unclear: Whether SC-1's "three workspace-path shapes" covers multi-repo, or whether multi-repo is a fourth shape worth testing.
   - RESOLVED: Add a fourth SC-1 test case ("multi-repo workspace conforms each volume independently"). It's a 30-line test using `setup_bare_repo_with_worktree` patterns; the extra coverage prevents regressions if `compute_workspace_volume_paths` is ever modified.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | Build/test | Assumed (Rust project) | none | none; required |
| `git` | Tests creating worktree fixtures | Assumed (already required by aoe runtime) | none | Existing tests `return` early if `git worktree add` fails (line 1317, 2575); Phase 3 tests should mirror this pattern |
| `tmux` | Not required for Phase 3 | none |; | Phase 3 tests do not need tmux; this is pure logic / config-building |
| `docker` / `podman` / `sbx` | Not required for Phase 3 | none |; | Phase 3 tests use `RuntimeBase::*.capabilities` const literals; no subprocess execution |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none

Phase 3 is pure-logic refactor: zero external runtime dependencies, zero subprocess calls, zero new tooling required.

## Validation Architecture

> Required because `workflow.nyquist_validation: true` in `.planning/config.json`.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | rustc built-in (`cargo test`) + `serial_test` crate (already dev-dep) |
| Config file | none; uses Cargo defaults; tests live in `#[cfg(test)] mod tests` blocks |
| Quick run command | `cargo test -p agent-of-empires --lib container_config::tests::test_build_container_config_sbx -- --include-ignored` (matches Phase 3 test prefix) |
| Full suite command | `cargo test --lib` (in-module unit tests) and `cargo test` (full Cargo workspace) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RT-04 SC-1a | sbx caps, plain workspace path → `working_dir == primary VolumeMount.container_path == primary VolumeMount.host_path` | unit | `cargo test --lib test_build_container_config_sbx_conforms_workspace_mount_plain_path` | ❌ Wave 0; new test |
| RT-04 SC-1b | sbx caps, path with spaces → same conformance | unit | `cargo test --lib test_build_container_config_sbx_conforms_workspace_mount_path_with_spaces` | ❌ Wave 0; new test |
| RT-04 SC-1c | sbx caps, path under `~/.agent-of-empires/worktrees/...` (bare-repo worktree) → workdir is the worktree's host_path, mount is the repo's host_path | unit | `cargo test --lib test_build_container_config_sbx_conforms_bare_repo_worktree` | ❌ Wave 0; new test |
| RT-04 SC-1d (recommended bonus) | sbx caps, multi-repo workspace → each volume independently conformed | unit | `cargo test --lib test_build_container_config_sbx_conforms_multi_repo_workspace` | ❌ Wave 0; new test |
| RT-04 SC-2 | Docker caps → byte-identical output to pre-Phase-3 baseline; convenience mounts preserved; working_dir at `/workspace/<dir_name>` | unit | `cargo test --lib test_build_container_config_docker_baseline_unchanged` | ❌ Wave 0; new test; baseline encoded as inline `Vec<(host, container, ro)>` literal |
| RT-04 SC-3 | sbx caps + `extra_volumes = ["/x:/y:ro"]` → `Err` downcasts to `ContainerConfigError::InconformableExtraVolume` with `entry == "/x:/y:ro"` | unit | `cargo test --lib test_build_container_config_sbx_inconformable_extra_volume_errors` | ❌ Wave 0; new test |
| RT-04 SC-3-companion | sbx caps + `extra_volumes = ["/x:/x"]` → conforms successfully (host == container, no error) | unit | `cargo test --lib test_build_container_config_sbx_conformant_extra_volume_passes` | ❌ Wave 0; new test (proves the error is precisely-targeted, not over-broad) |
| RT-04 SC-3-companion | Docker caps + `extra_volumes = ["/x:/y:ro"]` → succeeds (no error; capability gate makes the validator a no-op) | unit | `cargo test --lib test_build_container_config_docker_inconformable_extra_volume_ok` | ❌ Wave 0; new test |
| RT-06 SC-4 | `instance.rs::get_container_for_instance` call site contains `if runtime.capabilities().supports_image_pull` before `runtime.ensure_image(image)` | unit | `cargo test --lib test_ensure_image_call_site_is_capability_gated` | ❌ Wave 0; new test (source-substring assertion via `include_str!`) |
| RT-06 SC-4-companion | Capability matrices for Docker/Podman/AppleContainer report `supports_image_pull=true`; sbx reports `false` | unit | `cargo test --lib test_capability_matrices_drive_ensure_image_branch` | ❌ Wave 0; new test |
| Conform helper | `conform_workspace_paths(volumes, "/workspace/repo", caps_sbx)` rewrites both volumes and workdir correctly | unit | `cargo test --lib test_conform_workspace_paths_sbx_rewrites_workdir_under_volume` | ❌ Wave 0; new direct test of the helper |
| Conform helper | `conform_workspace_paths(volumes, wd, caps_docker)` is identity | unit | `cargo test --lib test_conform_workspace_paths_docker_is_identity` | ❌ Wave 0; new direct test |
| container_workdir | `Instance::container_workdir(caps_sbx)` returns the host_path; `container_workdir(caps_docker)` returns the `/workspace/...` path | unit | `cargo test --lib test_instance_container_workdir_threads_capabilities` | ❌ Wave 0; new test |

### Sampling Rate
- **Per task commit:** `cargo test --lib container_config::tests` (in-module unit tests, runs in <2s)
- **Per wave merge:** `cargo test --lib` plus `cargo fmt --check` plus `cargo clippy -- -D warnings`
- **Phase gate:** `cargo test` (full suite including integration tests) + `cargo deny check` per AGENTS.md TEST-05

### Wave 0 Gaps

- [ ] **No new fixture files needed.** The SC-2 baseline is encoded as an inline `Vec<(String, String, bool)>` literal in the test. Confirmed by inspection: existing tests in `mod tests` (lines 1156+, 2301+) all use inline literals, no `tests/fixtures/` directory has container_config baselines.
- [ ] **No new framework install.** `serial_test` (3.4), `tempfile` (3.14), `thiserror` (2.0), `anyhow` (1.0) all present in `Cargo.toml`. Zero new deps.
- [ ] **Test mod for `instance.rs`**; if SC-4 source-substring test lives in `container_config.rs::tests`, no new mod needed. If planner picks a different location, the planner adds `#[cfg(test)] mod tests` at the bottom of `instance.rs` (it has none today).
- [ ] **`build_container_config` signature change ripple test:** add a smoke test that confirms the single call site at `instance.rs:1274-1283` compiles and forwards `runtime.capabilities()` correctly. Pragmatically this is `cargo build`'s job; if the new param threads, it builds; if it doesn't, the planner finds out at compile time. No separate test needed.

### Validation Coverage Summary

| Success Criterion | Required Coverage | Tests Listed Above |
|-------------------|-------------------|---------------------|
| SC-1 (≥3 workspace shapes) | 3+ unit tests under sbx caps | SC-1a, SC-1b, SC-1c; covered; SC-1d bonus recommended |
| SC-2 (Docker output unchanged) | 1 unit test against known-good baseline | SC-2; covered (inline literal baseline) |
| SC-3 (inconformable extra_volumes → typed Err) | 1 positive case + companion negative cases | SC-3 + 2 companions; covered |
| SC-4 (ensure_image skipped when `!supports_image_pull`) | 1 unit test exercising the call-site guard + matrix check | SC-4 + companion; covered (source-substring approach) |

## AI Integration Phase

> Phase 3 is a Rust refactor of pure config-building logic. **No AI-integration surface.** No prompts, no LLM calls, no agent-facing behavior changes. The `tool` parameter to `build_container_config` is read to pick agent-specific mounts but no agent prompt or completion is invoked.

The phase satisfies `ai_integration_phase: true` from `.planning/config.json` by being a no-op for AI integration: no prompts to design, no token budgets to set, no LLM provider integration to research.

## Integration Points

### Files Phase 3 modifies

| File | Change | Line-count delta estimate |
|------|--------|---------------------------|
| `src/session/container_config.rs` | +`ContainerConfigError` enum (1 variant), +`conform_workspace_paths` free fn, +`conform_for_capabilities` free fn, +`capabilities` param on `build_container_config`, +`ContainerRuntimeName` param on `build_container_config` (so the error can reference it), +9 new tests in `mod tests`, signature update at line 825 | +~250 lines (mostly tests) |
| `src/session/instance.rs` | +`capabilities` param on `container_workdir` (line 1263), +`capabilities` param on the inner `build_container_config` wrapper (line 1269), +19 call-site updates passing caps, +2-line `ensure_image` guard (line 1249-1250), +optional `tests` mod (if SC-4 lands here) | +~30 lines (mostly mechanical param threading) |

### 19 `container_workdir()` call sites that must be updated

Enumerated by `grep -rn "container_workdir" src/`:

| File | Line | Context |
|------|------|---------|
| `src/session/instance.rs` | 608 | `try_retroactive_capture`: opencode sandboxed |
| `src/session/instance.rs` | 622 | `try_retroactive_capture`: vibe sandboxed |
| `src/session/instance.rs` | 635 | `try_retroactive_capture`: pi sandboxed |
| `src/session/instance.rs` | 648 | `try_retroactive_capture`: codex sandboxed |
| `src/session/instance.rs` | 661 | `try_retroactive_capture`: gemini sandboxed |
| `src/session/instance.rs` | 674 | `try_retroactive_capture`: hermes sandboxed |
| `src/session/instance.rs` | 816 | `start_container_terminal_with_size`: exec workdir |
| `src/session/instance.rs` | 935 | on-launch hook execution in container |
| `src/session/instance.rs` | 1308 | `maybe_start_poller`: claude_poll_fn_sandboxed |
| `src/session/instance.rs` | 1326 | `maybe_start_poller`: opencode_poll_fn_sandboxed |
| `src/session/instance.rs` | 1346 | `maybe_start_poller`: vibe_poll_fn_sandboxed |
| `src/session/instance.rs` | 1361 | `maybe_start_poller`: pi_poll_fn_sandboxed |
| `src/session/instance.rs` | 1376 | `maybe_start_poller`: codex_poll_fn_sandboxed |
| `src/session/instance.rs` | 1391 | `maybe_start_poller`: gemini_poll_fn_sandboxed |
| `src/session/instance.rs` | 1406 | `maybe_start_poller`: hermes_poll_fn_sandboxed |
| `src/tui/creation_poller.rs` | 167 | container ready check workdir |
| `src/tui/creation_poller.rs` | 208 | container ready check workdir |
| `src/cli/remove.rs` | 92 | remove flow workdir |
| `src/session/deletion.rs` | 361 | deletion-time workdir |

**Total: 19 call sites across 4 files** (15 in instance.rs, 2 in creation_poller.rs, 1 each in remove.rs and deletion.rs). CONTEXT.md mentioned "13" and "1308+"; that was an undercount. Plan must update all 19.

Each call site sits inside a function that has access to `Instance` (either as `self` or `instance: &Instance`), so each can derive `caps` via `containers::get_container_runtime().capabilities()` locally. The threading is mechanical but unavoidable.

**Threading recommendation:** Introduce a helper on `Instance` to keep callsite churn minimal:
```rust
// On Instance
fn container_workdir_now(&self) -> String {
    let caps = containers::get_container_runtime().capabilities();
    self.container_workdir(caps)
}
```
Then all 19 sites call `container_workdir_now()` (3-char append: `_now`). The pure `container_workdir(caps)` stays mockable for unit tests; the `_now` shim handles the global lookup once. **This wasn't in CONTEXT.md; it's a research-level recommendation the planner should consider; it cuts 19 mechanical edits to 19 trivial appends plus 1 new method.**

Alternative: don't add `_now`, just do 19 mechanical edits. Both work; the helper saves churn but adds one tiny indirection.

### Files Phase 3 reads (must not modify)

- `src/containers/container_interface.rs` (`RuntimeCapabilities`, `ContainerConfig`, `VolumeMount`)
- `src/containers/runtime_base.rs` (`RuntimeBase::DOCKER/PODMAN/APPLE_CONTAINER/SBX.capabilities` const literals)
- `src/containers/runtime.rs` (`ContainerRuntime::capabilities()` accessor; Phase 1/2 already wired)
- `src/containers/error.rs` (pattern reference for `ContainerConfigError`)
- `src/session/config.rs` (`SandboxConfig.extra_volumes`, `ContainerRuntimeName` enum)

### Files Phase 3 does NOT touch

- `src/containers/sbx/` (Phase 2 owns; Phase 3 reads only via `runtime.capabilities()`)
- `src/server/`, `src/tui/`, `src/cockpit/`, `src/cli/` (except the 4 `container_workdir` consumers above, which are mechanical param threads)
- `src/migrations/` (no schema changes per Out-of-Scope table in REQUIREMENTS.md)
- `src/process/` (no OS-specific logic added in Phase 3)
- `Cargo.toml` / `Cargo.lock` (no new deps)

## Sources

### Primary (HIGH confidence)
- `.planning/phases/03-container-config-capability-gating-and-workspace-path-confor/03-CONTEXT.md`; Locked decisions D-01..D-13, planner-discretion list, skeleton code (Read in full)
- `.planning/REQUIREMENTS.md`; RT-04 and RT-06 wording; Out-of-Scope table (Read in full)
- `.planning/ROADMAP.md`; Phase 3 success criteria SC-1..SC-4 (Read in full; SC-3 vs CONTEXT D-08 conflict identified)
- `.planning/STATE.md`; Confirms Phase 1 and Phase 2 complete (Read)
- `.planning/config.json`; `nyquist_validation: true`, `ai_integration_phase: true`, etc. (Read)
- `src/session/container_config.rs` (the file being modified); Lines 1-80, 620-1148 (function bodies), 1156-3014 (existing tests inventory)
- `src/session/instance.rs`; Lines 580-680, 790-840, 925-950, 1220-1420 (all `container_workdir` and `ensure_image` call sites)
- `src/containers/container_interface.rs` (1-137, full read)
- `src/containers/runtime_base.rs` (1-637, full read)
- `src/containers/error.rs` (1-53, full read for pattern reference)
- `src/session/config.rs` lines 680-742 (SandboxConfig and ContainerRuntimeName)
- `.planning/phases/01-capability-surface-and-trait-foundation/01-CONTEXT.md` (lines 1-60)
- `.planning/phases/02-sbxruntime-skeleton-and-pure-argv-builders/02-CONTEXT.md` (lines 1-100)
- `.planning/codebase/TESTING.md`; In-module unit-test pattern, serial_test convention
- `AGENTS.md` (project root, symlinked as CLAUDE.md); No-dead-code, no-emdash, no-cfg-outside-process, testing guidelines
- User memory `feedback_dependencies_and_gates.md`; "prefer stdlib + existing runtime checks over new deps"
- User memory `feedback_minimum_lib_churn.md`; "pick the smallest-delta option"

### Secondary (MEDIUM confidence)
- `.planning/sbx-cli-reference.md` (lines 216-275); Confirms sbx's host-path-only mount model (PRIMARY for the constraint; MEDIUM for tagging because it's a local mirror of the official Docker Sandboxes docs, not the docs directly)
- Docker Sandboxes architecture docs (https://docs.docker.com/ai/sandboxes/architecture/) referenced via canonical_refs

### Tertiary (LOW confidence)
- None. All Phase 3 claims are verifiable against the codebase or CONTEXT.md.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH; all libs are existing deps, version-verified via `Cargo.toml` grep
- Architecture: HIGH; CONTEXT.md locks D-01..D-13; research adds the owned-passthrough recommendation and 19-site count
- Pitfalls: HIGH; derived from reading the actual source files and existing tests; each pitfall has a concrete file:line reference
- Validation Architecture: HIGH; every success criterion mapped to a concrete test command; existing test patterns at lines 2301, 2395, 2543, 2658, 2871 establish the shape
- SC-3 ambiguity resolution: MEDIUM; CONTEXT D-08 is authoritative, but the planner must explicitly resolve the ROADMAP wording in PLAN.md to prevent future re-litigation

**Research date:** 2026-05-15
**Valid until:** 2026-06-15 (30 days; this is stable refactor work with no external API dependencies)

---

*Phase: 03-container-config-capability-gating-and-workspace-path-confor*
*Research output: ready for `gsd-planner` consumption*
