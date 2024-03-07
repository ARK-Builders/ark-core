use criterion::{black_box, criterion_group, criterion_main, Criterion};
use data_resource::ResourceId;
use pprof::criterion::{Output, PProfProfiler};
use rand::prelude::*;
use std::fs;

const FILE_PATHS: [&str; 2] = [
    // Add files to benchmark here
    "../test-assets/lena.jpg",
    "../test-assets/test.pdf",
];

fn generate_random_data(size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen()).collect()
}

fn compute_bytes_on_raw_data(c: &mut Criterion) {
    let inputs = [
        ("compute_bytes_small", 1024),
        ("compute_bytes_medium", 65536),
        ("compute_bytes_large", 1048576),
    ];

    for (name, size) in inputs.iter() {
        let input_data = generate_random_data(*size);
        let mut group = c.benchmark_group(name.to_string());
        group.measurement_time(std::time::Duration::from_secs(5)); // Set the measurement time here

        group.bench_function("compute_bytes", move |b| {
            b.iter(|| {
                ResourceId::compute_bytes(black_box(&input_data))
                    .expect("compute_bytes returned an error")
            });
        });

        group.finish();
    }
}

fn compute_bytes_on_files_benchmark(c: &mut Criterion) {
    // assert the files exist and are files
    for file_path in FILE_PATHS.iter() {
        assert!(
            std::path::Path::new(file_path).is_file(),
            "The file: {} does not exist or is not a file",
            file_path
        );
    }

    for file_path in FILE_PATHS.iter() {
        let raw_bytes = fs::read(file_path).unwrap();
        let mut group = c.benchmark_group(file_path.to_string());
        group.measurement_time(std::time::Duration::from_secs(10)); // Set the measurement time here

        group.bench_function("compute_bytes", move |b| {
            b.iter(|| {
                ResourceId::compute_bytes(black_box(&raw_bytes))
                    .expect("compute_bytes returned an error")
            });
        });

        group.finish();
    }
}

criterion_group!(
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = compute_bytes_on_raw_data, compute_bytes_on_files_benchmark
);
criterion_main!(benches);
