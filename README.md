# ArkLib

<<<<<<< HEAD
This is the home of core ARK-Navigator library.

Being implemented in Rust, it provides us capability to port ARK-Navigator to all common platforms.\
The purpose of the library is to manage _tag storage_ and _resource index_.

**The Concept of the library**

_Resource index_ is what allows ARK-Navigator to work store metadata of files without paying attention to their paths. Basically, it is just mapping between paths and ids. Currently, ids are CRC-32 checksums but more complicated ids should be invented for a numerous reasons, see <https://github.com/ARK-Builders/ARK-Navigator/issues/147>

We call a file as "resource" in our app, because resources could be decoupled from particular filesystems in future.

_Tag storage_ is what makes files searchable using simple concept of "tags". At the moment, tag storage is just mapping between resource ids to sets of tags.

Right now, both index and storage are defined only as Kotlin code in ARK-Navigator. They are being ported into Rust.

**Interfaces to the library**

At the moment, only Android wrapper exists:\
<https://github.com/ARK-Builders/arklib-android>

This connector can be used to implement apps with capability of marking local resources with tags for future search.

**Applications using the library**

ARK-Navigator itself, which is _resource navigator_ for Android. You can think about it as file browser enabled with tags feature for any file.\
<https://github.com/ARK-Builders/ARK-Navigator>

Auxiliary tool for working with folders containing _tags_, from your laptop using terminal.\
<https://github.com/ARK-Builders/ARK-CLI>

## Prerequisites

- PDFium prebuilt ([bblanchon](https://github.com/bblanchon/pdfium-binaries))
=======
This is the home of core ARK library.

Being implemented in Rust, it provides us capability to port all our apps to all common platforms. Right now, Android is supported by using the [arklib-android](https://github.com/arK-Builders/arklib-android) project. And for Linux/macOS/Windows, the library can be used as-is and easily embedded into an app, e.g. built with [Tauri](https://tauri.app/). Development docs will come sometime.

**The Concept of the library**

The purpose of the library is to manage _resource index_ of folders with various _user data_, as well as to manage user-defined metadata: tags, scores, arbitrary properties like movie title or description. Such a metadata is persisted to filesystem for easier sync and backup. The _resource index_ provides us with [content addressing](https://en.wikipedia.org/wiki/Content-addressable_storage) and allows easier storage and versions tracking. We also believe it'll allow easier cross-device sync implementation.

## Packages

<div align="center">

| Package         | Description                              |
| --------------- | ---------------------------------------- |
| `data-resource` | Resource hashing and ID construction     |
| `fs-index`      | Resource Index construction and updating |
| `fs-storage`    | Filesystem storage for resources         |
| `data-link`     | Linking resources                        |
| `data-pdf`      | PDF handling                             |
| `data-error`    | Error handling                           |
| `data-json`     | JSON serialization and deserialization   |

</div>
>>>>>>> main

## Build

Like most of Rust projects:

```bash
cargo build --release
```

Run unit tests:

```bash
cargo test
```
<<<<<<< HEAD
=======

## Development

For easier testing and debugging, we have the [ARK-CLI](https://github.com/ARK-Builders/ARK-CLI) tool working with ARK-enabled folders.

## Benchmarks

`fs-index` relies on the `criterion` crate for benchmarking to ensure optimal performance. Benchmarks are crucial for evaluating the efficiency of various functionalities within the library.

### Running Benchmarks

To execute the benchmarks, run this command:

```bash
cargo bench
```

This command runs all benchmarks and generates a report in HTML format located at `target/criterion/report`. If you wish to run a specific benchmark, you can specify its name as an argument as in:

```bash
cargo bench index_build
```

### Benchmarking Local Files

Our benchmark suite includes tests on local files and directories. These benchmarks are located in the `benches/` directory of some crates. Each benchmark sets a time limit using `group.measurement_time()`, which you can adjust manually based on your requirements.

You have the flexibility to benchmark specific files or folders by modifying the variables within the benchmark files. By default, the benchmarks operate on the [`test-assets/`](test-assets/) directory and its contents. You can change the directory/files by setting the `DIR_PATH` and `FILE_PATHS` variables to the desired values.

For pre-benchmark assessment of required time to index a huge local folder, you can modify `test_build_resource_index` test case in `src/index.rs`.
>>>>>>> main
