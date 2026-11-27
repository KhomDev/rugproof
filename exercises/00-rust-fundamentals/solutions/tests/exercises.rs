//! Full exercise test suite (unit, property-based, adversarial, and
//! integration). This file includes `shared/exercises.rs`, which is also run
//! against the other crate of the pair, so `starter` and `solutions` are
//! judged by the same tests.
//!
//! From the workspace root: `cargo test -p starter` or `cargo test -p solutions`.
//! From this directory: `cargo test`.
extern crate solutions as ex;
include!("../../shared/exercises.rs");
