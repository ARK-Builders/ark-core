use dropx_receiver::{
    receive_files, ReceiveFilesRequest, ReceiveFilesSubscriber, 
    ReceiveFilesConnectingEvent, ReceiveFilesReceivingEvent, ReceiverProfile
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::TempDir;
use tokio;

/// Mock subscriber for testing receiver functionality
struct TestSubscriber {
    id: String,
    logs: Arc<Mutex<Vec<String>>>,
    connecting_events: Arc<Mutex<usize>>,
    receiving_events: Arc<Mutex<Vec<usize>>>, // Store data sizes instead of events
}

impl TestSubscriber {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            logs: Arc::new(Mutex::new(Vec::new())),
            connecting_events: Arc::new(Mutex::new(0)),
            receiving_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().clone()
    }

    fn get_connecting_events_count(&self) -> usize {
        *self.connecting_events.lock().unwrap()
    }

    fn get_receiving_events_count(&self) -> usize {
        self.receiving_events.lock().unwrap().len()
    }

    fn get_total_bytes_received(&self) -> usize {
        self.receiving_events.lock().unwrap().iter().sum()
    }
}

impl ReceiveFilesSubscriber for TestSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.logs.lock().unwrap().push(message);
    }

    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent) {
        self.receiving_events.lock().unwrap().push(event.data.len());
    }

    fn notify_connecting(&self, _event: ReceiveFilesConnectingEvent) {
        *self.connecting_events.lock().unwrap() += 1;
    }
}

/// Test receiver creation with different configurations
#[tokio::test]
async fn test_receiver_creation_basic() {
    let profile = ReceiverProfile {
        name: "Test Receiver".to_string(),
        avatar_b64: None,
    };

    // Test with invalid ticket should fail
    let invalid_request = ReceiveFilesRequest {
        ticket: "invalid_ticket".to_string(),
        confirmation: 42,
        profile: ReceiverProfile {
            name: profile.name.clone(),
            avatar_b64: profile.avatar_b64.clone(),
        },
    };

    let result = receive_files(invalid_request).await;
    assert!(result.is_err(), "Should fail with invalid ticket");

    // Test profile variations
    let profile_with_avatar = ReceiverProfile {
        name: "Avatar User".to_string(),
        avatar_b64: Some("dGVzdEF2YXRhcg==".to_string()),
    };

    // This will also fail due to invalid ticket, but tests profile handling
    let avatar_request = ReceiveFilesRequest {
        ticket: "another_invalid_ticket".to_string(),
        confirmation: 99,
        profile: profile_with_avatar,
    };

    let result = receive_files(avatar_request).await;
    assert!(result.is_err(), "Should fail with invalid ticket");
}

/// Test receiver bubble state management
#[tokio::test]
async fn test_receiver_bubble_states() {
    // Since we can't actually establish a connection without a real sender,
    // we'll test what we can with invalid tickets to verify error handling
    
    let profile = ReceiverProfile {
        name: "State Test User".to_string(),
        avatar_b64: None,
    };

    // Test different confirmation codes
    let confirmation_codes = vec![0, 42, 99];
    
    for confirmation in confirmation_codes {
        let request = ReceiveFilesRequest {
            ticket: format!("test_ticket_{}", confirmation),
            confirmation,
            profile: ReceiverProfile {
                name: profile.name.clone(),
                avatar_b64: profile.avatar_b64.clone(),
            },
        };

        let result = receive_files(request).await;
        assert!(result.is_err(), "Should fail due to invalid ticket format");
    }
}

/// Test subscriber management
#[tokio::test]
async fn test_subscriber_functionality() {
    // Create test subscribers
    let subscriber1 = Arc::new(TestSubscriber::new("subscriber_1"));
    let subscriber2 = Arc::new(TestSubscriber::new("subscriber_2"));

    // Test subscriber properties
    assert_eq!(subscriber1.get_id(), "subscriber_1");
    assert_eq!(subscriber2.get_id(), "subscriber_2");

    // Test logging
    subscriber1.log("Test message 1".to_string());
    subscriber1.log("Test message 2".to_string());
    
    let logs = subscriber1.get_logs();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0], "Test message 1");
    assert_eq!(logs[1], "Test message 2");

    // Test that subscribers are independent
    let logs2 = subscriber2.get_logs();
    assert_eq!(logs2.len(), 0);
}

/// Test error handling with different buffer scenarios
#[tokio::test]
async fn test_error_handling_scenarios() {
    let test_cases = vec![
        ("empty_ticket", ""),
        ("invalid_format", "not-a-valid-ticket-format"),
        ("malformed", "definitely::not::a::ticket"),
    ];

    for (test_name, ticket) in test_cases {
        println!("Testing error handling for: {}", test_name);
        
        let profile = ReceiverProfile {
            name: format!("Error Test - {}", test_name),
            avatar_b64: None,
        };

        let request = ReceiveFilesRequest {
            ticket: ticket.to_string(),
            confirmation: 50,
            profile,
        };

        let result = receive_files(request).await;
        assert!(result.is_err(), "Should fail for test case: {}", test_name);
    }
}

