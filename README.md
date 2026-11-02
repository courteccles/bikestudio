# bikestudio

A free, open-source Linux and macOS app where boutique and hobby builders settle rigid, hardtail, or full suspension frame geometry and pivots. It exports a drawing and CAD-ready data for detailed design.

bikestudio is in early development. See:

- [`decisions.md`](decisions.md): the confirmed product decisions
- [`bikestudio-plan.md`](bikestudio-plan.md): the architecture and build plan
- [`specs/`](specs/): the version 1 specs, in build order

## Build

bikestudio builds with the Rust version pinned in `rust-toolchain.toml`. With [rustup](https://rustup.rs) installed, Cargo picks it up automatically.

```sh
cargo test --workspace
```

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
