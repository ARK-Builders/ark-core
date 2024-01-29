# ArkLib

This is the home of core ARK library.

Being implemented in Rust, it provides us capability to port all our apps to all common platforms. Right now, Android is supported by using the [arklib-android](https://github.com/arK-Builders/arklib-android) project. And for Linux/macOS/Windows, the library can be used as-is and easily embedded into an app, e.g. built with [Tauri](https://tauri.app/). Development docs will come sometime.

**The Concept of the library**

The purpose of the library is to manage _resource index_ of folders with various _user data_, as well as to manage user-defined metadata: tags, scores, arbitrary properties like movie title or description. Such a metadata is persisted to filesystem for easier sync and backup. The _resource index_ provides us with [content addressing](https://en.wikipedia.org/wiki/Content-addressable_storage) and allows easier storage and versions tracking. We also believe it'll allow easier cross-device sync implementation.

## Prerequisites

- PDFium prebuilt ([bblanchon](https://github.com/bblanchon/pdfium-binaries))

## Build

Like most of Rust projects:

```bash
cargo build --release
```

Run unit tests:

```bash
cargo test
```

## Development

For easier testing and debugging, we have the [ARK-CLI](https://github.com/ARK-Builders/ARK-CLI) tool working with ARK-enabled folders.
