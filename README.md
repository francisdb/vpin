# vpin

Rust library for the visual/virtual pinball ecosystem

https://crates.io/crates/vpin

## Documentation

https://docs.rs/vpin

## Projects using vpin

https://github.com/francisdb/vpxtool

## Other links

* Visual Pinball https://github.com/vpinball/vpinball
* VPUniverse https://vpuniverse.com/
* VPForums https://www.vpforums.org/
* Virtual Pinball Chat on Discord https://discord.com/invite/YHcBrtT

## Running the integration tests

We expect a folder `~/vpinball/tables` to exist that contains a lot of `vpx` and `directb2s` files. The tests will recursively search for these files and run the tests on them.

```bash
cargo test --release -- --ignored --nocapture
```

## Making a release

We use https://github.com/MarcoIeni/release-plz which creates a release pr on every commit to master
