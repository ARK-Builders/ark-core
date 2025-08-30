# Drop CLI

A clean, user-friendly CLI tool for sending and receiving files with customizable profiles.

## Features

- 📤 **Send files** with progress tracking
- 📥 **Receive files** to specified directories
- 👤 **Custom names** and avatars for personalization
- 🖼️ **Avatar support** via file path or base64 encoding
- 🔒 **Secure transfers** with ticket and confirmation system
- 📊 **Progress tracking** with emoji-rich output
- ❌ **Graceful cancellation** with Ctrl+C

## Installation

```bash
cargo install drop-cli
```

Or build from source:

```bash
git clone https://github.com/yourusername/drop-cli
cd drop-cli
cargo build --release
```

## Usage

### Sending Files

Basic file sending:
```bash
drop-cli send file1.txt file2.jpg document.pdf
```

With custom name:
```bash
drop-cli send --name "Alice" file1.txt file2.jpg
```

With avatar from file:
```bash
drop-cli send --name "Alice" --avatar avatar.png file1.txt
```

With base64 avatar:
```bash
drop-cli send --name "Alice" --avatar-b64 "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==" file1.txt
```

### Receiving Files

Basic file receiving:
```bash
drop-cli receive ./downloads "ticket-string" "123"
```

With custom name and avatar:
```bash
drop-cli receive --name "Bob" --avatar profile.jpg ./downloads "ticket-string" "123"
```

## Command Reference

### `send` command

Send files to another user.

**Arguments:**
- `files`: One or more files to send (required)

**Options:**
- `-n, --name <NAME>`: Your display name (default: "drop-cli-sender")
- `-a, --avatar <PATH>`: Path to avatar image file
- `--avatar-b64 <BASE64>`: Base64 encoded avatar image

**Example:**
```bash
drop-cli send --name "John" --avatar ./my-avatar.png file1.txt file2.pdf
```

### `receive` command

Receive files from another user.

**Arguments:**
- `output`: Output directory for received files (required)
- `ticket`: Transfer ticket from sender (required)
- `confirmation`: Confirmation code from sender (required)

**Options:**
- `-n, --name <NAME>`: Your display name (default: "drop-cli-receiver")
- `-a, --avatar <PATH>`: Path to avatar image file
- `--avatar-b64 <BASE64>`: Base64 encoded avatar image

**Example:**
```bash
drop-cli receive --name "Jane" ./downloads "abc123ticket" "456"
```

## Configuration

The tool supports runtime configuration through command-line arguments. You can set:

- **Display Name**: Shown to the other party during transfer
- **Avatar**: Profile picture shown during transfer (supports common image formats)

### Avatar Formats

Avatars can be provided in two ways:

1. **File Path**: `--avatar path/to/image.png`
   - Supports common image formats (PNG, JPG, GIF, etc.)
   - Automatically converted to base64

2. **Base64 String**: `--avatar-b64 "base64-encoded-string"`
   - Direct base64 input
   - Useful for programmatic usage

## Examples

### Complete Send Example

```bash
# Send multiple files with custom profile
drop-cli send \
  --name "Alice Smith" \
  --avatar ./profile.jpg \
  document.pdf \
  presentation.pptx \
  image.png
```

Output:
```
📤 Preparing to send 3 file(s)...
   📄 document.pdf
   📄 presentation.pptx
   📄 image.png
👤 Sender name: Alice Smith
🖼️  Avatar: Set
📦 Ready to send files!
🎫 Ticket: "abc123def456"
🔑 Confirmation: "789"
⏳ Waiting for receiver... (Press Ctrl+C to cancel)
```

### Complete Receive Example

```bash
# Receive files with custom profile
drop-cli receive \
  --name "Bob Johnson" \
  --avatar ./avatar.png \
  ./downloads \
  "abc123def456" \
  "789"
```

Output:
```
📥 Preparing to receive files...
📁 Output directory: ./downloads
🎫 Ticket: abc123def456
🔑 Confirmation: 789
👤 Receiver name: Bob Johnson
🖼️  Avatar: Set
📥 Starting file transfer...
📁 Files will be saved to: ./downloads/550e8400-e29b-41d4-a716-446655440000
🔗 Connected to sender:
   📛 Name: Alice Smith
   🆔 ID: 123e4567-e89b-12d3-a456-426614174000
   📁 Files to receive: 3
     📄 document.pdf
     📄 presentation.pptx
     📄 image.png
⏳ Receiving files... (Press Ctrl+C to cancel)
```

## Error Handling

The tool provides clear error messages for common issues:

- **Missing files**: Validates all files exist before starting transfer
- **Invalid paths**: Checks directory permissions and accessibility
- **Network issues**: Graceful handling of connection problems
- **Invalid arguments**: Clear guidance on correct usage

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running in Development

```bash
# Send files
cargo run -- send --name "Dev User" test-file.txt

# Receive files
cargo run -- receive --name "Dev User" ./output "ticket" "123"
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with Rust 🦀
- Uses [clap](https://github.com/clap-rs/clap) for CLI parsing
- Uses [tokio](https://github.com/tokio-rs/tokio) for async runtime