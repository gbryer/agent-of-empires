//! Process utilities for tmux session management

use std::collections::HashSet;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use nix::errno::Errno;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use nix::sys::signal::{kill, Signal};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use nix::unistd::Pid;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

/// Get the PID of the shell process running in a tmux pane
pub fn get_pane_pid(session_name: &str) -> Option<u32> {
    // Use `^.0` to target the first window's first pane regardless of
    // base-index or which pane is active, so we always query the agent's
    // pane even when the user has created additional tmux windows or split
    // panes.  See #435, #488.
    let target = format!("{session_name}:^.0");
    let output = Command::new("tmux")
        .args(["display-message", "-t", &target, "-p", "#{pane_pid}"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// Get the foreground process group leader PID for a given shell PID
/// This finds the actual process that has the terminal foreground
pub fn get_foreground_pid(shell_pid: u32) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        linux::get_foreground_pid(shell_pid)
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_foreground_pid(shell_pid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = shell_pid;
        None
    }
}

/// Kill a process and all its descendants
/// Sends SIGTERM first, then SIGKILL to any survivors
pub fn kill_process_tree(pid: u32) {
    #[cfg(target_os = "linux")]
    let pids = linux::collect_pid_tree(pid);

    #[cfg(target_os = "macos")]
    let pids = macos::collect_pid_tree(pid);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    kill_with_fallback(&pids);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        // No-op on unsupported platforms, fall back to tmux kill-session only
    }
}

/// SIGTERM every pid in reverse order (children first), wait briefly for
/// graceful shutdown, then SIGKILL anything still alive.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn kill_with_fallback(pids: &[u32]) {
    tracing::debug!(
        target: "process.tree",
        descendants = ?pids,
        "killing process tree"
    );

    for &p in pids.iter().rev() {
        tracing::debug!(target: "process.signal", pid = p, signal = "SIGTERM", "sending signal");
        let _ = kill(Pid::from_raw(p as i32), Signal::SIGTERM);
    }

    std::thread::sleep(Duration::from_millis(100));

    for &p in pids.iter().rev() {
        if process_exists(p) {
            tracing::warn!(
                target: "process.reap",
                pid = p,
                "pid survived SIGTERM after 100ms; sending SIGKILL"
            );
            tracing::info!(target: "process.signal", pid = p, signal = "SIGKILL", "sending signal");
            let _ = kill(Pid::from_raw(p as i32), Signal::SIGKILL);
        }
    }
}

/// Portable "is this pid still around?" check via kill(pid, 0).
/// EPERM means the process exists but we lack permission (still exists).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn process_exists(pid: u32) -> bool {
    match kill(Pid::from_raw(pid as i32), None) {
        Ok(()) => true,
        Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

/// Send SIGSTOP to a process and all its descendants. Used to pause
/// the agent (claude) while a mobile client is reading tmux scrollback
/// — without this, claude's continued output keeps pushing lines into
/// scrollback under the reader and shifts what they're trying to read.
///
/// Paired with [`continue_process_tree`] which sends SIGCONT. The web
/// server guarantees a SIGCONT on client disconnect so a dropped
/// connection cannot leave the pane's process permanently suspended.
pub fn stop_process_tree(pid: u32) {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    signal_process_tree(pid, Signal::SIGSTOP);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
    }
}

/// Send SIGCONT to a process and all its descendants. Inverse of
/// [`stop_process_tree`]; SIGCONT to a non-stopped process is a no-op,
/// so this is safe to invoke unconditionally as cleanup.
pub fn continue_process_tree(pid: u32) {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    signal_process_tree(pid, Signal::SIGCONT);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn signal_process_tree(pid: u32, signal: Signal) {
    #[cfg(target_os = "linux")]
    let pids = linux::collect_pid_tree(pid);
    #[cfg(target_os = "macos")]
    let pids = macos::collect_pid_tree(pid);

    tracing::debug!(
        target: "process.tree",
        descendants = ?pids,
        ?signal,
        "signaling process tree"
    );
    for &p in pids.iter().rev() {
        if let Err(e) = kill(Pid::from_raw(p as i32), signal) {
            if e != Errno::ESRCH {
                tracing::debug!(
                    target: "process.signal",
                    pid = p,
                    ?signal,
                    error = %e,
                    "kill failed"
                );
            }
        }
    }
}

/// Register a platform-specific handler that fires after the host wakes from
/// sleep, resyncing clocks in any aoe-owned sbx sandboxes. No-op on
/// unsupported platforms.
pub fn register_wake_handler() {
    #[cfg(target_os = "linux")]
    {
        linux::register_wake_handler();
    }

    #[cfg(target_os = "macos")]
    {
        macos::register_wake_handler();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // No-op on unsupported platforms
    }
}

#[derive(Deserialize)]
struct SbxLsOutput {
    #[serde(default)]
    sandboxes: Vec<SbxSandboxEntry>,
}

#[derive(Deserialize)]
struct SbxSandboxEntry {
    #[serde(default)]
    name: String,
}

/// Touch the clock in every aoe-owned sbx sandbox older than 5 minutes by
/// running `sbx exec <name> date`. Prevents HTTPS handshake failures caused
/// by stale guest clocks after host sleep.
pub(crate) fn resync_sbx_clocks() {
    let sbx_names = match Command::new("sbx").args(["ls", "--json"]).output() {
        Ok(output) if output.status.success() => {
            match serde_json::from_slice::<SbxLsOutput>(&output.stdout) {
                Ok(parsed) => parsed
                    .sandboxes
                    .into_iter()
                    .map(|s| s.name)
                    .collect::<HashSet<String>>(),
                Err(e) => {
                    tracing::warn!(target: "process.wake", error = %e, "failed to parse sbx ls output");
                    return;
                }
            }
        }
        Ok(output) => {
            tracing::warn!(
                target: "process.wake",
                status = %output.status,
                "sbx ls returned non-zero exit code"
            );
            return;
        }
        Err(e) => {
            tracing::warn!(target: "process.wake", error = %e, "failed to run sbx ls");
            return;
        }
    };

    if sbx_names.is_empty() {
        return;
    }

    let profiles = match crate::session::list_profiles() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(target: "process.wake", error = %e, "failed to list profiles for wake resync");
            return;
        }
    };

    let threshold = Utc::now() - chrono::Duration::minutes(5);
    let candidates = collect_resync_candidates(&profiles, &sbx_names, threshold);

    for name in candidates {
        match Command::new("sbx").args(["exec", &name, "date"]).output() {
            Ok(output) if output.status.success() => {
                tracing::info!(target: "process.wake", sandbox = %name, "clock resync successful");
            }
            Ok(output) => {
                tracing::warn!(
                    target: "process.wake",
                    sandbox = %name,
                    status = %output.status,
                    "clock resync command failed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "process.wake",
                    sandbox = %name,
                    error = %e,
                    "failed to exec clock resync"
                );
            }
        }
    }
}

