# Feature Research

**Domain:** sbx (Docker Sandboxes) container runtime backend for `aoe`
**Researched:** 2026-05-15
**Confidence:** HIGH (sbx CLI surface, kit format, hooks model verified against official docs and codebase)

## Summary

The sbx backend exists to add a fourth `ContainerRuntime` peer that gives users microVM-grade isolation without changing any aoe surface beyond the runtime dropdown. The feature set splits cleanly along three lines:

1. **Table stakes** — every behavior the existing `ContainerRuntimeInterface` exposes today must continue to work when the user selects sbx, plus the few sbx-specific orchestration glue points (out-of-band port publishing, kit-based agent install) that have no parallel in Docker/Podman/Apple Container.
2. **Differentiators worth surfacing** — sbx-only capabilities that align with aoe's existing model (single embedded kit reusing the user's Docker image, status hooks shimmed via `initFiles`).
3. **Anti-features** — sbx capabilities (per-session policy, secret CRUD, template snapshot/load, dynamic port re-publish UI) that look attractive but break the project's "drop-in 4th backend" goal or duplicate user-owned host state.

The hardest constraint is invisible: sbx's host-side global state (network policies, secrets, login) is owned by the user, not by aoe. Every "wouldn't it be neat if aoe managed X" idea in this document is rejected if X is global host state.

## Feature Landscape

### Table Stakes (Required for Parity)

Each row maps to either a `ContainerRuntimeInterface` method or an existing aoe UI flow. Missing any of these means "sbx isn't a real backend" — the runtime dropdown would lie.

