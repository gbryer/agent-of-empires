# Phase 4: Embedded sbx Kit and Materialization - Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 8 (3 new source files + 1 new asset dir + 4 modified files)
**Analogs found:** 6 with strong codebase precedent / 8 total (2 new asset files have no in-tree analog; mirror live `openclaw` kit cited in RESEARCH.md)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/containers/sbx/kit.rs` | service (cache manager) | compile-time-embed -> filesystem write (atomic) -> path return | `src/logging.rs:447-468` (`persist_runtime_filter`); `src/tui/mod.rs:50` (spawn_blocking); `src/session/mod.rs:73` (`get_app_dir`) | role-match (atomic-rename + lazy cache) |
| `src/containers/sbx_kit/spec.yaml` | config (static asset) | file -> include_str! -> sbx kit format | live `code-server` / `openclaw` kit `spec.yaml` cited in RESEARCH.md (no in-tree analog) | external reference only |
| `src/containers/sbx_kit/install.sh.template` | config (static asset, templated body) | file -> include_str! -> generated shell script | None in-tree; `src/agents.rs::install_hint` (lines 188-434) is the source of the templated body | derived (composed from agents.rs) |
| `src/containers/sbx_kit/files/` (dir w/ settings.json files) | config (static asset, per-agent) | file -> include_str!/include_bytes! -> `initFiles[*].content` | None in-tree; pattern derived from RESEARCH.md `initFiles` example + `src/agents.rs::hook_config` (lines 178/253/293/424) | external reference only |
| `src/hooks/status_file.rs` (modified) | utility (path resolution) | `&Path + &str -> PathBuf` | `src/hooks/status_file.rs:13` (`hook_status_dir`) | exact (sibling free fn in same module) |
| `src/session/poller.rs` (modified) | controller (status polling) | runtime caps + Instance -> dispatch -> Status | `src/containers/runtime.rs:108-119` (`fn capabilities()` dispatch); `src/tui/app.rs:1055` (`read_hook_status` call site) | role-match (capability gating) |
| `xtask/src/main.rs` (modified) | utility (build/CI guard) | embedded file -> serde_yaml::Value -> assertions | `xtask/src/main.rs:21,28,59` (`CheckSkill` variant + `fn check_skill()`) | exact (peer subcommand) |
| `.github/workflows/ci.yml` (modified) | config (CI job step) | `cargo xtask check-kit` run | `.github/workflows/ci.yml:82-93` (`skill:` job) | exact (mirror skill job step) |

## Pattern Assignments

---

### `src/containers/sbx/kit.rs` (service, atomic-rename + lazy cache + GC)

**Primary analog:** `src/logging.rs:447-468` (atomic-rename idiom)
**Secondary analog:** `src/tui/mod.rs:50` (spawn_blocking for background work)
**Constants seeding:** `src/session/mod.rs:73-93` (`get_app_dir()` plus `cfg!(debug_assertions)` debug-vs-release isolation already baked in)

**Imports pattern** (derived from RESEARCH.md "Code Examples"; mirrors existing `src/server/push.rs:748` for sha2 + `src/logging.rs:447` for fs::rename):
```rust
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::session::get_app_dir;

