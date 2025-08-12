use criterion::{
    BenchmarkId, Criterion, Throughput, black_box, criterion_group,
    criterion_main,
};
use dropx_sender::{
    SenderConfig, SenderFile, SenderFileData, SenderProfile, send_files,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const BENCHMARK_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Mock implementation of SenderFileData for benchmarking
struct BenchmarkSenderFileData {
    data: Vec<u8>,
    position: Arc<Mutex<usize>>,
}

impl BenchmarkSenderFileData {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            position: Arc::new(Mutex::new(0)),
        }
    }

    fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

impl SenderFileData for BenchmarkSenderFileData {
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

/// Generate test data with specific patterns for better benchmark consistency
fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| (i.wrapping_mul(137) ^ i.wrapping_mul(7)) as u8)
        .collect()
}

/// Benchmark buffer size impact on sender initialization
fn bench_sender_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("sender_initialization");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let file_sizes = vec![
        ("1KB", 1024),
        ("10KB", 10 * 1024),
        ("100KB", 100 * 1024),
        ("1MB", 1024 * 1024),
    ];

    let buffer_configs = vec![
        ("128B", SenderConfig { buffer_size: 128 }),
        ("1KB", SenderConfig { buffer_size: 1024 }),
        (
            "64KB",
            SenderConfig {
                buffer_size: 64 * 1024,
            },
        ),
        ("128KB", SenderConfig::low_bandwidth()),
        ("2MB", SenderConfig::balanced()),
        ("8MB", SenderConfig::high_performance()),
    ];

    for (file_size_name, file_size) in file_sizes {
        for (buffer_name, buffer_config) in &buffer_configs {
            let benchmark_id = BenchmarkId::new(
                format!("{}_{}", file_size_name, buffer_name),
                file_size,
            );

            group.throughput(Throughput::Bytes(file_size as u64));

            let rt = tokio::runtime::Runtime::new().unwrap();
            group.bench_with_input(
                benchmark_id,
                &(file_size, buffer_config.clone()),
                |b, (size, config)| {
                    b.iter(|| {
                        rt.block_on(async {
                            let data = generate_test_data(*size);
                            let mock_file_data =
                                Arc::new(BenchmarkSenderFileData::new(data));

                            let profile = SenderProfile {
                                name: "Benchmark User".to_string(),
                                avatar_b64: None,
                            };

                            let files = vec![SenderFile {
                                name: format!("benchmark_file_{}.dat", size),
                                data: mock_file_data,
                            }];

                            let request = dropx_sender::SendFilesRequest {
                                profile,
                                files,
                                config: config.clone(),
                            };

                            let bubble = send_files(black_box(request)).await;
                            // Just measure the initialization time, not the
                            // actual transfer
                            assert!(bubble.is_ok());
                            let bubble = bubble.unwrap();

                            // Touch these to ensure they're computed
                            black_box(bubble.get_ticket());
                            black_box(bubble.get_confirmation());
                            black_box(bubble.is_finished());
                        })
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark data reading performance with different buffer sizes
fn bench_data_reading(c: &mut Criterion) {
    let mut group = c.benchmark_group("data_reading");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let data_sizes = vec![
        ("1KB", 1024),
        ("10KB", 10 * 1024),
        ("100KB", 100 * 1024),
        ("1MB", 1024 * 1024),
    ];

    let chunk_sizes = vec![
        ("1B", 1),
        ("64B", 64),
        ("1KB", 1024),
        ("64KB", 64 * 1024),
        ("2MB", 2 * 1024 * 1024),
    ];

    for (data_size_name, data_size) in data_sizes {
        let test_data = generate_test_data(data_size);

        for (chunk_size_name, chunk_size) in &chunk_sizes {
            let benchmark_id = BenchmarkId::new(
                format!("{}_{}_chunks", data_size_name, chunk_size_name),
                data_size,
            );

            group.throughput(Throughput::Bytes(data_size as u64));

            group.bench_with_input(
                benchmark_id,
                &(test_data.clone(), *chunk_size),
                |b, (data, chunk_size)| {
                    b.iter(|| {
                        let mock_file =
                            BenchmarkSenderFileData::new(data.clone());
                        mock_file.reset(); // Reset position for each iteration

                        let mut total_bytes = 0;
                        loop {
                            let chunk = mock_file
                                .read_chunk(black_box(*chunk_size as u64));
                            if chunk.is_empty() {
                                break;
                            }
                            total_bytes += chunk.len();
                            black_box(chunk); // Prevent optimization
                        }

                        assert_eq!(total_bytes, data.len());
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark sequential byte reading performance
fn bench_sequential_reading(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_reading");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let data_sizes =
        vec![("1KB", 1024), ("10KB", 10 * 1024), ("100KB", 100 * 1024)];

    for (data_size_name, data_size) in data_sizes {
        let test_data = generate_test_data(data_size);

        group.throughput(Throughput::Bytes(data_size as u64));

        group.bench_function(
            BenchmarkId::new("sequential_read", data_size_name),
            |b| {
                b.iter(|| {
                    let mock_file =
                        BenchmarkSenderFileData::new(test_data.clone());
                    mock_file.reset();

                    let mut bytes_read = 0;
                    while let Some(byte) = mock_file.read() {
                        black_box(byte);
                        bytes_read += 1;
                    }

                    assert_eq!(bytes_read, data_size);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark multiple file handling with different configurations
fn bench_multiple_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_files");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let file_scenarios = vec![
        ("5_small_files", vec![1024; 5]), // 5 x 1KB files
        ("3_medium_files", vec![10 * 1024; 3]), // 3 x 10KB files
        (
            "10_mixed_files",
            vec![
                1024,
                5 * 1024,
                10 * 1024,
                2 * 1024,
                8 * 1024,
                1024,
                15 * 1024,
                3 * 1024,
                6 * 1024,
                4 * 1024,
            ],
        ), // Mixed sizes
    ];

    let buffer_configs = vec![
        ("low_bandwidth", SenderConfig::low_bandwidth()),
        ("balanced", SenderConfig::balanced()),
        ("high_performance", SenderConfig::high_performance()),
    ];

    for (scenario_name, file_sizes) in file_scenarios {
        let total_size: usize = file_sizes.iter().sum();

        for (config_name, config) in &buffer_configs {
            let benchmark_id = BenchmarkId::new(
                format!("{}_{}", scenario_name, config_name),
                total_size,
            );

            group.throughput(Throughput::Bytes(total_size as u64));

            let rt = tokio::runtime::Runtime::new().unwrap();
            group.bench_with_input(
                benchmark_id,
                &(file_sizes.clone(), config.clone()),
                |b, (sizes, config)| {
                    b.iter(|| {
                        rt.block_on(async {
                            let files: Vec<SenderFile> = sizes
                                .iter()
                                .enumerate()
                                .map(|(i, &size)| {
                                    let data = generate_test_data(size);
                                    SenderFile {
                                        name: format!("file_{}.dat", i),
                                        data: Arc::new(
                                            BenchmarkSenderFileData::new(data),
                                        ),
                                    }
                                })
                                .collect();

                            let profile = SenderProfile {
                                name: "Multi File Benchmark User".to_string(),
                                avatar_b64: None,
                            };

                            let request = dropx_sender::SendFilesRequest {
                                profile,
                                files,
                                config: config.clone(),
                            };

                            let bubble = send_files(black_box(request)).await;
                            assert!(bubble.is_ok());
                            let bubble = bubble.unwrap();

                            // Measure initialization of multiple files
                            black_box(bubble.get_ticket());
                            black_box(bubble.get_confirmation());
                        })
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark sender configuration performance impact
fn bench_sender_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("sender_configs");
    group.measurement_time(BENCHMARK_TIME_LIMIT);

    let configs = vec![
        ("custom_tiny", SenderConfig { buffer_size: 64 }),
        ("custom_small", SenderConfig { buffer_size: 512 }),
        ("low_bandwidth", SenderConfig::low_bandwidth()),
        ("balanced", SenderConfig::balanced()),
        ("default", SenderConfig::default()),
        ("high_performance", SenderConfig::high_performance()),
        (
            "custom_huge",
            SenderConfig {
                buffer_size: 32 * 1024 * 1024,
            },
        ), // 32MB
    ];

    let test_data = generate_test_data(1024 * 1024); // 1MB test file

    for (config_name, config) in configs {
        group.bench_function(
            BenchmarkId::new("config_creation", config_name),
            |b| {
                b.iter(|| {
                    // Benchmark the config creation and basic operations
                    let config_copy = SenderConfig {
                        buffer_size: black_box(config.buffer_size),
                    };

                    // Simulate some buffer operations
                    let chunks_needed = (test_data.len()
                        + config_copy.buffer_size as usize
                        - 1)
                        / config_copy.buffer_size as usize;

                    black_box(chunks_needed);
                    black_box(config_copy.buffer_size);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sender_initialization,
    bench_data_reading,
    bench_sequential_reading,
    bench_multiple_files,
    bench_sender_configs,
);

criterion_main!(benches);