| # | Feature | Maps To | Complexity | Notes |
|---|---------|---------|------------|-------|
| TS-01 | Detect sbx CLI on `PATH` | `ContainerRuntimeInterface::is_available` | LOW | `sbx --version` exit code, mirroring `RuntimeBase::is_available` for Docker. |
| TS-02 | Detect sbx daemon health | `ContainerRuntimeInterface::is_daemon_running` | LOW | `sbx ls` succeeds when the daemon is up; no equivalent of `docker info` exists. The daemon auto-starts on first command, so this is closer to a connectivity probe than a true health check. |
| TS-03 | Report runtime version | `ContainerRuntimeInterface::get_version` | LOW | `sbx version` (note: subcommand, not flag — unlike `docker --version`). |
| TS-04 | Create container | `ContainerRuntimeInterface::create_container` | HIGH | Maps to `sbx create shell <workspace> --kit <embedded-kit> --template <image> --name <name>`. Workspace mount uses positional args, not `-v`. CPU/memory map to `--cpus`/`-m`. Custom env and port mappings have no `sbx create` equivalent and must be applied post-create. |
| TS-05 | Start container | `ContainerRuntimeInterface::start_container` | LOW | No-op. sbx auto-starts on first `sbx exec`; explicit start does not exist. Capability flag `requires_explicit_start = false`. |
| TS-06 | Stop container | `ContainerRuntimeInterface::stop_container` | LOW | `sbx stop <name>`. Direct mapping. |
| TS-07 | Remove container | `ContainerRuntimeInterface::remove` | LOW | `sbx rm [-f] <name>`. The `-v` (anonymous volume) flag does not exist on sbx — already gated by existing `supports_remove_volumes` capability flag. |
| TS-08 | Container existence check | `ContainerRuntimeInterface::does_container_exist` | LOW | Parse `sbx ls --json` for the name. No `inspect` subcommand exists. |
| TS-09 | Container running-state check | `ContainerRuntimeInterface::is_container_running` | LOW | `sbx ls --json` exposes per-sandbox status. |
| TS-10 | Batch running states for prefix | `ContainerRuntimeInterface::batch_running_states` | LOW | Single `sbx ls --json` call, then filter by name prefix. Fits the existing pattern of one syscall per status sweep. |
| TS-11 | Exec into container (TUI command string) | `ContainerRuntimeInterface::exec_command` | LOW | `sbx exec -it <name> <cmd>` mirrors `docker exec -it`. Same `-i`/`-t`/`-e`/`-w` flags, so no Apple-Container-style `sh -c` wrapping needed. |
| TS-12 | Exec for programmatic output | `ContainerRuntimeInterface::exec` | LOW | `sbx exec <name> <cmd...>`. Captures stdout/stderr same as `docker exec`. |
| TS-13 | Workspace mount of project directory | `ContainerConfig.volumes` (read-write entries) | MEDIUM | sbx requires `host_path == container_path`. aoe currently uses `/workspace/<project-name>` as the in-container path. Either auto-conform (use `host_path` as `working_dir`) or surface a clear error per REQ-RT-04. Decision belongs to ARCHITECTURE; this surface must accept either. |
| TS-14 | Read-only auxiliary mounts | `ContainerConfig.volumes` (read-only entries) | LOW | `sbx create ... <path>:ro` — directly supported via the `:ro` suffix on positional args. |
| TS-15 | Custom environment variables (literal) | `ContainerConfig.environment` (Literal) | MEDIUM | `sbx exec -e KEY=VALUE` works at exec time but `sbx create` has no `-e`. Environment must therefore travel through the kit's `environment.variables` block or be passed at every `exec` call. The cleaner path is per-exec injection because aoe already builds the exec command per-attach. |
| TS-16 | Inherited environment variables (no value in argv) | `ContainerConfig.environment` (Inherit) | MEDIUM | Same as TS-15 plus the existing pattern of passing `-e KEY` (no value) and setting it in the parent process env. Works at exec time; no equivalent at create time. |
| TS-17 | CPU limits | `ContainerConfig.cpu_limit` | LOW | `sbx create --cpus N`. Note: sbx accepts an integer count; aoe's existing `Option<String>` value happens to be numeric in practice, but parsing/validation is needed. |
| TS-18 | Memory limits | `ContainerConfig.memory_limit` | LOW | `sbx create -m 4g`. Format matches Docker (binary units). |
| TS-19 | Port mappings | `ContainerConfig.port_mappings` | MEDIUM | `sbx create` has no `-p`. Resolved mappings must be applied via `sbx ports SANDBOX --publish <spec>` immediately after create succeeds. Per REQ-RT-05, partial failures surface but do not roll back the sandbox. |
| TS-20 | Image presence check | `ContainerRuntimeInterface::image_exists_locally` | LOW | sbx doesn't expose an image inspector. Fold into TS-21 and treat "image presence" as a no-op that returns true (sbx pulls on demand during `sbx create`). |
| TS-21 | Image pull | `ContainerRuntimeInterface::pull_image` / `ensure_image` | MEDIUM | sbx has no `pull` subcommand; templates are pulled implicitly by `sbx create -t <image>`. Implement as no-op + capability flag `supports_image_pull = false` per REQ-RT-06. The runtime trait must accept this without warning. |
| TS-22 | Default sandbox image | `ContainerRuntimeInterface::default_sandbox_image` | LOW | Same `ghcr.io/njbrake/aoe-sandbox:latest` used today. The kit's `--template` argument is the user's configured image. |
| TS-23 | Embedded sbx kit (single, agent-agnostic) | REQ-KIT-01..04 | HIGH | Materialize `<aoe data dir>/sbx-kit/` from binary-embedded files on first use; rewrite when the embedded version differs. Kit installs the configured agent + status-hook shim. Same kit handles every supported agent. |
| TS-24 | Status detection inside the sandbox | `src/hooks/`, `src/tmux/status_detection.rs` | HIGH | File-based hooks (`/tmp/aoe-hooks/<id>/status`) are the existing mechanism for Claude/Cursor/Gemini/Qwen. The status file must live on a path the host can read. See ARCHITECTURE — this is the single biggest open mechanical question. |
| TS-25 | Surface "sbx not installed" cleanly | `is_available()` consumers across TUI, web, cockpit | LOW | When `sbx` is missing, every flow that currently shows "Docker daemon not running" must show an equivalent error pointing the user at install docs, not crash. |
| TS-26 | Settings TUI wiring for `Sbx` runtime | `src/tui/settings/fields.rs` | MEDIUM | Per AGENTS.md: add to `ContainerRuntimeName` enum, FieldKey, build_*_fields, apply/clear, profile override merge. Mechanical, but missing any link makes the dropdown half-functional. |
| TS-27 | Per-profile and per-session runtime override | `SandboxConfigOverride.container_runtime` | LOW | Already exists as a settable enum field; just needs the new variant. |
| TS-28 | Cockpit / web dashboard / cross-machine compatibility | REQ-INTEGRATION-01 | LOW | Should be free if TS-01..TS-27 stay behind the trait. The spec calls this out as a non-regression requirement, not net-new work. |