/// Walk all profiles and collect container names for sandboxed instances that
/// are older than `threshold` and whose container name appears in `sbx_names`.
fn collect_resync_candidates(
    profiles: &[String],
    sbx_names: &HashSet<String>,
    threshold: DateTime<Utc>,
) -> Vec<String> {
    let mut candidates = Vec::new();

    for profile in profiles {
        let storage = match crate::session::Storage::new(profile) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "process.wake",
                    profile = %profile,
                    error = %e,
                    "failed to open profile storage"
                );
                continue;
            }
        };

        let instances = match storage.load() {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!(
                    target: "process.wake",
                    profile = %profile,
                    error = %e,
                    "failed to load instances"
                );
                continue;
            }
        };

        for inst in &instances {
            if !inst.is_sandboxed() {
                continue;
            }
            if inst.created_at >= threshold {
                continue;
            }
            if let Some(info) = &inst.sandbox_info {
                if sbx_names.contains(&info.container_name) {
                    candidates.push(info.container_name.clone());
                }
            }
        }
    }

    candidates
}

/// Pure filtering logic extracted for unit testing without needing a real sbx
/// binary. Given a set of instances, sbx sandbox names, and a threshold time,
/// returns the container names that should be resynced.
#[cfg(test)]
fn filter_resync_candidates(
    instances: &[crate::session::Instance],
    sbx_names: &HashSet<String>,
    threshold: DateTime<Utc>,
) -> Vec<String> {
    instances
        .iter()
        .filter(|inst| inst.is_sandboxed())
        .filter(|inst| inst.created_at < threshold)
        .filter_map(|inst| {
            inst.sandbox_info
                .as_ref()
                .filter(|info| sbx_names.contains(&info.container_name))
                .map(|info| info.container_name.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Instance, SandboxInfo};
    use chrono::TimeZone;

    fn make_instance(container_name: &str, created_at: DateTime<Utc>, sandboxed: bool) -> Instance {
        let mut inst = Instance::new(container_name, "/tmp/test");
        inst.created_at = created_at;
        if sandboxed {
            inst.sandbox_info = Some(SandboxInfo {
                enabled: true,
                container_id: None,
                image: "test:latest".to_string(),
                container_name: container_name.to_string(),
                extra_env: None,
                custom_instruction: None,
            });
        }
        inst
    }

    #[test]
    fn filter_skips_non_sandboxed_instances() {
        let threshold = Utc::now();
        let old_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let sbx_names: HashSet<String> = ["sbx-test"].iter().map(|s| s.to_string()).collect();

        let instances = vec![make_instance("sbx-test", old_time, false)];
        let result = filter_resync_candidates(&instances, &sbx_names, threshold);
        assert!(
            result.is_empty(),
            "non-sandboxed instance should be skipped"
        );
    }

    #[test]
    fn filter_skips_recent_instances() {
        let threshold = Utc::now() - chrono::Duration::minutes(5);
        let recent_time = Utc::now();
        let sbx_names: HashSet<String> = ["sbx-recent"].iter().map(|s| s.to_string()).collect();

        let instances = vec![make_instance("sbx-recent", recent_time, true)];
        let result = filter_resync_candidates(&instances, &sbx_names, threshold);
        assert!(result.is_empty(), "recent instance should be skipped");
    }

    #[test]
    fn filter_skips_instances_not_in_sbx_ls() {
        let threshold = Utc::now();
        let old_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let sbx_names: HashSet<String> = ["other-sandbox"].iter().map(|s| s.to_string()).collect();

        let instances = vec![make_instance("my-sandbox", old_time, true)];
        let result = filter_resync_candidates(&instances, &sbx_names, threshold);
        assert!(
            result.is_empty(),
            "instance not in sbx ls should be skipped"
        );
    }

    #[test]
    fn filter_includes_matching_candidates() {
        let threshold = Utc::now();
        let old_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let sbx_names: HashSet<String> = ["sbx-alpha", "sbx-beta"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let instances = vec![
            make_instance("sbx-alpha", old_time, true),
            make_instance("sbx-beta", old_time, true),
            make_instance("sbx-gamma", old_time, true),
            make_instance("sbx-delta", Utc::now(), true),
            make_instance("sbx-alpha", old_time, false),
        ];
        let result = filter_resync_candidates(&instances, &sbx_names, threshold);
        assert_eq!(result, vec!["sbx-alpha", "sbx-beta"]);
    }
}
