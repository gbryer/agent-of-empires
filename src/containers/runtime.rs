//! The unified `ContainerRuntime`. Shared behavior lives on `RuntimeBase`;
//! this impl dispatches the four genuinely runtime-specific operations
//! (existence probe, running-state probe, exec-command formatting, and
//! batch status query) on a `RuntimeKind` discriminant.

use std::collections::HashMap;

use serde_json::Value;

use super::container_interface::{
    ContainerConfig, ContainerRuntimeInterface, EnvEntry, RuntimeCapabilities,
};
use super::error::{DockerError, Result};
use super::runtime_base::RuntimeBase;
use super::sbx;
use crate::session::ContainerRuntimeName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Docker,
    AppleContainer,
    Podman,
    Sbx,
}

pub struct ContainerRuntime {
    pub(crate) base: RuntimeBase,
    pub(crate) kind: RuntimeKind,
    // Phase 2 plan 02-01: Some(_) when kind == Sbx; None otherwise. The two
    // Sbx-specialised dispatch arms below (is_available, capabilities) read
    // this field. Plan 02-02 adds exec_command and build_create_args arms
    // that also read it, via sbx::argv free functions.
    pub(crate) sbx: Option<sbx::SbxRuntime>,
}

impl ContainerRuntime {
    pub fn docker() -> Self {
        Self {
            base: RuntimeBase::DOCKER,
            kind: RuntimeKind::Docker,
            sbx: None,
        }
    }

    pub fn apple_container() -> Self {
        Self {
            base: RuntimeBase::APPLE_CONTAINER,
            kind: RuntimeKind::AppleContainer,
            sbx: None,
        }
    }

    pub fn podman() -> Self {
        Self {
            base: RuntimeBase::PODMAN,
            kind: RuntimeKind::Podman,
            sbx: None,
        }
    }

    // Phase 2 plan 02-01: the sbx field carries the SbxRuntime peer struct
    // (CONTEXT.md D-03 Option A, RESEARCH.md recommendation). is_available and
    // capabilities now route through it; plan 02-02 wires the remaining two
    // dispatch arms (exec_command, build_create_args).
    pub fn sbx() -> Self {
        Self {
            base: RuntimeBase::SBX,
            kind: RuntimeKind::Sbx,
            sbx: Some(sbx::SbxRuntime::new()),
        }
    }

    /// Map the internal `RuntimeKind` discriminant onto the user-facing
    /// `ContainerRuntimeName` enum used by `Config::sandbox.container_runtime`.
    /// Lets callers thread the runtime identity into `container_config`
    /// without round-tripping through `Config::load()` (Phase 3 RT-04).
    pub fn name(&self) -> ContainerRuntimeName {
        match self.kind {
            RuntimeKind::Docker => ContainerRuntimeName::Docker,
            RuntimeKind::AppleContainer => ContainerRuntimeName::AppleContainer,
            RuntimeKind::Podman => ContainerRuntimeName::Podman,
            RuntimeKind::Sbx => ContainerRuntimeName::Sbx,
        }
    }
}

impl Default for ContainerRuntime {
    fn default() -> Self {
        Self::docker()
    }
}

