//! Label normalization and category memory.

pub mod memory;
mod normalize;

pub use normalize::{normalize_label, renormalize_all};
