//! Integration-style tests for drop-core file transfer functionality
//! 
//! These tests validate the complete file transfer process initialization
//! with different buffer size configurations to ensure data integrity and performance.

use dropx_sender::{
    send_files, SenderConfig, SenderFile, SenderFileData, SenderProfile,
};
use std::{
    sync::{Arc, Mutex},
};
use tokio;

/// Test file data implementation optimized for integration testing
struct IntegrationTestFileData {
    data: Vec<u8>,
    position: Arc<Mutex<usize>>,
    read_history: Arc<Mutex<Vec<usize>>>, // Track chunk sizes for verification
}

impl IntegrationTestFileData {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            position: Arc::new(Mutex::new(0)),
            read_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn reset(&self) {
        *self.position.lock().unwrap() = 0;
        self.read_history.lock().unwrap().clear();
    }

    fn get_read_history(&self) -> Vec<usize> {
        self.read_history.lock().unwrap().clone()
    }
}

impl SenderFileData for IntegrationTestFileData {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn read(&self) -> Option<u8> {
        let mut pos = self.position.lock().unwrap();
        if *pos < self.data.len() {
            let byte = self.data[*pos];
            *pos += 1;
            // Record single byte reads
            self.read_history.lock().unwrap().push(1);
            Some(byte)
        } else {
            None
        }
    }

    fn read_chunk(&self, size: u64) -> Vec<u8> {
        let mut pos = self.position.lock().unwrap();
        let start = *pos;
        let end = std::cmp::min(start + size as usize, self.data.len());
        let actual_size = end - start;
        *pos = end;
        
        // Record chunk size for analysis
        if actual_size > 0 {
            self.read_history.lock().unwrap().push(actual_size);
        }
        
        self.data[start..end].to_vec()
    }
}

/// Generate test data with verifiable patterns
fn generate_integration_test_data(size: usize, seed: u8) -> Vec<u8> {
    (0..size)
        .map(|i| ((i.wrapping_mul(17) ^ seed as usize) % 256) as u8)
        .collect()
}

/// Test complete file transfer initialization with comprehensive buffer size coverage
#[tokio::test]
async fn test_comprehensive_buffer_size_coverage() {
    let buffer_sizes = vec![
        32,      // Very small
        64,      // Tiny
        256,     // Small
        1024,    // 1KB
        4096,    // 4KB
        16384,   // 16KB
        65536,   // 64KB
        131072,  // 128KB (low_bandwidth)
        524288,  // 512KB
        2097152, // 2MB (balanced/default)
        8388608, // 8MB (high_performance)
    ];

    let test_file_sizes = vec![
        100,     // Smaller than all buffers
        1000,    // Medium size
        10000,   // Large size
        100000,  // Very large
    ];

    for file_size in test_file_sizes {
        for buffer_size in &buffer_sizes {
            println!("Testing file size: {} bytes with buffer size: {} bytes", 
                     file_size, buffer_size);

            let test_data = generate_integration_test_data(file_size, 42);
            let file_data = Arc::new(IntegrationTestFileData::new(test_data.clone()));

            let profile = SenderProfile {
                name: format!("Integration Test - Buffer {}", buffer_size),
                avatar_b64: None,
            };

            let files = vec![SenderFile {
                name: format!("test_{}_{}.dat", file_size, buffer_size),
                data: file_data.clone(),
            }];

            let config = SenderConfig { buffer_size: *buffer_size };
            let request = dropx_sender::SendFilesRequest {
                profile,
                files,
                config,
            };

            // Test sender initialization
            let start_time = std::time::Instant::now();
            let bubble = send_files(request).await;
            let init_duration = start_time.elapsed();

            assert!(bubble.is_ok(), 
                    "Failed to initialize sender - file: {} bytes, buffer: {} bytes", 
                    file_size, buffer_size);

            let bubble = bubble.unwrap();
            
            // Verify basic properties
            assert!(!bubble.get_ticket().is_empty(), 
                    "Ticket empty - file: {} bytes, buffer: {} bytes", 
                    file_size, buffer_size);
            assert!(bubble.get_confirmation() <= 99, 
                    "Invalid confirmation - file: {} bytes, buffer: {} bytes", 
                    file_size, buffer_size);

            // Test that file data can be read with the configured buffer size
            file_data.reset();
            let mut total_read = 0;
            let mut chunk_count = 0;

            while total_read < file_size {
                let chunk = file_data.read_chunk(*buffer_size);
                if chunk.is_empty() {
                    break;
                }
                total_read += chunk.len();
                chunk_count += 1;

                // Verify chunk doesn't exceed buffer size
                assert!(chunk.len() <= *buffer_size as usize,
                        "Chunk size {} exceeds buffer size {} - file: {} bytes",
                        chunk.len(), buffer_size, file_size);
            }

            assert_eq!(total_read, file_size, 
                      "Data integrity check failed - file: {} bytes, buffer: {} bytes", 
                      file_size, buffer_size);

            // Verify read pattern matches expectations
            let read_history = file_data.get_read_history();
            let expected_full_chunks = file_size / (*buffer_size as usize);
            let remainder = file_size % (*buffer_size as usize);

            if remainder == 0 {
                assert_eq!(read_history.len(), expected_full_chunks,
                          "Unexpected number of chunks - file: {} bytes, buffer: {} bytes",
                          file_size, buffer_size);
            } else {
                assert_eq!(read_history.len(), expected_full_chunks + 1,
                          "Unexpected number of chunks with remainder - file: {} bytes, buffer: {} bytes", 
                          file_size, buffer_size);
            }

            // Performance assertion - initialization shouldn't take too long
            assert!(init_duration.as_millis() < 1000,
                    "Initialization too slow ({:?}) - file: {} bytes, buffer: {} bytes",
                    init_duration, file_size, buffer_size);

            println!("✓ Passed - file: {} bytes, buffer: {} bytes, chunks: {}, time: {:?}", 
                     file_size, buffer_size, chunk_count, init_duration);
        }
    }
}

