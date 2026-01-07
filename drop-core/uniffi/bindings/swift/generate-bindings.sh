#!/bin/bash
set -e

# Generate Swift bindings using uniffi-bindgen
# This creates the .swift file and .modulemap for the Swift module

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIFFI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
WORKSPACE_ROOT="$(cd "$UNIFFI_DIR/../.." && pwd)"
OUTPUT_DIR="$SCRIPT_DIR"

echo "Generating Swift bindings..."
echo "Workspace root: $WORKSPACE_ROOT"
echo "UniFFI directory: $UNIFFI_DIR"
echo "Output directory: $OUTPUT_DIR"

cd "$UNIFFI_DIR"

echo "Building library for binding generation..."
cargo build --lib --release

LIBRARY_PATH="$WORKSPACE_ROOT/target/release/libarkdrop_uniffi.a"
if [ ! -f "$LIBRARY_PATH" ]; then
    echo "ERROR: Library not found at $LIBRARY_PATH"
    echo "Looking for library in target directory..."
    find "$WORKSPACE_ROOT/target" -name "libarkdrop_uniffi.*" || true
    exit 1
fi

echo "Using library: $LIBRARY_PATH"

echo "Generating Swift sources..."
if ! cargo run --bin uniffi-bindgen-swift -- --swift-sources "$LIBRARY_PATH" "$OUTPUT_DIR"; then
    echo "ERROR: Failed to generate Swift sources"
    exit 1
fi

echo "Generating C headers..."
if ! cargo run --bin uniffi-bindgen-swift -- --headers "$LIBRARY_PATH" "$OUTPUT_DIR"; then
    echo "ERROR: Failed to generate headers"
    exit 1
fi

echo "Generating modulemap..."
if ! cargo run --bin uniffi-bindgen-swift -- --modulemap "$LIBRARY_PATH" "$OUTPUT_DIR"; then
    echo "ERROR: Failed to generate modulemap"
    exit 1
fi

echo ""
echo "Listing output directory contents:"
ls -la "$OUTPUT_DIR" | grep -E '\.(swift|h|modulemap)' || echo "Warning: No binding files found"

if [ -f "$OUTPUT_DIR/arkdrop_uniffi.modulemap" ]; then
    mv "$OUTPUT_DIR/arkdrop_uniffi.modulemap" "$OUTPUT_DIR/module.modulemap"
    echo "Renamed modulemap to module.modulemap"
fi

# Verify required files exist
echo ""
echo "Verifying generated files..."
for required_file in \
    "$OUTPUT_DIR/ArkDrop.swift" \
    "$OUTPUT_DIR/arkdrop_uniffiFFI.h" \
    "$OUTPUT_DIR/module.modulemap"; do
    if [ ! -f "$required_file" ]; then
        echo "ERROR: Required file not generated: $required_file"
        exit 1
    else
        echo "✓ Found: $required_file"
    fi
done

echo ""
echo "Swift bindings generated successfully!"
