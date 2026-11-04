//! Component data for bikestudio (S-COMPONENTS): forks, shocks, drive units
//! and batteries, their side-view drawings, and resolution of a project's
//! selections into concrete values for the solvers.
//!
//! Depends only on `bike-model` (plan section 1). The library is TOML under
//! `data/`, compiled in and checked by the tests (C2).

pub mod drawing;
mod library;
mod resolve;
pub mod validate;

pub use drawing::Drawing;
pub use library::{FILES, Library, LibraryError};
pub use resolve::resolve;
