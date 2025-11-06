use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::{PredicateBooleanExt, predicate};
use tempfile::TempDir;

/// Test CLI binary exists and is executable
#[test]
fn test_cli_binary_exists() {
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.arg("--version").assert().success();
}

/// Test CLI shows help information
#[test]
fn test_cli_help() {
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("ARK Drop tool"))
        .stdout(predicate::str::contains("send"))
        .stdout(predicate::str::contains("receive"))
        .stdout(predicate::str::contains("config"));
}

/// Test CLI version command
#[test]
fn test_cli_version() {
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("1.0.0"));
}

/// Test send command shows proper error for missing files
#[test]
fn test_send_missing_file() {
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["send", "nonexistent.txt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("File does not exist"));
}

/// Test receive command shows proper error for missing arguments
#[test]
fn test_receive_missing_args() {
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.arg("receive")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

/// Test config commands
#[test]
fn test_config_commands() {
    // Test config show
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["config", "show"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("directory")
                .or(predicate::str::contains("No default")),
        );

    // Test setting and clearing receive directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_str().unwrap();

    // Set receive directory
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["config", "set-output", temp_path])
        .assert()
        .success();

    // Verify it was set
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(temp_path));

    // Clear receive directory
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["config", "clear-output"])
        .assert()
        .success();
}

/// Test receive command parameter validation
#[test]
fn test_receive_parameters() {
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["receive", "test-ticket", "123", "--name", "test-receiver"])
        .assert()
        .stdout(
            predicate::str::contains("Preparing to receive")
                .or(predicate::str::contains("test-ticket"))
                .or(predicate::str::contains("Receiver name"))
                .or(predicate::str::contains("Output directory")),
        );
}
