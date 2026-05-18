# Pitfalls Research — sbx Container Runtime Backend

**Domain:** Brownfield integration of `sbx` (Docker Sandboxes microVM) as a 4th `ContainerRuntime` peer in `aoe`. Pitfalls cover sbx CLI integration, microVM-vs-container semantic mismatches, kit authoring, capability-flag trait extension, and multi-platform sbx differences.

**Researched:** 2026-05-15
**Confidence:** MEDIUM-HIGH (Docker Sandboxes is experimental; kit format is explicitly subject to change. CLI surface, mount semantics, platform support, and policy/login behavior are documented; kit cache invalidation and JSON schema stability are not.)

**Phase tag legend** (matches roadmap conventions):
- `foundation` — capability flags, trait change, ContainerRuntimeName wiring
- `sbx-runtime` — `SbxRuntime` impl (argv builders, lifecycle, batch state)
- `kit` — embedded kit (spec.yaml, files/, cache materialization)
- `port-publish` — out-of-band `sbx ports --publish` orchestration
- `settings-ui` — TUI/profile/override wiring
- `integration` — cockpit, web, cross-machine, ACP transparency
- `tests-docs` — unit + integration tests, user docs

---

## Critical Pitfalls (rewrite/data-loss class)

### Pitfall C1: Hook status file is invisible across the microVM boundary

**What goes wrong:**
File-based agent status detection is dead on arrival for sbx-backed sessions. `src/hooks/status_file.rs::read_hook_status` reads `/tmp/aoe-hooks/<id>/status` from the **host** filesystem. With Docker today, `src/session/container_config.rs:993-1006` bind-mounts the host hook dir at the same path inside the container, so writes by the in-container hook show up on the host. With sbx, even though workspaces are mounted at the same absolute path, `/tmp` inside the microVM is the **VM's** `/tmp`, not the host's. The hook fires inside the VM, writes to the VM's tmpfs, and aoe's status poll on the host sees a never-changing or missing file. Every hook-based agent (Claude Code, Cursor, Gemini, Qwen, Hermes, Kiro) silently degrades to `Idle` forever, breaking approval prompts, "running" badges, and anything that depends on `Status::Waiting`.

**Why it happens:**
The current bind-mount-the-host-tmp-dir trick assumes shared kernel namespaces. sbx is a microVM with its own kernel and its own filesystem. The kit `files/workspace/` and `files/home/` blocks place files at sandbox start, but they don't establish a shared writable directory between host and VM at runtime.

**How to avoid:**
Treat hook delivery as a runtime-specific concern at the abstraction layer. Two viable fixes:
1. Write the hook status under the workspace path (`<workspace>/.aoe-hooks/<id>/status`). The workspace IS shared at the same path on host and VM, so this round-trips. Migration risk: pollutes user repos unless the path is gitignored by the kit's `files/workspace/` (drop a `.gitignore` for `.aoe-hooks/`).
2. Have the kit's `startup` hook stream status via `sbx exec sandbox cat /tmp/aoe-hooks/<id>/status` polled from the host. Higher overhead but no workspace pollution.
Pick (1). Add a runtime capability `host_visible_tmpfs: bool` (false for sbx, true for Docker/Podman/AppleContainer); the hook installer reads this and rewrites the hook command's destination path. Add a unit test that asserts an sbx-targeted hook command writes under the workspace path, and a Docker-targeted one writes under `/tmp/aoe-hooks`.

**Warning signs:**
Hook-enabled agents in an sbx sandbox always show `Idle` even while the agent is clearly working. `read_hook_status` returns `None` for every poll. `tracing::debug!` for the status detector shows pane fallback path firing for every cycle.

**Phase to address:** `foundation` (capability flag) + `kit` (status writer path) + `tests-docs` (unit test on argv).

---

### Pitfall C2: `host_path != container_path` workspace mounts silently mismount or fail

**What goes wrong:**
sbx requires every workspace to mount at the **same absolute path** inside the sandbox as on the host. aoe's `VolumeMount { host_path, container_path }` allows arbitrary remapping (the existing default mounts under `/workspace/<name>` regardless of host path). If the SbxRuntime's `build_create_args` ignores the divergence and just passes the host path positionally, the agent's `working_dir` (e.g. `/workspace/project`) doesn't exist inside the sandbox; the agent starts in `$HOME` and writes go to the wrong place. If it errors out, the user sees an opaque sbx CLI failure.

**Why it happens:**
The Docker baseline lets you mount `/Users/me/project` at `/workspace/project`; aoe and many users rely on the canonical `/workspace/...` path. sbx removes that freedom but the trait API still accepts both fields.

**How to avoid:**
Add capability `supports_arbitrary_volume_paths: bool` (already in REQ-RT-03) and a capability `requires_same_host_container_path: bool`. In `SbxRuntime`, before invoking `sbx create`, validate every `VolumeMount` against `host_path == container_path`. On mismatch:
- If `host_path` is a parent/equal of `container_path` after canonicalization, normalize and emit `tracing::info!` once.
- Otherwise return `DockerError::CreateFailed` with a message that names the offending mount and tells the user to set `working_dir = <host_path>` in their sandbox config.
Update `resolve_container_config` (in `src/session/container_config.rs`) to set `working_dir = host_workspace_path` when the resolved runtime is sbx, so the existing `/workspace/<name>` convention is auto-rewritten. Add a unit test for each path: pure same-path, normalize-equivalent (`/tmp` vs `/private/tmp` — see line 644 of container_config.rs that already canonicalizes), and reject-divergent.

**Warning signs:**
`sbx exec` fails with `chdir: no such directory` or the agent prompts for the workspace path. `pwd` inside the sandbox shows `/home/agent` instead of the project. Worktree sessions especially affected because their host path is under `~/.agent-of-empires/worktrees/...`.

