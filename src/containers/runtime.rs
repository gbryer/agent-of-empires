//! The unified `ContainerRuntime`. Shared behavior lives on `RuntimeBase`;
//! this impl dispatches the four genuinely runtime-specific operations
//! (existence probe, running-state probe, exec-command formatting, and
//! batch status query) on a `RuntimeKind` discriminant.

use std::collections::HashMap;

use serde_json::Value;

use super::container_interface::{ContainerConfig, ContainerRuntimeInterface, RuntimeCapabilities};
use super::error::{DockerError, Result};
use super::runtime_base::RuntimeBase;

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
}

impl ContainerRuntime {
    pub fn docker() -> Self {
        Self {
            base: RuntimeBase::DOCKER,
            kind: RuntimeKind::Docker,
        }
    }

    pub fn apple_container() -> Self {
        Self {
            base: RuntimeBase::APPLE_CONTAINER,
            kind: RuntimeKind::AppleContainer,
        }
    }

    pub fn podman() -> Self {
        Self {
            base: RuntimeBase::PODMAN,
            kind: RuntimeKind::Podman,
        }
    }

    // Phase 1 stub; pairs RuntimeBase::SBX with RuntimeKind::Sbx so every
    // dispatch site reaches a safe-stub arm. Phase 2 swaps this for the real
    // SbxRuntime peer struct without re-touching the ~12 callers of
    // get_container_runtime().
    pub fn sbx() -> Self {
        Self {
            base: RuntimeBase::SBX,
            kind: RuntimeKind::Sbx,
        }
    }
}

impl Default for ContainerRuntime {
    fn default() -> Self {
        Self::docker()
    }
}

impl ContainerRuntimeInterface for ContainerRuntime {
    // Sbx short-circuits to false unconditionally (D-05 Phase 1 stub
    // contract). A binary named `sbx` on PATH cannot be exercised by aoe
    // during Phase 1; Phase 2 (RT-02) introduces real `which sbx` probing.
    // The match arm here intentionally does NOT delegate to
    // self.base.is_available() for Sbx, since base.is_available() runs
    // `sbx --version` which would return true if the binary is installed.
    fn is_available(&self) -> bool {
        match self.kind {
            RuntimeKind::Sbx => false,
            _ => self.base.is_available(),
        }
    }

    fn is_daemon_running(&self) -> bool {
        self.base.is_daemon_running()
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        self.base.capabilities
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
            RuntimeKind::Sbx => {
                // Phase 1 stub; no sbx container can exist while is_available
                // returns false. Phase 2 (RT-04) replaces with `sbx inspect`.
                let _ = name;
                Ok(false)
            }
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
            RuntimeKind::Sbx => {
                // Phase 1 stub; no sbx container can be running while
                // is_available returns false. Phase 2 (RT-04) replaces with
                // sbx's running-state probe.
                let _ = name;
                Ok(false)
            }
        }
    }

    fn build_create_args(&self, name: &str, image: &str, config: &ContainerConfig) -> Vec<String> {
        self.base.build_create_args(name, image, config)
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
        self.base.run_create(name, image, config)
    }

    fn start_container(&self, name: &str) -> Result<()> {
        self.base.start_container(name)
    }

    fn stop_container(&self, name: &str) -> Result<()> {
        self.base.stop_container(name)
    }

    fn remove(&self, name: &str, force: bool) -> Result<()> {
        self.base.remove(name, force)
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
                // Addressable string so settings code can format it, but
                // unreachable in normal flows because is_available returns
                // false. Phase 2 (RT-04) replaces with the real `sbx exec`
                // shape.
                let _ = options;
                let _ = cmd;
                format!("sbx exec {}", name)
            }
        }
    }

    fn exec(&self, name: &str, cmd: &[&str]) -> Result<std::process::Output> {
        self.base.exec(name, cmd)
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
            RuntimeKind::Sbx => {
                // Phase 1 stub; no sbx containers can exist while
                // is_available returns false, so the running-states map is
                // always empty. Phase 2 (RT-04) replaces with the real
                // `sbx ls` parse.
                let _ = prefix;
                HashMap::new()
            }
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

    // Phase 1 stub contract per D-05; Phase 2 (RT-02) replaces this with
    // `which sbx` probing. Until then, is_available must short-circuit to
    // false regardless of whether the sbx binary is on PATH; a malicious
    // or stale binary cannot influence aoe's behavior.
    #[test]
    fn test_sbx_is_available_returns_false_unconditionally() {
        let rt = ContainerRuntime::sbx();
        assert!(!rt.is_available());
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

    // The five match-self.kind dispatch sites must each have a safe Sbx
    // arm. is_available short-circuits to false, so callers never reach
    // these paths in normal flows; the safety here is defense-in-depth
    // for any future caller that bypasses the availability gate.
    // (is_available itself is a sixth match site, tested separately in
    // test_sbx_is_available_returns_false_unconditionally.)
    #[test]
    fn test_sbx_dispatch_arms_return_safe_stubs() {
        let rt = ContainerRuntime::sbx();
        assert!(!rt.does_container_exist("foo").unwrap());
        assert!(!rt.is_container_running("foo").unwrap());
        assert!(rt.batch_running_states("aoe-sandbox-").is_empty());
        let cmd = rt.exec_command("foo", None, "bar");
        assert!(cmd.contains("sbx"));
        assert!(cmd.contains("foo"));
    }

    // Mirrors test_capabilities_method_routes_through_base for the sbx
    // backend; proves the trait method routes through the SBX const and
    // not via kind-switching.
    #[test]
    fn test_sbx_capabilities_via_trait_method() {
        let rt = ContainerRuntime::sbx();
        assert_eq!(rt.capabilities(), RuntimeBase::SBX.capabilities);
    }
}