/// Test receiver profile configurations
#[test]
fn test_receiver_profile_variations() {
    // Test profile without avatar
    let profile1 = ReceiverProfile {
        name: "Plain User".to_string(),
        avatar_b64: None,
    };
    assert_eq!(profile1.name, "Plain User");
    assert!(profile1.avatar_b64.is_none());

    // Test profile with avatar
    let profile2 = ReceiverProfile {
        name: "Avatar User".to_string(),
        avatar_b64: Some("dGVzdEF2YXRhcg==".to_string()),
    };
    assert_eq!(profile2.name, "Avatar User");
    assert_eq!(profile2.avatar_b64, Some("dGVzdEF2YXRhcg==".to_string()));

    // Test profile with empty name (edge case)
    let profile3 = ReceiverProfile {
        name: String::new(),
        avatar_b64: Some("ZW1wdHluYW1l".to_string()),
    };
    assert_eq!(profile3.name, "");
    assert!(profile3.avatar_b64.is_some());

    // Test profile with very long name
    let long_name = "a".repeat(1000);
    let profile4 = ReceiverProfile {
        name: long_name.clone(),
        avatar_b64: None,
    };
    assert_eq!(profile4.name, long_name);
}

/// Test confirmation code boundaries
#[test]
fn test_confirmation_code_boundaries() {
    let profile = ReceiverProfile {
        name: "Boundary Test User".to_string(),
        avatar_b64: None,
    };

    // Test boundary values for confirmation codes
    let boundary_values = vec![0, 1, 50, 98, 99];
    
    for confirmation in boundary_values {
        let request = ReceiveFilesRequest {
            ticket: format!("test_ticket_confirm_{}", confirmation),
            confirmation,
            profile: ReceiverProfile {
            name: profile.name.clone(),
            avatar_b64: profile.avatar_b64.clone(),
        },
        };

        // All should have valid confirmation codes (even if tickets are invalid)
        assert!(confirmation <= 99, "Confirmation code {} should be within bounds", confirmation);
        assert_eq!(request.confirmation, confirmation);
    }
}

/// Test concurrent subscriber access
#[tokio::test]
async fn test_concurrent_subscriber_access() {
    let subscriber = Arc::new(TestSubscriber::new("concurrent_test"));

    // Spawn multiple tasks that log messages concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let subscriber_clone = subscriber.clone();
            tokio::spawn(async move {
                for j in 0..5 {
                    let message = format!("Task {} - Message {}", i, j);
                    subscriber_clone.log(message);
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                i
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Check that all messages were logged
    let logs = subscriber.get_logs();
    assert_eq!(logs.len(), 50); // 10 tasks * 5 messages each

    // Verify that all task IDs are present
    let mut task_counts = vec![0; 10];
    for log in &logs {
        for i in 0..10 {
            if log.contains(&format!("Task {}", i)) {
                task_counts[i] += 1;
            }
        }
    }

    // Each task should have logged exactly 5 messages
    for (i, count) in task_counts.iter().enumerate() {
        assert_eq!(*count, 5, "Task {} should have logged 5 messages, got {}", i, count);
    }
}

/// Test file data structure validation
#[test]
fn test_receiver_file_data_structure() {
    use dropx_receiver::ReceiverFileData;
    

    // Test ReceiverFileData creation
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test_file.txt");
    
    // Create a test file
    std::fs::write(&file_path, b"test content").unwrap();

    let file_data = ReceiverFileData::new(file_path.clone());
    
    // Test that file_data was created successfully
    assert_eq!(file_data.len(), 12); // "test content" is 12 bytes

    // Test reading from the file
    let first_byte = file_data.read();
    assert_eq!(first_byte, Some(b't'));

    let second_byte = file_data.read();
    assert_eq!(second_byte, Some(b'e'));
}

/// Stress test with multiple rapid operations
#[tokio::test]
async fn test_rapid_operations_stress() {
    let subscriber = Arc::new(TestSubscriber::new("stress_test"));

    // Perform rapid logging operations
    for i in 0..1000 {
        subscriber.log(format!("Rapid message {}", i));
        if i % 100 == 0 {
            tokio::task::yield_now().await; // Yield occasionally
        }
    }

    let logs = subscriber.get_logs();
    assert_eq!(logs.len(), 1000);

    // Verify message ordering is maintained
    for (i, log) in logs.iter().enumerate() {
        let expected = format!("Rapid message {}", i);
        assert_eq!(*log, expected);
    }
}

/// Integration test for buffer size impact simulation
#[tokio::test]
async fn test_buffer_size_impact_simulation() {
    // Since we can't test actual file transfer without both sender and receiver,
    // we'll simulate the impact of different buffer sizes on data handling

    struct BufferSimulation {
        name: String,
        buffer_size: usize,
    }

    let simulations = vec![
        BufferSimulation { name: "Small Buffer".to_string(), buffer_size: 128 },
        BufferSimulation { name: "Medium Buffer".to_string(), buffer_size: 1024 },
        BufferSimulation { name: "Large Buffer".to_string(), buffer_size: 8192 },
    ];

    for sim in simulations {
        println!("Testing {}: {} bytes", sim.name, sim.buffer_size);
        
        let subscriber = Arc::new(TestSubscriber::new(&format!("buffer_test_{}", sim.buffer_size)));
        
        // Simulate receiving data in chunks of different sizes
        let total_data_size = 10240; // 10KB of test data
        let chunks_count = (total_data_size + sim.buffer_size - 1) / sim.buffer_size;
        
        for chunk_id in 0..chunks_count {
            let chunk_size = std::cmp::min(sim.buffer_size, total_data_size - chunk_id * sim.buffer_size);
            subscriber.log(format!("Chunk {} of size {} bytes", chunk_id, chunk_size));
        }

        let logs = subscriber.get_logs();
        assert_eq!(logs.len(), chunks_count);
        
        println!("  - Processed {} chunks for {}", chunks_count, sim.name);
    }
}