const SPEC_YAML: &str = include_str!("../sbx_kit/spec.yaml");
const INSTALL_SH_TEMPLATE: &str = include_str!("../sbx_kit/install.sh.template");
// ... per-agent settings.json files via include_str!
```

**Atomic-rename pattern to MIRROR** (from `src/logging.rs:447-468`):
```rust
pub fn persist_runtime_filter(directive: &str, app_dir: &std::path::Path) {
    if let Err(e) = std::fs::create_dir_all(app_dir) {
        tracing::warn!(target: "log.runtime", error = %e, "could not create app dir for runtime_filter");
        return;
    }
    let path = runtime_filter_path(app_dir);
    let tmp = app_dir.join("runtime_filter.tmp");
    if let Err(e) = std::fs::write(&tmp, directive) {
        tracing::warn!(target: "log.runtime", error = %e, "could not write runtime_filter.tmp");
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        tracing::warn!(target: "log.runtime", error = %e, "could not rename runtime_filter");
    }
}
```

**Keep identical:**
- `tracing::warn!` for non-fatal I/O on tmp write/rename failure (Phase 4 uses `tracing::info!` for the GC sweep specifically per CONTEXT.md D-14, but `warn!` for materialize-time failure to mirror this fn).
- `std::fs::write` -> `std::fs::rename` ordering (write tmp first, rename last).
- Discrete error handling at each step; no chained `?` that hides which step failed.

**Adapt:**
- Rename a **directory** (`<target>.tmp.<pid>.<nanos>/`) not a file. Per RESEARCH.md Pitfall 1, `ErrorKind::AlreadyExists` is NOT reliable for dir rename across Linux (`ENOTEMPTY`)/macOS/Windows. Use post-rename `target.exists()` as authoritative signal:
  ```rust
  match std::fs::rename(&tmp, &target) {
      Ok(()) => {}
      Err(_) if target.exists() => { let _ = std::fs::remove_dir_all(&tmp); }
      Err(e) => { let _ = std::fs::remove_dir_all(&tmp); return Err(e.into()); }
  }
  ```
- Return `Result<PathBuf>` (anyhow) per the `SbxRuntime::create_container` consumer in Phase 5; logging fn above is `void`.
- Drop the `#[cfg(unix)]` permission block (kit dir contents are non-secret).

**Background-task pattern to MIRROR** (from `src/tui/mod.rs:50`):
```rust
let migration_handle = tokio::task::spawn_blocking(migrations::run_migrations);
```

**Adapt for GC** (per RESEARCH.md Pattern 2):
- Drop the JoinHandle (fire-and-forget); GC must not block session create.
- Move ownership of `gc_root: PathBuf` into the closure.
- Per CONTEXT.md D-14: log via `tracing::info!`, never propagate errors.
- Run once per cache-miss only (NOT on cache hit, NOT at startup; see RESEARCH.md Pitfall 6).

**App-dir resolution pattern to MIRROR** (from `src/session/mod.rs:73`):
```rust
pub fn get_app_dir() -> Result<PathBuf> {
    let dir = get_app_dir_path()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}
```

**Keep identical:** Call `get_app_dir()` (NOT `get_app_dir_path()`); it handles the debug-vs-release path namespace + `create_dir_all` already. KitMaterializer roots `<app_dir>/sbx-kit/` from this.

**Install-hint read-through pattern to MIRROR** (from `src/agents.rs:477-479`):
```rust
pub fn install_hint(name: &str) -> Option<&'static str> {
    get_agent(name).map(|a| a.install_hint)
}
```

**Keep identical:** Use this exact accessor (not a fresh lookup over `AGENTS`) so install.sh body composition has a single point of read; `None` is the v1-fail-loud signal for unsupported agents (CONTEXT.md D-10).

**In-module tests pattern to MIRROR** (from `src/hooks/status_file.rs:48` + Phase 4 RESEARCH.md confirms `tempfile` already in dep tree at `Cargo.toml:104`):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    // ... per-test setup helpers, deterministic instance_ids
}
```

**Adapt:** Use `tempfile::TempDir` (not `/tmp/aoe-hooks/`) for the app-dir substitute in materialization tests, so concurrent test runs don't collide. Pattern: `tempfile::tempdir()?` then pass `.path()` to a private `cache_dir(app_dir, agent)` helper exposed for tests via `pub(crate)`.

---

### `src/containers/sbx_kit/spec.yaml` (config, static asset)

**Analog:** No in-tree precedent. Format reference is the live Docker Sandboxes kit schema cited in RESEARCH.md (`code-server` + `openclaw` kits at `github.com/docker/sbx-kits-contrib`).

**Schema fields to declare** (per CONTEXT.md D-05/D-06 and RESEARCH.md Pattern 4):
```yaml
schemaVersion: "1"
kind: agent
name: aoe-agent
commands:
  install:
    - command: ["sh", "-c", "<idempotent install harness sourcing install.sh>"]
  startup:
    - command: ["sh", "-c", "command -v <agent-binary> >/dev/null || (printf 'agent install incomplete\\n' >&2; exit 1)"]
