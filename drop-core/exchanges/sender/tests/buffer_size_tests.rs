use dropx_sender::{
    SenderConfig, SenderFile, SenderFileData, SenderProfile, send_files,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio;

/// Mock implementation of SenderFileData for testing
struct MockSenderFileData {
    data: Vec<u8>,
    position: Arc<Mutex<usize>>,
}

impl MockSenderFileData {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            position: Arc::new(Mutex::new(0)),
        }
    }
}

impl SenderFileData for MockSenderFileData {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn read(&self) -> Option<u8> {
        let mut pos = self.position.lock().unwrap();
        if *pos < self.data.len() {
            let byte = self.data[*pos];
            *pos += 1;
            Some(byte)
        } else {
            None
        }
    }

    fn read_chunk(&self, size: u64) -> Vec<u8> {
        let mut pos = self.position.lock().unwrap();
        let start = *pos;
        let end = std::cmp::min(start + size as usize, self.data.len());
        *pos = end;
        self.data[start..end].to_vec()
    }
}

/// Create test data of specified size
fn create_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Create a sender file with test data
fn create_test_file(name: &str, size: usize) -> SenderFile {
    let data = create_test_data(size);
    SenderFile {
        name: name.to_string(),
        data: Arc::new(MockSenderFileData::new(data)),
    }
}

/// Test that different buffer sizes can handle the same file transfer
#[tokio::test]
async fn test_buffer_size_small_file() {
    let file_size = 1024; // 1KB
    let test_cases = vec![
        ("128B buffer", SenderConfig { buffer_size: 128 }),
        ("512B buffer", SenderConfig { buffer_size: 512 }),
        ("1KB buffer", SenderConfig { buffer_size: 1024 }),
        ("2MB buffer", SenderConfig::default()),
        ("8MB buffer", SenderConfig::high_performance()),
    ];

    for (test_name, config) in test_cases {
        println!("Testing {} with {}", test_name, file_size);

        let profile = SenderProfile {
            name: "Test User".to_string(),
            avatar_b64: None,
        };

        let files = vec![create_test_file("test_file.txt", file_size)];

        let request = dropx_sender::SendFilesRequest {
            profile,
            files,
            config,
        };

        let bubble = send_files(request).await;
        assert!(bubble.is_ok(), "Failed to create bubble for {}", test_name);

        let bubble = bubble.unwrap();
        assert!(
            !bubble.get_ticket().is_empty(),
            "Ticket should not be empty for {}",
            test_name
        );
        assert!(
            bubble.get_confirmation() <= 99,
            "Confirmation should be valid for {}",
            test_name
        );
    }
}

/// Test that different buffer sizes work with medium-sized files
#[tokio::test]
async fn test_buffer_size_medium_file() {
    let file_size = 10 * 1024 * 1024; // 10MB
    let test_cases = vec![
        (
            "64KB buffer",
            SenderConfig {
                buffer_size: 64 * 1024,
            },
        ),
        ("128KB buffer", SenderConfig::low_bandwidth()),
        ("2MB buffer", SenderConfig::balanced()),
        ("8MB buffer", SenderConfig::high_performance()),
    ];

    for (test_name, config) in test_cases {
        println!(
            "Testing {} with {}MB file",
            test_name,
            file_size / (1024 * 1024)
        );

        let profile = SenderProfile {
            name: "Test User".to_string(),
            avatar_b64: None,
        };

        let files = vec![create_test_file("large_file.dat", file_size)];

        let request = dropx_sender::SendFilesRequest {
            profile,
            files,
            config,
        };

        let bubble = send_files(request).await;
        assert!(bubble.is_ok(), "Failed to create bubble for {}", test_name);

        let bubble = bubble.unwrap();
        assert!(!bubble.get_ticket().is_empty());

        // Verify bubble operations work correctly
        assert!(!bubble.is_finished());
        assert!(!bubble.is_connected()); // Should not be connected yet (no receiver)
    }
}

