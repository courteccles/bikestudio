# bikestudio

An open-source Linux and macOS app for designing rigid, hardtail, and full suspension bike frames.

bikestudio is in early development. See:

- [`decisions.md`](decisions.md): the confirmed product decisions
- [`bikestudio-plan.md`](bikestudio-plan.md): the architecture and build plan
- [`specs/`](specs/): the version 1 specs, in build order

## Design

These mockups show the planned designer experience.

![The Specification step: geometry is pinned in the right pane, and the side view redraws with dimensions](designer%20experience%20images/Specification/10-reach.png)

<details>
<summary>The Specification step, screen by screen</summary>

| Screen | Step |
| --- | --- |
| ![Projects menu: new, open, recent and save](designer%20experience%20images/Specification/01-projects-menu.png) | Projects menu: new, open, recent and save |
| ![Change the projects folder](designer%20experience%20images/Specification/02-change-folder.png) | Change the projects folder |
| ![Start a new project](designer%20experience%20images/Specification/03-new-project.png) | Start a new project |
| ![A new project opens empty](designer%20experience%20images/Specification/04-blank-project.png) | A new project opens empty |
| ![Front travel set: a hardtail](designer%20experience%20images/Specification/05-front-travel-hardtail.png) | Front travel set: a hardtail |
| ![Rear travel set: full suspension](designer%20experience%20images/Specification/06-rear-travel-full-squish.png) | Rear travel set: full suspension |
| ![Pin the head angle](designer%20experience%20images/Specification/07-head-angle.png) | Pin the head angle |
| ![Pin the seat angle](designer%20experience%20images/Specification/08-seat-angle.png) | Pin the seat angle |
| ![Pin the stack](designer%20experience%20images/Specification/09-stack.png) | Pin the stack |
| ![Pin the reach](designer%20experience%20images/Specification/10-reach.png) | Pin the reach |
| ![Pin the chainstay length](designer%20experience%20images/Specification/11-chainstay.png) | Pin the chainstay length |
| ![Pin the bottom bracket drop](designer%20experience%20images/Specification/12-bb-drop.png) | Pin the bottom bracket drop |
| ![Save as a .bikestud file](designer%20experience%20images/Specification/13-save-as.png) | Save as a .bikestud file |
| ![Settings](designer%20experience%20images/Specification/14-settings.png) | Settings |

</details>

## Build

bikestudio builds with the Rust version pinned in `rust-toolchain.toml`. With [rustup](https://rustup.rs) installed, Cargo picks it up automatically.

```sh
cargo test --workspace
```

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
