# vpin

Rust library for working with Visual Pinball VPX files

Find it on crates.io:
https://crates.io/crates/vpin

Also available on npm as WASM package:
https://www.npmjs.com/package/@francisdb/vpin-wasm

Join [#vpxtool on "Virtual Pinball Chat" discord](https://discord.gg/eYsvyMu8) for support and questions.

## Documentation

https://docs.rs/vpin

## Features

The library provides several optional features that can be enabled:

- `parallel` (default): Enables parallel processing using rayon for better performance
- `wasm`: Enables WebAssembly bindings for browser/Node.js usage

To use only VPX functionality without DirectB2S support:

```toml
[dependencies]
vpin = { version = "0.20", default-features = false }
```

To enable specific features:

```toml
[dependencies]
vpin = { version = "0.20", default-features = false, features = ["parallel"] }
```

## Example code

Check the [examples folder](/examples)

## Expanded VPX Format

The library supports extracting VPX files to an expanded directory format for easier editing and version control.

For primitive mesh data, you can choose between two formats:

- **OBJ format** (default) - Text-based Wavefront OBJ, human-readable and widely supported
- **GLB format** (optional) - Binary GLTF, significantly faster I/O for large meshes and animation frames

Use `write_with_format()` to specify the format. Both formats are supported for reading, with OBJ checked first for
backward compatibility.

## Projects using vpin

https://github.com/francisdb/vpxtool
https://github.com/jsm174/vpx-editor

## Other links

* Visual Pinball https://github.com/vpinball/vpinball
* VPUniverse https://vpuniverse.com/
* VPForums https://www.vpforums.org/
* Virtual Pinball Chat on Discord https://discord.com/invite/YHcBrtT

## Running the integration tests

We expect a folder `~/vpinball/tables` to exist that contains a lot of `vpx` files. The tests will
recursively search for these files and run the tests on them.

```bash
cargo test --release -- --ignored --nocapture
```

### WASM tests for server-side WASM (wasmtime)

```bash
# Install the target and wasmtime (do this only once)
rustup target add wasm32-wasip1
cargo install wasmtime-cli

# Run tests
cargo test --target wasm32-wasip1 --features wasm
```

### WASM tests for browser (wasm-bindgen-test)

```bash
# Install the target and wasm-bindgen-test (do this only once)
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli

# Run tests
cargo test --target wasm32-unknown-unknown
```

## Making a release

We use https://github.com/MarcoIeni/release-plz which creates a release pr on every commit to master
