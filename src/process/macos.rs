//! macOS-specific process utilities

use std::collections::HashMap;
use std::process::Command;

/// Collect `pid` and every descendant by parsing `ps -A` once and walking the map.
pub(super) fn collect_pid_tree(pid: u32) -> Vec<u32> {
    let children_map = build_children_map();
    let mut pids = vec![pid];
    collect_descendants_from_map(pid, &children_map, &mut pids);
    pids
}

/// Build a map of parent PID -> list of child PIDs by parsing `ps` output once
fn build_children_map() -> HashMap<u32, Vec<u32>> {
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();

    let Ok(output) = Command::new("ps").args(["-o", "pid=,ppid=", "-A"]).output() else {
        return children_map;
    };

    if !output.status.success() {
        return children_map;
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Ok(child_pid), Ok(ppid)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                children_map.entry(ppid).or_default().push(child_pid);
            }
        }
    }

    children_map
}

/// Recursively collect all descendant PIDs using the pre-built children map
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
pub fn get_foreground_pid(shell_pid: u32) -> Option<u32> {
    // Use ps to get the foreground process group
    // ps -o tpgid= -p <pid> gives us the terminal foreground process group ID
    let output = Command::new("ps")
        .args(["-o", "tpgid=", "-p", &shell_pid.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return Some(shell_pid);
    }

    let tpgid: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;

    if tpgid <= 0 {
        return Some(shell_pid);
    }

    // Find a process in the foreground group
    find_process_in_group(tpgid as u32).or(Some(shell_pid))
}

/// Find a process belonging to the given process group
fn find_process_in_group(pgrp: u32) -> Option<u32> {
    // Use ps to find processes in this group
    // ps -o pid=,pgid= -A lists all processes with their PIDs and PGIDs
    let output = Command::new("ps")
        .args(["-o", "pid=,pgid=", "-A"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Ok(pid), Ok(proc_pgrp)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                if proc_pgrp == pgrp {
                    return Some(pid);
                }
            }
        }
    }

    None
}

/// Spawn a background thread that listens for IOKit power notifications. On
/// `kIOMessageSystemHasPoweredOn` (host woke from sleep), triggers
/// `resync_sbx_clocks` to fix stale guest clocks in sbx microVMs.
pub(super) fn register_wake_handler() {
    std::thread::spawn(|| {
        // IOKit FFI types
        type IONotificationPortRef = *mut std::ffi::c_void;
        type IOObject = u32;
        type CFRunLoopSourceRef = *mut std::ffi::c_void;
        type CFRunLoopRef = *mut std::ffi::c_void;
        type CFStringRef = *const std::ffi::c_void;

        type IOServiceInterestCallback = extern "C" fn(
            refcon: *mut std::ffi::c_void,
            service: IOObject,
            message_type: u32,
            message_argument: *mut std::ffi::c_void,
        );

        #[link(name = "IOKit", kind = "framework")]
        extern "C" {
            fn IORegisterForSystemPower(
                refcon: *mut std::ffi::c_void,
                the_port_ref: *mut IONotificationPortRef,
                callback: IOServiceInterestCallback,
                notifier: *mut IOObject,
            ) -> IOObject;
            fn IONotificationPortGetRunLoopSource(
                notify: IONotificationPortRef,
            ) -> CFRunLoopSourceRef;
        }

        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFRunLoopGetCurrent() -> CFRunLoopRef;
            fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
            fn CFRunLoopRun();

            static kCFRunLoopDefaultMode: CFStringRef;
        }

        extern "C" fn power_callback(
            _refcon: *mut std::ffi::c_void,
            _service: IOObject,
            message_type: u32,
            _message_argument: *mut std::ffi::c_void,
        ) {
            // 0xe0000300 == kIOMessageSystemHasPoweredOn
            if message_type == 0xe000_0300 {
                tracing::info!(target: "process.wake", "host woke from sleep; resyncing sbx clocks");
                super::resync_sbx_clocks();
            }
        }

        let mut notify_port: IONotificationPortRef = std::ptr::null_mut();
        let mut notifier: IOObject = 0;

        let root_port = unsafe {
            IORegisterForSystemPower(
                std::ptr::null_mut(),
                &mut notify_port,
                power_callback,
                &mut notifier,
            )
        };

        if root_port == 0 {
            tracing::warn!(
                target: "process.wake",
                "Failed to register for IOKit power notifications"
            );
            return;
        }

        unsafe {
            let source = IONotificationPortGetRunLoopSource(notify_port);
            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode);
            CFRunLoopRun();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_descendants_from_map_empty() {
        let children_map = HashMap::new();
        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100]);
    }

    #[test]
    fn test_collect_descendants_from_map_single_child() {
        let mut children_map = HashMap::new();
        children_map.insert(100, vec![101]);

        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100, 101]);
    }

    #[test]
    fn test_collect_descendants_from_map_multiple_children() {
        let mut children_map = HashMap::new();
        children_map.insert(100, vec![101, 102, 103]);

        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100, 101, 102, 103]);
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

        // Should contain all PIDs
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
        // Map has processes, but none are descendants of 100
        let mut children_map = HashMap::new();
        children_map.insert(200, vec![201, 202]);
        children_map.insert(300, vec![301]);

        let mut pids = vec![100];
        collect_descendants_from_map(100, &children_map, &mut pids);
        assert_eq!(pids, vec![100]);
    }
}
