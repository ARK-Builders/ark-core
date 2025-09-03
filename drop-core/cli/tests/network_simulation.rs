use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use tempfile::TempDir;

/// Test helper to simulate network conditions
pub struct NetworkSimulator {
    interface: String,
    original_state: Option<String>,
}

impl NetworkSimulator {
    pub fn new(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
            original_state: None,
        }
    }

    /// Add packet loss to the network interface
    pub fn add_packet_loss(
        &mut self,
        percentage: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.save_state()?;

        Command::new("sudo")
            .args(&[
                "tc",
                "qdisc",
                "add",
                "dev",
                &self.interface,
                "root",
                "netem",
                "loss",
                &format!("{}%", percentage),
            ])
            .output()?;

        Ok(())
    }

    /// Add latency to the network interface
    pub fn add_latency(
        &mut self,
        delay_ms: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.save_state()?;

        Command::new("sudo")
            .args(&[
                "tc",
                "qdisc",
                "add",
                "dev",
                &self.interface,
                "root",
                "netem",
                "delay",
                &format!("{}ms", delay_ms),
            ])
            .output()?;

        Ok(())
    }

    /// Add bandwidth limitation
    pub fn limit_bandwidth(
        &mut self,
        rate_kbps: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.save_state()?;

        Command::new("sudo")
            .args(&[
                "tc",
                "qdisc",
                "add",
                "dev",
                &self.interface,
                "root",
                "tbf",
                "rate",
                &format!("{}kbit", rate_kbps),
                "burst",
                "32kbit",
                "latency",
                "400ms",
            ])
            .output()?;

        Ok(())
    }

    /// Add jitter (variable latency)
    pub fn add_jitter(
        &mut self,
        base_delay_ms: u32,
        jitter_ms: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.save_state()?;

        Command::new("sudo")
            .args(&[
                "tc",
                "qdisc",
                "add",
                "dev",
                &self.interface,
                "root",
                "netem",
                "delay",
                &format!("{}ms", base_delay_ms),
                &format!("{}ms", jitter_ms),
                "distribution",
                "normal",
            ])
            .output()?;

        Ok(())
    }

    /// Save current network state
    fn save_state(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("sudo")
            .args(&["tc", "qdisc", "show", "dev", &self.interface])
            .output()?;

        self.original_state =
            Some(String::from_utf8_lossy(&output.stdout).to_string());
        Ok(())
    }

    /// Reset network to original state
    pub fn reset(&self) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("sudo")
            .args(&["tc", "qdisc", "del", "dev", &self.interface, "root"])
            .output()
            .ok(); // Ignore errors as there might not be any qdisc to delete

        Ok(())
    }
}

impl Drop for NetworkSimulator {
    fn drop(&mut self) {
        // Always try to reset network state on cleanup
        let _ = self.reset();
    }
}

/// Helper to run arkdrop commands
pub struct ARKDropRunner {
    binary_path: PathBuf,
}

impl ARKDropRunner {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Build the CLI in release mode if not already built
        Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(".")
            .output()?;

        Ok(Self {
            binary_path: PathBuf::from("target/release/arkdrop"),
        })
    }

    /// Start a receiver and return the process handle and connection info
    pub fn start_receiver(
        &self,
        name: &str,
        output_dir: &PathBuf,
    ) -> Result<(std::process::Child, String, String), Box<dyn std::error::Error>>
    {
        let child = Command::new(&self.binary_path)
            .args(&[
                "receive",
                "--name",
                name,
                "--dir",
                output_dir.to_str().unwrap(),
                "--verbose",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for receiver to start and extract connection info
        thread::sleep(Duration::from_secs(2));

        // In a real implementation, we'd parse stdout to get ticket and code
        // For testing, we'll use placeholder values
        let ticket = "test-ticket-123";
        let code = "test-code-456";

        Ok((child, ticket.to_string(), code.to_string()))
    }

    /// Send files
    pub fn send_files(
        &self,
        name: &str,
        files: Vec<PathBuf>,
        ticket: &str,
        code: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut args = vec![
            "send".to_string(),
            "--name".to_string(),
            name.to_string(),
            "--verbose".to_string(),
        ];

        for file in files {
            args.push(file.to_str().unwrap().to_string());
        }

        args.push(ticket.to_string());
        args.push(code.to_string());

        let output = Command::new(&self.binary_path)
            .args(&args)
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Send command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires sudo permissions
    fn test_packet_loss_simulation() {
        let mut simulator = NetworkSimulator::new("lo");
        simulator
            .add_packet_loss(5.0)
            .expect("Failed to add packet loss");

        // Run transfer test here

        simulator
            .reset()
            .expect("Failed to reset network");
    }

    #[test]
    #[ignore] // Requires sudo permissions
    fn test_latency_simulation() {
        let mut simulator = NetworkSimulator::new("lo");
        simulator
            .add_latency(100)
            .expect("Failed to add latency");

        // Run transfer test here

        simulator
            .reset()
            .expect("Failed to reset network");
    }

    #[test]
    fn test_basic_file_transfer() {
        let runner = ARKDropRunner::new().expect("Failed to create runner");
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_dir = temp_dir.path().join("output");
        fs::create_dir(&output_dir).expect("Failed to create output dir");

        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Test content")
            .expect("Failed to write test file");

        // Start receiver
        let (mut receiver, ticket, code) = runner
            .start_receiver("Test Receiver", &output_dir)
            .expect("Failed to start receiver");

        // Send file
        runner
            .send_files("Test Sender", vec![test_file.clone()], &ticket, &code)
            .expect("Failed to send files");

        // Wait for transfer to complete
        thread::sleep(Duration::from_secs(2));

        // Verify file was received
        let received_file = output_dir.join("test.txt");
        assert!(received_file.exists(), "File was not received");

        let content = fs::read_to_string(&received_file)
            .expect("Failed to read received file");
        assert_eq!(content, "Test content", "File content mismatch");

        // Clean up
        receiver.kill().ok();
    }

    #[test]
    fn test_multiple_file_transfer() {
        let runner = ARKDropRunner::new().expect("Failed to create runner");
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_dir = temp_dir.path().join("output");
        fs::create_dir(&output_dir).expect("Failed to create output dir");

        // Create multiple test files
        let mut test_files = Vec::new();
        for i in 0..3 {
            let file = temp_dir.path().join(format!("test{}.txt", i));
            fs::write(&file, format!("Content {}", i))
                .expect("Failed to write test file");
            test_files.push(file);
        }

        // Start receiver
        let (mut receiver, ticket, code) = runner
            .start_receiver("Multi Receiver", &output_dir)
            .expect("Failed to start receiver");

        // Send files
        runner
            .send_files("Multi Sender", test_files.clone(), &ticket, &code)
            .expect("Failed to send files");

        // Wait for transfer to complete
        thread::sleep(Duration::from_secs(3));

        // Verify all files were received
        for i in 0..3 {
            let received_file = output_dir.join(format!("test{}.txt", i));
            assert!(received_file.exists(), "File {} was not received", i);

            let content = fs::read_to_string(&received_file)
                .expect("Failed to read received file");
            assert_eq!(
                content,
                format!("Content {}", i),
                "File {} content mismatch",
                i
            );
        }

        // Clean up
        receiver.kill().ok();
    }

    #[test]
    #[ignore] // Requires network setup
    fn test_nat_traversal() {
        // This test would require setting up network namespaces
        // and is more suitable for the GitHub Actions workflow

        // The actual implementation would:
        // 1. Create network namespaces
        // 2. Setup NAT rules
        // 3. Run sender and receiver in different namespaces
        // 4. Verify successful transfer through NAT
    }
}