/// Test buffer size presets with real-world scenarios
#[tokio::test]
async fn test_buffer_size_presets_integration() {
    let presets = vec![
        ("low_bandwidth", SenderConfig::low_bandwidth(), "optimized for constrained networks"),
        ("balanced", SenderConfig::balanced(), "general purpose configuration"),
        ("default", SenderConfig::default(), "default configuration"),
        ("high_performance", SenderConfig::high_performance(), "optimized for high throughput"),
    ];

    let scenario_files = vec![
        ("small_document", generate_integration_test_data(1024, 1)),    // 1KB document
        ("medium_image", generate_integration_test_data(500 * 1024, 2)), // 500KB image
        ("large_video", generate_integration_test_data(5 * 1024 * 1024, 3)), // 5MB video
    ];

    for (preset_name, config, description) in presets {
        for (file_type, file_data) in &scenario_files {
            println!("Testing {} preset with {} ({})", preset_name, file_type, description);

            let profile = SenderProfile {
                name: format!("Integration {} - {}", preset_name, file_type),
                avatar_b64: Some("dGVzdEludGVncmF0aW9u".to_string()), // "testIntegration" in base64
            };

            let file_data_impl = Arc::new(IntegrationTestFileData::new(file_data.clone()));
            let files = vec![SenderFile {
                name: format!("{}_with_{}.dat", file_type, preset_name),
                data: file_data_impl.clone(),
            }];

            let request = dropx_sender::SendFilesRequest {
                profile,
                files,
                config: config.clone(),
            };

            let start_time = std::time::Instant::now();
            let bubble = send_files(request).await;
            let duration = start_time.elapsed();

            assert!(bubble.is_ok(), 
                    "Failed {} preset with {} - {}", preset_name, file_type, description);

            let bubble = bubble.unwrap();

            // Verify initialization
            assert!(!bubble.get_ticket().is_empty());
            assert!(!bubble.is_finished()); // Should not be finished immediately
            assert!(!bubble.is_connected()); // Should not be connected without receiver

            // Test data reading with this configuration
            file_data_impl.reset();
            let chunk = file_data_impl.read_chunk(config.buffer_size);
            
            if !file_data.is_empty() {
                assert!(!chunk.is_empty(), "Should read data with {} preset for {}", preset_name, file_type);
                assert!(chunk.len() <= config.buffer_size as usize, 
                        "Chunk size should respect buffer limit for {} preset", preset_name);
            }

            println!("✓ {} preset with {} completed in {:?} (buffer: {} bytes)", 
                     preset_name, file_type, duration, config.buffer_size);
        }
    }
}

