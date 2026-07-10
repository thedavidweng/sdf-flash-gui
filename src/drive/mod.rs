//! Optical drive discovery, list parsing, and selection.

mod os;
mod parse;

pub use os::{enumerate_drives, find_backend, find_sdf_bin};
pub use parse::*;
