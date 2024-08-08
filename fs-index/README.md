# fs-index

`fs-index` is a Rust crate for managing and indexing file system resources. It provides a flexible and efficient way to track changes, query resources, and keep files in sync across multiple devices or locations.

Originally developed for the ARK framework to support local-first applications, `fs-index` can also be used in various scenarios including backup systems, content management, and more.

## Features

The most important struct in this crate is `ResourceIndex` which comes with:

- **Reactive API**
  - `update_all`: Method to update the index by rescanning files and returning changes (additions/deletions).
- **Snapshot API**
  - `get_resources_by_id`: Query resources from the index by ID.
  - `get_resource_by_path`: Query a resource from the index by its path.

## Custom Serialization

The `ResourceIndex` struct includes a custom serialization implementation to avoid writing a large repetitive index file with double maps.

## Tests and Benchmarks

- Unit tests are located in `src/tests.rs`.
- The benchmarking suite is in `benches/resource_index_benchmark.rs`, benchmarking all methods of `ResourceIndex`.
  - Run benchmarks with `cargo bench`.

## Examples

To get started, take a look at the examples in the `examples/` directory.

To run a specific example:

```shell
cargo run --example resource_index
```