### Differentiators (sbx-Only Capabilities Worth Exposing)

These are net-new capabilities not present in any existing backend. Each is justified against the "drop-in 4th backend" Core Value.

| # | Feature | Value Proposition | Complexity | Decision |
|---|---------|-------------------|------------|----------|
| DF-01 | Single embedded kit that installs agents on top of the user's existing Docker image | Eliminates per-agent image proliferation. The user's existing `ghcr.io/njbrake/aoe-sandbox:latest` (or their override) keeps working unchanged. | HIGH | **Build (REQ-KIT-01..04).** Already in scope. Delivers the "no surprise" value. |
| DF-02 | Status hooks delivered via kit `initFiles` | Means hook-supporting agents (Claude, Cursor, Gemini, Qwen) get the same file-based status detection inside the sandbox as they do on the host today. No regression, no per-runtime branch. | MEDIUM | **Build (REQ-KIT-03).** Already in scope. |
| DF-03 | microVM isolation per session (the actual reason to use sbx) | The user's whole reason to pick sbx. aoe doesn't *do* anything to surface this — it's an emergent property of selecting the runtime. | NONE | **Inherent.** Requires nothing beyond TS-01..TS-28. Mention in docs. |
| DF-04 | Sandbox-scoped network policy presets per profile | sbx supports `sbx policy allow network <sandbox> <hosts>`. A profile-level "allow these hosts" setting could narrow the network surface per agent class. | HIGH | **Defer (out of scope per PROJECT.md decision).** Useful, but managing sandbox-scoped policies bleeds aoe into host-state ownership and conflicts with sbx's "policies persist across sessions" model. Document the manual workflow. |
| DF-05 | `sbx template save` integration for "save this sandbox as a base image" | Lets a user materialize a long-running session's installed state as a reusable template. | HIGH | **Defer (out of scope per PROJECT.md decision).** Nice power-user feature; not needed for parity. Promote later if users ask. |
| DF-06 | Dynamic port re-publish from the TUI ("publish port 9000 to this sandbox now") | sbx's `sbx ports --publish` works any time after start. Could surface as a TUI action. | MEDIUM | **Defer.** No equivalent in Docker/Podman/Apple Container; building this surface is gated by adding ports UI, which PROJECT.md explicitly defers. Once port editing lands, this becomes a freebie. |
| DF-07 | Surface sbx's per-sandbox secrets in the new-session dialog | Could let the user toggle which `sbx secret` services are scoped to this sandbox. | HIGH | **Reject (anti-feature, see AF-02).** Aoe touching `sbx secret` would duplicate user/team credential management. Sbx already injects globally-scoped secrets transparently — that's the right interaction. |
| DF-08 | Auto-detect "balanced" default policy and warn if `deny-all` is set | A user who forgot to run `sbx policy set-default` will get a sandbox with no network access. Aoe can detect this via `sbx policy ls` and surface a one-time hint. | LOW | **Build as docs/diagnostic only.** Don't auto-set the policy (anti-feature, see AF-01). On first sbx-runtime use, `aoe doctor`-style output can show the resolved default policy and link to docs. |
| DF-09 | Reuse `sbx diagnose` output in `aoe`'s health/diagnostic commands | sbx ships its own diagnostic. Surfacing it inside aoe's existing diagnostic flows lowers the support burden. | LOW | **Build (small).** When `aoe doctor` or the equivalent runs and the runtime is sbx, append `sbx diagnose` output. Pure read-only. |

### Anti-Features (Tempting, Actively Harmful)

Each entry's "harm" column explains why this would actively damage the "drop-in 4th backend" goal — not just "extra scope," but *worse than not building it*.

