## Requirements

cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android

## Usage Guide

### Go to Rust Directory

```sh
cd rust/
```

### Build the dylib

```sh
cargo build
```

### Build the Android libraries in jniLibs

```sh
cargo ndk -o ../android/app/src/main/jniLibs --manifest-path ./Cargo.toml -t armeabi-v7a -t arm64-v8a -t x86 -t x86_64 build --release
```

### Create Kotlin bindings

```sh
cargo run --bin uniffi-bingen generate --library ./target/debug/rpc_core.dll --language kotlin --out-dir ../android/app/src/main/java/ark/rpc_core
```
