# `dev-hash`

`dev-hash` is a crate that provides example implementations of `ResourceIdTrait`. It can be extended to include additional hash function implementations. The crate also includes benchmarks for the defined `ResourceId` types.

## Defined Types

| Type               | Description                                         |
| ------------------ | --------------------------------------------------- |
| `Blake3ResourceId` | Uses the Blake3 cryptographic hash function         |
| `Crc32ResourceId`  | Uses the CRC32 fast non-cryptographic hash function |
