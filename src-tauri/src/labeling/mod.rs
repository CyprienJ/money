//! Label normalization, category memory and categorization rules.

pub mod memory;
mod normalize;
pub mod rules;

pub use normalize::{normalize_label, renormalize_all};
