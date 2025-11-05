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
cargo run --bin uniffi-bindgen generate \
    ./src/drop.udl \
    --language swift \
    --out-dir "$OUTPUT_DIR"

# Rename modulemap to module.modulemap (required for XCFramework)
if [ -f "$OUTPUT_DIR/arkdrop_uniffiFFI.modulemap" ]; then
    mv "$OUTPUT_DIR/arkdrop_uniffiFFI.modulemap" "$OUTPUT_DIR/module.modulemap"
    echo "Renamed modulemap to module.modulemap"
fi

echo "Swift bindings generated successfully!"
echo "Generated files:"
ls -la "$OUTPUT_DIR"/*.swift "$OUTPUT_DIR"/*.modulemap 2>/dev/null || true
