//! Integration tests for sbx sandbox lifecycle. Gated behind #[ignore]
//! per D-17; CI skips when sbx is unavailable.

use agent_of_empires::containers::container_interface::ContainerConfig;
use agent_of_empires::containers::sbx::ports::PortPublishResult;
use agent_of_empires::containers::sbx::SbxRuntime;

fn sbx_available() -> bool {
    SbxRuntime::new().is_available()
}

#[test]
#[ignore = "requires sbx daemon"]
fn lifecycle_create_exec_stop_rm() {
    if !sbx_available() {
        eprintln!("Skipping: sbx not available");
        return;
    }

    let name = format!("aoe-test-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let sbx = SbxRuntime::new();

    let config = ContainerConfig {
        working_dir: "/workspace".to_string(),
        volumes: vec![],
        anonymous_volumes: vec![],
        environment: vec![],
        cpu_limit: None,
        memory_limit: None,
        port_mappings: vec!["8080:8080".to_string()],
        agent_name: Some("claude".to_string()),
    };

    // Create
    let result = sbx.create_container(&name, "ubuntu:latest", &config, "claude");
    assert!(result.is_ok(), "create failed: {:?}", result.err());

    // Exec
    let exec_result = sbx.exec(&name, &["echo", "hello"], None, &[], false, false);
    assert!(exec_result.is_ok(), "exec failed: {:?}", exec_result.err());
    let output = exec_result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello"),
        "expected 'hello' in stdout: {}",
        stdout
    );

    // Publish ports
    let port_results = sbx.publish_ports(&name, &["8080:8080".to_string()]);
    assert!(!port_results.is_empty(), "publish_ports returned empty vec");

    // Stop
    let stop_result = sbx.stop_container(&name);
    assert!(stop_result.is_ok(), "stop failed: {:?}", stop_result.err());

    // Remove
    let rm_result = sbx.remove(&name, true);
    assert!(rm_result.is_ok(), "remove failed: {:?}", rm_result.err());

    // Verify gone
    let exists = sbx.does_container_exist(&name).unwrap_or(true);
    assert!(!exists, "sandbox should not exist after removal");
}

#[test]
#[ignore = "requires sbx daemon"]
fn batch_running_states_includes_created_sandbox() {
    if !sbx_available() {
        eprintln!("Skipping: sbx not available");
        return;
    }

    let name = format!("aoe-test-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let sbx = SbxRuntime::new();

    let config = ContainerConfig {
        working_dir: "/workspace".to_string(),
        volumes: vec![],
        anonymous_volumes: vec![],
        environment: vec![],
        cpu_limit: None,
        memory_limit: None,
        port_mappings: vec![],
        agent_name: Some("claude".to_string()),
    };

    let result = sbx.create_container(&name, "ubuntu:latest", &config, "claude");
    assert!(result.is_ok(), "create failed: {:?}", result.err());

    let states = sbx.batch_running_states("aoe-test-");
    assert!(
        states.contains_key(&name),
        "expected '{}' in batch states: {:?}",
        name,
        states
    );

    // Cleanup
    let _ = sbx.remove(&name, true);
}

#[test]
#[ignore = "requires sbx daemon"]
fn is_daemon_running_returns_true_when_sbx_configured() {
    if !sbx_available() {
        eprintln!("Skipping: sbx not available");
        return;
    }

    assert!(
        SbxRuntime::new().is_daemon_running(),
        "expected is_daemon_running() == true when sbx is available"
    );
}

// Suppress unused import warning when PortPublishResult is only used in
// type-checking the publish_ports return value above.
const _: () = {
    fn _assert_port_result_variants(r: &PortPublishResult) {
        match r {
            PortPublishResult::Ok(_) => {}
            PortPublishResult::Failed { .. } => {}
        }
    }
};
