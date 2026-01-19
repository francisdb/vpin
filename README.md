# vpin

Rust library for the visual/virtual pinball ecosystem

https://crates.io/crates/vpin

Join [#vpxtool on "Virtual Pinball Chat" discord](https://discord.gg/eYsvyMu8) for support and questions.

## Documentation

https://docs.rs/vpin

## Example code

Check the [examples folder](examples/)

## Projects using vpin

https://github.com/francisdb/vpxtool

## Other links

* Visual Pinball https://github.com/vpinball/vpinball
* VPUniverse https://vpuniverse.com/
* VPForums https://www.vpforums.org/
* Virtual Pinball Chat on Discord https://discord.com/invite/YHcBrtT

## Running the integration tests

We expect a folder `~/vpinball/tables` to exist that contains a lot of `vpx` and `directb2s` files. The tests will
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