**Phase to address:** `foundation` (capability flag) + `sbx-runtime` (validator + normalizer) + `tests-docs`.

---

### Pitfall C3: Re-publishing a sbx port that's already taken on the host eats the failure silently

**What goes wrong:**
REQ-RT-05 says: "failures surface clearly without rolling back the sandbox." The naive impl runs `sbx ports SANDBOX --publish 3000:3000` after `sbx create` succeeds, ignores stderr, and reports success because the sandbox itself was created. User sees a sandbox that "ran" but nothing on `localhost:3000`. Worse, if multiple ports are requested and the first fails (port taken on host), the loop may continue or abort, leaving partial state with no clear log of which ports actually got published.

**Why it happens:**
Two-stage create+publish replaces a single Docker `-p` flag. The `sbx ports --publish` exit status and stderr aren't inspected with the same care as `sbx create`. There's no native rollback hook in sbx — so if a port fails, you're stuck deciding: leave the sandbox up with partial ports, tear it down, or retry?

**How to avoid:**
1. After `sbx create`, run each `--publish` individually (not batched), capturing exit code and stderr per port.
2. On failure, do NOT roll back the sandbox (per REQ-RT-05). Instead aggregate failures into a `Vec<(port, error)>`, attach to the runtime result struct, and surface at the call site (creation poller, web API session create response). The TUI session row should show a warning glyph if any port failed.
3. Pre-flight: before the publish loop, walk `port_mappings` and refuse host ports `< 1024` unless explicitly allowed (sbx-side likely refuses anyway, but a clean error beats a process exec failure).
4. Special-case the "port already in use" stderr substring (test what sbx emits — likely `bind: address already in use` or `port is already allocated`) and produce a typed `RuntimeError::PortAlreadyInUse(host_port)` so the UI can suggest "pick a different host port" rather than "unknown failure."
5. Unit test: feed a mock `sbx ports` script that fails on the second port; assert sandbox stays up, error list contains exactly one entry for the second port, no entries for the first.

**Warning signs:**
Sandbox `Running` but `curl localhost:<port>` is connection refused. `sbx ports SANDBOX` (no flags) returns fewer entries than `port_mappings` length. Users complain "the web dashboard says my session is up but the dev server isn't reachable."

**Phase to address:** `port-publish` + `tests-docs`.

---

### Pitfall C4: Race between `sbx create` returning and the sandbox being ready for `sbx exec`

**What goes wrong:**
Per `sbx exec` docs: "If the sandbox is stopped, it is started first." This *implies* the sandbox is started lazily on first exec. But in our lifecycle (REQ-LIFE-01), `start_container` is a no-op and the very first thing aoe does after create is `exec` the agent install / status hooks / agent CLI itself. If aoe issues `sbx exec` immediately after `sbx create` returns, you may hit a transient window where the microVM is still booting (kernel handoff, agent uid setup, network proxy bring-up). Symptoms: first `exec` either hangs for ~5-30s, or fails with "sandbox not ready" / "sandbox not found." If aoe interprets failure as a fatal create error, the session is marked broken even though a 2-second retry would succeed.

**Why it happens:**
microVM cold-start latency is real (the Andrew Lock blog calls performance "crippling" for larger projects). Docker container start is 100ms-class; microVM cold-start is seconds-class. Existing aoe flows assume container-class startup.

**How to avoid:**
1. Add a readiness probe in `SbxRuntime::create_container` that polls `sbx ls --json` for `name == sandbox && status in ("running", "created")` with exponential backoff, max 30s. Only return success after the probe passes.
2. For the first `exec` after create, wrap in retry-with-backoff (3 attempts, 1s/2s/4s) on the specific stderr substrings sbx returns for "not ready." Match on substring; do NOT retry generic exec failures (commands that exited 1 should not be retried).
3. `tracing::info!(target: "containers.sbx", duration_ms)` for both probe and first exec so cold-start latency is visible in logs and we can tune the timeout.
4. Add an integration test (gated `#[ignore]` like existing Docker tests) that creates 5 sandboxes back-to-back and verifies all pass the readiness probe within 30s.

**Warning signs:**
`tracing` shows `sbx exec` failing with "container not running" or similar within 1s of `sbx create` returning. New session creation succeeds 90% of the time but mysteriously fails on cold caches or under CPU load. Users on macOS arm64 report flakiness right after laptop wake (related to clock-drift, see C12).

**Phase to address:** `sbx-runtime` (readiness probe + retry) + `tests-docs`.

---

### Pitfall C5: Kit cache reuses a stale kit after `aoe` upgrade

**What goes wrong:**
REQ-KIT-01/04 says we materialize the embedded kit to a cache dir on first use and "invalidate when embedded version differs." The naive impl uses a fixed dir like `~/.agent-of-empires/kits/aoe/` and a version sentinel file `.aoe-version`. Two failure modes:
1. Bug in version comparison (e.g. comparing strings lexicographically: `"1.10.0" < "1.9.0"`) leaves a stale kit cached after upgrade. Users hit fixed bugs that "didn't get fixed" because their old kit still ran.
2. Sbx itself caches resolved kits (the GitHub repo guidance recommends pinning by tag for reproducibility, implying sbx fingerprints kit refs). If our cache dir keeps the same path but its content changes, sbx may not detect the change at all. This is undocumented, so we can't rely on sbx to do the right thing.

**Why it happens:**
"Invalidate on version change" is one of those things that's trivial to get subtly wrong, and the kit format is explicitly experimental, so there's no guarantee sbx's resolver behavior will stay stable.