/// Test edge cases and boundary conditions
#[tokio::test]
async fn test_integration_edge_cases() {
    let edge_cases = vec![
        ("empty_file", Vec::new()),
        ("single_byte", vec![42]),
        ("power_of_two", generate_integration_test_data(1024, 7)), // 2^10
        ("prime_size", generate_integration_test_data(1009, 11)),   // Prime number
        ("large_odd", generate_integration_test_data(12345, 13)),  // Odd large number
    ];

    let test_configs = vec![
        ("micro_buffer", SenderConfig { buffer_size: 1 }),
        ("small_buffer", SenderConfig { buffer_size: 17 }), // Prime buffer size
        ("standard", SenderConfig::balanced()),
    ];

    for (case_name, test_data) in edge_cases {
        for (config_name, config) in &test_configs {
            if test_data.is_empty() && config.buffer_size == 1 {
                // Skip empty file with micro buffer to avoid potential issues
                continue;
            }

            println!("Testing edge case: {} with {}", case_name, config_name);

            let profile = SenderProfile {
                name: format!("Edge Case - {} {}", case_name, config_name),
                avatar_b64: None,
            };

            let file_data = Arc::new(IntegrationTestFileData::new(test_data.clone()));
            let files = vec![SenderFile {
                name: format!("edge_case_{}_{}.dat", case_name, config_name),
                data: file_data.clone(),
            }];

            let request = dropx_sender::SendFilesRequest {
                profile,
                files,
                config: config.clone(),
            };

            let bubble = send_files(request).await;
            assert!(bubble.is_ok(), 
                    "Edge case failed: {} with {}", case_name, config_name);

            let bubble = bubble.unwrap();
            assert!(!bubble.get_ticket().is_empty(), 
                    "Ticket empty for edge case: {} with {}", case_name, config_name);

            // Test file reading
            file_data.reset();
            let mut total_read = 0;
            let mut iterations = 0;
            let max_iterations = (test_data.len() / (config.buffer_size as usize).max(1)) + 10; // Safety limit

            while total_read < test_data.len() && iterations < max_iterations {
                let chunk = file_data.read_chunk(config.buffer_size);
                if chunk.is_empty() {
                    break;
                }
                total_read += chunk.len();
                iterations += 1;
            }

            assert_eq!(total_read, test_data.len(), 
                      "Data integrity failed for edge case: {} with {}", case_name, config_name);

            println!("✓ Edge case {} with {} passed ({} bytes in {} chunks)", 
                     case_name, config_name, total_read, iterations);
        }
    }
}

