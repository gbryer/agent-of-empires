use std::collections::HashMap;

use super::error::Result;

pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: bool,
}

/// An environment variable entry for a container.
///
/// `Inherit` entries use Docker's `-e KEY` form (no value in argv), which reads
/// the value from the calling process's environment. This prevents secrets from
/// leaking into `ps` output.
///
/// `Literal` entries use `-e KEY=VALUE` and are appropriate for non-secret,
/// hard-coded values.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvEntry {
    /// Value inherited from host environment. Only the key appears in argv;
    /// the value is passed to Docker via the process environment.
    Inherit { key: String, value: String },
    /// Literal (non-secret) value. Both key and value appear in argv.
    Literal { key: String, value: String },
}

impl EnvEntry {
    pub fn key(&self) -> &str {
        match self {
            EnvEntry::Inherit { key, .. } | EnvEntry::Literal { key, .. } => key,
        }
    }

    pub fn value(&self) -> &str {
        match self {
            EnvEntry::Inherit { value, .. } | EnvEntry::Literal { value, .. } => value,
        }
    }
}

pub struct ContainerConfig {
    pub working_dir: String,
    pub volumes: Vec<VolumeMount>,
    pub anonymous_volumes: Vec<String>,
    pub environment: Vec<EnvEntry>,
    pub cpu_limit: Option<String>,
    pub memory_limit: Option<String>,
    pub port_mappings: Vec<String>,
    /// Agent name for sbx kit materialization. Populated by the session layer
    /// from `Instance::tool`; consumed only by the sbx create path.
    pub agent_name: Option<String>,
}

/// Declarative capability matrix for a container runtime.
///
/// Every backend literal MUST initialize every field by name; `..Default::default()`
/// is forbidden so that adding an eighth flag fails the build on every const literal
/// until each backend declares an honest value. That compile-time exhaustiveness is
/// what lets new flags fan out without silently producing broken output for any
/// backend that forgot to opt in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    /// Whether this runtime honors `:ro` on volume mounts.
    pub supports_read_only_volumes: bool,
    /// Whether this runtime accepts `-v` on the remove subcommand to clean up
    /// anonymous volumes alongside the container.
    pub supports_remove_volumes: bool,
    /// Whether this runtime publishes host ports at container-create time via `-p`.
    pub supports_port_publish_at_create: bool,
    /// Whether this runtime exposes an image-pull verb (e.g., `pull`) that
    /// downloads a remote image into local storage.
    pub supports_image_pull: bool,
    /// Whether this runtime honors `-v PATH` without a host-side counterpart
    /// (anonymous volumes for caches, etc.).
    pub supports_anonymous_volumes: bool,
    /// Whether this runtime accepts any `HOST:CONTAINER` pairing for bind mounts,
    /// as opposed to requiring the host path and container path to match.
    pub supports_arbitrary_volume_paths: bool,
    /// Whether this runtime can publish ports after the container has been
    /// created, without recreate/restart workarounds.
    ///
    /// Caller-facing metadata; `build_create_args` does not consume this
    /// flag because port publish at create is gated separately by
    /// `supports_port_publish_at_create`. Code that publishes ports after
    /// create (e.g. via an out-of-band `sbx ports` call) reads this flag
    /// to decide whether the post-create path is available.
    pub supports_dynamic_port_publish: bool,
}

pub trait ContainerRuntimeInterface {
    /// Check if the container runtime CLI is available
    fn is_available(&self) -> bool;

    /// Check if the container runtime daemon is running
    fn is_daemon_running(&self) -> bool;

    /// Return the runtime's declared capability matrix. Pure data accessor; no
    /// subprocess calls. Consumers gate behavior on individual flags.
    fn capabilities(&self) -> RuntimeCapabilities;

    /// Get the container runtime version string
    fn get_version(&self) -> Result<String>;

    fn pull_image(&self, image: &str) -> Result<()>;

    fn ensure_image(&self, image: &str) -> Result<()>;

    fn default_sandbox_image(&self) -> &'static str;

    fn effective_default_image(&self) -> String;

    fn image_exists_locally(&self, image: &str) -> bool;

    // container management
    fn does_container_exist(&self, name: &str) -> Result<bool>;

    fn is_container_running(&self, name: &str) -> Result<bool>;

    /// Build the docker run arguments from the container config.
    /// Separated from `create` to enable unit testing.
    fn build_create_args(&self, name: &str, image: &str, config: &ContainerConfig) -> Vec<String>;

    fn create_container(&self, name: &str, image: &str, config: &ContainerConfig)
        -> Result<String>;

    fn start_container(&self, name: &str) -> Result<()>;

    fn stop_container(&self, name: &str) -> Result<()>;

    fn remove(&self, name: &str, force: bool) -> Result<()>;

    fn exec_command(&self, name: &str, options: Option<&str>, cmd: &str) -> String;

    fn exec(&self, name: &str, cmd: &[&str]) -> Result<std::process::Output>;

    /// Check running state of all containers matching a name prefix in a single call.
    /// Returns a map of container name -> is_running.
    fn batch_running_states(&self, prefix: &str) -> HashMap<String, bool>;
}
