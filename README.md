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

### Derived Mesh Generation

When extracting VPX files, the library can optionally generate mesh files for game items that don't store explicit mesh
data but are defined by drag points (walls, ramps, rubbers, flashers). Use `ExpandOptions` to enable this:

```rust
use vpin::vpx::expanded::{ExpandOptions, PrimitiveMeshFormat};

let options = ExpandOptions::new()
.mesh_format(PrimitiveMeshFormat::Glb)
.generate_derived_meshes(true);
```

## VPinball Coordinate System

VPinball uses a left-handed coordinate system:

```
    (0,0)───────────────→ +X (right)
      │   ┌───────────┐
      │   │           │
      │   │ Playfield │
      │   │           │
      ↓   └───────────┘
     +Y (towards player)

    +Z points up (towards the cabinet top glass)
```

- **Origin**: Top-left corner of the playfield (near the back of the cabinet)
- **X-axis**: Positive to the right (across the playfield)
- **Y-axis**: Positive towards the player (down the playfield)
- **Z-axis**: Positive upward (towards the glass)

### Polygon Winding Order

Due to the Y-axis pointing down, winding order appears reversed compared to standard mathematical conventions:

- A polygon whose vertices go **clockwise on screen** has a **positive** orientation determinant
- A polygon whose vertices go **counter-clockwise on screen** has a **negative** orientation determinant

VPinball's triangulation algorithm (used for flashers, walls, etc.) normalizes all polygons to counter-clockwise order
before processing. This library matches that behavior.

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