| # | Feature | Surface Appeal | Why Harmful | Alternative |
|---|---------|----------------|-------------|-------------|
| AF-01 | aoe sets the global sbx default network policy | "Just make it work for the user — call `sbx policy set-default balanced` on first use." | Mutates global host state the user shares across every other tool that uses sbx. A user who deliberately set `deny-all` would have aoe silently relax their security posture. Violates the "host-side state is user-owned" decision. | Detect missing/strict default; emit a one-time hint with the exact command to run. |
| AF-02 | aoe writes to `sbx secret set -g` on the user's behalf | "We already collect API keys via env vars — copy them into `sbx secret` so the proxy can inject them." | Conflates aoe's per-session env-var injection (visible to the agent process via `-e`) with sbx's proxy-managed secrets (never visible to the agent). The two security models are *incompatible*: a user who deliberately scopes a token to one sandbox would have it leak into all sandboxes. Plus: aoe has no way to know if a value is OAuth or API key, which `sbx secret` distinguishes. | Document `sbx secret set -g <service>` as a one-time setup step. Existing `EnvEntry::Inherit` keeps working unchanged. |
| AF-03 | Per-session sbx network policy controlled from the new-session dialog | "Let the user pick allow/deny rules per session." | sbx policies persist after the sandbox is deleted (`sbx policy ls` shows them indefinitely). aoe creating per-session rules generates persistent host state aoe never cleans up; over time the user's policy list becomes a graveyard of stale rules. Plus: changes to global state per session interact unpredictably with concurrent sessions on different policies. | Profile-level docs that point to `sbx policy allow network <sandbox> <hosts>` for advanced users. |
| AF-04 | Per-agent native sbx mappings (`sbx create claude` for Claude, etc.) | "Use sbx's own agent presets — we get free image selection." | Forces users to abandon their current `default_image` setting (which uses an aoe-curated image with all agents pre-installed). Creates a divergent matrix where some aoe agents map to sbx natives and others fall back to `shell`, leaking sbx-specific behavior into agent selection. PROJECT.md already rejects this. | One kit, one image, `sbx create shell --kit <ours> --template <user's image>`. |
| AF-05 | Authoring or fetching kits at runtime | "Let users supply their own kit URL or path." | Adds auth, network, supply-chain, and validation surface for a feature that doesn't move parity forward. Aoe controls the kit because it's the contract between the runtime layer and the agent registry. | Single embedded kit; users override via the existing `default_image` setting if they need a custom base. |
| AF-06 | Dynamic port re-publish UI without a port editor | "Add a 'publish a new port' modal in the TUI." | Surfaces only on sbx, creating a runtime-specific UX divergence the rest of aoe avoids. The data layer already supports per-profile/per-session port mappings via `SandboxConfigOverride.port_mappings`; the missing piece is the editor, which is a backend-agnostic change. | Wait until generic port-mapping UI lands (out of scope for this milestone), then auto-publish covers all backends uniformly. |
| AF-07 | Auto-migrate existing Docker-backed sessions to sbx on runtime change | "When the user switches the runtime setting, recreate live sessions." | Sessions are stateful (running agents, scrollback, git worktrees). Recreating them silently loses work. Per the existing model, the runtime change applies to *new* sessions only. PROJECT.md already rejects this. | Doc the opt-in model. Existing sessions keep their original runtime. |
| AF-08 | Cleanup `sbx ports`/`sbx policy`/`sbx secret` state on session removal | "Tidy up after ourselves." | Aoe doesn't own these entries (per AF-01..AF-03). Even policy/secret entries that aoe *did* create would risk stomping on user-modified rules. The exception is published ports, which sbx itself cleans up on `sbx rm`. | Trust sbx's lifecycle. Don't try to GC global state. |
| AF-09 | Pre-flight `sbx login` flow | "If the user isn't logged in, open the OAuth flow inside aoe." | sbx's login is a Docker Hub OAuth flow with browser handoff — wrapping it in a TUI/web modal duplicates surface for marginal benefit and breaks if Docker changes its flow. | Detect missing login at runtime selection time; surface "run `sbx login` then come back" with exact command. |
| AF-10 | Use `sbx run` (the auto-attach version) instead of `sbx create` + `sbx exec` | "Fewer commands per session." | `sbx run` couples create-and-attach into a single foreground process, which doesn't fit aoe's tmux-pane model where create happens in the background and attach happens later when the user opens a session. PROJECT.md already commits to `sbx create` + `sbx exec`. | `sbx create` (no attach) + `sbx exec [-i] [-t]` from inside the tmux pane. |

## Feature Dependencies

