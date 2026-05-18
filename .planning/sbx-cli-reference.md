# sbx CLI Reference

> **Docker Sandboxes** — Manage AI coding agent sandboxes.  
> Source: https://docs.docker.com/reference/cli/sbx/

Docker Sandboxes creates isolated sandbox environments for AI agents, powered by Docker. Run without a command to launch interactive mode, or pass a command for CLI usage.

---

## Global Options

| Option | Default | Description |
|--------|---------|-------------|
| `-D, --debug` | | Enable debug logging |

---

## Command Index

| Command | Description |
|---------|-------------|
| [`sbx completion`](#sbx-completion) | Generate the autocompletion script for the specified shell |
| [`sbx cp`](#sbx-cp) | Copy files or directories between a sandbox and the host |
| [`sbx create`](#sbx-create) | Create a sandbox for an agent |
| [`sbx diagnose`](#sbx-diagnose) | Diagnose common issues with your sbx installation |
| [`sbx exec`](#sbx-exec) | Execute a command inside a sandbox |
| [`sbx login`](#sbx-login) | Sign in to Docker |
| [`sbx logout`](#sbx-logout) | Sign out of Docker |
| [`sbx ls`](#sbx-ls) | List sandboxes |
| [`sbx policy`](#sbx-policy) | Manage sandbox policies |
| [`sbx ports`](#sbx-ports) | Manage sandbox port publishing |
| [`sbx reset`](#sbx-reset) | Reset all sandboxes and clean up state |
| [`sbx rm`](#sbx-rm) | Remove one or more sandboxes |
| [`sbx run`](#sbx-run) | Run an agent in a sandbox |
| [`sbx secret`](#sbx-secret) | Manage stored secrets |
| [`sbx stop`](#sbx-stop) | Stop one or more sandboxes without removing them |
| [`sbx template`](#sbx-template) | Manage sandbox templates |
| [`sbx version`](#sbx-version) | Show Docker Sandboxes version information |

---

## sbx completion

Generate the autocompletion script for the specified shell.

**Usage:** `sbx`

Generate the autocompletion script for `sbx` for the specified shell. See each sub-command's help for details on how to use the generated script.

### Subcommands

| Command | Description |
|---------|-------------|
| [`sbx completion bash`](#sbx-completion-bash) | Generate the autocompletion script for bash |
| [`sbx completion fish`](#sbx-completion-fish) | Generate the autocompletion script for fish |
| [`sbx completion powershell`](#sbx-completion-powershell) | Generate the autocompletion script for powershell |
| [`sbx completion zsh`](#sbx-completion-zsh) | Generate the autocompletion script for zsh |

---

### sbx completion bash

Generate the autocompletion script for bash.

**Usage:** `sbx completion bash`

Generate the autocompletion script for the bash shell. This script depends on the `bash-completion` package. If it is not installed already, you can install it via your OS's package manager.

**Load completions in your current shell session:**
```bash
source <(sbx completion bash)
```

**Load completions for every new session, execute once:**

Linux:
```bash
sbx completion bash > /etc/bash_completion.d/sbx
```

macOS:
```bash
sbx completion bash > $(brew --prefix)/etc/bash_completion.d/sbx
```

You will need to start a new shell for this setup to take effect.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--no-descriptions` | | Disable completion descriptions |

---

### sbx completion fish

Generate the autocompletion script for fish.

**Usage:** `sbx completion fish [flags]`

Generate the autocompletion script for the fish shell.

**Load completions in your current shell session:**
```fish
sbx completion fish | source
```

**Load completions for every new session, execute once:**
```fish
sbx completion fish > ~/.config/fish/completions/sbx.fish
```

You will need to start a new shell for this setup to take effect.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--no-descriptions` | | Disable completion descriptions |

---

### sbx completion powershell

Generate the autocompletion script for powershell.

**Usage:** `sbx completion powershell [flags]`

Generate the autocompletion script for powershell.

**Load completions in your current shell session:**
```powershell
sbx completion powershell | Out-String | Invoke-Expression
```

**Load completions for every new session:** Add the output of the above command to your powershell profile.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--no-descriptions` | | Disable completion descriptions |

---

### sbx completion zsh

Generate the autocompletion script for zsh.

**Usage:** `sbx completion zsh [flags]`

Generate the autocompletion script for the zsh shell. If shell completion is not already enabled in your environment you will need to enable it. You can execute the following once:

```zsh
echo "autoload -U compinit; compinit" >> ~/.zshrc
```

**Load completions in your current shell session:**
```zsh
source <(sbx completion zsh)
```

**Load completions for every new session, execute once:**

Linux:
```zsh
sbx completion zsh > "${fpath[1]}/_sbx"
```

macOS:
```zsh
sbx completion zsh > $(brew --prefix)/share/zsh/site-functions/_sbx
```

You will need to start a new shell for this setup to take effect.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--no-descriptions` | | Disable completion descriptions |

---

## sbx cp

Copy files or directories between a sandbox and the host.

**Usage:** `sbx cp [flags] SRC DST`

Either `SRC` or `DST` must be a sandbox path, written as `SANDBOX:PATH`. The other must be a local path. Copying between two sandboxes is not supported.

When copying a directory, the directory itself is placed at the destination. If the destination path does not exist it is created; if it already exists as a directory, the source is placed inside it.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-L, --follow-link` | | Follow symbolic links in the source path when copying from host to sandbox |

**Examples:**
```bash
# Copy a file from host to sandbox
sbx cp ./config.json my-sandbox:/home/user/

# Copy a file from sandbox to host
sbx cp my-sandbox:/home/user/output.log ./

# Copy a directory
sbx cp ./src/ my-sandbox:/home/user/src
```

---

## sbx create

Create a sandbox for an agent.

**Usage:** `sbx create [flags] AGENT PATH [PATH...]`

Create a sandbox with access to a host workspace for an agent. Use `sbx run SANDBOX` to attach to the agent after creation.

### Subcommands

| Command | Description |
|---------|-------------|
| [`sbx create claude`](#sbx-create-claude) | Create a sandbox for claude |
| [`sbx create codex`](#sbx-create-codex) | Create a sandbox for codex |
| [`sbx create copilot`](#sbx-create-copilot) | Create a sandbox for copilot |
| [`sbx create cursor`](#sbx-create-cursor) | Create a sandbox for cursor |
| [`sbx create docker-agent`](#sbx-create-docker-agent) | Create a sandbox for docker-agent |
| [`sbx create droid`](#sbx-create-droid) | Create a sandbox for droid |
| [`sbx create gemini`](#sbx-create-gemini) | Create a sandbox for gemini |
| [`sbx create kiro`](#sbx-create-kiro) | Create a sandbox for kiro |
| [`sbx create opencode`](#sbx-create-opencode) | Create a sandbox for opencode |
| [`sbx create shell`](#sbx-create-shell) | Create a sandbox for shell |

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--branch` | | Create a Git worktree on the given branch |
| `--cpus` | `0` | Number of CPUs to allocate to the sandbox (0 = auto: N-1 host CPUs, min 1) |
| `--kit` | | Kit reference (directory, ZIP, or OCI). Can be specified multiple times |
| `-m, --memory` | | Memory limit in binary units (e.g., `1024m`, `8g`). Default: 50% of host memory, max 32 GiB |
| `--name` | | Name for the sandbox (default: `<agent>-<workdir>`, letters, numbers, hyphens, periods, plus signs and minus signs only) |
| `-q, --quiet` | | Suppress verbose output |
| `-t, --template` | | Container image to use for the sandbox (default: agent-specific image) |

**Examples:**
```bash
# Create a sandbox for Claude in the current directory
sbx create claude .

# Create a sandbox with a custom name
sbx create --name my-project claude /path/to/project

# Create with additional read-only workspaces
sbx create claude . /path/to/docs:ro

# Create with a Git worktree for isolated changes
sbx create --branch=feature/login claude .
```

---

### sbx create claude

Create a sandbox for claude.

**Usage:** `sbx create claude PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for claude. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
# Create in the current directory
sbx create claude .

# Create with a specific path
sbx create claude /path/to/project

# Create with additional read-only workspaces
sbx create claude . /path/to/docs:ro
```

---

### sbx create codex

Create a sandbox for codex.

**Usage:** `sbx create codex PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for codex. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create codex .
sbx create codex /path/to/project
sbx create codex . /path/to/docs:ro
```

---

### sbx create copilot

Create a sandbox for copilot.

**Usage:** `sbx create copilot PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for copilot. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create copilot .
sbx create copilot /path/to/project
sbx create copilot . /path/to/docs:ro
```

---

### sbx create cursor

Create a sandbox for cursor.

**Usage:** `sbx create cursor PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for cursor. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create cursor .
sbx create cursor /path/to/project
sbx create cursor . /path/to/docs:ro
```

---

### sbx create docker-agent

Create a sandbox for docker-agent.

**Usage:** `sbx create docker-agent PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for docker-agent. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create docker-agent .
sbx create docker-agent /path/to/project
sbx create docker-agent . /path/to/docs:ro
```

---

### sbx create droid

Create a sandbox for droid.

**Usage:** `sbx create droid PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for droid. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create droid .
sbx create droid /path/to/project
sbx create droid . /path/to/docs:ro
```

---

### sbx create gemini

Create a sandbox for gemini.

**Usage:** `sbx create gemini PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for gemini. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create gemini .
sbx create gemini /path/to/project
sbx create gemini . /path/to/docs:ro
```

---

### sbx create kiro

Create a sandbox for kiro.

**Usage:** `sbx create kiro PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for kiro. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create kiro .
sbx create kiro /path/to/project
sbx create kiro . /path/to/docs:ro
```

---

### sbx create opencode

Create a sandbox for opencode.

**Usage:** `sbx create opencode PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for opencode. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create opencode .
sbx create opencode /path/to/project
sbx create opencode . /path/to/docs:ro
```

---

### sbx create shell

Create a sandbox for shell.

**Usage:** `sbx create shell PATH [PATH...] [flags]`

Create a sandbox with access to a host workspace for shell. The workspace path is required and will be mounted inside the sandbox at the same path as on the host. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

Use `sbx run SANDBOX` to attach to the agent after creation.

**Options:** *(same as `sbx create` — see above)*

**Examples:**
```bash
sbx create shell .
sbx create shell /path/to/project
sbx create shell . /path/to/docs:ro
```

---

## sbx diagnose

Diagnose common issues with your sbx installation.

**Usage:** `sbx diagnose`

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-o, --output` | | Output format: `"json"` or `"github-issue"` |
| `--upload` | | Upload diagnostics to Docker support |

---

## sbx exec

Execute a command inside a sandbox.

**Usage:** `sbx exec [flags] SANDBOX COMMAND [ARG...]`

Execute a command in a sandbox. If the sandbox is stopped, it is started first. Flags match the behavior of `docker exec`.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-d, --detach` | | Detached mode: run command in the background |
| `--detach-keys` | | Override the key sequence for detaching a container |
| `-e, --env` | | Set environment variables |
| `--env-file` | | Read in a file of environment variables |
| `-i, --interactive` | | Keep STDIN open even if not attached |
| `--privileged` | | Give extended privileges to the command |
| `-t, --tty` | | Allocate a pseudo-TTY |
| `-u, --user` | | Username or UID (format: `<name|uid>[:<group|gid>]`) |
| `-w, --workdir` | | Working directory inside the container |

**Examples:**
```bash
# Open a shell inside a sandbox
sbx exec -it my-sandbox bash

# Run a command in the background
sbx exec -d my-sandbox npm start

# Run as root
sbx exec -u root my-sandbox apt-get update
```

---

## sbx login

Sign in to Docker.

**Usage:** `sbx login [flags]`

---

## sbx logout

Sign out of Docker.

**Usage:** `sbx logout [flags]`

---

## sbx ls

List sandboxes.

**Usage:** `sbx ls [flags]`

List all sandboxes with their agent, status, published ports, and workspace.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--json` | | Output in JSON format |
| `-q, --quiet` | | Only display sandbox names |

---

## sbx policy

Manage sandbox policies.

**Usage:** `sbx policy COMMAND`

Manage persistent access policies for sandboxes. Policies are rules stored locally that control what sandboxes can access. They apply globally across all sandboxes and persist across restarts.

### Subcommands

| Command | Description |
|---------|-------------|
| [`sbx policy allow`](#sbx-policy-allow) | Add an allow policy for sandboxes |
| [`sbx policy deny`](#sbx-policy-deny) | Add a deny policy for sandboxes |
| [`sbx policy log`](#sbx-policy-log) | Show sandbox policy logs |
| [`sbx policy ls`](#sbx-policy-ls) | List sandbox policies |
| [`sbx policy reset`](#sbx-policy-reset) | Reset policies to defaults |
| [`sbx policy rm`](#sbx-policy-rm) | Remove a policy |
| [`sbx policy set-default`](#sbx-policy-set-default) | Set the default network policy |

---

### sbx policy allow

Add an allow policy for sandboxes.

**Usage:** `sbx policy allow COMMAND`

Add a policy that permits sandboxes to access specified resources. Allowed resources are accessible to all sandboxes. If a resource matches both an allow and a deny rule, the deny rule takes precedence.

| Command | Description |
|---------|-------------|
| [`sbx policy allow network`](#sbx-policy-allow-network) | Allow network access to specified hosts |

---

### sbx policy allow network

Allow network access to specified hosts.

**Usage:** `sbx policy allow network [-g | SANDBOX] RESOURCES [flags]`

Allow sandbox network access to the specified hosts.

`RESOURCES` is a comma-separated list of hostnames, domains, or IP addresses. Supports:
- Exact domains: `example.com`
- Wildcard subdomains: `*.example.com`
- Optional port suffixes: `example.com:443`
- Use `"**"` to allow all hosts.

Use `-g/--global` to apply the rule globally to all sandboxes, or provide `SANDBOX` before `RESOURCES` to scope the rule to a specific sandbox.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-g, --global` | | Apply the rule globally to all sandboxes |

**Examples:**
```bash
# Allow access to a single host globally
sbx policy allow network -g api.example.com

# Allow access to multiple hosts globally
sbx policy allow network -g "api.example.com,cdn.example.com"

# Allow a host only for a specific sandbox
sbx policy allow network my-sandbox api.example.com

# Allow all subdomains of a host
sbx policy allow network -g "*.npmjs.org"

# Allow all outbound traffic globally
sbx policy allow network -g "**"
```

---

### sbx policy deny

Add a deny policy for sandboxes.

**Usage:** `sbx policy deny COMMAND`

Add a policy that blocks sandboxes from accessing specified resources. Deny rules always take precedence over allow rules. If a resource matches both an allow and a deny rule, the request is blocked.

| Command | Description |
|---------|-------------|
| [`sbx policy deny network`](#sbx-policy-deny-network) | Deny network access to specified hosts |

---

### sbx policy deny network

Deny network access to specified hosts.

**Usage:** `sbx policy deny network [-g | SANDBOX] RESOURCES [flags]`

Block sandbox network access to the specified hosts. `RESOURCES` is a comma-separated list of hostnames, domains, or IP addresses. Deny rules always take precedence over allow rules.

Use `-g/--global` to apply the rule globally to all sandboxes, or provide `SANDBOX` before `RESOURCES` to scope the rule to a specific sandbox.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-g, --global` | | Apply the rule globally to all sandboxes |

**Examples:**
```bash
# Block access to a host globally
sbx policy deny network -g ads.example.com

# Block a host only for a specific sandbox
sbx policy deny network my-sandbox ads.example.com

# Block all outbound traffic globally
sbx policy deny network -g "**"
```

---

### sbx policy log

Show sandbox policy logs.

**Usage:** `sbx policy log [SANDBOX] [flags]`

Show policy logs for all sandboxes, or filter by a specific sandbox name. Displays which hosts were allowed or blocked by the proxy, along with the matching rule, proxy type, and request count. Useful for debugging connectivity issues or auditing network activity.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--json` | | Output in JSON format |
| `--limit` | `0` | Maximum number of log entries to show |
| `-q, --quiet` | | Only display log entries |
| `--type` | `all` | Filter logs by type: `"all"` or `"network"` (default: `"all"`) |

**Examples:**
```bash
# Show all policy logs
sbx policy log

# Show logs for a specific sandbox
sbx policy log my-sandbox

# Output in JSON format
sbx policy log --json

# Show the last 20 entries
sbx policy log --limit 20
```

---

### sbx policy ls

List sandbox policies.

**Usage:** `sbx policy ls [SANDBOX] [flags]`

List all active policies. Displays the policy name (or ID if no name is set), type, decision (allow/deny), and the associated resources for each rule.

When `SANDBOX` is specified, only policies that apply to that sandbox are shown (global rules plus rules scoped to that sandbox).

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--type` | `all` | Filter policies by type: `"all"` or `"network"` (default: `"all"`) |

**Examples:**
```bash
# List all policies
sbx policy ls

# List only network policies
sbx policy ls --type network

# List policies that apply to a specific sandbox
sbx policy ls my-sandbox
```

---

### sbx policy reset

Reset policies to defaults.

**Usage:** `sbx policy reset [flags]`

Remove all custom policies and restart the daemon to restore defaults. This deletes the local policy store and stops the daemon. When the daemon restarts (automatically on next command), the default policy is installed.

If sandboxes are currently running, they will be stopped when the daemon shuts down. You will be prompted for confirmation unless `--force` is used.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-f, --force` | | Skip confirmation prompt |

**Examples:**
```bash
# Reset policies (prompts if sandboxes are running)
sbx policy reset

# Reset policies without confirmation
sbx policy reset --force
```

---

### sbx policy rm

Remove a policy.

**Usage:** `sbx policy rm COMMAND`

Remove a previously added allow or deny policy.

| Command | Description |
|---------|-------------|
| [`sbx policy rm network`](#sbx-policy-rm-network) | Remove a network policy |

---

### sbx policy rm network

Remove a network policy.

**Usage:** `sbx policy rm network [-g | SANDBOX] [flags]`

Remove a network policy by rule ID, resource, or both. Use `-g/--global` to remove from the global policy, or provide `SANDBOX` to remove from a sandbox-scoped policy. Use `sbx policy ls` to see active policies and their IDs/resources.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-g, --global` | | Remove from the global policy |
| `--id` | | Remove by rule ID |
| `--resource` | | Remove by resource value(s), comma-separated |

**Examples:**
```bash
# List policies to find the ID or resource to remove
sbx policy ls

# Remove a global rule by resource
sbx policy rm network -g --resource api.example.com

# Remove a global rule by ID
sbx policy rm network -g --id 2d3c1f0e-4a73-4e05-bc9d-f2f9a4b50d67

# Remove a sandbox-scoped rule by resource
sbx policy rm network my-sandbox --resource api.example.com
```

---

### sbx policy set-default

Set the default network policy.

**Usage:** `sbx policy set-default <allow-all|balanced|deny-all> [flags]`

Set the default network policy for all sandboxes. This must be run before adding custom allow/deny rules or starting a sandbox for the first time. The default policy determines the baseline network access.

**Available policies:**

| Policy | Description |
|--------|-------------|
| `allow-all` | All outbound network traffic is allowed |
| `balanced` | Common dev traffic allowed (AI services, package registries, etc.) |
| `deny-all` | All outbound network traffic is blocked |

After setting defaults, use `sbx policy allow/deny` to add custom rules. Use `sbx policy reset` to clear all policies and start over.

**Examples:**
```bash
# Set balanced defaults (recommended)
sbx policy set-default balanced

# Allow all traffic
sbx policy set-default allow-all

# Block everything, then allow specific sites
sbx policy set-default deny-all
sbx policy allow network -g api.example.com:443
```

---

## sbx ports

Manage sandbox port publishing.

**Usage:** `sbx ports SANDBOX [flags]`

List, publish, or unpublish ports for a running sandbox. Without `--publish` or `--unpublish` flags, lists all published ports.

**Port spec format:** `[[HOST_IP:]HOST_PORT:]SANDBOX_PORT[/PROTOCOL]`

- If `HOST_PORT` is omitted, an ephemeral port is allocated automatically.
- `HOST_IP` defaults to `127.0.0.1`
- `PROTOCOL` defaults to `tcp`
- Supported protocols: `tcp`, `tcp4`, `tcp6`, `udp`, `udp4`, `udp6`

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--json` | | Output in JSON format (for port listing) |
| `--publish` | | Publish a port (can be repeated): `[[HOST_IP:]HOST_PORT:]SANDBOX_PORT[/PROTOCOL]` |
| `--unpublish` | | Unpublish a port (can be repeated): `[HOST_IP:]HOST_PORT:SANDBOX_PORT[/PROTOCOL]` |

**Examples:**
```bash
# List published ports
sbx ports my-sandbox

# Publish sandbox port 8080 to an ephemeral host port
sbx ports my-sandbox --publish 8080

# Publish with a specific host port
sbx ports my-sandbox --publish 3000:8080

# Unpublish a port
sbx ports my-sandbox --unpublish 3000:8080
```

---

## sbx reset

Reset all sandboxes and clean up state.

**Usage:** `sbx reset [flags]`

Reset Docker Sandboxes to a freshly-installed state. This command will:

- Stop all running sandboxes gracefully (30s timeout)
- Clear image cache
- Clear all internal registries
- Delete all sandbox state
- Remove all policies
- Delete all stored secrets
- Sign out of Docker Sandboxes
- Stop the daemon
- Remove all state, cache, and config directories

> **WARNING:** This is destructive and cannot be undone. Running agents will be terminated and their work lost. Cached images will be deleted and recreated on next use. Stored secrets will need to be re-entered.

Use `--preserve-secrets` to keep stored secrets. By default, you will be prompted to confirm (`y/N`). Use `--force` to skip the confirmation prompt.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-f, --force` | | Skip confirmation prompt |
| `--preserve-secrets` | | Keep stored secrets |

---

## sbx rm

Remove one or more sandboxes.

**Usage:** `sbx rm [SANDBOX...] [flags]`

Remove one or more sandboxes and all associated resources. Stops running sandboxes, removes their containers, cleans up any Git worktrees, and deletes sandbox state. This action cannot be undone.

Use `--all` to remove every sandbox (requires confirmation). Use `--force` to skip confirmation prompts (for non-interactive scripts).

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--all` | | Remove all sandboxes |
| `-f, --force` | | Skip confirmation prompts |

---

## sbx run

Run an agent in a sandbox.

**Usage:** `sbx run [flags] SANDBOX | AGENT [PATH...] [-- AGENT_ARGS...]`

Run an agent in a sandbox, creating the sandbox if it does not already exist. Pass agent arguments after the `--` separator. Additional workspaces can be provided as extra arguments. Append `:ro` to mount them read-only.

To create a sandbox without attaching, use `sbx create` instead.

**Available agents:** `claude`, `codex`, `copilot`, `cursor`, `docker-agent`, `droid`, `gemini`, `kiro`, `opencode`, `shell`

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--branch` | | Create a Git worktree on the given branch (use `--branch auto` to auto-generate) |
| `--cpus` | `0` | Number of CPUs to allocate to the sandbox (0 = auto: N-1 host CPUs, min 1) |
| `--kit` | | Kit reference (directory, ZIP, or OCI). Can be specified multiple times |
| `-m, --memory` | | Memory limit in binary units (e.g., `1024m`, `8g`). Default: 50% of host memory, max 32 GiB |
| `--name` | | Name for the sandbox (default: `<agent>-<workdir>`) |
| `-t, --template` | | Container image to use for the sandbox (default: agent-specific image) |

**Examples:**
```bash
# Create and run a sandbox with claude in current directory
sbx run claude

# Create and run with additional workspaces (read-only)
sbx run claude . /path/to/docs:ro

# Run an existing sandbox
sbx run existing-sandbox

# Run a sandbox with agent arguments
sbx run claude -- --continue
```

---

## sbx secret

Manage stored secrets.

**Usage:** `sbx secret COMMAND`

Manage stored secrets for sandbox environments. Secrets are stored per service name (e.g., `"github"`, `"anthropic"`, `"openai"`). When a sandbox starts, the proxy uses stored secrets to authenticate API requests on behalf of the agent. The secret is never exposed directly to the agent.

Secrets can be scoped globally (shared across all sandboxes) or to a specific sandbox.

### Subcommands

| Command | Description |
|---------|-------------|
| [`sbx secret ls`](#sbx-secret-ls) | List stored secrets |
| [`sbx secret rm`](#sbx-secret-rm) | Remove a secret |
| [`sbx secret set`](#sbx-secret-set) | Create or update a secret |

---

### sbx secret ls

List stored secrets.

**Usage:** `sbx secret ls [sandbox] [OPTIONS] [flags]`

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-g, --global` | | Only list global secrets |
| `--service` | | Filter by secret service name |

**Examples:**
```bash
# List all secrets
sbx secret ls

# List only global secrets
sbx secret ls -g

# List secrets for a specific sandbox
sbx secret ls my-sandbox

# Filter by service
sbx secret ls --service github
```

---

### sbx secret rm

Remove a secret.

**Usage:** `sbx secret rm [-g | sandbox] [service] [flags]`

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-f, --force` | | Delete without confirmation prompt |
| `-g, --global` | | Use global secret scope |

**Examples:**
```bash
# Remove a global secret
sbx secret rm -g github

# Remove a sandbox-scoped secret
sbx secret rm my-sandbox openai

# Remove without confirmation prompt
sbx secret rm -g github -f

# Remove OpenAI or Anthropic credential(s) from global scope (OAuth and/or API key)
sbx secret rm -g openai
sbx secret rm -g anthropic
```

---

### sbx secret set

Create or update a secret.

**Usage:** `sbx secret set [-g | sandbox] [service] [flags]`

Create or update a secret for a service. When no arguments are provided, an interactive prompt guides you through scope and service selection.

**Available services:** `anthropic`, `aws`, `cursor`, `droid`, `github`, `google`, `groq`, `mistral`, `nebius`, `openai`, `xai`

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-f, --force` | | Overwrite an existing secret when `--token` is used |
| `-g, --global` | | Use global secret scope |
| `--oauth` | | Start OAuth flow and store OAuth tokens (openai/global only) |
| `-t, --token` | | Secret value (less secure: visible in shell history) |

**Examples:**
```bash
# Store a GitHub token globally (available to all sandboxes)
sbx secret set -g github

# Store an OpenAI key for a specific sandbox
sbx secret set my-sandbox openai

# Non-interactive via stdin (e.g., from a secret manager or env var)
echo "$ANTHROPIC_API_KEY" | sbx secret set -g anthropic

# Start OpenAI OAuth flow and store global OAuth tokens
sbx secret set -g openai --oauth
```

---

## sbx stop

Stop one or more sandboxes without removing them.

**Usage:** `sbx stop SANDBOX [SANDBOX...]`

Stop one or more running sandboxes without removing them. Stopped sandboxes retain their state and can be restarted with `sbx run`.

---

## sbx template

Manage sandbox templates.

**Usage:** `sbx template COMMAND`

Manage sandbox templates. Templates are saved snapshots of sandboxes that can be reused to create new sandboxes with:

```bash
sbx run -t TAG AGENT [WORKSPACE]
```

### Subcommands

| Command | Description |
|---------|-------------|
| [`sbx template load`](#sbx-template-load) | Load an image from a tar file into the sandbox runtime |
| [`sbx template ls`](#sbx-template-ls) | List template images |
| [`sbx template rm`](#sbx-template-rm) | Remove a template image |
| [`sbx template save`](#sbx-template-save) | Save a snapshot of the sandbox as a template |

---

### sbx template load

Load an image from a tar file into the sandbox runtime.

**Usage:** `sbx template load FILE [flags]`

Load an image from a tar file into the sandbox runtime's image store. The loaded image can be used as a template for new sandboxes. Tar files are typically created with:

```bash
sbx template save SANDBOX TAG --output FILE
```

**Examples:**
```bash
# Load an image from a tar file
sbx template load /tmp/myimage.tar            # Linux/macOS
sbx template load C:\Users\me\myimage.tar     # Windows

# Use the loaded image as a template
sbx run -t myimage:v1.0 claude
```

---

### sbx template ls

List template images.

**Usage:** `sbx template ls [flags]`

List all template images stored in the sandbox runtime's image store.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--json` | | Output in JSON format |

**Examples:**
```bash
# List all template images
sbx template ls

# Output in JSON format
sbx template ls --json
```

---

### sbx template rm

Remove a template image.

**Usage:** `sbx template rm TAG|ID [flags]`

Remove a template image from the sandbox runtime's image store. The image can be identified by tag (e.g. `"myimage:v1.0"`) or by image ID (full or prefix, e.g. `"abc123"`). Use `sbx template ls` to see available images and their IDs.

**Examples:**
```bash
# Remove by tag
sbx template rm myimage:v1.0

# Remove by image ID (prefix)
sbx template rm abc123
```

---

### sbx template save

Save a snapshot of the sandbox as a template.

**Usage:** `sbx template save SANDBOX TAG [flags]`

Save a snapshot of the sandbox as a template. The saved image is stored in the sandbox runtime's image store and can be used as a template for new sandboxes with:

```bash
sbx run -t TAG AGENT [WORKSPACE]
```

Use `--output` to also export the image to a tar file that can be shared and loaded on another host with `sbx template load FILE`.

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-o, --output` | | Also export the image to a tar file |

**Examples:**
```bash
# Save as a template for new sandboxes on this host
sbx template save my-sandbox myimage:v1.0

# Also export to a shareable tar file
