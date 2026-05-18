# Docker Sandboxes (sbx)

## Overview

Docker Sandboxes (sbx) provide microVM-based isolation for AI coding agents via Docker Desktop. Unlike standard Docker containers that share the host kernel, each sbx sandbox runs inside its own lightweight microVM. This makes sbx a good choice for users wanting stronger isolation between sessions.

Once configured, AoE drives sbx sandboxes with the same session lifecycle as Docker and Podman: one sandbox per session, automatic cleanup on removal, and full project access via volume mounts.

## Prerequisites

1. **Docker Desktop** installed with sbx support. The `sbx` CLI ships with Docker Desktop; no separate installation is needed.

2. **Sign in to Docker:**

   ```bash
   sbx login
   ```

   This opens a browser-based Docker authentication flow. You must be signed in before creating sandboxes.

3. **Set a default network policy:**

   ```bash
   sbx policy set-default <policy>
   ```

   Choose one of the three built-in policies:

   | Policy | Description |
   |--------|-------------|
   | `allow-all` | All outbound network traffic is allowed. |
   | `balanced` | Common dev traffic allowed (AI services, package registries, etc.). |
   | `deny-all` | All outbound network traffic is blocked. |

   For most users, `balanced` is a sensible default. Use `allow-all` if you want no restrictions on outbound traffic.

4. **Store API secrets** for each agent you plan to use. The sbx proxy injects secrets into the sandbox at runtime; the credential never enters the microVM directly.

   | Agent | Service | Command |
   |-------|---------|---------|
   | Claude Code | anthropic | `sbx secret set -g anthropic` |
   | Codex | openai | `sbx secret set -g openai` |
   | Gemini CLI | google | `sbx secret set -g google` |
   | Hermes | mistral | `sbx secret set -g mistral` |
   | droid | droid | `sbx secret set -g droid` |

   For multi-provider agents (OpenCode, Pi, Qwen Code, Kiro, Mistral Vibe), set the secret for whichever provider you use. Run `sbx secret set` without arguments to launch interactive mode, which lists all available services.

### Verify Installation

Confirm that sbx is available and you are signed in:

```bash
sbx version
sbx ls
```

`sbx version` prints the installed Docker Sandboxes version. `sbx ls` lists your sandboxes (empty on first use).

## Configuration

To use sbx as your sandbox runtime, update your config file.

**Linux:** `~/.config/agent-of-empires/config.toml`
**macOS:** `~/.agent-of-empires/config.toml`

```toml
[sandbox]
container_runtime = "sbx"
```

### Profile-Specific Runtime

You can scope sbx to a specific profile if you want to keep Docker as your global default:

```toml
[profiles.sbx]
sandbox.container_runtime = "sbx"
```

Then use it with: `aoe add --profile sbx .`

You can also pick the runtime per-session in the TUI settings under **Sandbox > Container Runtime**.

## Usage

Once configured, usage is identical to the Docker sandbox. All the standard sandbox flags work unchanged:

```bash
# Create an sbx-backed sandboxed session
aoe add --sandbox .

# Launch with a specific image
aoe add --sandbox --sandbox-image my-custom-image:latest .
```

In the TUI, the **Sandbox** toggle uses whichever runtime is configured in `container_runtime`.

## Important Notes

### Disk Usage

Each sbx sandbox is a full microVM with its own filesystem. Expect approximately ImageSize x ActiveSessions of host disk usage. Stopped sandboxes still consume disk. Remove sandboxes you no longer need with `aoe remove` to reclaim space.

### Kit Format (Experimental)

AoE ships an embedded sbx kit that configures each sandbox with agent tooling and status hooks. The sbx kit format is experimental per Docker documentation; schema changes in future Docker Desktop updates may require an AoE update. If sandbox creation starts failing after a Docker Desktop upgrade, check for an AoE update.

### Upgrade Path

sbx updates are delivered through Docker Desktop. To upgrade, update Docker Desktop through its normal update mechanism.

> **Warning:** Do NOT run `sbx reset` to troubleshoot issues. This command destroys all sandboxes, cached data, and stored secrets. Use it only as a last resort.

### No Read-Only Volume Mounts

sbx does not support the `:ro` flag for volume mounts. If `mount_ssh` is enabled, AoE mounts SSH keys read-write into the sandbox.

### No Anonymous Volumes

sbx does not support anonymous volumes, so the `volume_ignores` configuration option is not applicable when using sbx.

### Port Publishing

Ports are published after sandbox creation rather than at create time. AoE handles this automatically; port-publish failures are surfaced as warnings without rolling back the sandbox.

## Troubleshooting

### "Not logged in" / sbx login error

If AoE reports sbx as unavailable with a login error, run `sbx login` on the command line to re-authenticate.

### "Policy not configured"

sbx requires a default network policy before sandboxes can be created. Run:

```bash
sbx policy set-default balanced
```

### Sandbox creation fails after Docker Desktop update

The sbx kit format may have changed in the new Docker Desktop version. Check for an AoE update and upgrade if one is available.

### Debug Logging

To capture detailed logs for troubleshooting, enable debug logging:

```bash
AGENT_OF_EMPIRES_DEBUG=1 aoe add --sandbox .
```

The debug log is written to:

- **Linux:** `~/.config/agent-of-empires/debug.log`
- **macOS:** `~/.agent-of-empires/debug.log`
