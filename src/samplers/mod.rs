//! Type-specific samplers. Arrays and objects recurse, so they live on
//! [`crate::traverse::Sampler`]; the leaf samplers are plain functions.

mod number;
mod string;
mod string_regex;

pub(crate) use number::sample_number;
pub(crate) use string::sample_string;
