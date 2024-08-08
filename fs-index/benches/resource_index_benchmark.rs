use std::path::PathBuf;

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};
use tempfile::TempDir;

use dev_hash::Crc32;
use fs_index::ResourceIndex;

fn resource_index_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_index");
    group.measurement_time(std::time::Duration::from_secs(20)); // Set the measurement time here

    let benchmarks_dir = setup_temp_dir();
    let benchmarks_dir = benchmarks_dir.path();
    let benchmarks_dir_str = benchmarks_dir.to_str().unwrap();

    // Benchmark `ResourceIndex::build()`

    let mut collisions_size = 0;
    group.bench_with_input(
        BenchmarkId::new("index_build", benchmarks_dir_str),
        &benchmarks_dir,
        |b, path| {
            b.iter(|| {
                let index: ResourceIndex<Crc32> =
                    ResourceIndex::build(black_box(path)).unwrap();
                collisions_size = index.collisions().len();
            });
        },
    );
    println!("Collisions: {}", collisions_size);

    // Benchmark `ResourceIndex::get_resources_by_id()`
    let index: ResourceIndex<Crc32> =
        ResourceIndex::build(benchmarks_dir).unwrap();
    let resources = index.resources();
    let resource_id = resources[0].id();
    group.bench_function("index_get_resource_by_id", |b| {
        b.iter(|| {
            let _resource = index.get_resources_by_id(black_box(resource_id));
        });
    });

    // Benchmark `ResourceIndex::get_resource_by_path()`
    let resource_path = resources[0].path();
    group.bench_function("index_get_resource_by_path", |b| {
        b.iter(|| {
            let _resource =
                index.get_resource_by_path(black_box(resource_path));
        });
    });

    // Benchmark `ResourceIndex::update_all()`

    // First, create a new temp directory specifically for the update_all
    // benchmark since we will be creating new files, removing files, and
    // modifying files
    let update_all_benchmarks_dir =
        TempDir::with_prefix("ark-fs-index-benchmarks-update-all").unwrap();
    let update_all_benchmarks_dir = update_all_benchmarks_dir.path();

    group.bench_function("index_update_all", |b| {
        b.iter(|| {
            // Clear the directory
            std::fs::remove_dir_all(&update_all_benchmarks_dir).unwrap();
            std::fs::create_dir(&update_all_benchmarks_dir).unwrap();

            // Create 5000 new files
            for i in 0..5000 {
                let new_file =
                    update_all_benchmarks_dir.join(format!("file_{}.txt", i));
                std::fs::File::create(&new_file).unwrap();
                std::fs::write(&new_file, format!("Hello, World! {}", i))
                    .unwrap();
            }
            let mut index: ResourceIndex<Crc32> =
                ResourceIndex::build(black_box(&update_all_benchmarks_dir))
                    .unwrap();

            update_all_files(&update_all_benchmarks_dir.to_path_buf());
            let _update_result = index.update_all().unwrap();
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = resource_index_benchmark
}
criterion_main!(benches);

/// A helper function to setup a temp directory for the benchmarks
fn setup_temp_dir() -> TempDir {
    // Create a temp directory
    let temp_dir = TempDir::with_prefix("ark-fs-index-benchmarks").unwrap();
    let benchmarks_dir = temp_dir.path();
    log::info!("Temp directory for benchmarks: {:?}", benchmarks_dir);

    // Create 10,000 files in the temp directory
    for i in 0..10000 {
        let new_file = benchmarks_dir.join(format!("file_{}.txt", i));
        std::fs::File::create(&new_file).unwrap();
        // We add the index `i` to the file content to make sure the content is
        // unique This is to avoid collisions in the index
        std::fs::write(&new_file, format!("Hello, World! {}", i)).unwrap();
    }

    temp_dir
}

/// A helper function that takes a directory and creates 5000 new files, removes
/// 3000 files, and modifies 1000 files
///
/// Note: The function assumes that the directory already contains 5000 files
/// with the names `file_0.txt` to `file_4999.txt`
fn update_all_files(dir: &PathBuf) {
    // Create 5000 new files
    for i in 5001..10001 {
        let new_file = dir.join(format!("file_{}.txt", i));
        std::fs::File::create(&new_file).unwrap();
        // We add the index `i` to the file content to make sure the content is
        // unique This is to avoid collisions in the index
        std::fs::write(&new_file, format!("Hello, World! {}", i)).unwrap();
    }

    // Remove 3000 files
    for i in 0..3000 {
        let removed_file = dir.join(format!("file_{}.txt", i));
        std::fs::remove_file(&removed_file).unwrap();
    }

    // Modify 1000 files
    for i in 4000..5000 {
        let modified_file = dir.join(format!("file_{}.txt", i));
        std::fs::write(&modified_file, "Hello, World!").unwrap();
    }
}
