use std::process::Command;
use std::path::Path;
use tempfile::TempDir;

/// Test CLI binary exists and is executable
#[test]
fn test_cli_binary_exists() {
    let output = Command::new("cargo")
        .args(&["build", "--bin", "arkdrop"])
        .output()
        .expect("Failed to execute cargo build");
    
    assert!(output.status.success(), "Failed to build CLI binary");
    
    // Check that binary exists
    let binary_path = Path::new("target/debug/arkdrop");
    assert!(binary_path.exists(), "CLI binary not found at expected location");
}

/// Test CLI shows help information
#[test]
fn test_cli_help() {
    let output = Command::new("target/debug/arkdrop")
        .arg("--help")
        .output()
        .expect("Failed to run CLI help");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ARK Drop tool"));
    assert!(stdout.contains("send"));
    assert!(stdout.contains("receive"));
    assert!(stdout.contains("config"));
}

/// Test CLI version command
#[test]
fn test_cli_version() {
    let output = Command::new("target/debug/arkdrop")
        .arg("--version")
        .output()
        .expect("Failed to run CLI version");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.0.0"));
}

/// Test send command shows proper error for missing files
#[test]
fn test_send_missing_file() {
    let output = Command::new("target/debug/arkdrop")
        .args(&["send", "nonexistent.txt"])
        .output()
        .expect("Failed to run send command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The CLI should either show an error about missing file or show help due to missing arguments
    assert!(!output.status.success());
    assert!(stderr.contains("File does not exist") || stderr.contains("required") || stderr.contains("help"));
}

/// Test receive command shows proper error for missing arguments
#[test] 
fn test_receive_missing_args() {
    let output = Command::new("target/debug/arkdrop")
        .args(&["receive"])
        .output()
        .expect("Failed to run receive command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should show error about missing required arguments
    assert!(!output.status.success());
    assert!(stderr.contains("required") || stderr.contains("ticket") || stderr.contains("confirmation"));
}

/// Test config commands
#[test]
fn test_config_commands() {
    // Test config show
    let output = Command::new("target/debug/arkdrop")
        .args(&["config", "show"])
        .output()
        .expect("Failed to run config show");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show either a directory or "No default receive directory set"
    assert!(stdout.contains("directory") || stdout.contains("No default"));
    
    // Test setting and clearing receive directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_str().unwrap();
    
    // Set receive directory
    let output = Command::new("target/debug/arkdrop")
        .args(&["config", "set-receive-dir", temp_path])
        .output()
        .expect("Failed to set receive directory");
    
    assert!(output.status.success(), "Failed to set receive directory");
    
    // Verify it was set
    let output = Command::new("target/debug/arkdrop")
        .args(&["config", "show"])
        .output()
        .expect("Failed to show config");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(temp_path));
    
    // Clear receive directory
    let output = Command::new("target/debug/arkdrop")
        .args(&["config", "clear-receive-dir"])
        .output()
        .expect("Failed to clear receive directory");
    
    assert!(output.status.success(), "Failed to clear receive directory");
}

/// Test send command with valid files (should fail at connection stage)
#[test]
fn test_send_valid_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "test content").expect("Failed to create test file");
    
    // This should fail because there's no receiver, but it should validate the file first
    let output = Command::new("target/debug/arkdrop")
        .args(&["send", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run send command");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should show that it's preparing to send (file validation passed)
    // but will fail at network/connection stage
    assert!(
        stdout.contains("Preparing to send") || 
        stdout.contains("file(s)") ||
        stderr.contains("ticket") ||
        stderr.contains("confirmation")
    );
}

/// Test CLI handles file validation correctly
#[test]
fn test_file_validation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Create test files
    let valid_file = temp_dir.path().join("valid.txt");
    std::fs::write(&valid_file, "valid content").expect("Failed to create valid file");
    
    // Test with valid file - should pass validation, fail at connection
    let output = Command::new("target/debug/arkdrop")
        .args(&["send", "--name", "test-sender", valid_file.to_str().unwrap()])
        .output()
        .expect("Failed to run send with valid file");
    
    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Should mention preparing to send or show file info
    assert!(
        combined_output.contains("Preparing to send") ||
        combined_output.contains("valid.txt") ||
        combined_output.contains("Sender name")
    );
    
    // Test with nonexistent file - should fail validation
    let output = Command::new("target/debug/arkdrop")
        .args(&["send", "--name", "test-sender", "nonexistent.txt"])
        .output()
        .expect("Failed to run send with invalid file");
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("File does not exist") || stderr.contains("not found"));
}

/// Test receive command parameter validation
#[test]
fn test_receive_parameters() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let output_path = temp_dir.path().to_str().unwrap();
    
    // Test with all required parameters - should attempt connection and fail
    let output = Command::new("target/debug/arkdrop")
        .args(&[
            "receive", 
            "test-ticket", 
            "123", 
            "--output", output_path,
            "--name", "test-receiver"
        ])
        .output()
        .expect("Failed to run receive command");
    
    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Should show that it's preparing to receive or connection attempts
    assert!(
        combined_output.contains("Preparing to receive") ||
        combined_output.contains("test-ticket") ||
        combined_output.contains("Receiver name") ||
        combined_output.contains("Output directory")
    );
}

/// Test avatar handling
#[test]
fn test_avatar_options() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let avatar_file = temp_dir.path().join("avatar.png");
    let test_file = temp_dir.path().join("test.txt");
    
    // Create dummy files
    std::fs::write(&avatar_file, b"fake image data").expect("Failed to create avatar file");
    std::fs::write(&test_file, "test content").expect("Failed to create test file");
    
    // Test with avatar file
    let output = Command::new("target/debug/arkdrop")
        .args(&[
            "send",
            "--name", "avatar-sender",
            "--avatar", avatar_file.to_str().unwrap(),
            test_file.to_str().unwrap()
        ])
        .output()
        .expect("Failed to run send with avatar");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should indicate avatar is set
    assert!(stdout.contains("Avatar: Set") || stdout.contains("avatar-sender"));
    
    // Test with base64 avatar
    let output = Command::new("target/debug/arkdrop")
        .args(&[
            "send",
            "--name", "b64-sender", 
            "--avatar-b64", "dGVzdA==",
            test_file.to_str().unwrap()
        ])
        .output()
        .expect("Failed to run send with base64 avatar");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Avatar: Set") || stdout.contains("b64-sender"));
}

/// Test verbose flag
#[test]
fn test_verbose_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test content").expect("Failed to create test file");
    
    let output = Command::new("target/debug/arkdrop")
        .args(&[
            "send",
            "--verbose",
            "--name", "verbose-sender",
            test_file.to_str().unwrap()
        ])
        .output()
        .expect("Failed to run send with verbose flag");
    
    // Verbose flag should not cause errors and should be accepted
    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    
    assert!(
        combined_output.contains("verbose-sender") ||
        combined_output.contains("Preparing to send")
    );
}