# ArkLib

This is the home of core ARK-Navigator library.

Being implemented in Rust, it provides us capability to port ARK-Navigator to all common platforms.\
The purpose of the library is to manage _tag storage_ and _resource index_.

**The Concept of the library**

_Resource index_ is what allows ARK-Navigator to work store metadata of files without paying attention to their paths. Basically, it is just mapping between paths and ids. Currently, ids are CRC-32 checksums but more complicated ids should be invented for a numerous reasons, see https://github.com/ARK-Builders/ARK-Navigator/issues/147

We call a file as "resource" in our app, because resources could be decoupled from particular filesystems in future.

_Tag storage_ is what makes files searchable using simple concept of "tags". At the moment, tag storage is just mapping between resource ids to sets of tags.

Right now, both index and storage are defined only as Kotlin code in ARK-Navigator. They are being ported into Rust.

**Interfaces to the library**

At the moment, only Android wrapper exists:\
https://github.com/ARK-Builders/arklib-android

This connector can be used to implement apps with capability of marking local resources with tags for future search.

**Applications using the library**

ARK-Navigator itself, which is _resource navigator_ for Android. You can think about it as file browser enabled with tags feature for any file.\
https://github.com/ARK-Builders/ARK-Navigator

Auxiliary tool for working with folders containing _tags_, from your laptop using terminal.\
https://github.com/ARK-Builders/ARK-CLI

## Build

Like most of Rust projects:

```bash
cargo build --release
```

Run unit tests:

```bash
cargo test
```
