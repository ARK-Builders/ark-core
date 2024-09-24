## Requirements

cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android

## Usage Guide

### Build the dylib

```sh
cargo build -p rpc_example --release
```

### Build the Android libraries in jniLibs

```sh
cargo ndk -o ./rpc_example/jniLibs --manifest-path ./rpc_example/Cargo.toml -t armeabi-v7a -t arm64-v8a -t x86 -t x86_64 build --release
```

### Create Kotlin bindings

```sh
cargo run -p rpc_example --features=uniffi/cli --bin uniffi-bindgen generate --library ./target/release/rpc_example.dll --language kotlin --out-dir ./rpc_example/bindings
```