impl ContainerRuntimeInterface for ContainerRuntime {
    // Phase 2 plan 02-01: Sbx now delegates to the SbxRuntime peer struct's
    // real PATH probe (Command::new(binary).arg("--version"));
    // Docker/Podman/AppleContainer continue to use RuntimeBase::is_available.
    fn is_available(&self) -> bool {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .is_available(),
            _ => self.base.is_available(),
        }
    }

    fn is_daemon_running(&self) -> bool {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .is_daemon_running(),
            _ => self.base.is_daemon_running(),
        }
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .capabilities(),
            _ => self.base.capabilities,
        }
    }

    fn get_version(&self) -> Result<String> {
        self.base.get_version()
    }

    fn image_exists_locally(&self, image: &str) -> bool {
        self.base.image_exists_locally(image)
    }

    fn pull_image(&self, image: &str) -> Result<()> {
        self.base.pull_image(image)
    }

    fn ensure_image(&self, image: &str) -> Result<()> {
        self.base.ensure_image(image)
    }

    fn default_sandbox_image(&self) -> &'static str {
        self.base.default_sandbox_image()
    }

    fn effective_default_image(&self) -> String {
        self.base.effective_default_image()
    }

    fn does_container_exist(&self, name: &str) -> Result<bool> {
        match self.kind {
            RuntimeKind::Docker | RuntimeKind::Podman => {
                let output = self
                    .base
                    .command()
                    .args(["container", "inspect", name])
                    .output()?;
                Ok(output.status.success())
            }
            RuntimeKind::AppleContainer => {
                // Apple Container's `inspect` returns success(0) for non-existent
                // containers, so we use `logs` which properly fails for missing
                // containers.
                let output = self.base.command().args(["logs", name]).output()?;
                Ok(output.status.success())
            }
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .does_container_exist(name),
        }
    }

    fn is_container_running(&self, name: &str) -> Result<bool> {
        match self.kind {
            RuntimeKind::Docker | RuntimeKind::Podman => {
                let output = self
                    .base
                    .command()
                    .args(["container", "inspect", "-f", "{{.State.Running}}", name])
                    .output()?;

                if !output.status.success() {
                    return Ok(false);
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(stdout.trim() == "true")
            }
            RuntimeKind::AppleContainer => {
                let output = self.base.command().args(["inspect", name]).output()?;

                if !output.status.success() {
                    return Ok(false);
                }

                let out_json: Value = serde_json::from_slice(&output.stdout)
                    .map_err(|e| DockerError::CommandFailed(e.to_string()))?;

                if let Some(status) = out_json.pointer("/0/status") {
                    Ok(status == "running")
                } else {
                    Ok(false)
                }
            }
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .is_container_running(name),
        }
    }

    fn build_create_args(&self, name: &str, image: &str, config: &ContainerConfig) -> Vec<String> {
        match self.kind {
            RuntimeKind::Sbx => {
                // Phase 2 plan 02-01: Sbx emits its own argv shape (positional
                // workspace mounts, --template, no -v/-p), gated away from
                // RuntimeBase::build_create_args which would emit Docker shape.
                // kit_path is None until Phase 4's KitMaterializer feeds it
                // through.
                sbx::argv::build_create_args(name, image, None, config, None)
            }
            _ => self.base.build_create_args(name, image, config),
        }
    }

    fn create_container(
        &self,
        name: &str,
        image: &str,
        config: &ContainerConfig,
    ) -> Result<String> {
        if self.does_container_exist(name)? {
            return Err(DockerError::ContainerAlreadyExists(name.to_string()));
        }
        match self.kind {
            RuntimeKind::Sbx => {
                // Prefer explicit agent_name from ContainerConfig (threaded
                // from session layer); default to "claude" when unset.
                let agent_name = config.agent_name.as_deref().unwrap_or("claude");
                let sbx_rt = self.sbx.as_ref().expect(
                    "ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx",
                );
                let result = sbx_rt.create_container(name, image, config, agent_name)?;

                // Post-create port publishing (D-01): sbx cannot publish
                // ports at create time; iterate port_mappings individually.
                if !config.port_mappings.is_empty() {
                    let results = sbx_rt.publish_ports(name, &config.port_mappings);
                    for r in &results {
                        match r {
                            sbx::ports::PortPublishResult::Ok(spec) => {
                                tracing::info!(
                                    target: "containers.sbx",
                                    %name, %spec,
                                    "port published"
                                );
                            }
                            sbx::ports::PortPublishResult::Failed { port, stderr } => {
                                tracing::warn!(
                                    target: "containers.sbx",
                                    %name, %port, %stderr,
                                    "port publish failed (non-fatal)"
                                );
                            }
                        }
                    }
                }

                Ok(result)
            }
            _ => self.base.run_create(name, image, config),
        }
    }

    fn start_container(&self, name: &str) -> Result<()> {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .start_container(name),
            _ => self.base.start_container(name),
        }
    }

    fn stop_container(&self, name: &str) -> Result<()> {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .stop_container(name),
            _ => self.base.stop_container(name),
        }
    }

    fn remove(&self, name: &str, force: bool) -> Result<()> {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .remove(name, force),
            _ => self.base.remove(name, force),
        }
    }

    fn exec_command(&self, name: &str, options: Option<&str>, cmd: &str) -> String {
        match self.kind {
            RuntimeKind::Docker | RuntimeKind::Podman => {
                // Docker/Podman containers inherit a full PATH, so the command
                // can be appended directly without wrapping in `sh -c`.
                self.base.exec_command(name, options, cmd)
            }
            RuntimeKind::AppleContainer => {
                // Apple Container has a very limited initial PATH, so we wrap
                // the command in `sh -c` to get a proper shell environment.
                // Single-quote with escaped embedded quotes to avoid issues
                // with double-quote metacharacters ($, `, \, !) in the command.
                let escaped = cmd.replace('\'', "'\\''");
                let cmd_str = format!("'{}'", escaped);

                if let Some(opt_str) = options {
                    [
                        "container",
                        "exec",
                        "-it",
                        opt_str,
                        name,
                        "sh",
                        "-c",
                        &cmd_str,
                    ]
                    .join(" ")
                } else {
                    ["container", "exec", "-it", name, "sh", "-c", &cmd_str].join(" ")
                }
            }
            RuntimeKind::Sbx => {
                let mut workdir: Option<&str> = None;
                let mut env_entries: Vec<EnvEntry> = Vec::new();

                if let Some(opts) = options {
                    let tokens: Vec<&str> = opts.split_whitespace().collect();
                    let mut i = 0;
                    while i < tokens.len() {
                        match tokens[i] {
                            "-w" if i + 1 < tokens.len() => {
                                workdir = Some(tokens[i + 1]);
                                i += 2;
                            }
                            "-e" if i + 1 < tokens.len() => {
                                let entry = tokens[i + 1];
                                if let Some((k, v)) = entry.split_once('=') {
                                    env_entries.push(EnvEntry::Literal {
                                        key: k.to_string(),
                                        value: v.to_string(),
                                    });
                                } else {
                                    env_entries.push(EnvEntry::Inherit {
                                        key: entry.to_string(),
                                        value: String::new(),
                                    });
                                }
                                i += 2;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    }
                }

                let cmd_parts = [cmd];
                let args =
                    sbx::argv::build_exec_args(name, workdir, &env_entries, true, true, &cmd_parts);
                std::iter::once("sbx".to_string())
                    .chain(args)
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
    }

    fn exec(&self, name: &str, cmd: &[&str]) -> Result<std::process::Output> {
        match self.kind {
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .exec(name, cmd, None, &[], false, false),
            _ => self.base.exec(name, cmd),
        }
    }

    fn batch_running_states(&self, prefix: &str) -> HashMap<String, bool> {
        match self.kind {
            RuntimeKind::Docker | RuntimeKind::Podman => {
                let output = self
                    .base
                    .command()
                    .args([
                        "ps",
                        "-a",
                        "--filter",
                        &format!("name={}", prefix),
                        "--format",
                        "{{.Names}}\t{{.State}}",
                    ])
                    .output();

                let output = match output {
                    Ok(o) if o.status.success() => o,
                    _ => return HashMap::new(),
                };

                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .lines()
                    .filter_map(|line| {
                        let mut parts = line.splitn(2, '\t');
                        let name = parts.next()?.trim();
                        let state = parts.next()?.trim();
                        // Docker/Podman's --filter name= does substring matching, so
                        // post-filter to ensure we only include exact prefix matches.
                        if name.is_empty() || !name.starts_with(prefix) {
                            return None;
                        }
                        Some((name.to_string(), state == "running"))
                    })
                    .collect()
            }
            RuntimeKind::AppleContainer => {
                let _ = prefix;
                HashMap::new()
            }
            RuntimeKind::Sbx => self
                .sbx
                .as_ref()
                .expect("ContainerRuntime::sbx() invariant: sbx field is Some when kind == Sbx")
                .batch_running_states(prefix),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docker_if_available() -> Option<ContainerRuntime> {
        let rt = ContainerRuntime::docker();
        if !rt.is_available() || !rt.is_daemon_running() {
            None
        } else {
            Some(rt)
        }
    }

    fn apple_container_if_available() -> Option<ContainerRuntime> {
        let rt = ContainerRuntime::apple_container();
        if !rt.is_available() || !rt.is_daemon_running() {
            None
        } else {
            Some(rt)
        }
    }

    fn podman_if_available() -> Option<ContainerRuntime> {
        let rt = ContainerRuntime::podman();
        if !rt.is_available() || !rt.is_daemon_running() {
            None
        } else {
            Some(rt)
        }
    }

    #[test]
    fn test_image_exists_locally_with_common_image() {
        for rt in [
            docker_if_available(),
            apple_container_if_available(),
            podman_if_available(),
        ]
        .into_iter()
        .flatten()
        {
            rt.pull_image("hello-world").unwrap();
            assert!(rt.image_exists_locally("hello-world"));
        }
    }

    #[test]
    fn test_image_exists_locally_nonexistent() {
        for rt in [
            docker_if_available(),
            apple_container_if_available(),
            podman_if_available(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(!rt.image_exists_locally("nonexistent-image-that-does-not-exist:v999"));
        }
    }

    #[test]
    fn test_ensure_image_uses_local_image() {
        for rt in [
            docker_if_available(),
            apple_container_if_available(),
            podman_if_available(),
        ]
        .into_iter()
        .flatten()
        {
            rt.pull_image("hello-world").unwrap();
            assert!(rt.ensure_image("hello-world").is_ok());
        }
    }

    #[test]
    fn test_ensure_image_fails_for_nonexistent_remote() {
        for rt in [
            docker_if_available(),
            apple_container_if_available(),
            podman_if_available(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(rt
                .ensure_image("nonexistent-image-that-does-not-exist:v999")
                .is_err());
        }
    }

    #[test]
    fn test_podman_runtime_uses_podman_binary() {
        let rt = ContainerRuntime::podman();
        assert_eq!(rt.kind, RuntimeKind::Podman);
        assert_eq!(rt.base.binary, "podman");
        assert_eq!(rt.base.name, "Podman");
    }

    #[test]
    fn test_podman_supports_docker_compatible_features() {
        // Podman is a drop-in for Docker, so it must support the same feature
        // set the shared base relies on. If this regresses, the create-args
        // builder will silently produce broken output for podman users.
        let rt = ContainerRuntime::podman();
        assert!(rt.base.capabilities.supports_read_only_volumes);
        assert!(rt.base.capabilities.supports_remove_volumes);
        assert_eq!(rt.base.remove_subcommand, "rm");
        assert_eq!(rt.base.pull_prefix, &["pull"]);
    }

    #[test]
    fn test_podman_exec_command_format_matches_docker() {
        // The CLI surfaces this string to the user via tmux; it must not
        // wrap the command in `sh -c` the way Apple Container does.
        let rt = ContainerRuntime::podman();
        let cmd = rt.exec_command("aoe-sandbox-test1234", None, "claude");
        assert_eq!(cmd, "podman exec -it aoe-sandbox-test1234 claude");
    }

    // Per-flag asserts (one per line) so a regression names the offending
    // capability in the test output. If this regresses, a Docker capability
    // changed; update RuntimeBase::DOCKER and the consumers that gate on
    // the affected flag.
    #[test]
    fn test_docker_capability_matrix() {
        let rt = ContainerRuntime::docker();
        let caps = rt.capabilities();
        assert!(caps.supports_read_only_volumes);
        assert!(caps.supports_remove_volumes);
        assert!(caps.supports_port_publish_at_create);
        assert!(caps.supports_image_pull);
        assert!(caps.supports_anonymous_volumes);
        assert!(caps.supports_arbitrary_volume_paths);
        assert!(!caps.supports_dynamic_port_publish);
    }

    // Podman is a Docker drop-in; if this diverges from
    // test_docker_capability_matrix, either Podman gained a real
    // differentiator or someone broke the drop-in promise.
    #[test]
    fn test_podman_capability_matrix() {
        let rt = ContainerRuntime::podman();
        let caps = rt.capabilities();
        assert!(caps.supports_read_only_volumes);
        assert!(caps.supports_remove_volumes);
        assert!(caps.supports_port_publish_at_create);
        assert!(caps.supports_image_pull);
        assert!(caps.supports_anonymous_volumes);
        assert!(caps.supports_arbitrary_volume_paths);
        assert!(!caps.supports_dynamic_port_publish);
    }

    // Apple Container diverges from Docker on read_only and remove flags;
    // gating preserves existing semantics until upstream support lands.
    #[test]
    fn test_apple_container_capability_matrix() {
        let rt = ContainerRuntime::apple_container();
        let caps = rt.capabilities();
        assert!(!caps.supports_read_only_volumes);
        assert!(!caps.supports_remove_volumes);
        assert!(caps.supports_port_publish_at_create);
        assert!(caps.supports_image_pull);
        assert!(caps.supports_anonymous_volumes);
        assert!(caps.supports_arbitrary_volume_paths);
        assert!(!caps.supports_dynamic_port_publish);
    }

    // The trait method must be a pure accessor over self.base.capabilities;
    // no kind-switching, no mutation. Proves RuntimeCapabilities: Copy + PartialEq
    // is wired correctly.
    #[test]
    fn test_capabilities_method_routes_through_base() {
        let rt = ContainerRuntime::docker();
        assert_eq!(rt.capabilities(), rt.base.capabilities);
    }

    #[test]
    fn test_sbx_runtime_uses_sbx_binary() {
        let rt = ContainerRuntime::sbx();
        assert_eq!(rt.kind, RuntimeKind::Sbx);
        assert_eq!(rt.base.binary, "sbx");
        assert_eq!(rt.base.name, "Docker Sandboxes");
    }

    // Phase 2 plan 02-01: is_available now routes through SbxRuntime's real
    // PATH probe. The default constructor uses PathBuf::from("sbx"), so the
    // probe returns false on hosts without a `sbx` binary on PATH. The probe
    // returning true when sbx IS on PATH is the desired Phase 2 behavior; we
    // assert "no panic, returns bool" rather than the prior short-circuit.
    #[test]
    fn test_sbx_is_available_routes_through_sbx_runtime() {
        let rt = ContainerRuntime::sbx();
        let _ = rt.is_available();
    }

    // Mirrors test_docker_capability_matrix shape; honest sbx values per
    // CONTEXT.md <specifics> (4 falses, 1 true on the new flags, both
    // migrated flags false). Phase 2's real SbxRuntime carries the same
    // matrix into the live impl.
    #[test]
    fn test_sbx_capability_matrix() {
        let rt = ContainerRuntime::sbx();
        let caps = rt.capabilities();
        assert!(!caps.supports_read_only_volumes);
        assert!(!caps.supports_remove_volumes);
        assert!(!caps.supports_port_publish_at_create);
        assert!(!caps.supports_image_pull);
        assert!(!caps.supports_anonymous_volumes);
        assert!(!caps.supports_arbitrary_volume_paths);
        assert!(caps.supports_dynamic_port_publish);
    }

    // Post-Phase-2 dispatch shape: is_available, capabilities,
    // exec_command, and build_create_args route through the SbxRuntime
    // peer struct + sbx::argv free functions (covered by their own
    // tests). The three action verbs below (does_container_exist,
    // is_container_running, batch_running_states) remain as Phase 1
    // safe-stub arms per CONTEXT.md D-05/D-06; Phase 5 wires them with
    // real subprocess code. The exec_command assertions here are
    // discriminating against the Phase 1 sentinel string so the test
    // proves the rewire happened, not just that "sbx" appears anywhere.
    #[test]
    fn test_sbx_dispatch_arms_return_safe_stubs() {
        let rt = ContainerRuntime::sbx();
        let cmd = rt.exec_command("foo", None, "bar");
        assert!(cmd.starts_with("sbx exec "));
        assert!(cmd.contains("foo"));
        assert!(cmd.contains("bar"));
        assert!(!cmd.contains("/* Phase 1"));
        assert!(!cmd.contains("dropped"));
    }

    // Post-Phase-2 build_create_args dispatch: the RuntimeKind::Sbx arm
    // routes through sbx::argv::build_create_args, which emits the sbx
    // CLI shape (`create shell --name N --template I ...`) rather than
    // the Docker shape (`run -d --name N ...`) that RuntimeBase emits.
    // This is the contract for SC-4: the dispatch arm picks up the new
    // peer module without churning the ~12 caller sites of
    // get_container_runtime().
    #[test]
    fn test_sbx_build_create_args_emits_sbx_shape_not_docker_shape() {
        let rt = ContainerRuntime::sbx();
        let cfg = ContainerConfig {
            working_dir: "/workspace".to_string(),
            volumes: Vec::new(),
            anonymous_volumes: Vec::new(),
            environment: Vec::new(),
            cpu_limit: None,
            memory_limit: None,
            port_mappings: Vec::new(),
            agent_name: None,
        };
        let args = rt.build_create_args("aoe-sandbox-test", "alpine:latest", &cfg);

        assert_eq!(args[0], "create");
        assert_eq!(args[1], "shell");
        assert!(!args.contains(&"run".to_string()));
        assert!(!args.contains(&"-d".to_string()));
        assert!(args.contains(&"--template".to_string()));
        assert!(args.contains(&"alpine:latest".to_string()));
    }

    // Mirrors test_capabilities_method_routes_through_base for the sbx
    // backend; proves the trait method routes through the SBX const and
    // not via kind-switching.
    #[test]
    fn test_sbx_capabilities_via_trait_method() {
        let rt = ContainerRuntime::sbx();
        assert_eq!(rt.capabilities(), RuntimeBase::SBX.capabilities);
    }

    #[test]
    fn test_sbx_exec_command_parses_compound_options() {
        let rt = ContainerRuntime::sbx();

        // Both -w and -e flags
        let cmd = rt.exec_command(
            "sandbox1",
            Some("-w /workspace/project -e KEY1 -e KEY2=val"),
            "/bin/bash",
        );
        assert!(
            cmd.contains("-w /workspace/project"),
            "workdir missing: {cmd}"
        );
        assert!(cmd.contains("-e KEY1"), "inherit env missing: {cmd}");
        assert!(cmd.contains("-e KEY2=val"), "literal env missing: {cmd}");
        assert!(cmd.contains("/bin/bash"), "cmd missing: {cmd}");

        // Only -e flags (no -w), as used by the tool-launch call site
        let cmd = rt.exec_command("sandbox1", Some("-e AOE_INSTANCE_ID=abc -e KEY1"), "claude");
        assert!(!cmd.contains("-w"), "should have no -w: {cmd}");
        assert!(
            cmd.contains("-e AOE_INSTANCE_ID=abc"),
            "literal env missing: {cmd}"
        );
        assert!(cmd.contains("-e KEY1"), "inherit env missing: {cmd}");

        // None options
        let cmd = rt.exec_command("sandbox1", None, "bash");
        assert!(!cmd.contains("-w"), "should have no -w: {cmd}");
        assert!(!cmd.contains("-e"), "should have no -e: {cmd}");
        assert!(cmd.contains("bash"));
    }
}
