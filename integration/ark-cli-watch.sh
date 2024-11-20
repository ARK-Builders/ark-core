#!/bin/bash

# Set up paths
WATCH_DIR="test-assets"
OUTPUT_FILE="ark-watch-output.txt"
INDEX_FILE="$WATCH_DIR/.ark/index"
ARK_CLI="./target/release/ark-cli"

# Function to check the index file content
check_index() {
  # Expecting a certain number of resources based on the operations done
  expected_count=$1
  shift  
  expected_resources=("$@")  

  # Get the actual count of resources in the index
  resources_count=$(jq '.resources | keys | length' "$INDEX_FILE")
  
  if [ "$resources_count" -ne "$expected_count" ]; then
    echo "Index sanity check failed: expected $expected_count resources, found $resources_count"
    exit 1
  fi

  # Check the paths of the resources in the index
  for resource in "${expected_resources[@]}"; do
    if ! jq -e ".resources | has(\"$resource\")" "$INDEX_FILE" > /dev/null; then
      echo "Index sanity check failed: resource \"$resource\" not found in index."
      exit 1
    fi
  done
  
  echo "Current resources in index:"
  jq '.resources' "$INDEX_FILE"
}

# Start `ark-cli watch` in the background and capture output
echo "Starting ark-cli watch on $WATCH_DIR..."
$ARK_CLI watch "$WATCH_DIR" > "$OUTPUT_FILE" &
WATCH_PID=$!
sleep 1  # Wait a bit to ensure the watch command is up

# Initial sanity check for index file
check_index 2 "test.pdf" "lena.jpg"  # Initially should contain lena.jpg and test.pdf

echo "Modifying files in $WATCH_DIR..."

# Step 1: Copy `lena.jpg` to `lena_copy.jpg`
cp "$WATCH_DIR/lena.jpg" "$WATCH_DIR/lena_copy.jpg"
sleep 3

check_index 3 "lena.jpg" "lena_copy.jpg" "test.pdf"  

# Step 2: Remove `test.pdf`
rm "$WATCH_DIR/test.pdf"
sleep 3

check_index 2 "lena.jpg" "lena_copy.jpg"  

# Step 3: Create a new empty file `note.txt`
touch "$WATCH_DIR/note.txt"
sleep 3

# Final index check after all operations
echo "Verifying final index state..."
check_index 3 "lena.jpg" "lena_copy.jpg" "note.txt"  # Expect three resources now

# Allow `ark-cli watch` time to process and then kill it
sleep 1
kill $WATCH_PID

# Wait briefly for output to complete
wait $WATCH_PID 2>/dev/null

# Read and verify the ark-watch-output.txt contents
echo "Checking ark-cli watch output..."
expected_change_count=3  # Three file changes done
actual_change_count=$(grep -c "Index updated with a single file change" "$OUTPUT_FILE")

if [ "$actual_change_count" -ne "$expected_change_count" ]; then
  echo "Output verification failed: expected $expected_change_count updates, found $actual_change_count"
  exit 1
fi

echo "All checks passed successfully!"
