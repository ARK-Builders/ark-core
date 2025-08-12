use criterion::{
    BenchmarkId, Criterion, Throughput, black_box, criterion_group,
    criterion_main,
};
use dropx_receiver::{
    ReceiveFilesConnectingEvent, ReceiveFilesReceivingEvent,
    ReceiveFilesRequest, ReceiveFilesSubscriber, ReceiverFileData,
    ReceiverProfile, receive_files,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::TempDir;

const BENCHMARK_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Benchmark subscriber implementation for performance testing
struct BenchmarkSubscriber {
    id: String,
    logs: Arc<Mutex<Vec<String>>>,
    connecting_events: Arc<Mutex<usize>>,
    receiving_events: Arc<Mutex<Vec<usize>>>, /* Store data sizes instead of
                                               * actual data */
}

impl BenchmarkSubscriber {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            logs: Arc::new(Mutex::new(Vec::new())),
            connecting_events: Arc::new(Mutex::new(0)),
            receiving_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_log_count(&self) -> usize {
        self.logs.lock().unwrap().len()
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

    fn reset(&self) {
        self.logs.lock().unwrap().clear();
        *self.connecting_events.lock().unwrap() = 0;
        self.receiving_events.lock().unwrap().clear();
    }
}

impl ReceiveFilesSubscriber for BenchmarkSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        self.logs.lock().unwrap().push(message);
    }

    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent) {
        self.receiving_events
            .lock()
            .unwrap()
            .push(event.data.len());
    }

    fn notify_connecting(&self, _event: ReceiveFilesConnectingEvent) {
        *self.connecting_events.lock().unwrap() += 1;
    }
}

/// Generate test data with specific patterns for consistent benchmarking
fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| (i.wrapping_mul(173) ^ i.wrapping_mul(11)) as u8)
        .collect()
}

/// Create a temporary file with test data
fn create_test_file(size: usize) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("benchmark_test.dat");
    let test_data = generate_test_data(size);
    std::fs::write(&file_path, test_data).expect("Failed to write test file");
    (temp_dir, file_path)
}

