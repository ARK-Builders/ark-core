# `dev-hash`

`dev-hash` contains reference implementations of `ResourceId` trait and should be used only as an example, or as a dependency in test cases. It will be extended to include additional hash function implementations. The crate also includes benchmarks for the defined `ResourceId` types.

## Defined Types

| Type     | Description                                                                 |
|----------|-----------------------------------------------------------------------------|
| `Blake3` | Impl of `ResourceId` that uses the Blake3 cryptographic hash function       |
| `Crc32`  | Impl of `ResourceId` that uses the CRC32 non-cryptographic hash function |
