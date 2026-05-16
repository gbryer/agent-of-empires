//! Integration tests for `cargo xtask check-kit`.
//!
//! Each test spawns the xtask binary via `cargo run -p xtask -- check-kit ...`
//! against a fixture spec.yaml or the embedded project spec.yaml, asserting on
//! the exit code and stderr substring.

fn run_check_kit(fixture: &str) -> std::process::Output {
    std::process::Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "xtask",
            "--",
            "check-kit",
            "--spec-path",
            fixture,
        ])
        .output()
        .expect("xtask invocation failed")
}

#[test]
fn ok_fixture_passes() {
    let out = run_check_kit("tests/fixtures/sbx_kits/ok/spec.yaml");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn missing_schema_version_fails() {
    let out = run_check_kit("tests/fixtures/sbx_kits/missing-schema-version/spec.yaml");
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("schemaVersion"),
        "stderr did not mention schemaVersion: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn naked_install_verb_fails() {
    let out = run_check_kit("tests/fixtures/sbx_kits/naked-install-verb/spec.yaml");
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("naked install verb"),
        "stderr did not mention naked install verb: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn guarded_install_verb_passes() {
    let out = run_check_kit("tests/fixtures/sbx_kits/guarded-install-verb/spec.yaml");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn install_command_array_format_fails() {
    let out = run_check_kit("tests/fixtures/sbx_kits/install-array-command/spec.yaml");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must be a string"),
        "stderr did not mention 'must be a string': {}",
        stderr
    );
}

#[test]
fn embedded_spec_passes() {
    // The project's own spec.yaml (authored in 04-01) must satisfy the lint.
    let out = std::process::Command::new("cargo")
        .args(["run", "-q", "-p", "xtask", "--", "check-kit"])
        .output()
        .expect("xtask invocation failed");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
