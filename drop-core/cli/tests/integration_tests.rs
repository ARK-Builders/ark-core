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
    cmd.args(["config", "set-receive-dir", temp_path])
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
    cmd.args(["config", "clear-receive-dir"])
        .assert()
        .success();
}

/// Test send command with valid files (should fail at connection stage)
#[test]
fn test_send_valid_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "test content")
        .expect("Failed to create test file");

    // This should fail because there's no receiver, but it should validate the
    // file first
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["send", file_path.to_str().unwrap()])
        .assert()
        .code(predicate::ne(0))
        .stdout(
            predicate::str::contains("Preparing to send")
                .or(predicate::str::contains("file(s)"))
                .or(predicate::str::contains("ticket"))
                .or(predicate::str::contains("confirmation")),
        );
}

/// Test CLI handles file validation correctly
#[test]
fn test_file_validation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create test files
    let valid_file = temp_dir.path().join("valid.txt");
    std::fs::write(&valid_file, "valid content")
        .expect("Failed to create valid file");

    // Test with valid file - should pass validation, fail at connection
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args([
        "send",
        "--name",
        "test-sender",
        valid_file.to_str().unwrap(),
    ])
    .assert()
    .stdout(
        predicate::str::contains("Preparing to send")
            .or(predicate::str::contains("valid.txt"))
            .or(predicate::str::contains("Sender name")),
    );

    // Test with nonexistent file - should fail validation
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args(["send", "--name", "test-sender", "nonexistent.txt"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("File does not exist")
                .or(predicate::str::contains("not found")),
        );
}

/// Test receive command parameter validation
#[test]
fn test_receive_parameters() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let output_path = temp_dir.path().to_str().unwrap();

    // Test with all required parameters - should attempt connection and fail
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args([
        "receive",
        "test-ticket",
        "123",
        "--output",
        output_path,
        "--name",
        "test-receiver",
    ])
    .assert()
    .stdout(
        predicate::str::contains("Preparing to receive")
            .or(predicate::str::contains("test-ticket"))
            .or(predicate::str::contains("Receiver name"))
            .or(predicate::str::contains("Output directory")),
    );
}

/// Test avatar handling
#[test]
fn test_avatar_options() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let avatar_file = temp_dir.path().join("avatar.png");
    let test_file = temp_dir.path().join("test.txt");

    // Create dummy files
    std::fs::write(&avatar_file, b"fake image data")
        .expect("Failed to create avatar file");
    std::fs::write(&test_file, "test content")
        .expect("Failed to create test file");

    // Test with avatar file
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args([
        "send",
        "--name",
        "avatar-sender",
        "--avatar",
        avatar_file.to_str().unwrap(),
        test_file.to_str().unwrap(),
    ])
    .assert()
    .stdout(
        predicate::str::contains("Avatar: Set")
            .or(predicate::str::contains("avatar-sender")),
    );

    // Test with base64 avatar
    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args([
        "send",
        "--name",
        "b64-sender",
        "--avatar-b64",
        "dGVzdA==",
        test_file.to_str().unwrap(),
    ])
    .assert()
    .stdout(
        predicate::str::contains("Avatar: Set")
            .or(predicate::str::contains("b64-sender")),
    );
}

/// Test verbose flag
#[test]
fn test_verbose_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test content")
        .expect("Failed to create test file");

    let mut cmd = cargo_bin_cmd!("arkdrop-cli");
    cmd.args([
        "send",
        "--verbose",
        "--name",
        "verbose-sender",
        test_file.to_str().unwrap(),
    ])
    .assert()
    .stdout(
        predicate::str::contains("verbose-sender")
            .or(predicate::str::contains("Preparing to send")),
    );
}
