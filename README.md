# r2proto3 - Rust types to Protobuf 3 convertor

It simply spreads the idea that you shouldn't write Protobuf files for yourself if you write type-safe Rust code.

## Usage

For use, you should install `r2proto3` first:

```bash
git clone https://github.com/markcda/r2proto3.git
cd r2proto3
cargo install --path .
```

Prepare the code of your crate to being parsed. You should place `// NOTE: ToProtobuf` comment line right before struct or enum declaration.

Execute the command:

```bash
r2proto3 --crate-root {path to crate} --output-file generated.proto
```

Here we go!

## Notes

Services and RPC are not supported for now. Anyway, you should write these with `tonic` library.

Supported map types are `std::collections::HashMap` and `std::collections::BTreeMap`.