initFiles:
  - path: /home/agent/.claude/settings.json
    mode: "0644"
    content: |
      { "hooks": { "PreToolUse": [...], "Stop": [...] } }
  # ... per-agent hook-shim settings.json files
credentials:
  sources:
    - type: ssh-agent
environment:
  proxyManaged:
    - ANTHROPIC_API_KEY
    - OPENAI_API_KEY
    - GEMINI_API_KEY
    # ... per-agent provider env vars
```

**Keep identical to live `code-server` precedent:**
- `${WORKDIR}` substitution lives in `initFiles[*].content` ONLY (NEVER in `path:` — see RESEARCH.md Pitfall 5).
- `commands.startup` is an array, NOT a shell string; each entry has `command:` (string or array form).
- `commands.startup` must be idempotent (existence check, `command -v ... ||`, etc.); install verbs (`apt-get`, `npm install -g`, `curl | bash`) belong only in `commands.install` (which runs once as root). `cargo xtask check-kit` enforces this (CONTEXT.md D-16).

**Adapt:**
- Do NOT declare `network.allowedDomains` / `deniedDomains` (CONTEXT.md `<deferred>` + RESEARCH.md Pitfall 4; host policy wins, kit allow-lists give false confidence).
- Do NOT declare gitconfig identity / `~/.claude/` config dirs beyond the hook-shim settings.json (CONTEXT.md D-07; deferred to Phase 5/6).

---

### `src/containers/sbx_kit/install.sh.template` (config, templated body)

**Analog:** No in-tree precedent. Body composed from `src/agents.rs::install_hint`.

**Source field reference** (from `src/agents.rs:103`):
```rust
pub install_hint: &'static str,
```

**Per-agent install_hint values** (from `src/agents.rs:188-434`):
```rust
install_hint: "npm install -g @anthropic-ai/claude-code",   // line 188 — claude
install_hint: "curl -fsSL https://opencode.ai/install | bash", // line 204 — opencode
install_hint: "npm install -g @openai/codex",               // line 241 — codex
install_hint: "npm install -g @google/gemini-cli",          // line 281 — gemini
// ... etc.
```

**Template shape** (KitMaterializer composes at materialize time):
```sh
#!/bin/sh
set -eu
# Body substituted from agents::install_hint(agent_name)
{{INSTALL_HINT_BODY}}
```

**Keep identical:** Use the existing `install_hint` accessor at `src/agents.rs:477` (`install_hint(name) -> Option<&'static str>`); `None` triggers v1-fail-loud (CONTEXT.md D-10).

**Adapt:** Wrap raw `install_hint` with a `sh -eu` harness; do NOT modify the registry. v1 subset (per CONTEXT.md D-10): claude, codex, opencode, gemini, cursor CLI, droid, pi, qwen — those with `npm install -g ...` or `curl | bash` shapes. Agents like Kiro (Docker desktop) and settl (Homebrew Cask) get a fail-loud kit body.

---

### `src/containers/sbx_kit/files/` (config, per-agent hook-shim settings.json)

**Analog:** No in-tree precedent for the file format. Hook-event source is `src/agents.rs::hook_config`.

**Hook-supporting agents** (from `src/agents.rs`):
- claude (line 178): `hook_config: Some(...)` with `CLAUDE_CURSOR_HOOK_EVENTS`
- gemini (line 253)
- cursor (line 293)
- qwen (line 424)

**Shared hook event shape to mirror** (from `src/agents.rs:107-122`):
```rust
const CLAUDE_CURSOR_HOOK_EVENTS: &[HookEvent] = &[
    HookEvent { name: "PreToolUse", matcher: None, status: Some("running") },
    HookEvent { name: "UserPromptSubmit", matcher: None, status: Some("running") },
    HookEvent { name: "Stop", ... },
    // ...
];
```

