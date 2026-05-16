//! Status file I/O for hooks-based agent status detection.
//!
//! Agent hooks write `running`, `waiting`, or `idle` to a well-known
//! file path so AoE can detect agent status without parsing tmux pane content.

use std::path::{Path, PathBuf};

use crate::containers::ContainerRuntimeInterface;
use crate::session::Status;

use super::HOOK_STATUS_BASE;

/// Return the directory for a given instance's hook status file.
///
/// Docker, Podman, and Apple-Container runtimes share `/tmp/aoe-hooks/<id>/`
/// because they honor arbitrary `HOST:CONTAINER` bind mounts: the host poller
/// reads the same absolute path the in-container hook writes.
pub fn hook_status_dir(instance_id: &str) -> PathBuf {
    PathBuf::from(HOOK_STATUS_BASE).join(instance_id)
}

/// Return the workspace-relative status directory for the sbx runtime.
///
/// Returns `<project_path>/.aoe-hooks/<instance_id>`. Per CONTEXT.md D-02,
/// sbx-class runtimes report `!supports_arbitrary_volume_paths`: they cannot
/// mount the host's `/tmp/aoe-hooks` into the microVM at a different path.
/// Instead, the in-VM hook shim writes through the workspace mount that the
/// sandbox provides natively at the same absolute path, and the host poller
/// reads through the same workspace path on the host side, so no dedicated
/// host-side hook-volume mount is needed.
pub fn sbx_hook_status_dir(project_path: &Path, instance_id: &str) -> PathBuf {
    project_path.join(".aoe-hooks").join(instance_id)
}

/// Pick the status directory for a runtime based on its capability flag.
///
/// Per CONTEXT.md D-03, this is the single dispatch site for status-dir
/// resolution. The function is pure so both branches are trivially
/// unit-testable without stubbing the global runtime factory. The two
/// `_for_instance` shims below call this with
/// `runtime.capabilities().supports_arbitrary_volume_paths` so the dispatch
/// collapses to a one-liner at the call site.
pub fn resolve_status_dir(
    supports_arbitrary_volume_paths: bool,
    project_path: &Path,
    instance_id: &str,
) -> PathBuf {
    if supports_arbitrary_volume_paths {
        hook_status_dir(instance_id)
    } else {
        sbx_hook_status_dir(project_path, instance_id)
    }
}

/// Parse the trimmed contents of a status file into a `Status`.
///
/// Shared by `read_hook_status` and `read_hook_status_for_instance` so the
/// legacy and dispatching code paths agree on what counts as a valid value.
fn parse_status_content(s: &str) -> Option<Status> {
    match s {
        "running" => Some(Status::Running),
        "waiting" => Some(Status::Waiting),
        "idle" => Some(Status::Idle),
        other => {
            tracing::warn!("Unexpected hook status value: {:?}", other);
            None
        }
    }
}

/// Read the hook-written status file for the given instance.
///
/// Returns `None` if the file doesn't exist. When `Some`, the hook is
/// actively tracking the session and shell detection is unreliable
/// (wrapper scripts may keep a shell alive). Callers should still use
/// `is_pane_dead()` to detect truly dead panes.
pub fn read_hook_status(instance_id: &str) -> Option<Status> {
    let status_path = hook_status_dir(instance_id).join("status");

    let content = std::fs::read_to_string(&status_path).ok()?;
    parse_status_content(content.trim())
}

/// Capability-aware variant of `read_hook_status`: routes to the legacy
/// `/tmp/aoe-hooks/<id>/status` path for Docker/Podman/Apple-Container, or
/// the workspace-relative `<project_path>/.aoe-hooks/<id>/status` path for
/// sbx-class runtimes.
///
/// Trust-boundary invariant (T-04-02-02 / T-04-02-03): `instance.project_path`
/// is validated at session-create time by `compute_volume_paths`; `instance.id`
/// is UUID-shaped (`uuid::Uuid::new_v4().to_string()` in `Instance::new`), so
/// neither input can introduce `..` traversal into the joined path.
pub fn read_hook_status_for_instance(instance: &crate::session::Instance) -> Option<Status> {
    let caps = crate::containers::get_container_runtime().capabilities();
    let dir = resolve_status_dir(
        caps.supports_arbitrary_volume_paths,
        Path::new(&instance.project_path),
        &instance.id,
    );
    let content = std::fs::read_to_string(dir.join("status")).ok()?;
    parse_status_content(content.trim())
}

