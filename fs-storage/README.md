# Ark file system storage

File system storage implementation for writing key value pairs to disk.

## Steps to use CLI

- Create a test.json file of key:values pairs you want to store.

```json
{
  "key1": "value1",
  "key2": "value2",
  "key3": "value3"
}
```

- Select between two storage options:
  - file: Stores multiple key-value pairs in a single file.
  - folder: Stores each value in a separate file within a folder.

- Run Write Command

```bash
cargo run --example cli [file|folder] write /tmp/z test.json
```

Alternatively, you can directly provide the input data as a comma-separated list of key-value pairs

```bash
cargo run --example cli [file|folder] write /tmp/z a:1,b:2,c:3
```

- Run Read Command

```bash
cargo run --example cli [file|folder] read /tmp/z key1,key2
```

- Get Output

```bash
key1: value1
key2: value2
```

- To get all key value pairs

```bash
cargo run --example cli [file|folder] read /tmp/z
```
