use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

use crate::containers::container_interface::{ContainerConfig, RuntimeCapabilities};
use crate::containers::error::{DockerError, Result as ContainerResult};
use crate::containers::runtime_base::RuntimeBase;

pub mod argv;
pub mod kit;
pub(crate) mod parse;
pub(crate) mod ports;

/// Peer struct for the Docker Sandboxes (`sbx`) backend. Phase 2 plan 02-01
/// owns the skeleton: an injectable binary path, an availability probe, and a
/// capability accessor. Phase 5 adds all action verb implementations.
///
/// `binary` is a `PathBuf` instead of the `&'static str` used by `RuntimeBase`
/// so unit tests can inject a tempfile path to exercise methods without
/// a host `sbx` install (CONTEXT.md D-10).
pub(crate) struct SbxRuntime {
    pub(crate) binary: PathBuf,
}

impl SbxRuntime {
    pub fn new() -> Self {
        Self {
            binary: PathBuf::from("sbx"),
        }
    }

    pub fn is_available(&self) -> bool {
        Command::new(&self.binary)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeBase::SBX.capabilities
    }

    /// Subprocess wrapper: runs `sbx <args>`, returning stdout on success or
    /// `DockerError::CommandFailed` with stderr on non-zero exit (D-10).
    pub(crate) fn run(&self, args: &[&str]) -> ContainerResult<Output> {
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .map_err(|e| {
                DockerError::CommandFailed(format!(
                    "sbx {} spawn failed: {}",
                    args.first().unwrap_or(&""),
                    e
                ))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DockerError::CommandFailed(format!(
                "sbx {} failed: {}",
                args.first().unwrap_or(&""),
                stderr.trim()
            )));
        }
        Ok(output)
    }