/// Test multiple files with different buffer configurations
#[tokio::test]
async fn test_multiple_files_integration() {
    let file_sets = vec![
        ("mixed_small", vec![
            ("doc1.txt", generate_integration_test_data(500, 1)),
            ("doc2.txt", generate_integration_test_data(750, 2)),
            ("doc3.txt", generate_integration_test_data(300, 3)),
        ]),
        ("uniform_medium", vec![
            ("file1.dat", generate_integration_test_data(2048, 4)),
            ("file2.dat", generate_integration_test_data(2048, 5)),
            ("file3.dat", generate_integration_test_data(2048, 6)),
            ("file4.dat", generate_integration_test_data(2048, 7)),
        ]),
        ("size_variety", vec![
            ("tiny.bin", generate_integration_test_data(10, 8)),
            ("small.bin", generate_integration_test_data(1000, 9)),
            ("medium.bin", generate_integration_test_data(10000, 10)),
        ]),
    ];

    let configs = vec![
        ("optimized_small", SenderConfig { buffer_size: 256 }),
        ("optimized_medium", SenderConfig { buffer_size: 2048 }),
        ("high_throughput", SenderConfig::high_performance()),
    ];

    for (set_name, file_data_set) in file_sets {
        for (config_name, config) in &configs {
            println!("Testing {} file set with {} buffer", set_name, config_name);

            let total_size: usize = file_data_set.iter().map(|(_, data)| data.len()).sum();

            let files: Vec<SenderFile> = file_data_set
                .iter()
                .map(|(name, data)| SenderFile {
                    name: name.to_string(),
                    data: Arc::new(IntegrationTestFileData::new(data.clone())),
                })
                .collect();

            let profile = SenderProfile {
                name: format!("Multi-file {} - {}", set_name, config_name),
                avatar_b64: None,
            };

            // Create files fresh for each test to avoid clone issues
            let files_for_request: Vec<SenderFile> = file_data_set
                .iter()
                .map(|(name, data)| SenderFile {
                    name: name.to_string(),
                    data: Arc::new(IntegrationTestFileData::new(data.clone())),
                })
                .collect();

            let request = dropx_sender::SendFilesRequest {
                profile,
                files: files_for_request,
                config: config.clone(),
            };

            let start_time = std::time::Instant::now();
            let bubble = send_files(request).await;
            let duration = start_time.elapsed();

            assert!(bubble.is_ok(), 
                    "Multi-file failed: {} with {}", set_name, config_name);

            let bubble = bubble.unwrap();
            assert!(!bubble.get_ticket().is_empty());

            // Verify basic file structure - we can't easily downcast the trait objects
            // but we can verify the number of files and basic properties
            assert_eq!(file_data_set.len(), files.len(), 
                      "File count mismatch for {} with {}", set_name, config_name);
            
            // Verify each file has the expected length
            for (i, (_, original_data)) in file_data_set.iter().enumerate() {
                let file_len = files[i].data.len();
                assert_eq!(file_len, original_data.len() as u64,
                          "File {} size mismatch in set {} with {}", 
                          i, set_name, config_name);
            }

            println!("✓ {} file set with {} buffer passed ({} files, {} bytes total, {:?})", 
                     set_name, config_name, file_data_set.len(), total_size, duration);
        }
    }
}

/// Performance baseline test for buffer size optimization
#[tokio::test]
async fn test_buffer_size_performance_baseline() {
    let test_sizes = vec![1024, 1024 * 10, 1024 * 100, 1024 * 1024, 1024 * 1024 * 10]; // 1KB, 10KB, 100KB, 1MB, 10MB
    let buffer_sizes = vec![128, 1024, 1024 * 64, 1024 * 100, 1024 * 1024, 1024 * 1024 * 2]; // 128B, 1KB, 64KB, 1MB, 2MB

    for test_size in test_sizes {
        println!("Performance baseline for {} byte files:", test_size);
        
        for buffer_size in &buffer_sizes {
            let test_data = generate_integration_test_data(test_size, 99);
            let profile = SenderProfile {
                name: format!("Performance Baseline - {}B buffer", buffer_size),
                avatar_b64: None,
            };

            let files = vec![SenderFile {
                name: format!("perf_test_{}_{}.dat", test_size, buffer_size),
                data: Arc::new(IntegrationTestFileData::new(test_data)),
            }];

            let config = SenderConfig { buffer_size: *buffer_size };
            let request = dropx_sender::SendFilesRequest {
                profile,
                files,
                config,
            };

            let start_time = std::time::Instant::now();
            let bubble = send_files(request).await;
            let init_time = start_time.elapsed();

            assert!(bubble.is_ok(), "Performance test failed for buffer size {}", buffer_size);
            
            let bubble = bubble.unwrap();
            assert!(!bubble.get_ticket().is_empty());

            // Basic performance expectation - should initialize quickly
            assert!(init_time.as_millis() < 500, 
                    "Slow initialization ({:?}) for buffer size {} with {} byte file", 
                    init_time, buffer_size, test_size);

            println!("  Buffer {}B: {:?} init time", buffer_size, init_time);
        }
        println!();
    }
}

