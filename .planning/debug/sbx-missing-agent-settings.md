---
status: resolved
trigger: "sbx backend does not mount agent config dirs (~/.claude etc.) into new sessions; Docker backend does"
created: 2026-05-17
updated: 2026-05-17
---

# Debug: sbx-missing-agent-settings

## Symptoms

- expected: Agent settings from the host (e.g., ~/.claude for Claude Code) should be present in sbx containers, matching Docker backend behavior
- actual: Settings are completely missing in sbx sessions; user must re-authenticate every session
- errors: None reported
- timeline: New sbx backend; appears to have never worked for agent config mounting
- reproduction: Create a new session using the sbx backend with Claude Code agent

## Current Focus

- hypothesis: CONFIRMED. conform_for_capabilities drops agent config bind mounts because sbx requires host_path == container_path, but agent staging dirs map e.g. ~/.claude/sandbox -> /root/.claude.
- test: Post-create sbx cp injection of staged agent config
- expecting: Agent config (credentials, plugins, settings) present in sbx sandbox after creation
- next_action: none (fixed)

## Evidence

- timestamp: 2026-05-17
  - build_container_config (container_config.rs) adds agent config mounts with host_path=sandbox_dir, container_path=/root/.claude
  - conform_for_capabilities drops all mounts where host_path != container_path for sbx (supports_arbitrary_volume_paths=false)
  - Docker hooks write to /tmp/aoe-hooks/ but sbx hooks write to ${WORKDIR}/.aoe-hooks/ (different status file paths)
  - Kit install command writes sbx-specific hooks into settings.json during sbx create
  - sbx has a `cp` command: `sbx cp SRC SANDBOX:DST` that works on stopped sandboxes

## Eliminated

- sbx native agent subcommands (create claude, create codex): AoE supports more agents than sbx has named; using these would bypass AoE's kit system
- Kit install commands for config: kit is content-addressed/cached, cannot carry per-session credential data
- Volume conformance: sbx volumes must satisfy host_path == container_path; agent config sandbox dirs can never satisfy this constraint

## Resolution

- root_cause: conform_for_capabilities in container_config.rs drops agent config bind mounts for sbx because the host staging path (~/.claude/sandbox/) differs from the container path (/root/.claude). No post-create mechanism existed to inject the staged config into the sandbox via an alternative to bind mounts.
- fix: After sbx create, use `sbx cp` to copy each entry from the staged agent config directory into the sandbox. settings.json is handled specially: a merged version is built combining the user's host settings with sbx-specific hooks (writing to ${WORKDIR}/.aoe-hooks/ instead of Docker's /tmp/aoe-hooks/). The sbx kit settings JSON files (already embedded at compile time) provide the hooks source.
- verification: 4 new tests pass (3 for merged settings, 1 for sbx cp args), all existing tests pass, clippy clean, fmt clean
- files_changed: src/containers/sbx/mod.rs (cp method), src/containers/sbx/kit.rs (pub(crate) settings consts), src/session/container_config.rs (sbx_agent_config_copies, build_sbx_merged_settings, CONTAINER_HOME consolidation), src/containers/runtime.rs (post-create injection in sbx branch)
