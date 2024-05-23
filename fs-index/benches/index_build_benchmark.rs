use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};
use dev_hash::Crc32;
use fs_index::index::ResourceIndex;

const DIR_PATH: &str = "../test-assets/"; // Set the path to the directory containing the resources here

fn index_build_benchmark(c: &mut Criterion) {
    // assert the path exists and is a directory
    assert!(
        std::path::Path::new(DIR_PATH).is_dir(),
        "The path: {} does not exist or is not a directory",
        DIR_PATH
    );

    let mut group = c.benchmark_group("index_build");
    group.measurement_time(std::time::Duration::from_secs(20)); // Set the measurement time here

    let mut collisions_size = 0;

    group.bench_with_input(
        BenchmarkId::new("index_build", DIR_PATH),
        &DIR_PATH,
        |b, path| {
            b.iter(|| {
                let index: ResourceIndex<Crc32> =
                    ResourceIndex::build(black_box(path.to_string()));
                collisions_size = index.collisions.len();
            });
        },
    );
    group.finish();

    println!("Collisions: {}", collisions_size);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = index_build_benchmark
}
criterion_main!(benches);
