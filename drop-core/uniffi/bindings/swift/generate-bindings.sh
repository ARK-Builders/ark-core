#!/bin/bash
set -e

# Generate Swift bindings using uniffi-bindgen
# This creates the .swift file and .modulemap for the Swift module

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIFFI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$SCRIPT_DIR"

echo "Generating Swift bindings..."
echo "UniFFI directory: $UNIFFI_DIR"
echo "Output directory: $OUTPUT_DIR"

cd "$UNIFFI_DIR"

# Generate Swift bindings
echo "Running: cargo run --bin uniffi-bindgen generate ./src/drop.udl --language swift --out-dir $OUTPUT_DIR"
if ! cargo run --bin uniffi-bindgen generate \
    ./src/drop.udl \
    --language swift \
    --out-dir "$OUTPUT_DIR"; then
    echo "ERROR: Failed to generate Swift bindings"
    exit 1
fi

echo ""
echo "Listing output directory contents:"
ls -la "$OUTPUT_DIR" | grep -E '\.(swift|h|modulemap)' || echo "Warning: No binding files found"

# Rename modulemap to module.modulemap (required for XCFramework)
if [ -f "$OUTPUT_DIR/arkdrop_uniffiFFI.modulemap" ]; then
    mv "$OUTPUT_DIR/arkdrop_uniffiFFI.modulemap" "$OUTPUT_DIR/module.modulemap"
    echo "Renamed modulemap to module.modulemap"
fi

# Verify required files exist
echo ""
echo "Verifying generated files..."
for required_file in \
    "$OUTPUT_DIR/drop.swift" \
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