    /// Parse `sbx ls --json` and return the status string for a named sandbox.
    fn sandbox_status(&self, name: &str) -> ContainerResult<String> {
        let output = self.run(&["ls", "--json"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let listing: parse::SbxListOutput = serde_json::from_str(&stdout)
            .map_err(|e| DockerError::CommandFailed(format!("sbx ls --json parse error: {}", e)))?;
        match parse::find_by_name(&listing.sandboxes, name) {
            Some(info) => Ok(info.status.to_lowercase()),
            None => Err(DockerError::CommandFailed(format!(
                "sandbox '{}' not found in sbx ls output",
                name
            ))),
        }
    }

    /// Exponential backoff waiting for a sandbox to reach "running" or "created"
    /// state (D-07, D-08). Total budget ~25s across 7 attempts.
    fn wait_for_ready(&self, name: &str) -> ContainerResult<()> {
        let delays = [200, 400, 800, 1600, 3200, 6400, 12800];
        for delay in &delays {
            match self.sandbox_status(name) {
                Ok(status) if status == "running" || status == "created" => {
                    return Ok(());
                }
                _ => {}
            }
            std::thread::sleep(Duration::from_millis(*delay));
        }
        Err(DockerError::CommandFailed(format!(
            "sbx sandbox '{}' not ready after ~25s",
            name
        )))
    }

    /// Create a sandbox via `sbx create shell`, materializing the kit first
    /// (D-15). Blocks until the sandbox reaches a ready state.
    pub fn create_container(
        &self,
        name: &str,
        image: &str,
        config: &ContainerConfig,
        agent_name: &str,
    ) -> ContainerResult<String> {
        tracing::info!(target: "containers.sbx", %name, %image, "creating sandbox");

        let kit_path = kit::ensure(agent_name).map_err(|e| {
            DockerError::CommandFailed(format!("kit materialization failed: {}", e))
        })?;

        let args = argv::build_create_args(name, image, Some(&kit_path), config);
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run(&arg_refs)?;

        self.wait_for_ready(name)?;
        Ok(name.to_string())
    }

    /// No-op: sbx auto-starts on first exec (D-14).
    pub fn start_container(&self, _name: &str) -> ContainerResult<()> {
        Ok(())
    }

    pub fn stop_container(&self, name: &str) -> ContainerResult<()> {
        tracing::info!(target: "containers.sbx", %name, "stopping sandbox");
        self.run(&["stop", name])?;
        Ok(())
    }

    pub fn remove(&self, name: &str, force: bool) -> ContainerResult<()> {
        tracing::debug!(target: "containers.sbx", %name, force, "removing sandbox");
        let mut args = vec!["rm"];
        if force {
            args.push("-f");
        }
        args.push(name);
        self.run(&args)?;
        Ok(())
    }

    /// Execute a command inside a sandbox with first-exec retry (D-09).
    /// Retries up to 3 times with 1s, 2s, 4s delays if stderr indicates
    /// "not ready" or "not running".
    pub fn exec(
        &self,
        name: &str,
        cmd: &[&str],
        workdir: Option<&str>,
        env: &[(&str, Option<&str>)],
        interactive: bool,
        tty: bool,
    ) -> ContainerResult<Output> {
        let env_entries: Vec<crate::containers::container_interface::EnvEntry> = env
            .iter()
            .map(|(k, v)| match v {
                Some(val) => crate::containers::container_interface::EnvEntry::Literal {
                    key: k.to_string(),
                    value: val.to_string(),
                },
                None => crate::containers::container_interface::EnvEntry::Inherit {
                    key: k.to_string(),
                    value: String::new(),
                },
            })
            .collect();

        let args = argv::build_exec_args(name, workdir, &env_entries, interactive, tty, cmd);
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let all_delays = [0, 1000, 2000, 4000];
        let mut last_err =
            DockerError::CommandFailed(format!("sbx exec in '{}' failed after retries", name));

        for (i, delay) in all_delays.iter().enumerate() {
            if i > 0 {
                std::thread::sleep(Duration::from_millis(*delay));
            }
            match self.run(&arg_refs) {
                Ok(output) => return Ok(output),
                Err(e) => {
                    let msg = e.to_string().to_lowercase();
                    if msg.contains("not ready") || msg.contains("not running") {
                        last_err = e;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(last_err)
    }

    pub fn does_container_exist(&self, name: &str) -> ContainerResult<bool> {
        let output = self.run(&["ls", "--json"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let listing: parse::SbxListOutput = serde_json::from_str(&stdout)
            .map_err(|e| DockerError::CommandFailed(format!("sbx ls --json parse error: {}", e)))?;
        match parse::find_by_name(&listing.sandboxes, name) {
            Some(info) => {
                tracing::debug!(
                    target: "containers.sbx",
                    id = %info.id,
                    agent = %info.agent,
                    status = %info.status,
                    socket_path = %info.socket_path,
                    workspaces = ?info.workspaces,
                    "sandbox found"
                );
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn is_container_running(&self, name: &str) -> ContainerResult<bool> {
        let output = self.run(&["ls", "--json"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let listing: parse::SbxListOutput = serde_json::from_str(&stdout)
            .map_err(|e| DockerError::CommandFailed(format!("sbx ls --json parse error: {}", e)))?;
        Ok(parse::find_by_name(&listing.sandboxes, name)
            .map(|s| s.status.to_lowercase() == "running")
            .unwrap_or(false))
    }

    /// Composite probe: returns true only when `daemon_health()` reports
    /// `Running` (D-06). Internally runs `sbx version` then `sbx ls --json`.
    pub fn is_daemon_running(&self) -> bool {
        self.daemon_health() == parse::SbxDaemonHealth::Running
    }

    /// Typed daemon health for richer diagnostics (D-05). Inspects stderr
    /// from `sbx version` for known failure patterns.
    pub fn daemon_health(&self) -> parse::SbxDaemonHealth {
        let version_output = match Command::new(&self.binary).args(["version"]).output() {
            Ok(o) => o,
            Err(_) => return parse::SbxDaemonHealth::NotInstalled,
        };

        if !version_output.status.success() {
            let stderr = String::from_utf8_lossy(&version_output.stderr).to_lowercase();
            if stderr.contains("not logged in") {
                return parse::SbxDaemonHealth::NotLoggedIn;
            }
            if stderr.contains("policy not configured") {
                return parse::SbxDaemonHealth::PolicyNotConfigured;
            }
            return parse::SbxDaemonHealth::Unknown(
                String::from_utf8_lossy(&version_output.stderr)
                    .trim()
                    .to_string(),
            );
        }

        // Version passed; confirm daemon responds to queries
        match self.run(&["ls", "--json"]) {
            Ok(_) => parse::SbxDaemonHealth::Running,
            Err(e) => parse::SbxDaemonHealth::Unknown(e.to_string()),
        }
    }

    /// Check running state of all sandboxes matching a name prefix in a single
    /// `sbx ls --json` call. Client-side filter (D-Discretion).
    pub fn batch_running_states(&self, prefix: &str) -> HashMap<String, bool> {
        let output = match self.run(&["ls", "--json"]) {
            Ok(o) => o,
            Err(_) => return HashMap::new(),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let listing: parse::SbxListOutput = match serde_json::from_str(&stdout) {
            Ok(l) => l,
            Err(_) => return HashMap::new(),
        };
        listing
            .sandboxes
            .iter()
            .filter(|s| s.name.starts_with(prefix))
            .map(|s| (s.name.clone(), s.status.to_lowercase() == "running"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn make_mock_script(dir: &tempfile::TempDir, script: &str) -> PathBuf {
        let path = dir.path().join("fake-sbx");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(script.as_bytes()).unwrap();
        }
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[test]
    fn capabilities_returns_locked_sbx_matrix() {
        let caps = SbxRuntime::new().capabilities();
        assert!(!caps.supports_read_only_volumes);
        assert!(!caps.supports_remove_volumes);
        assert!(!caps.supports_port_publish_at_create);
        assert!(!caps.supports_image_pull);
        assert!(!caps.supports_anonymous_volumes);
        assert!(!caps.supports_arbitrary_volume_paths);
        assert!(caps.supports_dynamic_port_publish);
        assert_eq!(caps, RuntimeBase::SBX.capabilities);
    }

    #[test]
    fn is_available_with_injected_fake_binary_returns_true() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = make_mock_script(&dir, "#!/bin/sh\nexit 0\n");
        let sbx = SbxRuntime { binary: path };
        assert!(sbx.is_available());
    }

    #[test]
    fn is_available_with_missing_binary_returns_false() {
        let sbx = SbxRuntime {
            binary: PathBuf::from("/nonexistent/sbx-binary"),
        };
        assert!(!sbx.is_available());
    }

    #[test]
    fn test_run_helper_propagates_stderr() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = make_mock_script(&dir, "#!/bin/sh\necho 'some error' >&2\nexit 1\n");
        let sbx = SbxRuntime { binary: path };
        let err = sbx.run(&["test"]).unwrap_err();
        assert!(
            err.to_string().contains("some error"),
            "expected stderr in error: {}",
            err
        );
    }

    #[test]
    fn test_run_helper_returns_stdout_on_success() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = make_mock_script(&dir, "#!/bin/sh\necho 'output'\nexit 0\n");
        let sbx = SbxRuntime { binary: path };
        let output = sbx.run(&["test"]).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("output"));
    }

    #[test]
    fn test_stop_container_calls_stop() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_file = dir.path().join("args.log");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
            log_file.display()
        );
        let path = make_mock_script(&dir, &script);
        let sbx = SbxRuntime { binary: path };
        sbx.stop_container("my-sandbox").unwrap();
        let logged = std::fs::read_to_string(&log_file).unwrap();
        assert!(logged.contains("stop"), "expected 'stop' in: {}", logged);
        assert!(
            logged.contains("my-sandbox"),
            "expected 'my-sandbox' in: {}",
            logged
        );
    }

    #[test]
    fn test_remove_with_force_includes_flag() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_file = dir.path().join("args.log");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
            log_file.display()
        );
        let path = make_mock_script(&dir, &script);
        let sbx = SbxRuntime { binary: path };
        sbx.remove("my-sandbox", true).unwrap();
        let logged = std::fs::read_to_string(&log_file).unwrap();
        assert!(logged.contains("-f"), "expected '-f' in: {}", logged);
        assert!(
            logged.contains("my-sandbox"),
            "expected 'my-sandbox' in: {}",
            logged
        );
    }

    #[test]
    fn test_start_container_is_noop() {
        let sbx = SbxRuntime {
            binary: PathBuf::from("/nonexistent/sbx-binary"),
        };
        assert!(sbx.start_container("anything").is_ok());
    }

    #[test]
    fn test_is_daemon_running_false_when_binary_missing() {
        let sbx = SbxRuntime {
            binary: PathBuf::from("/nonexistent/sbx-binary"),
        };
        assert!(!sbx.is_daemon_running());
    }

    #[test]
    fn test_daemon_health_not_installed() {
        let sbx = SbxRuntime {
            binary: PathBuf::from("/nonexistent/sbx-binary"),
        };
        assert_eq!(sbx.daemon_health(), parse::SbxDaemonHealth::NotInstalled);
    }

    #[test]
    fn test_exec_retries_on_not_ready() {
        let dir = tempfile::TempDir::new().unwrap();
        let counter_file = dir.path().join("counter");
        std::fs::write(&counter_file, "0").unwrap();
        // Script fails with "not ready" on first 2 calls, then succeeds
        let script = format!(
            r#"#!/bin/sh
COUNT=$(cat "{counter}")
COUNT=$((COUNT + 1))
echo "$COUNT" > "{counter}"
if [ "$COUNT" -le 2 ]; then
  echo "sandbox not ready" >&2
  exit 1
fi
echo "success"
exit 0
"#,
            counter = counter_file.display()
        );
        let path = make_mock_script(&dir, &script);
        let sbx = SbxRuntime { binary: path };
        let result = sbx.exec("my-sandbox", &["echo", "hi"], None, &[], false, false);
        assert!(result.is_ok(), "expected Ok after retry, got: {:?}", result);
        let final_count: i32 = std::fs::read_to_string(&counter_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(
            final_count >= 3,
            "expected at least 3 attempts, got {}",
            final_count
        );
    }

    #[test]
    fn test_does_container_exist_true() {
        let dir = tempfile::TempDir::new().unwrap();
        let fixture = include_str!("../../../tests/fixtures/sbx_ls.json");
        let script = format!("#!/bin/sh\ncat << 'FIXTURE'\n{}\nFIXTURE\n", fixture);
        let path = make_mock_script(&dir, &script);
        let sbx = SbxRuntime { binary: path };
        assert!(sbx.does_container_exist("claude-Lymow-HA").unwrap());
        assert!(!sbx.does_container_exist("nonexistent").unwrap());
    }

    #[test]
    fn test_batch_running_states_filters_by_prefix() {
        let dir = tempfile::TempDir::new().unwrap();
        let json = r#"{"sandboxes":[{"name":"aoe-abc","status":"running"},{"name":"aoe-def","status":"stopped"},{"name":"other-xyz","status":"running"}]}"#;
        let script = format!("#!/bin/sh\necho '{}'\n", json);
        let path = make_mock_script(&dir, &script);
        let sbx = SbxRuntime { binary: path };
        let states = sbx.batch_running_states("aoe-");
        assert_eq!(states.len(), 2);
        assert_eq!(states.get("aoe-abc"), Some(&true));
        assert_eq!(states.get("aoe-def"), Some(&false));
        assert!(!states.contains_key("other-xyz"));
    }
}