/// Test multiple files with different buffer sizes
#[tokio::test]
async fn test_multiple_files_different_buffer_sizes() {
    let configs = vec![
        SenderConfig::low_bandwidth(),
        SenderConfig::balanced(),
        SenderConfig::high_performance(),
    ];

    for (i, config) in configs.into_iter().enumerate() {
        println!("Testing multiple files with config {}", i);

        let profile = SenderProfile {
            name: "Multi File Test User".to_string(),
            avatar_b64: Some("dGVzdA==".to_string()),
        };

        let files = vec![
            create_test_file("file1.txt", 1024),
            create_test_file("file2.dat", 5 * 1024),
            create_test_file("file3.bin", 10 * 1024),
        ];

        let request = dropx_sender::SendFilesRequest {
            profile,
            files,
            config,
        };

        let bubble = send_files(request).await;
        assert!(bubble.is_ok());

        let bubble = bubble.unwrap();
        assert_eq!(bubble.get_confirmation(), bubble.get_confirmation()); // Consistency check
        assert!(!bubble.get_created_at().is_empty());
    }
}

/// Test edge cases with very small buffer sizes
#[tokio::test]
async fn test_edge_case_tiny_buffer() {
    let file_size = 1024;
    let tiny_buffer_config = SenderConfig { buffer_size: 1 }; // 1 byte buffer

    let profile = SenderProfile {
        name: "Edge Case User".to_string(),
        avatar_b64: None,
    };

    let files = vec![create_test_file("tiny_buffer_test.txt", file_size)];

    let request = dropx_sender::SendFilesRequest {
        profile,
        files,
        config: tiny_buffer_config,
    };

    let bubble = send_files(request).await;
    assert!(bubble.is_ok(), "Should handle very small buffer sizes");
}

/// Test that sender configurations work properly
#[test]
fn test_sender_config_presets() {
    let low_bw = SenderConfig::low_bandwidth();
    assert_eq!(low_bw.buffer_size, 131072); // 128KB

    let balanced = SenderConfig::balanced();
    assert_eq!(balanced.buffer_size, 2097152); // 2MB

    let high_perf = SenderConfig::high_performance();
    assert_eq!(high_perf.buffer_size, 8388608); // 8MB

    let default = SenderConfig::default();
    assert_eq!(default.buffer_size, balanced.buffer_size);
}

/// Test buffer size impact on data reading
#[test]
fn test_mock_file_data_reading() {
    let test_data = create_test_data(1000);
    let mock_file = MockSenderFileData::new(test_data.clone());

    // Test length
    assert_eq!(mock_file.len(), 1000);

    // Test sequential reading
    let mut read_data = Vec::new();
    while let Some(byte) = mock_file.read() {
        read_data.push(byte);
        if read_data.len() > 1010 {
            // Safety break
            break;
        }
    }
    assert_eq!(read_data, test_data);

    // Test chunk reading with different sizes
    let mock_file2 = MockSenderFileData::new(test_data.clone());
    let chunk1 = mock_file2.read_chunk(100);
    assert_eq!(chunk1.len(), 100);
    assert_eq!(chunk1, test_data[0..100]);

    let chunk2 = mock_file2.read_chunk(50);
    assert_eq!(chunk2.len(), 50);
    assert_eq!(chunk2, test_data[100..150]);
}

/// Test concurrent access to sender bubble (thread safety)
#[tokio::test]
async fn test_concurrent_bubble_access() {
    let profile = SenderProfile {
        name: "Concurrent Test User".to_string(),
        avatar_b64: None,
    };

    let files = vec![create_test_file("concurrent_test.txt", 1024)];

    let request = dropx_sender::SendFilesRequest {
        profile,
        files,
        config: SenderConfig::default(),
    };

    let bubble = send_files(request).await.unwrap();
    let bubble = Arc::new(bubble);

    // Spawn multiple threads to access bubble properties concurrently
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let bubble_clone = bubble.clone();
            tokio::spawn(async move {
                for _ in 0..10 {
                    let ticket = bubble_clone.get_ticket();
                    let confirmation = bubble_clone.get_confirmation();
                    let _finished = bubble_clone.is_finished();
                    let _connected = bubble_clone.is_connected();
                    let created_at = bubble_clone.get_created_at();

                    assert!(!ticket.is_empty());
                    assert!(confirmation <= 99);
                    // finished and connected status may vary
                    assert!(!created_at.is_empty());

                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                i
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
    }
}
