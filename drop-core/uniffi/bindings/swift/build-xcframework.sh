#!/bin/bash
set -e

# Build the Rust library for iOS and macOS and create an XCFramework

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIFFI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$SCRIPT_DIR/build"
XCFRAMEWORK_NAME="arkdrop_uniffiFFI.xcframework"

echo "Building Rust library for iOS and macOS..."

# Clean previous builds
rm -rf "$BUILD_DIR"
rm -rf "$SCRIPT_DIR/$XCFRAMEWORK_NAME"
mkdir -p "$BUILD_DIR"

cd "$UNIFFI_DIR"

# Build for iOS device (arm64)
echo "Building for iOS (aarch64-apple-ios)..."
cargo build --release --target aarch64-apple-ios

# Build for iOS Simulator (x86_64 and arm64)
echo "Building for iOS Simulator..."
cargo build --release --target x86_64-apple-ios
cargo build --release --target aarch64-apple-ios-sim

# Build for macOS (x86_64 and arm64)
echo "Building for macOS..."
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Create fat libraries
echo "Creating universal libraries..."
mkdir -p "$BUILD_DIR/ios-simulator"
mkdir -p "$BUILD_DIR/macos"

lipo -create \
    "$UNIFFI_DIR/target/x86_64-apple-ios/release/libarkdrop_uniffi.a" \
    "$UNIFFI_DIR/target/aarch64-apple-ios-sim/release/libarkdrop_uniffi.a" \
    -output "$BUILD_DIR/ios-simulator/libarkdrop_uniffi.a"

lipo -create \
    "$UNIFFI_DIR/target/x86_64-apple-darwin/release/libarkdrop_uniffi.a" \
    "$UNIFFI_DIR/target/aarch64-apple-darwin/release/libarkdrop_uniffi.a" \
    -output "$BUILD_DIR/macos/libarkdrop_uniffi.a"

# Copy iOS device library
mkdir -p "$BUILD_DIR/ios"
cp "$UNIFFI_DIR/target/aarch64-apple-ios/release/libarkdrop_uniffi.a" "$BUILD_DIR/ios/"

# Copy headers and modulemap to each platform directory
for platform_dir in "$BUILD_DIR/ios" "$BUILD_DIR/ios-simulator" "$BUILD_DIR/macos"; do
    mkdir -p "$platform_dir/Headers"

    # Copy the C header
    if [ -f "$SCRIPT_DIR/arkdrop_uniffiFFI.h" ]; then
        cp "$SCRIPT_DIR/arkdrop_uniffiFFI.h" "$platform_dir/Headers/"
    else
        echo "Error: arkdrop_uniffiFFI.h not found. Run generate-bindings.sh first."
        exit 1
    fi

    # Copy the modulemap
    if [ -f "$SCRIPT_DIR/module.modulemap" ]; then
        cp "$SCRIPT_DIR/module.modulemap" "$platform_dir/Headers/"
    else
        echo "Error: module.modulemap not found. Run generate-bindings.sh first."
        exit 1
    fi
done

# Create XCFramework
echo "Creating XCFramework..."
xcodebuild -create-xcframework \
    -library "$BUILD_DIR/ios/libarkdrop_uniffi.a" \
    -headers "$BUILD_DIR/ios/Headers" \
    -library "$BUILD_DIR/ios-simulator/libarkdrop_uniffi.a" \
    -headers "$BUILD_DIR/ios-simulator/Headers" \
    -library "$BUILD_DIR/macos/libarkdrop_uniffi.a" \
    -headers "$BUILD_DIR/macos/Headers" \
    -output "$SCRIPT_DIR/$XCFRAMEWORK_NAME"

echo "XCFramework created successfully: $SCRIPT_DIR/$XCFRAMEWORK_NAME"
echo ""
echo "You can now use the Swift package with Xcode or Swift Package Manager"