```
TS-01 (CLI detected)
    └──gates──> TS-02..TS-28 (everything else)

TS-04 (create_container)
    ├──requires──> TS-08 (existence check, to avoid duplicate-name errors)
    ├──requires──> TS-23 (kit must be materialized first)
    └──followed by──> TS-19 (post-create port publish)

TS-23 (embedded kit)
    ├──contains──> TS-24 hook shim (initFiles)
    └──contains──> agent install scripts (install hook)

TS-24 (status detection)
    ├──requires──> TS-23 (initFiles delivery)
    └──requires──> /tmp/aoe-hooks readability across host/sandbox boundary
       (microVM filesystem isolation is a real concern; see ARCHITECTURE)

TS-15/TS-16 (env injection)
    └──happens at──> TS-11/TS-12 (exec time, not create time)
       (sbx create has no -e flag; this is a real architectural delta)

TS-26 (settings UI) ──gates──> TS-27 (override resolution)
    └──gates──> first user being able to actually pick sbx

DF-08 (policy diagnostic) ──reads──> sbx policy ls (no aoe writes)
DF-09 (sbx diagnose passthrough) ──reads──> sbx diagnose (no aoe writes)
```

### Dependency Notes

- **TS-19 (port publish) is post-hoc, not atomic with create:** sbx has no create-time port flag. If the publish step fails, the sandbox exists with no ports. Per REQ-RT-05, surface the failure but do not roll back — matches the user's intuition that a sandbox without ports is still a sandbox.
- **TS-15/TS-16 (env) is per-exec, not per-create:** This is the most uncomfortable departure from the Docker model. Every `sbx exec` call needs the env list re-injected. Cleaner than baking env into the kit, because env can be session-scoped (e.g., `AOE_INSTANCE_ID` for hooks).
- **TS-24 (status detection) crosses the microVM boundary:** sbx mounts the workspace at `host_path == container_path`, so `/tmp/aoe-hooks` is *not* shared by default. Either: (a) mount `/tmp/aoe-hooks/<instance-id>` as an additional workspace, (b) write the status into the workspace itself under a hidden path, or (c) poll `sbx exec cat <path>`. Belongs in ARCHITECTURE; the feature must work, the mechanism is a research output.
- **DF-01 (single kit) and AF-04 (per-agent natives) are mutually exclusive:** Picking DF-01 commits aoe to `sbx create shell` only. This is the right call — see PROJECT.md decision.

## MVP Definition

### Launch With (this milestone)

Required for "selecting sbx in Settings produces a working session":

- [ ] TS-01..TS-12 — every `ContainerRuntimeInterface` method backed by an sbx call (or no-op with capability flag)
- [ ] TS-13, TS-14 — workspace mounts respecting `host_path == container_path` and `:ro`
- [ ] TS-15, TS-16 — env injection at exec time
- [ ] TS-17, TS-18 — CPU/memory passthrough
- [ ] TS-19 — post-create port publish via `sbx ports --publish`
- [ ] TS-20, TS-21 — image-pull no-ops behind capability flags
- [ ] TS-22 — default image hand-off to kit `--template`
- [ ] TS-23 — embedded kit, materialized to cache, version-checked
- [ ] TS-24 — status hooks reachable across the microVM boundary
- [ ] TS-25 — clean error when `sbx` is absent
- [ ] TS-26, TS-27 — Settings UI wiring per AGENTS.md
- [ ] TS-28 — cockpit / web / cross-machine non-regression
- [ ] DF-01, DF-02 — embedded kit installs agent + drops in hook shim

### Add After Validation

- [ ] DF-08 — first-run diagnostic that reports the resolved default policy
- [ ] DF-09 — `sbx diagnose` passthrough inside aoe's diagnostic commands

### Future Consideration

- [ ] DF-04 — per-profile network allow lists (after host-state-ownership policy is revisited)
- [ ] DF-05 — `sbx template save` integration as a session action
- [ ] DF-06 — dynamic port re-publish UI (lands when generic port editor lands)

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| TS-01..TS-12 (trait parity, simple methods) | HIGH | LOW each | P1 |
| TS-13, TS-14 (workspace mounts) | HIGH | MEDIUM | P1 |
| TS-15, TS-16 (env at exec) | HIGH | MEDIUM | P1 |
| TS-17, TS-18 (resource limits) | MEDIUM | LOW | P1 |
| TS-19 (post-create port publish) | HIGH | MEDIUM | P1 |
| TS-20, TS-21 (image no-ops) | LOW | LOW | P1 (gating, not value) |
| TS-23 (embedded kit) | HIGH | HIGH | P1 |
| TS-24 (status across microVM boundary) | HIGH | HIGH | P1 |
| TS-25 (missing-binary error) | MEDIUM | LOW | P1 |
| TS-26, TS-27 (settings wiring) | HIGH | MEDIUM | P1 |
| DF-01, DF-02 (kit value props) | HIGH | inherits from TS-23 | P1 |
| DF-08 (policy diagnostic) | MEDIUM | LOW | P2 |
| DF-09 (sbx diagnose passthrough) | LOW | LOW | P2 |
| DF-04..DF-07 | LOW (now) | HIGH | P3 |
| Anti-features AF-01..AF-10 | NEGATIVE | n/a | DO NOT BUILD |

