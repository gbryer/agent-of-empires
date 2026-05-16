//! Linux-specific process utilities

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Collect `pid` and every descendant by walking `/proc` once to build a
/// parent -> children map, then descending it. One `/proc` scan regardless of
/// tree depth.
pub(super) fn collect_pid_tree(pid: u32) -> Vec<u32> {
    let children_map = build_children_map();
    let mut pids = vec![pid];
    collect_descendants_from_map(pid, &children_map, &mut pids);
    pids
}

/// Scan `/proc` once and group every live PID by its parent.
fn build_children_map() -> HashMap<u32, Vec<u32>> {
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let proc_dir = Path::new("/proc");
    let Ok(entries) = fs::read_dir(proc_dir) else {
        return children_map;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        let Ok(child_pid) = name_str.parse::<u32>() else {
            continue;
        };

        let stat_path = entry.path().join("stat");
        let Ok(content) = fs::read_to_string(&stat_path) else {
            continue;
        };

        if let Some(ppid) = parse_stat_field(&content, 3) {
            children_map.entry(ppid as u32).or_default().push(child_pid);
        }
    }

    children_map
}

fn collect_descendants_from_map(
    pid: u32,
    children_map: &HashMap<u32, Vec<u32>>,
    pids: &mut Vec<u32>,
) {
    if let Some(children) = children_map.get(&pid) {
        for &child_pid in children {
            pids.push(child_pid);
            collect_descendants_from_map(child_pid, children_map, pids);
        }
    }
}

/// Get the foreground process group leader for a shell PID
/// Walks the process tree to find the actual foreground process
pub fn get_foreground_pid(shell_pid: u32) -> Option<u32> {
    // Read the shell's stat to get its controlling terminal
    let stat_path = format!("/proc/{}/stat", shell_pid);
    let stat_content = fs::read_to_string(&stat_path).ok()?;

    // Parse stat: pid (comm) state ppid pgrp session tty_nr tpgid ...
    // tpgid (field 8, 0-indexed 7) is the foreground process group ID
    let tpgid = parse_stat_field(&stat_content, 7)?;

    if tpgid <= 0 {
        return Some(shell_pid);
    }

    // Find a process in the foreground process group
    // The tpgid is a process group ID, we need to find a process in that group
    find_process_in_group(tpgid as u32).or(Some(shell_pid))
}

fn find_process_in_group(pgrp: u32) -> Option<u32> {
    let proc_dir = Path::new("/proc");
    if !proc_dir.exists() {
        return None;
    }

    for entry in fs::read_dir(proc_dir).ok()? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if !name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let Ok(pid) = name_str.parse::<u32>() else {
            continue;
        };
        let stat_path = entry.path().join("stat");

        if let Ok(content) = fs::read_to_string(&stat_path) {
            if let Some(proc_pgrp) = parse_stat_field(&content, 4) {
                if proc_pgrp as u32 == pgrp {
                    return Some(pid);
                }
            }
        }
    }

    None
}

fn parse_stat_field(content: &str, field_idx: usize) -> Option<i64> {
    let close_paren = content.rfind(')')?;
    if close_paren + 2 > content.len() {
        return None;
    }
    let after_comm = &content[close_paren + 2..];

    let adjusted_idx = field_idx.checked_sub(2)?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    fields.get(adjusted_idx)?.parse().ok()
}

/// Spawn a background thread that monitors D-Bus for `PrepareForSleep(false)`
/// signals from systemd-logind. When detected (host just woke), triggers
/// `resync_sbx_clocks` to fix stale guest clocks in sbx microVMs.
pub(super) fn register_wake_handler() {
    std::thread::spawn(|| {
        use std::io::BufRead;
        use std::process::{Command, Stdio};

        let child = Command::new("busctl")
            .args([
                "monitor",
                "--system",
                "--match",
                "type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    target: "process.wake",
                    error = %e,
                    "Failed to spawn busctl for wake monitoring"
                );
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };

        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim() == "BOOLEAN false" || line.contains("PrepareForSleep(false)") {
                tracing::info!(target: "process.wake", "host woke from sleep; resyncing sbx clocks");
                super::resync_sbx_clocks();
            }
        }

        let _ = child.kill();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stat_field() {
        let stat = "1234 (bash) S 1233 1234 1234 34816 1234 4194304 1234 0 0 0";

        assert_eq!(parse_stat_field(stat, 3), Some(1233));
        assert_eq!(parse_stat_field(stat, 4), Some(1234));
        assert_eq!(parse_stat_field(stat, 7), Some(1234));
    }

    #[test]
    fn test_parse_stat_field_truncated() {
        assert_eq!(parse_stat_field("1 (x)", 3), None);
        assert_eq!(parse_stat_field("1 (x)Z", 3), None);
        assert_eq!(parse_stat_field("no paren here", 3), None);
    }

    #[test]
    fn test_collect_descendants_from_map_empty() {
        let children_map = HashMap::new();
        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100]);
    }

    #[test]
    fn test_collect_descendants_from_map_nested() {
        // Tree: 100 -> 101 -> 102 -> 103
        let mut children_map = HashMap::new();
        children_map.insert(100, vec![101]);
        children_map.insert(101, vec![102]);
        children_map.insert(102, vec![103]);

        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100, 101, 102, 103]);
    }

    #[test]
    fn test_collect_descendants_from_map_branching() {
        // Tree: 100 -> [101, 102], 101 -> [103, 104], 102 -> [105]
        let mut children_map = HashMap::new();
        children_map.insert(100, vec![101, 102]);
        children_map.insert(101, vec![103, 104]);
        children_map.insert(102, vec![105]);

        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);

        assert!(pids.contains(&100));
        assert!(pids.contains(&101));
        assert!(pids.contains(&102));
        assert!(pids.contains(&103));
        assert!(pids.contains(&104));
        assert!(pids.contains(&105));
        assert_eq!(pids.len(), 6);
    }

    #[test]
    fn test_collect_descendants_unrelated_processes() {
        let mut children_map = HashMap::new();
        children_map.insert(200, vec![201, 202]);
        children_map.insert(300, vec![301]);

        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100]);
    }
}
