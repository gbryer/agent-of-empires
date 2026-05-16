use serial_test::serial;

use crate::harness::{app_dir_in, require_tmux, TuiTestHarness};

/// Exercises `aoe add --sandbox` which builds the full container config.
/// This would have caught the duplicate mount points bug (commit 92d2e53).
///
/// Requires a running Docker daemon -- marked `#[ignore]` for CI.
#[test]
#[serial]
#[ignore = "requires Docker daemon"]
fn test_cli_add_with_sandbox() {
    let h = TuiTestHarness::new("cli_sandbox");
    let project = h.project_path();

    let output = h.run_cli(&[
        "add",
        project.to_str().unwrap(),
        "-t",
        "Sandbox E2E",
        "--sandbox",
    ]);
    assert!(
        output.status.success(),
        "aoe add --sandbox failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let list_output = h.run_cli(&["list", "--json"]);
    assert!(list_output.status.success());

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        stdout.contains("Sandbox E2E"),
        "list should contain the sandboxed session.\nOutput:\n{}",
        stdout
    );
}

/// Verifies the sbx runtime appears in the settings TUI runtime selector and
/// that sbx-incompatible fields (ExtraVolumes, VolumeIgnores, MountSsh) are
/// hidden when the runtime is set to sbx.
///
/// Pre-seeds config.toml with `container_runtime = "sbx"`, opens the TUI
/// settings Sandbox category, and asserts the expected labels are (or are not)
/// present on screen.
#[test]
#[serial]
#[ignore = "requires sbx daemon"]
fn test_sbx_runtime_selector() {
    require_tmux!();

    let mut h = TuiTestHarness::new("sbx_runtime_selector");

    // Pre-seed config with sbx as the container runtime.
    let config_dir = app_dir_in(h.home_path());
    let config_path = config_dir.join("config.toml");
    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    let updated = if existing.contains("[sandbox]") {
        existing.replace("[sandbox]", "[sandbox]\ncontainer_runtime = \"sbx\"")
    } else {
        format!("{}\n[sandbox]\ncontainer_runtime = \"sbx\"\n", existing)
    };
    std::fs::write(&config_path, updated).expect("write config.toml");

    h.spawn_tui();

    // Open settings with 's'.
    h.send_keys("s");
    h.wait_for("Theme");

    // Navigate to Sandbox category (index 3: Theme=0, Session=1, Hooks=2, Sandbox=3).
    h.send_keys("Tab");
    h.send_keys("Tab");
    h.send_keys("Tab");
    h.wait_for("Sandbox");

    // Enter the Sandbox fields.
    h.send_keys("Enter");
    h.wait_for("Container Runtime");

    // The runtime label for Sbx is "Docker Sandboxes".
    h.assert_screen_contains("Docker Sandboxes");

    // Sbx-incompatible fields should NOT appear.
    h.assert_screen_not_contains("Extra Volumes");
    h.assert_screen_not_contains("Volume Ignores");
    h.assert_screen_not_contains("Mount SSH");

    // Fields that should still be present.
    h.assert_screen_contains("Enabled by Default");
    h.assert_screen_contains("Default Image");

    // Verify config persists on disk.
    let config_content = std::fs::read_to_string(&config_path).expect("read config");
    assert!(
        config_content.contains("container_runtime = \"sbx\""),
        "sbx runtime should persist in config.toml"
    );
}