## Competitor / Prior Art Analysis

Research into how comparable tools structure their integrations with microVM and sandbox systems.

| Aspect | nerdctl (containerd) | Podman (rootless) | Firecracker SDK consumers | Our (sbx) approach |
|--------|----------------------|-------------------|---------------------------|--------------------|
| API surface vs. underlying capability | Exposes a Docker-CLI subset; missing flags are explicitly listed as "unimplemented" rather than silently ignored ([nerdctl docs](https://github.com/containerd/nerdctl/blob/main/docs/command-reference.md)). Surfaces some containerd-only flags (`--snapshotter`, OCIcrypt) where they don't conflict with Docker semantics. | Mirrors Docker CLI 1:1 for compatibility, then adds Podman-only commands (`pod`, `generate kube`) as new subcommands rather than reinterpreting existing Docker flags. | Treat Firecracker's REST API as a building block, not a UX. Higher-level orchestrators (Fly.io machines, AWS Lambda) hide it behind their own product surface ([Firecracker design](https://github.com/firecracker-microvm/firecracker/blob/main/docs/design.md)). | Match: every `ContainerRuntimeInterface` method maps to an sbx call or a documented no-op. sbx-only knobs (kits, policy, secrets) live behind capability flags and config docs, not new aoe surfaces. |
| Capability divergence | Capability flags per snapshotter; runtime-specific subcommands clearly namespaced. | Per-feature feature flags (e.g., `--cgroup-manager=cgroupfs\|systemd`); runtime-specific defaults documented per OS. | Each consumer wraps the API at its own level. No common pattern for "the same orchestrator targets both Firecracker and Docker." | Match: extend existing `RuntimeBase` capability flags (`supports_*`); add `supports_image_pull`, `supports_dynamic_port_publish`, `supports_anonymous_volumes`, `requires_explicit_start`. Already validated by PROJECT.md REQ-RT-03. |
| Host-state ownership | Doesn't manage containerd config; just talks to the daemon. | Manages user-owned config in `~/.config/containers/`; deliberately doesn't touch system config. | None — Firecracker is a library; consumers own all state. | Match: aoe never writes to `~/.docker/sbx/`, `sbx policy`, or `sbx secret`. Documents prerequisites only. |
| Network policy | Inherits Docker network model; no policy of its own. | Inherits CNI/Netavark; no opinion. | Caller-owned (each consumer wires its own networking). | Inherit sbx's global policy; surface a diagnostic if `deny-all` is the default and the user is about to start their first sbx session. |

**Key takeaway:** The pattern across nerdctl, Podman, and Firecracker SDK consumers is consistent — *don't reinterpret the underlying tool's host state*, just shell out cleanly and expose capability flags where the tools differ. sbx is closer in shape to nerdctl (Docker-compatible CLI subset with extras) than to Firecracker (raw API). The aoe layer should stay at the CLI-shell-out level the existing runtimes use; resist the temptation to reach into sbx's daemon state.

## sbx Kits Ecosystem Reference

Authoritative sources reviewed for the embedded aoe kit design:

- **[docker/sbx-kits-contrib](https://github.com/docker/sbx-kits-contrib)** — official community kits repo. Existing kits include `pi`, `nanobot`, `openclaw`, `nanoclaw`, `code-server`, `mise`, `task`, `trivy`. All follow the `<kit-dir>/{spec.yaml, files/, *_tck_test.go}` layout.
- **[Kits doc](https://docs.docker.com/ai/sandboxes/customize/kits/)** — current spec format. Schema fields confirmed: `schemaVersion`, `kind` (`mixin`|`agent`), `name`, `displayName`, `description`, `commands.{install,startup}`, `initFiles`, `network.{allowedDomains,deniedDomains,serviceDomains,serviceAuth}`, `environment.{variables,proxyManaged}`, `credentials.sources`, `agent.{image,aiFilename,persistence,entrypoint}`. Kits are explicitly experimental — schema may change.
- **[Kit examples](https://docs.docker.com/ai/sandboxes/customize/kit-examples/)** — reference patterns for install + startup hook composition.
- **[Build your own agent kit](https://docs.docker.com/ai/sandboxes/customize/build-an-agent/)** — confirms `kind: agent` requires the full `agent:` block; `kind: mixin` does not.

**Implication for the embedded aoe kit:**
- Use `kind: mixin` (not `agent`) so the user's existing image stays the source of truth.
- `commands.install` runs the agent install script for whichever agent the kit user requested.
- `initFiles` writes the hook shim (a small shell wrapper that the agent's settings.json points at) to the workspace path so it can write status without crossing the microVM filesystem boundary.
- Don't use `network.{allowedDomains,deniedDomains}` in the kit — those would override user policy. Leave network policy entirely to the user.
- Don't use `credentials.sources` — that's the sbx secrets path, see AF-02.
- Use `environment.variables` only for non-secret values (e.g., `AOE_INSTANCE_ID`); never for tokens.

## Status Detection Across the microVM Boundary

The single biggest architectural question raised by this research. Summary of options for ARCHITECTURE consumption:

| Option | How | Pros | Cons |
|--------|-----|------|------|
| A. Mount `/tmp/aoe-hooks/<id>` as a workspace | `sbx create shell . /tmp/aoe-hooks/<id>` (host_path == container_path constraint satisfied because `/tmp/...` is a real host path) | Reuses the existing hook code unchanged. Atomic writes from inside the sandbox visible to host immediately. | Adds a second mount to every sandbox. May surprise users browsing `sbx ls`. |
| B. Write status under the workspace itself | Hook shim writes to `<workspace>/.aoe-status` instead of `/tmp/aoe-hooks/<id>/status` | Zero extra mounts. | Pollutes the user's project directory. Conflicts with `.gitignore` discipline. Caller must filter. |
| C. Poll via `sbx exec cat /tmp/aoe-hooks/<id>/status` | Run an sbx exec on every status sweep | No mount changes. | Per-sandbox process spawn on every poll = expensive. Defeats the existing `batch_running_states` optimization. |

**Recommendation for ARCHITECTURE:** Option A. The hook directory is small, the mount is invisible to user-facing flows, and the existing hook code stays unchanged. Option B leaks aoe state into the user's repo. Option C is too expensive at aoe's poll frequency.

For non-hook agents (Codex, OpenCode, Vibe, etc.), pane-content scraping via `tmux capture-pane` continues to work — `sbx exec -it` is a normal tmux pane subprocess from aoe's perspective.

## Sources

- [Docker Sandboxes documentation](https://docs.docker.com/ai/sandboxes/) (HIGH confidence)
- [sbx CLI reference](https://docs.docker.com/reference/cli/sbx/) and local snapshot at `.planning/sbx-cli-reference.md` (HIGH confidence)
- [Kits documentation](https://docs.docker.com/ai/sandboxes/customize/kits/) (HIGH confidence; schema marked experimental)
- [Build your own agent kit](https://docs.docker.com/ai/sandboxes/customize/build-an-agent/) (HIGH confidence)
- [Kit examples](https://docs.docker.com/ai/sandboxes/customize/kit-examples/) (HIGH confidence)
- [docker/sbx-kits-contrib](https://github.com/docker/sbx-kits-contrib) (HIGH confidence; official repo)
- [containerd/nerdctl command reference](https://github.com/containerd/nerdctl/blob/main/docs/command-reference.md) — prior art for "Docker-compatible CLI with documented unimplemented gaps" (MEDIUM confidence; current docs)
- [Firecracker design](https://github.com/firecracker-microvm/firecracker/blob/main/docs/design.md) — prior art for "treat the underlying API as a building block, not a UX" (MEDIUM confidence)
- Codebase: `src/containers/container_interface.rs`, `src/containers/runtime.rs`, `src/containers/runtime_base.rs`, `src/agents.rs`, `src/hooks/mod.rs`, `src/hooks/status_file.rs`, `src/session/profile_config.rs`, `src/tui/settings/fields.rs` (HIGH confidence; verified directly)
- Project context: `.planning/PROJECT.md`, `.planning/codebase/INTEGRATIONS.md` (HIGH confidence)

---
*Feature research for: sbx (Docker Sandboxes) backend for `aoe`*
*Researched: 2026-05-15*
