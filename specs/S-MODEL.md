# S-MODEL — Workspace, Types and Save/Load

**Status:** approved 2026-11-01. **Source:** plan section 1 and 2, D13, D15, D16, D18, D25 in `../decisions.md`.

## Purpose

Set up the repository, workspace and CI, and define the `Project` types and the versioned save and load that every later spec builds on.

## Decisions

| # | Decision | Status |
|---|---|---|
| M1 | Hosting: git on GitHub, CI on GitHub Actions. Local repository created on branch `main`; no GitHub repository created or pushed yet. | Confirmed |
| M2 | One `.bikestud` file per project, pretty-printed JSON. Users organize projects in ordinary folders (for example `~/Bikes/Customers`), as the mockups show. | Confirmed |
| M3 | The "Next: Linkage →" button in the mockups is tab navigation, not a wizard (consistent with D20). | Confirmed |
| M4 | No file compatibility promise until after version 1. Files saved during development may stop opening. Every file still carries `schema_version`, so migrations can be added when the promise begins. | Confirmed |
| M5 | Toolchain: Rust 1.99.1 pinned in `rust-toolchain.toml`, raised on purpose. CI and local builds use it through rustup (the owner switches from Arch's `rust` package to `rustup`). | Confirmed |
| M6 | S-MODEL defines only the skeleton: `Project` with `schema_version`, wheels and travel, the unit types, and save/load. Each later spec adds its own fields and types to `Project`. | Confirmed |

## Scope

1. **Repository files:** `Cargo.toml` workspace, `rust-toolchain.toml` (1.99.1), `LICENSE-MIT`, `LICENSE-APACHE`, `README.md`, `.gitignore`.
2. **Crates:** `bike-model` only. Other crates are created by the specs that need them.
3. **Unit types:** `Millimeters` and `Degrees` only. Later specs add others.
4. **`Project` skeleton:**
   ```rust
   pub struct Project {
       pub schema_version: u32,
       pub front_wheel: Option<WheelSize>,
       pub rear_wheel: Option<WheelSize>,
       pub front_travel: Option<Millimeters>, // None = rigid fork
       pub rear_travel: Option<Millimeters>,  // None = rigid rear
   }
   ```
   `Project::default()` is an empty project. Later specs add nesting such as `Specification` and `RearSuspension` when they need it.
5. **Save and load:** pretty-printed JSON. An unknown `schema_version` or an unparseable file returns a clear error, never a crash.
6. **CI:** GitHub Actions on Linux and macOS, running `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`.

## Done when

An empty project and a filled-in project both round-trip through save and load unchanged, and CI passes on Linux and macOS.

## Open questions

- None.