**`initFiles` content shape** (per RESEARCH.md Pattern 4, illustrative):
```yaml
- path: /home/agent/.claude/settings.json
  mode: "0644"
  content: |
    {
      "hooks": {
        "PreToolUse": [{"hooks": [{"type":"command","command":"sh -c '[ -n \"$AOE_INSTANCE_ID\" ] || exit 0; mkdir -p ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID && printf running > ${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status'"}]}],
        "Stop": [...]
      }
    }
```

**Keep identical:**
- `${WORKDIR}` lives inside `content:` only (RESEARCH.md Pitfall 5).
- `$AOE_INSTANCE_ID` comes from `sbx exec -e KEY=VAL` (Phase 2's `build_exec_args` already emits this per `EnvEntry::Literal`).
- Per-agent settings.json absolute path matches the in-VM agent install location (Claude: `/home/agent/.claude/settings.json`; Cursor: per-agent; etc.).

**Adapt:** Planner picks whether to ship one combined settings.json or per-agent files (CONTEXT.md Claude's Discretion item). One-per-agent is the natural fit for `KitMaterializer::ensure(agent)` which builds a per-agent dir.

---

### `src/hooks/status_file.rs` (modified — ADD sibling free fn)

**Analog:** `src/hooks/status_file.rs:13` (existing `hook_status_dir` in the same module).

**Existing fn to mirror** (lines 12-15):
```rust
/// Return the directory for a given instance's hook status file.
pub fn hook_status_dir(instance_id: &str) -> PathBuf {
    PathBuf::from(HOOK_STATUS_BASE).join(instance_id)
}
```

**New sibling fn to ADD** (per CONTEXT.md D-02):
```rust
/// Return the workspace-rooted directory for a sbx-backed instance's hook status file.
/// Used when the resolved runtime has !supports_arbitrary_volume_paths (sbx microVM
/// cannot mount host /tmp/aoe-hooks; agent writes inside the workspace mount instead).
pub fn sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf {
    project_path.join(".aoe-hooks").join(instance_id)
}
```

**Keep identical:**
- Free fn in same module, NOT a method on `Instance` (CONTEXT.md D-02 explicitly rejects method-on-Instance refactor).
- `PathBuf` return type, no `Result` (pure path composition).
- `pub use` re-export in `src/hooks/mod.rs:16` alongside the existing trio.

**Adapt:**
- Signature is `(project_path: &Path, instance_id: &str)` not `(instance_id: &str)`; the project_path becomes the dispatch input rather than the static `HOOK_STATUS_BASE` constant.
- Existing `hook_status_dir`, `read_hook_status`, `cleanup_hook_status_dir` STAY UNCHANGED (CONTEXT.md D-02 NO rename).

**Test pattern to mirror** (from `src/hooks/status_file.rs:48-138`):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn setup_status_file(instance_id: &str, content: &str) -> PathBuf {
        let dir = hook_status_dir(instance_id);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("status");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        dir
    }

    #[test]
    fn test_hook_status_dir_path() {
        let dir = hook_status_dir("abc123");
        assert_eq!(dir, PathBuf::from("/tmp/aoe-hooks/abc123"));
    }
}
```

**Keep identical:**
- `#[cfg(test)] mod tests` block in same file.
- Deterministic instance_ids (`"abc123"`, `"test_read_running"`, etc. — no PIDs, no timestamps).
- Per-test cleanup (`fs::remove_dir_all(dir).ok();`).

**Add tests for the new fn:**
- `test_sbx_hook_status_dir_path` — assert exact PathBuf composition for a tempdir project_path.
- Use `tempfile::TempDir` for the project_path substitute so concurrent test runs don't collide on a static `/tmp/aoe-hooks/` (the existing tests get away with static paths because each uses a unique instance_id; new tests can be similarly disciplined, but tempfile is cleaner).

---

### `src/session/poller.rs` (modified — single new dispatch site)

**Analog:** `src/containers/runtime.rs:108-119` (existing `fn capabilities()` dispatch in `ContainerRuntime`); call-site precedent at `src/tui/app.rs:1055`.

**Capability access pattern to mirror** (from `src/containers/runtime.rs:491`):
```rust
let caps = rt.capabilities();
// ... then read flags off caps
```

**Existing call site to refactor** (illustrative — exact line is planner discretion; from `src/tui/app.rs:1055`):
```rust
} else if crate::hooks::read_hook_status(&instance.id).is_some() {
    // Hook status is tracking this session; shell detection is unreliable
    false
}
```

**New dispatch shape to MIRROR** (per CONTEXT.md D-03 and RESEARCH.md Example 5):
```rust
let caps = crate::containers::get_container_runtime().capabilities();
let status_dir = if caps.supports_arbitrary_volume_paths {
    crate::hooks::hook_status_dir(&instance.id)
} else {
    crate::hooks::sbx_hook_status_dir(
        std::path::Path::new(&instance.project_path),
        &instance.id,
    )
};
let status_file = status_dir.join("status");
let status = std::fs::read_to_string(&status_file).ok();
```

**Keep identical:**
- Cap read via `runtime.capabilities()` (matches existing `src/containers/runtime.rs:491,507,522,567` test sites).
- Single dispatch site only (CONTEXT.md D-03); do NOT scatter `if caps.supports_arbitrary_volume_paths` across multiple call sites.

**Adapt:**
- Planner picks whether to inline the dispatch at the existing `read_hook_status` call site or introduce a thin `read_hook_status_for(instance: &Instance) -> Option<Status>` wrapper in `src/hooks/status_file.rs` that closes over the dispatch. CONTEXT.md D-03 leaves this open; minimum-churn favors the wrapper so the existing 3 call sites (tui/app.rs, server/api/sessions.rs, cli/session.rs) get fixed by one diff.
- `cleanup_hook_status_dir` follows the same dispatch shape when runtime is sbx (per CONTEXT.md D-03).

---

### `xtask/src/main.rs` (modified — ADD CheckKit variant + check_kit fn)

**Analog:** `xtask/src/main.rs:21,28,59` (existing `CheckSkill` variant + `fn check_skill()`).

**Enum variant to mirror** (lines 16-22):
```rust
#[derive(Subcommand)]
enum Commands {
    /// Generate CLI documentation from clap definitions
    GenDocs,
    /// Check that contrib skill files reference valid CLI commands
    CheckSkill,
}
```

**Dispatch pattern to mirror** (lines 24-30):
```rust
fn main() {
    let args = Xtask::parse();
    match args.command {
        Commands::GenDocs => generate_cli_docs(),
        Commands::CheckSkill => check_skill(),
    }
}
```

**Function structure to mirror** (lines 59-175):
```rust
fn check_skill() {
    let skill_path = Path::new("contrib/openclaw-skill/SKILL.md");
    if !skill_path.exists() {
        eprintln!("Skill file not found: {}", skill_path.display());
        std::process::exit(1);
    }
    let content = fs::read_to_string(skill_path).expect("Failed to read SKILL.md");
    let mut has_error = false;
    // ... assertions; eprintln! + has_error = true; on each violation
    if has_error {
        std::process::exit(1);
    }
    println!("Skill check passed.");
}
```

**Keep identical:**
- Triple doc-comment on each `Commands` variant (clap surfaces it as `--help`).
- `match args.command` arm pattern.
- `eprintln!` for each violation, accumulate `has_error: bool`, single `process::exit(1)` at the end (NOT per-assertion exit).
- Trailing `println!("check-kit: OK")` on success (mirror `Skill check passed.`).

**Adapt:**
- `check_kit` parses `include_str!("../../src/containers/sbx_kit/spec.yaml")` via `serde_yaml::from_str::<serde_yaml::Value>` (NOT `fs::read_to_string`) per CONTEXT.md D-17: the embedded source is the artifact under test.
- Per CONTEXT.md D-16: assert `schemaVersion == "1"`, assert required fields (`kind`, `name`, `commands.install`, `commands.startup`), lint `commands.startup[*].command` for naked install verbs without an idempotency guard.
- Per RESEARCH.md key-implementation-specific #2: `commands.startup` is an array of `{command: ...}` entries; `command:` may be a string OR an array. When `command: ["sh", "-c", "..."]`, extract `command[2]` and substring-scan; when `command: "string"`, scan directly. See RESEARCH.md "Example 6" for verb list + guard prefixes.

---

### `.github/workflows/ci.yml` (modified — ADD check-kit job/step)

**Analog:** `.github/workflows/ci.yml:82-93` (existing `skill:` job that runs `cargo xtask check-skill`).

**Job structure to mirror** (lines 82-93):
```yaml
  skill:
    name: Skill Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4 # v2.9.1
        with:
          cache-bin: "false"
          save-if: ${{ github.ref == 'refs/heads/main' }}
      - name: Check skill file references valid CLI commands
        run: cargo xtask check-skill
```

**Keep identical:**
- Pinned action SHAs (NEVER use floating tags like `@v6`).
- `Swatinem/rust-cache` block with `cache-bin: "false"` + `save-if: ${{ github.ref == 'refs/heads/main' }}`.
- Single `run: cargo xtask <name>` step.

**Adapt:**
- Planner picks: add as a peer top-level job (`kit:` alongside `skill:`) OR add as a step inside the existing `skill:` job. Peer job parallelizes cleanly; same-job step shares cache. Either is defensible; mirror what the rest of `ci.yml` does for similar single-line guards.
- Step name: `Check sbx kit spec is well-formed and idempotent`.

---

## Shared Patterns

### Atomic file/directory write via tmp + rename
**Source:** `src/logging.rs:447-468`
**Apply to:** `src/containers/sbx/kit.rs` (`KitMaterializer::ensure`)
```rust
let tmp = app_dir.join("runtime_filter.tmp");
if let Err(e) = std::fs::write(&tmp, directive) { tracing::warn!(...); return; }
if let Err(e) = std::fs::rename(&tmp, &path) { tracing::warn!(...); }
```
**Adaptation:** Directory rename uses a unique tmp suffix (`<target>.tmp.<pid>.<nanos>/`) and post-rename `target.exists()` check, per RESEARCH.md Pitfall 1.

### Fire-and-forget background work via spawn_blocking
**Source:** `src/tui/mod.rs:50` (`tokio::task::spawn_blocking(migrations::run_migrations)`)
**Apply to:** `src/containers/sbx/kit.rs` (30-day stale-kit GC)
```rust
tokio::task::spawn_blocking(move || { /* gc_stale_kit_dirs(...) */ });
```
**Adaptation:** Drop the JoinHandle (don't `.await`); log GC errors at `tracing::info!` per CONTEXT.md D-14; run once per cache-miss only (NOT per startup).

### Per-platform app-dir resolution with debug/release isolation
**Source:** `src/session/mod.rs:73-93` (`get_app_dir()` + `APP_DIR_NAME_*` constants with `cfg!(debug_assertions)`)
**Apply to:** All filesystem roots for new kit cache; `KitMaterializer` MUST root `<app_dir>/sbx-kit/` here, NOT a hardcoded `/tmp/...` or hand-rolled platform branch.

### Capability-flag dispatch via `runtime.capabilities()`
**Source:** `src/containers/runtime.rs:108-119,491,507,522,567` (Phase 1/3 precedent)
**Apply to:** `src/session/poller.rs` (single new dispatch site)
```rust
let caps = runtime.capabilities();
if caps.supports_arbitrary_volume_paths { /* legacy /tmp path */ }
else { /* sbx workspace path */ }
```
**Adaptation:** No NEW 6th flag (CONTEXT.md D-04); reuse existing `supports_arbitrary_volume_paths`.

### Reading agent registry without modifying it
**Source:** `src/agents.rs:477-479` (`pub fn install_hint(name: &str) -> Option<&'static str>`)
**Apply to:** `src/containers/sbx/kit.rs` (composing `install.sh` body)
```rust
let body = crate::agents::install_hint(agent).ok_or_else(|| anyhow!("..."))?;
```
**Adaptation:** Use the accessor, not direct iteration over `AGENTS`. `None` is the fail-loud signal for unsupported agents (CONTEXT.md D-10).

### In-module unit tests with deterministic cleanup
**Source:** `src/hooks/status_file.rs:48-138`
**Apply to:** `src/hooks/status_file.rs` (new `sbx_hook_status_dir` tests) and `src/containers/sbx/kit.rs` (materializer + GC tests)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // deterministic ids, tempfile::TempDir for cache roots, .ok() cleanup
}
```
**Adaptation:** `tempfile::TempDir` for KitMaterializer's app-dir substitute (vs the static `/tmp/aoe-hooks/` the existing tests use); concurrent cargo test runs MUST not collide.

### xtask subcommand for build-time invariant checking
**Source:** `xtask/src/main.rs:21,28,59` (CheckSkill variant + fn)
**Apply to:** `xtask/src/main.rs` (new `CheckKit` variant + `check_kit()` fn)
**Adaptation:** Parse `include_str!` of the static asset (NOT `fs::read_to_string`); use `serde_yaml::Value` indexing (NOT a typed schema struct); accumulate violations into `has_error: bool` and call `process::exit(1)` once at the end.

### CI guard job for an xtask check
**Source:** `.github/workflows/ci.yml:82-93` (`skill:` job)
**Apply to:** `.github/workflows/ci.yml` (new `kit:` job OR new step in same job)
**Adaptation:** Mirror the SHA-pinned action versions, the rust-cache block, and the single `run: cargo xtask check-kit` step.

---

## No Analog Found

Files where no close in-tree match exists; planner uses external references from RESEARCH.md:

| File | Role | Data Flow | Reason | Reference |
|------|------|-----------|--------|-----------|
| `src/containers/sbx_kit/spec.yaml` | static kit asset | sbx kit format | First aoe-authored sbx kit. The repo has no other kit `spec.yaml`. | RESEARCH.md "Pattern 4" + live `code-server` kit (cited via WebFetch of docker.com docs) |
| `src/containers/sbx_kit/install.sh.template` | static asset, templated | shell script body | First aoe-authored kit install script. Body composed from `src/agents.rs::install_hint`. | RESEARCH.md "Pattern 4" + `src/agents.rs:103,477` |
| `src/containers/sbx_kit/files/<agent>-settings.json` | static asset, per-agent | hook settings JSON | First aoe-authored agent settings.json shims. Hook events come from `src/agents.rs::hook_config`. | RESEARCH.md "Pattern 4" + `src/agents.rs:107-122` (hook event shapes) |

**Compensating signal:** The Phase 4 `cargo xtask check-kit` guard (CONTEXT.md D-16) IS the in-tree validator for these new assets; CI runs it on every push.

---

## Metadata

**Analog search scope:**
- `src/logging.rs` (atomic-rename idiom)
- `src/hooks/status_file.rs` (sibling free-fn + in-module tests)
- `src/session/{mod.rs,poller.rs}` (app dir + poller)
- `src/containers/{runtime.rs,runtime_base.rs}` (capability dispatch)
- `src/agents.rs` (registry accessors, install_hint, hook_config)
- `src/tui/{mod.rs,app.rs}` (spawn_blocking, read_hook_status call site)
- `src/server/api/system.rs` (spawn_blocking precedents — 5 occurrences)
- `xtask/src/main.rs` (subcommand pattern, full file)
- `.github/workflows/ci.yml` (CI job step pattern, full file)
- `contrib/` (verified: only `openclaw-skill/`, no other kit assets)

**Files scanned:** 11 source files + 1 CI workflow + 1 contrib subdirectory listing
**Pattern extraction date:** 2026-05-16