/// Remove the hook status directory for a given instance (cleanup on stop/delete).
pub fn cleanup_hook_status_dir(instance_id: &str) {
    let dir = hook_status_dir(instance_id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("Failed to cleanup hook status dir {}: {}", dir.display(), e);
        }
    }
}

/// Capability-aware variant of `cleanup_hook_status_dir`. Shares the same
/// trust-boundary invariant as `read_hook_status_for_instance`: the workspace
/// path comes from the validated `Instance::project_path` and the id is
/// UUID-shaped, so the `remove_dir_all` target cannot escape the workspace.
pub fn cleanup_hook_status_dir_for_instance(instance: &crate::session::Instance) {
    let caps = crate::containers::get_container_runtime().capabilities();
    let dir = resolve_status_dir(
        caps.supports_arbitrary_volume_paths,
        Path::new(&instance.project_path),
        &instance.id,
    );
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("Failed to cleanup hook status dir {}: {}", dir.display(), e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn setup_status_file(instance_id: &str, content: &str) -> PathBuf {
        let dir = hook_status_dir(instance_id);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("status");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        dir
    }

    #[test]
    fn test_read_running_status() {
        let id = "test_read_running";
        let dir = setup_status_file(id, "running");
        assert_eq!(read_hook_status(id), Some(Status::Running));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_read_waiting_status() {
        let id = "test_read_waiting";
        let dir = setup_status_file(id, "waiting");
        assert_eq!(read_hook_status(id), Some(Status::Waiting));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_read_idle_status() {
        let id = "test_read_idle";
        let dir = setup_status_file(id, "idle");
        assert_eq!(read_hook_status(id), Some(Status::Idle));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_read_waiting_with_newline() {
        let id = "test_read_newline";
        let dir = setup_status_file(id, "waiting\n");
        assert_eq!(read_hook_status(id), Some(Status::Waiting));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_read_missing_file() {
        assert_eq!(read_hook_status("nonexistent_instance_id"), None);
    }

    #[test]
    fn test_read_dangling_symlink() {
        let id = "test_dangling_symlink";
        let dir = hook_status_dir(id);
        fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink("/nonexistent/target", dir.join("status")).unwrap();
        assert_eq!(read_hook_status(id), None);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_read_unexpected_content() {
        let id = "test_read_unexpected";
        let dir = setup_status_file(id, "something_else");
        assert_eq!(read_hook_status(id), None);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_cleanup_existing_dir() {
        let id = "test_cleanup_existing";
        let dir = setup_status_file(id, "running");
        assert!(dir.exists());
        cleanup_hook_status_dir(id);
        assert!(!dir.exists());
    }

    #[test]
    fn test_cleanup_nonexistent_dir() {
        // Should not panic
        cleanup_hook_status_dir("nonexistent_cleanup_test");
    }

    #[test]
    fn test_hook_status_dir_path() {
        let dir = hook_status_dir("abc123");
        assert_eq!(dir, PathBuf::from("/tmp/aoe-hooks/abc123"));
    }

    #[test]
    fn test_sbx_hook_status_dir_path() {
        let dir = sbx_hook_status_dir(Path::new("/p/q"), "abc");
        assert_eq!(dir, PathBuf::from("/p/q/.aoe-hooks/abc"));
    }

    #[test]
    fn test_sbx_hook_status_dir_with_tempdir() {
        let td = tempfile::TempDir::new().unwrap();
        let dir = sbx_hook_status_dir(td.path(), "instance-x");
        assert_eq!(dir, td.path().join(".aoe-hooks").join("instance-x"));
    }

    #[test]
    fn resolve_status_dir_legacy_when_arbitrary_paths_supported() {
        let dir = resolve_status_dir(true, Path::new("/proj"), "id-x");
        assert_eq!(dir, hook_status_dir("id-x"));
        assert!(
            dir.starts_with("/tmp/aoe-hooks/"),
            "legacy branch must root under /tmp/aoe-hooks: {}",
            dir.display()
        );
    }

    #[test]
    fn resolve_status_dir_sbx_when_arbitrary_paths_unsupported() {
        let dir = resolve_status_dir(false, Path::new("/proj"), "id-x");
        assert_eq!(dir, PathBuf::from("/proj/.aoe-hooks/id-x"));
    }

    #[test]
    fn test_parse_status_content_running() {
        assert_eq!(parse_status_content("running"), Some(Status::Running));
        assert_eq!(parse_status_content("waiting"), Some(Status::Waiting));
        assert_eq!(parse_status_content("idle"), Some(Status::Idle));
        assert_eq!(parse_status_content("garbage"), None);
    }
}
