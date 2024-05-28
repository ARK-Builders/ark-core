use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};
use tempfile::{tempdir, TempDir};

use dev_hash::Crc32;
use fs_index::index::ResourceIndex;

const DIR_PATH: &str = "../test-assets/"; // Set the path to the directory containing the resources here

/// A helper function to setup a temp directory for the benchmarks using the test assets
fn setup_temp_dir() -> TempDir {
    // assert the path exists and is a directory
    assert!(
        std::path::Path::new(DIR_PATH).is_dir(),
        "The path: {} does not exist or is not a directory",
        DIR_PATH
    );

    // Create a temp directory
    let temp_dir = tempdir().unwrap();
    let benchmarks_dir = temp_dir.path();
    let benchmarks_dir_str = benchmarks_dir.to_str().unwrap();
    log::info!("Temp directory for benchmarks: {}", benchmarks_dir_str);

    // Copy the test assets to the temp directory
    let source = std::path::Path::new(DIR_PATH);
    // Can't use fs::copy because the source is a directory
    let output = std::process::Command::new("cp")
        .arg("-r")
        .arg(source)
        .arg(benchmarks_dir_str)
        .output()
        .expect("Failed to copy test assets to temp directory");
    if !output.status.success() {
        panic!(
            "Failed to copy test assets to temp directory: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    temp_dir
}

fn resource_index_benchmark(c: &mut Criterion) {
    let benchmarks_dir = setup_temp_dir();
    let benchmarks_dir = benchmarks_dir.path();
    let benchmarks_dir_str = benchmarks_dir.to_str().unwrap();

    let mut group = c.benchmark_group("resource_index");
    group.measurement_time(std::time::Duration::from_secs(20)); // Set the measurement time here

    // Benchmark `ResourceIndex::build()`

    let mut collisions_size = 0;
    group.bench_with_input(
        BenchmarkId::new("index_build", benchmarks_dir_str),
        &benchmarks_dir,
        |b, path| {
            b.iter(|| {
                let index: ResourceIndex<Crc32> =
                    ResourceIndex::build(black_box(path));
                collisions_size = index.collisions.len();
            });
        },
    );
    println!("Collisions: {}", collisions_size);

    // Benchmark `ResourceIndex::update_all()`

    // First, create the index with the current state of the directory
    let mut index: ResourceIndex<Crc32> = ResourceIndex::build(&benchmarks_dir);

    // Create a new file
    assert!(
        std::path::Path::new(&benchmarks_dir).is_dir(),
        // THIS FAILS
        "The path: {:?} does not exist or is not a directory",
        benchmarks_dir
    );
    let new_file = benchmarks_dir.join("new_file.txt");
    std::fs::File::create(&new_file).unwrap();
    std::fs::write(&new_file, "Hello, World!").unwrap();

    // Remove a file
    let removed_file = benchmarks_dir.join("lena.jpg");
    std::fs::remove_file(&removed_file).unwrap();

    // Modify a file
    let modified_file = benchmarks_dir.join("test.pdf");
    std::fs::write(&modified_file, "Hello, World!").unwrap();

    group.bench_function("index_update_all", |b| {
        b.iter(|| {
            index.update_all().unwrap();
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
