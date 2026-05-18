# Docker Sandboxes (sbx)

## Overview

Docker Sandboxes (sbx) provide microVM-based isolation for AI coding agents via Docker Desktop. Each sandbox runs inside its own lightweight microVM rather than sharing the host kernel, providing stronger isolation than standard Docker containers.

## Prerequisites

1. **Docker Desktop** with sbx support. The `sbx` CLI ships with Docker Desktop.

2. **Sign in to Docker:**

   ```bash
   sbx login
   ```

3. **Set a default network policy:**

   ```bash
   sbx policy set-default <policy>
   ```

   | Policy | Description |
   |--------|-------------|
   | `allow-all` | All outbound traffic allowed. |
   | `balanced` | Common dev traffic allowed (AI services, package registries). |
   | `deny-all` | All outbound traffic blocked. |

   `balanced` is a sensible default.

4. **Store API secrets** for each agent you plan to use. The sbx proxy injects secrets at runtime; credentials never enter the microVM directly.

   | Agent | Service | Command |
   |-------|---------|---------|
   | Claude Code | anthropic | `sbx secret set -g anthropic` |
   | Codex | openai | `sbx secret set -g openai` |
   | Gemini CLI | google | `sbx secret set -g google` |
   | Hermes | mistral | `sbx secret set -g mistral` |
   | droid | droid | `sbx secret set -g droid` |

   For multi-provider agents (OpenCode, Pi, Qwen Code, Kiro, Mistral Vibe), set the secret for whichever provider you use. Run `sbx secret set` without arguments for interactive mode.

### Verify Installation

```bash
sbx version
sbx ls
```

## Configuration

**Linux:** `~/.config/agent-of-empires/config.toml`
**macOS:** `~/.agent-of-empires/config.toml`

```toml
[sandbox]
container_runtime = "sbx"
```

### Profile-Specific Runtime

```toml
[profiles.sbx]
sandbox.container_runtime = "sbx"
```

Then use it with: `aoe add --profile sbx .`

You can also pick the runtime in the TUI under **Sandbox > Container Runtime**.

## Usage

Once configured, usage is identical to the Docker sandbox:

```bash
aoe add --sandbox .
aoe add --sandbox --sandbox-image my-custom-image:latest .
```

## Important Notes

### Disk Usage

Each sandbox is a full microVM with its own filesystem. Expect roughly ImageSize x ActiveSessions of host disk. Stopped sandboxes still consume disk; remove them with `aoe remove`.

### Kit Format (Experimental)

AoE ships an embedded sbx kit that configures each sandbox. The kit format is experimental per Docker; schema changes in future Docker Desktop updates may require an AoE update.

### Upgrade Path

sbx updates via Docker Desktop. Update Docker Desktop through its normal mechanism.

> **Warning:** Do NOT run `sbx reset` to troubleshoot. This destroys all sandboxes, cached data, and stored secrets.

### Volume Mount Limitations

sbx does not support read-only (`:ro`) volume mounts or anonymous volumes. The `volume_ignores` setting has no effect. Convenience mounts whose host and container paths differ (like `.gitconfig`) are silently dropped; AoE logs a warning when this happens.

### SSH Agent Forwarding

SSH access inside sbx sandboxes uses agent forwarding via the sbx proxy (`SSH_AUTH_SOCK`), not volume-mounted keys. The `mount_ssh` config option has no effect for sbx; the SSH mount is dropped because sbx cannot map arbitrary host paths into the microVM.

### Port Publishing

Ports are published after sandbox creation, not at create time. AoE handles this automatically; failures surface as warnings without rolling back the sandbox.

## Troubleshooting

### "Not logged in"

Run `sbx login` to re-authenticate.

### "Policy not configured"

```bash
sbx policy set-default balanced
```

### Sandbox creation fails after Docker Desktop update

The kit format may have changed. Check for an AoE update.

### Debug Logging

```bash
AGENT_OF_EMPIRES_DEBUG=1 aoe add --sandbox .
```

Log location:

- **Linux:** `~/.config/agent-of-empires/debug.log`
- **macOS:** `~/.agent-of-empires/debug.log`