**How to avoid:**
1. Materialize the kit to `~/.agent-of-empires/kits/aoe-<aoe_version>-<spec_hash>/` where `aoe_version` is `env!("CARGO_PKG_VERSION")` and `spec_hash` is a SHA256 over the embedded kit bytes captured via `include_bytes!`. The path itself is the cache key — rename = full invalidation, no stale-version logic to get wrong.
2. Garbage-collect old `aoe-*` kit dirs older than 30 days at startup (background, log-only, never block).
3. Pass `--kit <materialized-path>` to sbx using the new path so sbx doesn't need to detect content changes.
4. Unit test: build kit hash for a known fixture, assert path is deterministic; mutate one byte, assert path changes.
5. Integration test (`#[ignore]`): bump in-memory `aoe_version`, materialize, verify a fresh dir was written and the old one still exists (don't delete in unit tests — that's the GC's job).

**Warning signs:**
Bug-fix release ships, users report "still broken" with logs showing the old kit's hook script content. Two `aoe` versions on the same machine fight over the same kit path.

**Phase to address:** `kit` + `tests-docs`.

---

### Pitfall C6: Default-impl trap on capability flags hides bugs in existing backends

**What goes wrong:**
REQ-RT-03 adds `supports_port_publish_at_create`, `supports_image_pull`, `supports_anonymous_volumes`, `supports_arbitrary_volume_paths`, `supports_dynamic_port_publish` to the trait. If you give them default implementations (e.g. `fn supports_image_pull(&self) -> bool { true }`), three things break:
1. Adding the 5th runtime later (or a custom impl in tests) silently inherits Docker-shaped assumptions and breaks at runtime, not compile time.
2. The capability-aware code (`if runtime.supports_image_pull() { runtime.pull_image(image) }`) calls `pull_image` on `SbxRuntime`, which the impl forgot to stub correctly because the default said "true."
3. Existing tests that exercise capability gating pass because the defaults match Docker, but the gating is effectively dead code.

**Why it happens:**
Default impls feel ergonomic ("I don't have to update every backend"), but they erase compile-time safety which is the whole point of the trait change.

**How to avoid:**
Capability flags live as **plain const fields on `RuntimeBase`** (mirroring the existing `supports_read_only_volumes` / `supports_remove_volumes` precedent), not as trait methods with default impls. Add a new `RuntimeBase::SBX = Self { ..., supports_image_pull: false, supports_arbitrary_volume_paths: false, supports_dynamic_port_publish: true, ... }`. The compiler then *forces* every const literal to be exhaustive — adding a new field requires updating DOCKER, PODMAN, APPLE_CONTAINER, and SBX in one PR or it doesn't compile. For runtime-only checks (e.g. queries from `tui/settings/fields.rs`), expose `runtime.capabilities() -> &RuntimeCapabilities` returning the const struct.

For each capability, write a unit test asserting the four backends report the expected values, so a typo at definition time fails CI.

**Warning signs:**
A capability getter has a default implementation in the trait. A new backend compiles without touching every existing capability site. A capability check is `if runtime.is_sbx()` rather than `if !runtime.capabilities().supports_image_pull`.

**Phase to address:** `foundation` + `tests-docs`.

---

### Pitfall C7: UI shows fields that the runtime doesn't honor (capability lies)

**What goes wrong:**
The settings TUI lets the user edit `port_mappings` (already supported per `src/tui/settings/fields.rs`). For sbx, the data layer accepts these and they DO take effect via `sbx ports --publish`. But the user can also edit `cpu_limit` (`--cpus`), `memory_limit` (`-m`), and `mount_ssh` — and aoe must verify each is honored. Worse: if a future capability says "sbx doesn't support read-only volumes," but the settings UI still shows a "Read-only" toggle for additional mounts, the user toggles it, the field gets saved, and the runtime silently mounts read-write. Silent disagreement between UI affordances and runtime behavior is a worse bug than an outright error.

**Why it happens:**
Settings TUI (per AGENTS.md "every field must be editable") and runtime backends evolve independently. Capability flags get added to the trait but the settings panel forgets to gate visibility/editability on them.

**How to avoid:**
1. Each `SettingField` in `src/tui/settings/fields.rs` gains an optional `requires_capability: Option<fn(&RuntimeCapabilities) -> bool>`. When the resolved runtime's capabilities don't satisfy, the field is rendered greyed out with a footer note ("Not supported by current runtime: <runtime name>") and key input is no-op.
2. Add a unit test that walks every `FieldKey`, computes the cross-product against every `ContainerRuntimeName`, and asserts that the field's editability matches the runtime's capability declaration.
3. For sbx specifically: validate that `supports_read_only_volumes`, `supports_arbitrary_volume_paths`, and `supports_dynamic_port_publish` flow into the right field gates on day one, not as a follow-up.

**Warning signs:**
User reports "I set CPU limit to 2 but `sbx ls --json` shows different." A capability flag exists but no settings field references it. The settings panel renders identically regardless of selected runtime.

**Phase to address:** `settings-ui` + `tests-docs`.

---

### Pitfall C8: `sbx login` is mandatory, interactive, and not validated up front

**What goes wrong:**
Per `docs.docker.com/ai/sandboxes/get-started/`: `sbx login` is mandatory (browser OAuth) and the **first run also prompts for a default network policy** (open / balanced / locked-down). aoe spawns `sbx create ...` and the underlying daemon either:
1. Refuses, returning a "not logged in" error that aoe surfaces as an opaque "container creation failed."
2. Tries to launch an OAuth browser flow from inside a TUI subprocess context where there's no controlling terminal for the prompt, hanging forever.
3. (First-run on a fresh machine) Stalls on the policy picker, blocking session creation indefinitely with no UI feedback.

**Why it happens:**
aoe's existing runtimes (Docker, Podman, AppleContainer) don't have a "log in to a vendor cloud" prerequisite. `is_daemon_running` returning true for sbx doesn't mean usable.

**How to avoid:**
1. Implement `SbxRuntime::is_daemon_running` as a composite check: (a) `sbx version` succeeds AND (b) `sbx ls --json` returns a parseable JSON array (even if empty) without prompting. If `sbx ls` ever exits non-zero with stderr matching "not logged in" or "policy not configured," report `DockerError::DaemonNotRunning` with a typed sub-reason `SbxNotConfigured(reason)`.
2. The TUI runtime selector and the new-session pre-flight should call this and surface a typed banner: "sbx requires `sbx login` and `sbx policy set-default balanced` on the host. Run them, then retry." Link to docs.
3. Never spawn `sbx login` from inside aoe — it owns user identity. Document this in REQ-DOCS-01.
4. Add an `aoe doctor` (or extend `cargo xtask check-skill` style health command) that runs the composite probe and prints actionable next steps. Aligns with `sbx diagnose`.

**Warning signs:**
First-run users on a fresh machine see "container creation failed" with no actionable error. Logs show `sbx ls` exiting non-zero with stderr mentioning auth or policy. Sessions hang at "creating sandbox" with no progress.

**Phase to address:** `sbx-runtime` (composite daemon probe) + `tests-docs` (docs for prereqs).

---

### Pitfall C9: Startup hooks accidentally do install work and break on every restart

**What goes wrong:**
Per kit docs: `install` runs once at sandbox creation as root; `startup` runs on every sandbox start as the agent user (uid 1000). Authors slip and put `apt-get install ...` or `npm install -g claude-code` into `startup` "because it works the first time." On the second start, `apt-get` either re-runs (wasteful + slow + may fail offline) or, if it errors out (locked dpkg, missing network), the sandbox launches without the agent CLI present and `sbx exec` finds nothing.

**Why it happens:**
The TWO hook phases look interchangeable in YAML. The constraint that "Startup commands must be idempotent ... they replay on every sandbox start" is documented but easy to overlook. The hooks fire silently — install/startup output is **not retained** by sbx after `sbx create` exits, so debugging is by inference from cold-start failures.

**How to avoid:**
1. Lint our own kit: a unit test parses our embedded `spec.yaml` and asserts that `commands.install[*].command` contains any of `{apt, apt-get, dpkg, pip, npm, curl, wget}` and `commands.startup[*].command` contains NONE of them (or only inside an idempotency guard like `command -v X >/dev/null || ...`).
2. Every startup line MUST start with an existence check (`[ -f /marker ] || ...` or `command -v X || ...`). Document this in `kit/README.md` and the lint enforces it.
3. After `sbx create`, capture `sbx exec sandbox <agent_binary> --version` exit code and assert the install hook completed successfully. Failure is fatal for create — don't return success and let the user discover it later.
4. Add a "kit smoke test" in CI (gated `#[ignore]`) that creates and stops/restarts a sandbox 3x and asserts startup time stays bounded after the first.

**Warning signs:**
A sandbox that "worked yesterday" fails today after a stop/start cycle. `sbx exec sandbox claude --version` returns "command not found" while the same sandbox just started fine. Cold-start time on restart is similar to first creation (should be much faster).

**Phase to address:** `kit` (lint test) + `tests-docs` (startup smoke test).

---

### Pitfall C10: Network policy conflict between kit `allowedDomains` and host-global `sbx policy`

**What goes wrong:**
Our kit might declare `network.allowedDomains: ["api.anthropic.com", "registry.npmjs.org", ...]` to make Claude Code work out of the box. The user has globally set `sbx policy set-default deny-all` and added a strict allowlist. Two layers interact:
- Kit-declared allowedDomains apply per-sandbox.
- Host policy applies globally (deny rules take precedence over allow rules).

If the global `deny-all` blocks anthropic.com and the kit "allows" it, the request is **blocked** — deny wins. The agent CLI silently fails to reach its model provider, the session shows running but nothing happens. Users blame aoe, not their host policy.

Conversely, if the kit forgets to declare a critical domain, on a fresh machine where the user accepted `balanced` defaults that domain may already be allowed (no problem); but on `deny-all` the agent fails. We can't predict which.

**Why it happens:**
Two layers of policy is a known footgun. Kit authors think their `allowedDomains` is sufficient; users think their global policy is sufficient. Each is half right.

**How to avoid:**
1. Embed a list of domains REQUIRED by every supported agent (Claude → api.anthropic.com, Cursor → cursor.sh + api.openai.com + api.anthropic.com, Gemini → generativelanguage.googleapis.com, etc.) in the kit `network.allowedDomains`. Be generous; deny still wins host-side.
2. Expose `sbx policy log <sandbox>` output in the cockpit/web view as a "blocked requests" pane so users see immediately when a request is being killed by their host policy.
3. Document REQ-DOCS-01: "If your global policy is `deny-all`, you must `sbx policy allow network -g api.<provider>` for each agent provider you intend to use. The aoe kit can request domains, but your host policy is authoritative."
4. At session create, if the user has `deny-all` set globally and we can detect it via `sbx policy ls --type network`, show a one-time TUI warning before creating the first sbx session. Don't block — they may want it.
5. Test: a unit test parsing our kit asserts that for each agent we ship support for, the canonical provider domain is in `allowedDomains`.

**Warning signs:**
Agent UI hangs on "Connecting..." or returns an opaque "request failed." `sbx policy log <sandbox>` shows the provider domain as denied. User has `deny-all` set globally.

**Phase to address:** `kit` (declare domains) + `integration` (cockpit warning + log surface) + `tests-docs`.

---

### Pitfall C11: `proxyManaged` credentials hide auth state from aoe

**What goes wrong:**
The kit can list `environment.proxyManaged: [ANTHROPIC_API_KEY]`. When the agent reads this env var at runtime, the sbx proxy injects a sentinel (`proxy-managed`) and rewrites outbound auth headers using a host-side stored secret (`sbx secret set -g anthropic`). aoe never sees the actual credential. Two failure modes:
1. The user has not run `sbx secret set -g anthropic`. The agent sees a bogus `proxy-managed` value, the proxy has no real secret to inject, the model API rejects the request, the agent shows an "auth failed" error inside the sandbox. aoe's UI shows a healthy session but the actual interaction is broken. aoe can't pre-flight detect this because credential ownership lives outside its view.
2. The user rotates a credential on the host. aoe doesn't restart sandboxes; the proxy picks up the new value at next request anyway, but if the timing collides with an in-flight request, it fails opaquely.

**Why it happens:**
Per the Out of Scope list, "Managing sbx host-side global state (secrets, policies)" is intentionally NOT aoe's responsibility. Good. But the boundary needs to be visible to users.

**How to avoid:**
1. At session create with sbx + a credential-using agent, run `sbx secret ls --service <service> --json` (host scope) and warn (do not block) if the required service isn't present. The kit declares which services its agents need (mirror the `proxyManaged` list).
2. Add to REQ-DOCS-01: a table mapping aoe agent → required `sbx secret set -g <service>` for sbx backend.
3. When `sbx exec` returns non-zero with stderr matching credential-error patterns, surface a typed `RuntimeError::CredentialNotConfigured(service)` rather than generic "command failed."
4. Test: parse our kit, assert every agent that aoe supports and that `proxyManaged` references a service is documented in our docs file.

**Warning signs:**
Agent runs but every model call fails with auth errors visible in the pane. `sbx secret ls -g` shows the required service is absent. Users complain "Claude doesn't work in sbx" but works in Docker (which uses their host `~/.claude/` credentials directly).

**Phase to address:** `sbx-runtime` (pre-flight check) + `tests-docs` (docs).

---

### Pitfall C12: Clock drift after laptop sleep breaks long-running sandboxes

**What goes wrong:**
Per Docker Sandboxes troubleshooting: "VM clock desynchronization after laptop sleep causes timestamp validation failures. Fix: `sbx stop` and `sbx run` to resync." aoe sessions are designed to outlive laptop sleep cycles — users close their lid, reopen hours later, and expect the session to resume. After sleep, the sbx microVM clock is hours behind wall time, and any HTTPS handshake (`requesting a model`, `cloning a repo`) fails with cert validation or token-expired errors. The agent fails silently or with cryptic errors; aoe shows "running."

**Why it happens:**
microVMs don't share the host's CLOCK_REALTIME refresh. Container runtimes do (they share the kernel).

**How to avoid:**
1. On the host, listen for sleep/wake events (macOS: `IORegisterForSystemPower`; Linux: `org.freedesktop.login1.Manager` `PrepareForSleep` signal). aoe's existing `tmux/status_detection.rs` poll loop is a good place to mark sessions as "potentially clock-drifted." On wake, automatically `sbx stop && sbx run` (or equivalent) for each sbx-backed session that's been running >5min.
2. Alternatively, schedule `sbx exec sandbox sudo hwclock -s` (if available) or `sbx exec sandbox bash -c "sudo date -s @$(date +%s)"` post-wake. Less invasive than a stop/start, but requires sudo inside the sandbox. The `agent` user has passwordless sudo per kit base-image requirements, so this works.
3. Document the issue and the auto-recovery behavior in REQ-DOCS-01.
4. Test: hard to test in CI; gate behind manual checklist for the milestone.

**Warning signs:**
After waking laptop, sbx-backed sessions report random TLS / certificate errors that resolve after restart. Time inside `sbx exec sandbox date` is hours behind. Issue does not affect Docker-backed sessions.

**Phase to address:** `integration` (sleep/wake handler) + `tests-docs` (docs + manual test plan).

---

## Moderate Pitfalls (UX and reliability)

### Pitfall M1: `sbx ls --json` and `sbx ports --json` schema can drift (kit format is experimental)

**What goes wrong:**
We rely on `sbx ls --json` for `is_container_running`, `does_container_exist`, and `batch_running_states`. The kit docs explicitly say "The kit file format, CLI commands, and experience for creating, loading, and managing kits are subject to change as the feature evolves." It would be surprising if the JSON schemas were declared stable when the kit format isn't. A field rename (`status` → `state`, `published_ports` → `ports`) breaks aoe across sbx version bumps, and the failure mode is "every sbx session reports as not running."

**Why it happens:**
Treating an experimental CLI's JSON output as a stable contract is exactly the kind of dependency that bites later.

**How to avoid:**
1. Define an `SbxLsRow` struct with `#[serde(rename_all = "camelCase")]` (or whatever sbx uses) and `#[serde(default)]` on every field. Use `serde_json::Value` as a fallback for fields we don't care about so unknown additions don't break parsing.
2. Run `sbx version` at startup (memoized for the process), parse the version, and emit a `tracing::warn!` once if the version is newer than our last-tested-against version. Update the constant on every sbx version bump.
3. Snapshot test: commit a sample JSON output from a known sbx version into `tests/fixtures/sbx_ls.json` and unit-test `parse_sbx_ls` against it. When we test against a new sbx version, capture and commit a new fixture.
4. On JSON parse failure, fall back to text parsing of the same `sbx ls` output (without `--json`) so a single bad field doesn't take down the runtime entirely.

**Warning signs:**
After updating sbx, every sbx-backed session in the TUI shows as "not running." Logs show `serde_json` errors mentioning a missing or unexpected field. `cargo test --test sbx_parsing` fails on a new fixture.

**Phase to address:** `sbx-runtime` (parser + fixture test).

---

### Pitfall M2: `sbx ports` lifecycle on stop/restart leaks or loses publish state

**What goes wrong:**
`sbx ports --publish` was not designed for orchestration. Open questions the docs don't answer:
- Does `sbx stop` remove published ports, or restore them on next `sbx run`?
- If sbx is restarted (daemon restart, machine reboot) does the publish state survive?
- If aoe's `serve.pid` is killed and the daemon restarts mid-session, do we re-publish on reattach?

Without a concrete answer, aoe might end up publishing the same port twice (some sbx versions accept, some error), or never re-publishing after a stop/start cycle (user sees "session is running but my dev server doesn't work after I restarted aoe").

**How to avoid:**
1. After `sbx create`, call `sbx ports SANDBOX --publish ...` once. Then on every aoe attach/reattach to an existing sbx session, call `sbx ports SANDBOX` (no flags) to enumerate the current published set; diff against what aoe expects from `port_mappings`; publish missing entries; never unpublish (user may have manually published extras).
2. Test the diff logic with a unit test against a mock sbx ports JSON.
3. Document in code comments: "We do not assume sbx persists port publishes across daemon restarts; we re-reconcile on attach."
4. Manual test: stop & restart `aoe serve` while an sbx session is running, verify ports come back without user action.

**Warning signs:**
After `aoe serve` restart, web dashboard shows session running but port mappings broken. `sbx ports SANDBOX` returns fewer entries than expected. Re-publishing the same port returns an error rather than a no-op.

**Phase to address:** `port-publish` + `integration` (attach reconciliation).

---

### Pitfall M3: Per-sandbox image cache balloons disk usage

**What goes wrong:**
Per architecture docs: "Each sandbox maintains its own Docker daemon state, image cache, and package installations. Multiple sandboxes don't share images or layers." With Docker, 5 aoe sessions share one image. With sbx, 5 sessions = 5 full copies of the base image + 5 separate kit installs. A 2GB base image becomes 10GB across sessions. Users on small SSDs hit "no space left on device" mid-session.

**How to avoid:**
1. Add disk-usage telemetry: at session create, check available host disk space (existing utility in `src/server/` or `nix::sys::statvfs`). Warn if estimated total (current sessions × image size) would exceed 70% of free space. Don't block.
2. Document the multiplier in REQ-DOCS-01 ("Each sbx sandbox is a full microVM with its own image cache; expect ~Image Size × Active Sessions of host disk usage").
3. Surface `sbx ls` total disk usage if the JSON includes it; if not, skip.
4. Make sure session deletion calls `sbx rm` (REQ-LIFE-01 already does this) so disk usage releases.

**Warning signs:**
Users hit ENOSPC errors mid-session. `df -h` shows a giant `/var/lib/docker` or sbx state dir. Sessions fail to create with "no space" stderr.

**Phase to address:** `tests-docs` (docs) + `integration` (optional: warn on low disk).

---

### Pitfall M4: PTY/TTY flag drift between `docker exec` and `sbx exec`

**What goes wrong:**
The docs say `sbx exec` "mirrors `docker exec` semantics" with `-i`, `-t`, `-e`, `-w`. "Mirrors" is not "is identical to." Specific risks:
- TTY allocation behavior when stdin is not a TTY (when aoe spawns from a tmux pane vs a non-tty server context).
- Signal forwarding (Ctrl+C from the user → SIGINT to the agent CLI) — sbx may swallow or transform.
- Stream buffering: `sbx exec` may line-buffer where `docker exec` was unbuffered, causing delayed output that breaks status detection in pane-content fallbacks.

**How to avoid:**
1. Add an integration test (`#[ignore]`) that does `sbx exec -it SANDBOX bash -c 'printf "x"; sleep 5; printf "y"'` and asserts byte-by-byte arrival timing matches `docker exec` ±200ms.
2. For `tmux` panes that wrap `sbx exec`, explicitly set `script -qfc` or `unbuffer` if buffering issues show up.
3. Do NOT detach `sbx exec` (-d flag) for the primary agent process — this changes signal handling. Use the foreground form so SIGINT propagates.
4. Test signal forwarding: send SIGINT to a long-running `sbx exec` and verify the inner command receives it within 1s.

**Warning signs:**
Status detection works in Docker mode but is slower-to-fire in sbx mode. User presses Ctrl+C and the agent doesn't stop. Pane content lags real-time activity by 1-3s.

**Phase to address:** `sbx-runtime` (TTY flag selection) + `tests-docs`.

---

### Pitfall M5: `aoe` cleanup hooks (`cleanup_hook_status_dir`) leave VM-side cruft

**What goes wrong:**
`src/session/instance.rs:1605` calls `cleanup_hook_status_dir` on session stop. With Docker the dir was on the host and gets removed. With sbx, even if we relocate to under-workspace per C1, the cleanup writes/deletes a path on the host workspace — but the agent inside the VM may still have a write open to the mirrored path. Side effects: orphan files inside the VM that the next sandbox creation under the same workspace inherits.

**How to avoid:**
1. After choosing the new hook-status path (under workspace per C1), make cleanup target only files that match the instance ID; never rm-rf a directory.
2. On session delete, before `sbx rm`, run `sbx exec SANDBOX rm -rf /tmp/aoe-hooks/<id>` (best-effort, don't fail) so the VM-side state is also wiped.
3. Test: create + delete + re-create an aoe session in the same workspace, assert the new instance's hook status is fresh.

**Warning signs:**
A new sbx session reports a hook status from a previous session. Files named `.aoe-hooks/<old-id>` clutter the workspace.

**Phase to address:** `kit` + `sbx-runtime` (VM-side cleanup).

---

### Pitfall M6: Workspace path with spaces, symlinks, or git worktrees breaks `sbx create`

**What goes wrong:**
sbx mounts the workspace at the same absolute path. If the host path contains spaces, the positional argument needs proper quoting (Rust's `Command::arg` handles this, but composed shell strings like our `exec_command` at line 317 of runtime_base.rs do not). If the path is a symlink, sbx may follow it (mounting the target's path inside the sandbox, which differs from what aoe records). For git worktrees (a common aoe pattern, see `--branch` flag in sbx and `src/git/`), the worktree directory is not a git repo on its own — its `.git` is a file pointing to the parent. If sbx creates its own worktree on top of an existing aoe worktree, the inner agent finds neither `.git` directory nor the parent repo (Andrew Lock blog: "creating a worktree yourself, and then running a sandbox directly in this folder prevents agents from accessing the parent repository, breaking commit functionality").

**How to avoid:**
1. Always use `Command::arg` / `Command::args` (never compose shell strings) when invoking sbx with paths.
2. In `SbxRuntime`, canonicalize host paths with `std::fs::canonicalize` before passing to `sbx create` AND store the canonical path on the instance, so subsequent `sbx exec -w` calls match.
3. If the workspace IS already an aoe-created git worktree, do NOT pass `--branch` to sbx (which would create a nested worktree). Detect via reading `.git` (file vs dir).
4. Add a unit test for argv construction with paths containing spaces, unicode, symlinks (in a tempdir).
5. Document: "If you've manually created a worktree under `~/.agent-of-empires/worktrees/`, do not also pass `--branch` to the sbx runtime."

**Warning signs:**
Sessions in paths with spaces fail with cryptic errors. Sessions in worktrees show "not a git repository." Symlinked paths cause status hooks to write to surprise locations.

**Phase to address:** `sbx-runtime` (canonicalization) + `tests-docs`.

---

## Minor Pitfalls (cleanups, polish)

### Pitfall N1: `sbx version` parsing assumes one format

**What goes wrong:**
`get_version` reads `--version` output (line 82 of runtime_base.rs) and returns the first line. sbx may emit multi-line version output (semver + build SHA + build date) that downstream code accidentally trims wrong.

**How to avoid:**
Define an `SbxVersion { semver: Version, build: String }` parser; unit test against a captured fixture; surface only the semver in user-visible strings.

**Phase to address:** `sbx-runtime`.

---

### Pitfall N2: Daemon auto-start has a first-run cost users blame on aoe

**What goes wrong:**
sbx auto-starts its daemon on first invocation (no explicit start command in the CLI reference). On a cold machine, the first `sbx ls` may take 5-15s while the daemon boots. aoe currently treats `is_daemon_running` as cheap; if it's slow, the TUI startup feels unresponsive.

**How to avoid:**
1. Make `is_daemon_running` non-blocking with a 2s timeout in the SbxRuntime impl. If it times out, return false and let the user retry; don't block UI startup.
2. In the TUI, run runtime availability checks asynchronously — they update in place rather than blocking the initial render.

**Phase to address:** `sbx-runtime` + `integration`.

---

### Pitfall N3: Cross-machine TUI assumes host==target for paths

**What goes wrong:**
Cross-machine TUI relays a session running on Machine A from Machine B. The hook status file path is `/tmp/aoe-hooks/<id>` on Machine A. Machine B can't read it directly. Same problem applies to status detection but now over the network. With Docker today this same problem exists (and presumably is solved by the cockpit/PTY-relay design); with sbx it just adds a layer.

**How to avoid:**
1. Verify cockpit / cross-machine status detection already routes through Machine A's `aoe serve`, NOT Machine B reading filesystem paths directly. If yes, no new work for sbx.
2. Add an integration test (manual / `#[ignore]`) that runs an sbx session on one machine and a TUI on another and verifies status updates flow correctly.

**Phase to address:** `integration` + `tests-docs`.

---

### Pitfall N4: REQ-PLATFORM-01 overstates supported platforms

**What goes wrong:**
REQ-PLATFORM-01 says "macOS arm64, macOS x86_64, Linux x86_64, Linux arm64." Per Docker Sandboxes docs:
- macOS: **Apple Silicon only (Tahoe/26+).** Intel Macs are not supported.
- Windows: x86_64, Windows 11 with Hypervisor Platform.
- Linux: Ubuntu 24.04+ x86_64 with KVM. **arm64 not in the supported list as of 2026-05.**

If aoe ships with `Sbx` selectable on macOS Intel or Linux arm64, users hit a `sbx: command not found` or "your platform is not supported" error; aoe's UX needs to gracefully degrade (not crash, not show sbx as selectable on unsupported platforms).

**How to avoid:**
1. Update REQ-PLATFORM-01 to: "macOS arm64, Linux x86_64. macOS x86_64 and Linux arm64 explicitly OUT due to upstream sbx platform limitations."
2. `SbxRuntime::is_available` does `which sbx` AND consults a static "supported platforms" const; on unsupported platforms, return false and surface a typed reason in the runtime selector ("sbx not supported on this platform").
3. Add a unit test asserting the platform table matches docs.
4. Re-check sbx platform support at the start of every milestone; update the const.

**Warning signs:**
User on Intel Mac reports "the sbx option crashes." Linux arm64 users see "sbx command not found" but aoe still tries to invoke it.

**Phase to address:** `foundation` (PROJECT.md scope correction) + `sbx-runtime` (platform gate).

---

### Pitfall N5: No retention of install-hook output makes kit debugging painful

**What goes wrong:**
Per kit docs: "Install and startup command output is only emitted during `sbx run` or `sbx create`; sbx doesn't retain it for later inspection." If our kit's install hook fails, aoe has no way to surface the failure beyond "create exited non-zero."

**How to avoid:**
1. Capture stderr/stdout of `sbx create` (we already need exit code; preserve full output) and write a tail to `~/.agent-of-empires/logs/sbx-create-<sandbox>-<timestamp>.log`. Show path in TUI on create failure.
2. The kit's install hook should `tee` its output to a known path under the workspace so it survives even after sbx forgets it.
3. Document the log location.

**Phase to address:** `sbx-runtime` (capture + log) + `kit` (tee in install).

---

### Pitfall N6: `sbx reset --preserve-secrets` after daemon-version mismatch is destructive

**What goes wrong:**
Per troubleshooting docs: a daemon-version mismatch on downgrade requires `sbx reset --preserve-secrets`, which **stops every running sandbox and deletes all sandbox state**. If a user downgrades sbx (or aoe ships a kit that requires a newer sbx than what's installed), they lose all their aoe sessions in one command.

**How to avoid:**
1. Embedded kit declares a min-sbx-version in spec.yaml or as an external constant. At startup, `SbxRuntime::is_daemon_running` checks `sbx version` against the min and surfaces a typed error if too old: "sbx <X> required; you have <Y>. Upgrade with <command>." Don't suggest `sbx reset` unless explicitly needed.
2. Document upgrade path (per docs: re-install via package manager, not reset).
3. Test parse: assert `parse_sbx_version("Docker Sandboxes 0.x.y") == Version { ... }`.

**Warning signs:**
Users on older sbx versions see daemon failures aoe can't explain. Confused users run `sbx reset` and lose all sessions.

**Phase to address:** `sbx-runtime` (version gate) + `tests-docs` (upgrade docs).

---

## Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| `foundation` | C6 (default-impl trap), C7 (UI lies), N4 (platform overreach) | Plain const fields not trait defaults; capability-aware field rendering; correct PROJECT.md platform list |
| `sbx-runtime` | C2 (mount divergence), C4 (create→exec race), C8 (login pre-flight), C11 (credential check), M1 (JSON drift), M4 (TTY semantics), N2 (daemon timeout), N4 (platform gate), N6 (version gate) | Validators + retry probes + composite daemon check + serde with defaults + manual TTY tests + 2s timeout + platform const + version min |
| `kit` | C1 (status across VM boundary), C5 (cache invalidation), C9 (idempotent startup), C10 (network policy), M5 (cleanup) | Workspace-relative status path + content-hash cache key + lint test on commands + declared allowedDomains + scoped rm |
| `port-publish` | C3 (publish failure surfaces), M2 (publish lifecycle) | Per-port error capture + reconcile-on-attach |
| `settings-ui` | C7 (capability lies), C6 (visibility gates) | Field-level capability gate + cross-product test |
| `integration` | C10 (policy log surfacing), C12 (clock drift), M2 (reconciliation), N3 (cross-machine paths) | Cockpit warning panel + sleep/wake handler + attach-time port reconcile + verify cockpit relay path |
| `tests-docs` | C8/C11 (prereqs documented), C12 (sleep/wake docs), M3 (disk usage docs), N5 (debug-log location), N6 (upgrade path) | REQ-DOCS-01 expanded to include sbx login, secret table, policy table, disk usage warning, debug log location, upgrade path |

---

## Sources

| Source | Confidence | Notes |
|--------|-----------|-------|
| `.planning/PROJECT.md` | HIGH | Authoritative scope/requirements |
| `.planning/sbx-cli-reference.md` | HIGH | Authoritative CLI surface |
| `src/containers/runtime_base.rs` | HIGH | Existing capability-flag precedent (`supports_read_only_volumes`, `supports_remove_volumes`) |
| `src/hooks/status_file.rs` and `src/session/container_config.rs:993-1006` | HIGH | Confirms hook status file mechanism reads host `/tmp/aoe-hooks/<id>/status` and Docker bind-mounts host dir |
| `src/agents.rs` | HIGH | Confirms file-based hook status is the primary detection mechanism for Claude / Cursor / Gemini / Qwen / Hermes / Kiro |
| `.planning/codebase/CONCERNS.md` | HIGH | Existing "Docker integration tests are fully `#[ignore]`" precedent (we will follow the same pattern for sbx) |
| https://docs.docker.com/ai/sandboxes/get-started/ | HIGH | Platform support (macOS arm64, Windows 11 x86_64, Linux Ubuntu 24.04+ x86_64), `sbx login` mandatory, default-policy prompt on first run |
| https://docs.docker.com/ai/sandboxes/customize/kits/ | HIGH | Kit spec.yaml schemaVersion "1", install vs startup vs initFiles, agent uid 1000, idempotent startup constraint, install/startup output not retained, `proxyManaged` semantics, kit format explicitly experimental |
| https://docs.docker.com/ai/sandboxes/architecture/ | HIGH | "filesystem passthrough", "mounted at the same absolute path as on your host", per-sandbox Docker daemon and image cache (no sharing), HTTP/HTTPS proxy injects credentials |
| https://docs.docker.com/ai/sandboxes/troubleshooting/ | HIGH | UDP/ICMP blocked at network layer; clock drift after sleep; daemon version mismatch requires `sbx reset --preserve-secrets`; failed worktree cleanup; credential proxy can fail if bypassed |
| https://docs.docker.com/ai/sandboxes/usage/ | HIGH | Confirms no `--publish` at create time, services must bind 0.0.0.0 not 127.0.0.1, ports can be added any time after start, primary workspace = first positional arg |
| https://github.com/docker/sbx-kits-contrib | MEDIUM | Pinned-tag versioning convention; kit format explicitly subject to change; cache mechanics not documented |
| https://andrewlock.net/running-ai-agents-safely-in-a-microvm-using-docker-sandbox/ | MEDIUM | Practitioner report: "performance hit can be crippling" for larger projects; SSH agent forwarding for commit signing not solved; "creating a worktree yourself, and then running a sandbox directly in this folder" breaks parent-repo access |
| WebSearch (multiple) | MEDIUM | Linux Ubuntu support confirmed via multiple sources after initial blog post said macOS+Windows only — Linux added in subsequent releases |