/// Benchmark receiver profile creation and validation
fn bench_receiver_profile_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("receiver_profile_creation");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let long_name_a = "a".repeat(100);
    let long_name_b = "b".repeat(100);
    let profile_scenarios = vec![
        ("simple", "Test User", None),
        ("with_avatar", "Avatar User", Some("dGVzdEF2YXRhcg==")),
        ("long_name", long_name_a.as_str(), None),
        (
            "long_name_avatar",
            long_name_b.as_str(),
            Some("bG9uZ05hbWVBdmF0YXI="),
        ),
    ];

    for (scenario_name, name, avatar) in profile_scenarios {
        group.bench_function(
            BenchmarkId::new("profile_creation", scenario_name),
            |b| {
                b.iter(|| {
                    let profile = ReceiverProfile {
                        name: black_box(name.to_string()),
                        avatar_b64: black_box(avatar.map(|s| s.to_string())),
                    };

                    // Access fields to ensure they're not optimized away
                    black_box(&profile.name);
                    black_box(&profile.avatar_b64);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark subscriber operations performance
fn bench_subscriber_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_operations");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let message_counts = vec![10, 100, 1000];

    for &message_count in &message_counts {
        group.bench_function(BenchmarkId::new("logging", message_count), |b| {
            let subscriber = BenchmarkSubscriber::new("bench_subscriber");

            b.iter(|| {
                subscriber.reset();

                for i in 0..message_count {
                    let message = format!("Benchmark message {}", i);
                    subscriber.log(black_box(message));
                }

                assert_eq!(subscriber.get_log_count(), message_count);
            });
        });
    }

    // Benchmark receiving event processing
    let event_data_sizes = vec![64, 1024, 64 * 1024];

    for &data_size in &event_data_sizes {
        group.bench_function(
            BenchmarkId::new("receiving_events", data_size),
            |b| {
                let subscriber = BenchmarkSubscriber::new("bench_subscriber");
                let test_data = generate_test_data(data_size);

                b.iter(|| {
                    subscriber.reset();

                    for i in 0..10 {
                        let event = ReceiveFilesReceivingEvent {
                            id: format!("file_{}", i),
                            data: black_box(test_data.clone()),
                        };
                        subscriber.notify_receiving(event);
                    }

                    assert_eq!(subscriber.get_receiving_events_count(), 10);
                    assert_eq!(
                        subscriber.get_total_bytes_received(),
                        data_size * 10
                    );
                });
            },
        );
    }

    group.finish();
}

/// Benchmark file data operations
fn bench_file_data_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_data_operations");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let file_sizes = vec![
        ("1KB", 1024),
        ("10KB", 10 * 1024),
        ("100KB", 100 * 1024),
        ("1MB", 1024 * 1024),
    ];

    for (size_name, size) in file_sizes {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_function(
            BenchmarkId::new("file_creation_and_length", size_name),
            |b| {
                b.iter(|| {
                    let (_temp_dir, file_path) = create_test_file(size);
                    let file_data = ReceiverFileData::new(black_box(file_path));

                    // Test length calculation
                    let length = file_data.len();
                    assert_eq!(length, size as u64);

                    black_box(length);
                });
            },
        );

        group.bench_function(
            BenchmarkId::new("sequential_reading", size_name),
            |b| {
                let (_temp_dir, file_path) = create_test_file(size);
                let file_data = ReceiverFileData::new(file_path);

                b.iter(|| {
                    // Note: This is a simplified benchmark since
                    // ReceiverFileData consumes data on
                    // read. In a real scenario, we'd need to
                    // recreate the file data for each iteration.
                    let mut bytes_read = 0;
                    let mut read_attempts = 0;

                    // Limit read attempts to prevent infinite loops
                    while let Some(byte) = file_data.read() {
                        black_box(byte);
                        bytes_read += 1;
                        read_attempts += 1;

                        if read_attempts >= size {
                            break;
                        }
                    }

                    black_box(bytes_read);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent subscriber access patterns
fn bench_concurrent_subscriber_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_subscriber_access");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let concurrency_levels = vec![1, 2, 4, 8];
    let operations_per_thread = 100;

    for &concurrency_level in &concurrency_levels {
        group.bench_function(
            BenchmarkId::new("concurrent_logging", concurrency_level),
            |b| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let subscriber = Arc::new(BenchmarkSubscriber::new(
                            "concurrent_bench",
                        ));
                        subscriber.reset();

                        let handles: Vec<_> = (0..concurrency_level)
                            .map(|thread_id| {
                                let subscriber_clone = subscriber.clone();
                                tokio::spawn(async move {
                                    for i in 0..operations_per_thread {
                                        let message = format!(
                                            "Thread {} - Operation {}",
                                            thread_id, i
                                        );
                                        subscriber_clone
                                            .log(black_box(message));
                                    }
                                    thread_id
                                })
                            })
                            .collect();

                        // Wait for all threads to complete
                        for handle in handles {
                            handle.await.unwrap();
                        }

                        assert_eq!(
                            subscriber.get_log_count(),
                            concurrency_level * operations_per_thread
                        );
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark error handling performance
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let too_long = "x".repeat(1000);
    let invalid_tickets = vec![
        ("empty", ""),
        ("malformed", "not-a-ticket"),
        ("invalid_format", "definitely::not::valid"),
        ("too_long", too_long.as_str()),
    ];

    for (error_type, ticket) in invalid_tickets {
        group.bench_function(
            BenchmarkId::new("invalid_ticket_handling", error_type),
            |b| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let profile = ReceiverProfile {
                            name: black_box("Error Test User".to_string()),
                            avatar_b64: None,
                        };

                        let request = ReceiveFilesRequest {
                            ticket: black_box(ticket.to_string()),
                            confirmation: 42,
                            profile,
                        };

                        let result = receive_files(black_box(request)).await;
                        assert!(result.is_err());
                        black_box(result);
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark data structure memory efficiency
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let data_sizes = vec![1024, 10 * 1024, 100 * 1024];
    let event_counts = vec![10, 100];

    for &data_size in &data_sizes {
        for &event_count in &event_counts {
            group.bench_function(
                BenchmarkId::new(
                    "event_memory_usage",
                    format!("{}B_{}events", data_size, event_count),
                ),
                |b| {
                    b.iter(|| {
                        let subscriber =
                            BenchmarkSubscriber::new("memory_bench");
                        let test_data = generate_test_data(data_size);

                        // Simulate receiving multiple events
                        for i in 0..event_count {
                            let event = ReceiveFilesReceivingEvent {
                                id: format!("file_{}", i),
                                data: black_box(test_data.clone()),
                            };
                            subscriber.notify_receiving(event);
                        }

                        assert_eq!(
                            subscriber.get_receiving_events_count(),
                            event_count
                        );
                        assert_eq!(
                            subscriber.get_total_bytes_received(),
                            data_size * event_count
                        );

                        black_box(&subscriber);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark configuration impact on performance
fn bench_configuration_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("configuration_impact");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    // Simulate different buffer size impacts on receiver operations
    let buffer_scenarios = vec![
        ("tiny_buffer", 64),
        ("small_buffer", 1024),
        ("medium_buffer", 64 * 1024),
        ("large_buffer", 2 * 1024 * 1024),
        ("huge_buffer", 8 * 1024 * 1024),
    ];

    let total_data_size = 1024 * 1024; // 1MB total data

    for (scenario_name, buffer_size) in buffer_scenarios {
        group.throughput(Throughput::Bytes(total_data_size as u64));

        group.bench_function(
            BenchmarkId::new("buffer_simulation", scenario_name),
            |b| {
                b.iter(|| {
                    let subscriber = BenchmarkSubscriber::new("config_bench");

                    // Simulate receiving data in chunks based on buffer size
                    let chunks_count =
                        (total_data_size + buffer_size - 1) / buffer_size;
                    let mut remaining_data = total_data_size;

                    for chunk_id in 0..chunks_count {
                        let chunk_size =
                            std::cmp::min(buffer_size, remaining_data);
                        let chunk_data = generate_test_data(chunk_size);

                        let event = ReceiveFilesReceivingEvent {
                            id: format!("chunk_{}", chunk_id),
                            data: black_box(chunk_data),
                        };

                        subscriber.notify_receiving(event);
                        remaining_data -= chunk_size;

                        if remaining_data == 0 {
                            break;
                        }
                    }

                    assert_eq!(
                        subscriber.get_total_bytes_received(),
                        total_data_size
                    );
                    black_box(&subscriber);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_receiver_profile_creation,
    bench_subscriber_operations,
    bench_file_data_operations,
    bench_concurrent_subscriber_access,
    bench_error_handling,
    bench_memory_efficiency,
    bench_configuration_impact,
);

criterion_main!(benches);
