//! Post-create port publishing for sbx sandboxes (D-01, D-02).
//!
//! Iterates `port_mappings` one at a time via `sbx ports SANDBOX --publish
//! <spec>`, capturing per-port success/failure so the session layer can
//! surface partial failures without rolling back the sandbox.

use super::argv;
use super::SbxRuntime;

/// Per-port result from `publish_ports`. The session layer logs failures via
/// `tracing::warn!` and stores them for TUI/web to surface (Phase 6).
#[derive(Debug, Clone)]
pub enum PortPublishResult {
    Ok(String),
    Failed { port: String, stderr: String },
}

impl SbxRuntime {
    /// Publish each port mapping individually via `sbx ports --publish`.
    /// Returns a vec with one result per port; failures do NOT abort
    /// remaining ports (D-01).
    pub fn publish_ports(&self, name: &str, port_mappings: &[String]) -> Vec<PortPublishResult> {
        port_mappings
            .iter()
            .map(|spec| {
                let args = argv::build_ports_args(name, spec);
                let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                match self.run(&arg_refs) {
                    std::result::Result::Ok(_) => PortPublishResult::Ok(spec.clone()),
                    Err(e) => PortPublishResult::Failed {
                        port: spec.clone(),
                        stderr: e.to_string(),
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

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
    fn publish_ports_all_succeed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = make_mock_script(&dir, "#!/bin/sh\nexit 0\n");
        let sbx = SbxRuntime { binary: path };
        let results = sbx.publish_ports(
            "my-sandbox",
            &["3000:3000".to_string(), "8080:80".to_string()],
        );
        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(matches!(r, PortPublishResult::Ok(_)));
        }
    }

    #[test]
    fn publish_ports_captures_failure_per_port() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = make_mock_script(&dir, "#!/bin/sh\necho 'port already in use' >&2\nexit 1\n");
        let sbx = SbxRuntime { binary: path };
        let results = sbx.publish_ports("my-sandbox", &["9090:9090".to_string()]);
        assert_eq!(results.len(), 1);
        match &results[0] {
            PortPublishResult::Failed { port, stderr } => {
                assert_eq!(port, "9090:9090");
                assert!(
                    stderr.contains("port already in use"),
                    "expected 'port already in use' in: {}",
                    stderr
                );
            }
            other => panic!("expected Failed variant, got {:?}", other),
        }
    }
}
