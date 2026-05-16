use std::path::PathBuf;
use std::process::Command;

use crate::containers::container_interface::RuntimeCapabilities;
use crate::containers::runtime_base::RuntimeBase;

pub mod argv;
pub mod kit;

/// Peer struct for the Docker Sandboxes (`sbx`) backend. Phase 2 plan 02-01
/// owns the skeleton: an injectable binary path, an availability probe, and a
/// capability accessor. Action verbs (create, exec, etc.) arrive in Phase 5.
///
/// `binary` is a `PathBuf` instead of the `&'static str` used by `RuntimeBase`
/// so unit tests can inject a tempfile path to exercise `is_available` without
/// a host `sbx` install (CONTEXT.md D-10).
pub(crate) struct SbxRuntime {
    pub(crate) binary: PathBuf,
}

impl SbxRuntime {
    pub fn new() -> Self {
        // `Command::new(PathBuf)` resolves bare names against PATH, matching
        // `RuntimeBase::command()` at runtime_base.rs:121.
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
        // Route through the Phase 1 source of truth (CONTEXT.md D-12 routes,
        // PATTERNS.md Pattern D); zero drift if Phase 1's matrix shifts.
        RuntimeBase::SBX.capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

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
        // Use TempDir + a separately-scoped File handle rather than the
        // CONTEXT.md NamedTempFile recipe: on Linux, exec'ing a file while
        // its writer is still open returns ETXTBSY (Text file busy).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fake-sbx");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
        }
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();

        let sbx = SbxRuntime { binary: path };
        assert!(sbx.is_available());
        drop(dir);
    }

    #[test]
    fn is_available_with_missing_binary_returns_false() {
        let sbx = SbxRuntime {
            binary: PathBuf::from("/nonexistent/sbx-binary"),
        };
        assert!(!sbx.is_available());
    }
}
