# Docker Sandboxes (sbx)

## Overview

> **Early Access.** Docker Sandboxes are in Early Access. The sbx CLI, kit format, and behavior may change between releases.

AoE supports [Docker Sandboxes (sbx)](https://docs.docker.com/sandbox/) as a sandbox runtime. Each sandbox runs inside its own microVM rather than sharing the host kernel, providing stronger isolation than standard Docker containers.

## Prerequisites

1. **sbx installed** and available on your `PATH`. See the [sbx documentation](https://docs.docker.com/sandbox/) for installation.

2. **Signed in and configured.** AoE checks sbx health at startup and will tell you what to do if sbx is not ready. The two common setup steps are `sbx login` and `sbx policy set-default`.

3. **API secrets stored** for the agents you use. sbx injects secrets via its proxy; run `sbx secret set` for interactive setup, or `sbx secret set -g <service>` for a specific service (e.g., `sbx secret set -g anthropic` for Claude Code).

### Verify Installation

```bash
sbx version
sbx ls
```

## Configuration

Update your config file to use sbx as the sandbox runtime.

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

## Compatibility Notes

sbx sandboxes are microVMs, not containers. A few things work differently from Docker:

- **No read-only mounts.** The `:ro` volume flag is not supported.
- **No anonymous volumes.** The `volume_ignores` setting has no effect.
- **SSH agent forwarding** uses the sbx proxy (`SSH_AUTH_SOCK`), not volume-mounted keys. The `mount_ssh` setting has no effect; SSH works automatically if you have `sbx secret set` configured for GitHub.
- **Convenience mounts dropped.** Mounts whose host and container paths differ (like `.gitconfig`) are silently dropped. AoE logs a warning when this happens.
- **Port publishing** happens after sandbox creation, not at create time. AoE handles this automatically; failures surface as warnings.
- **Disk usage.** Each sandbox has its own filesystem. Stopped sandboxes still consume disk; remove them with `aoe remove`.

> **Warning:** Do NOT run `sbx reset` to troubleshoot. It destroys all sandboxes, cached data, and stored secrets.

## Troubleshooting

### AoE reports sbx as unavailable

AoE runs `sbx version` and `sbx ls` to check health. Common issues:

- **"Not logged in"**: run `sbx login`.
- **"Policy not configured"**: run `sbx policy set-default balanced`.

### Sandbox creation fails after sbx update

The sbx kit format is experimental; schema changes may require an AoE update.