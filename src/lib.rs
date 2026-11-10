//! Generate deterministic example values from OpenAPI 3.x, Swagger 2.0 and
//! JSON Schema schemas.
//!
//! This is an independent Rust port of Redocly's
//! [openapi-sampler](https://github.com/Redocly/openapi-sampler) (MIT). It is
//! not affiliated with or endorsed by Redocly. The goal is to produce the same
//! samples the JavaScript library produces for the same input.
//!
//! ```
//! use openapi_sampler::{sample, Options};
//! use serde_json::json;
//!
//! let spec = json!({
//!     "openapi": "3.1.0",
//!     "components": { "schemas": {
//!         "Pet": {
//!             "type": "object",
//!             "required": ["name"],
//!             "properties": {
//!                 "id": { "type": "string", "format": "uuid", "readOnly": true },
//!                 "name": { "type": "string", "example": "Rex" },
//!                 "tags": { "type": "array", "items": { "type": "string" } },
//!                 "born": { "type": "string", "format": "date" }
//!             }
//!         }
//!     }}
//! });
//!
//! let schema = json!({ "$ref": "#/components/schemas/Pet" });
//! let value = sample(&schema, &Options::default(), Some(&spec)).unwrap();
//! assert_eq!(value, json!({
//!     "id": "497f6eca-6276-4993-bfeb-53cbbbba6f08",
//!     "name": "Rex",
//!     "tags": ["string"],
//!     "born": "2019-08-24"
//! }));
//!
//! // A request body: leave out what the server fills in.
//! let request = sample(&schema, &Options::new().skip_read_only(true), Some(&spec)).unwrap();
//! assert_eq!(request, json!({ "name": "Rex", "tags": ["string"], "born": "2019-08-24" }));
//! ```
//!
//! Warnings (for example `allOf` children of different types) are emitted with
//! the [`log`](https://docs.rs/log) crate unless [`Options::quiet`] is set.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod infer;
mod samplers;
mod traverse;
mod utils;
mod value;

use std::fmt;

use serde_json::Value;

pub use infer::infer_type;

/// Sampling options. Mirrors openapi-sampler's `options` object (except
/// `format: "xml"`, which this crate does not implement).
///
/// ```
/// let options = openapi_sampler::Options::new()
///     .skip_read_only(true)
///     .max_sample_depth(5);
/// assert!(options.skip_read_only);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    /// Leave out properties marked `readOnly` (`skipReadOnly`). Default `false`.
    pub skip_read_only: bool,
    /// Leave out properties marked `writeOnly` (`skipWriteOnly`). Default `false`.
    pub skip_write_only: bool,
    /// Leave out properties that are not `required` (`skipNonRequired`). Default `false`.
    pub skip_non_required: bool,
    /// How deep nested objects and arrays are sampled before they are cut off
    /// as `{}` / `[]` (`maxSampleDepth`). Default `15`.
    pub max_sample_depth: u32,
    /// Do not log warnings (`quiet`). Default `false`.
    pub quiet: bool,
    /// Generate strings that match `pattern` (`enablePatterns`). Default `false`,
    /// in which case `pattern` is ignored.
    pub enable_patterns: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            skip_read_only: false,
            skip_write_only: false,
            skip_non_required: false,
            max_sample_depth: 15,
            quiet: false,
            enable_patterns: false,
        }
    }
}

impl Options {
    /// The default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [`Options::skip_read_only`].
    pub fn skip_read_only(mut self, value: bool) -> Self {
        self.skip_read_only = value;
        self
    }

    /// Sets [`Options::skip_write_only`].
    pub fn skip_write_only(mut self, value: bool) -> Self {
        self.skip_write_only = value;
        self
    }

    /// Sets [`Options::skip_non_required`].
    pub fn skip_non_required(mut self, value: bool) -> Self {
        self.skip_non_required = value;
        self
    }

    /// Sets [`Options::max_sample_depth`].
    pub fn max_sample_depth(mut self, value: u32) -> Self {
        self.max_sample_depth = value;
        self
    }

    /// Sets [`Options::quiet`].
    pub fn quiet(mut self, value: bool) -> Self {
        self.quiet = value;
        self
    }

    /// Sets [`Options::enable_patterns`].
    pub fn enable_patterns(mut self, value: bool) -> Self {
        self.enable_patterns = value;
        self
    }
}

/// Why a sample could not be generated. openapi-sampler throws in the same cases.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The schema has a `$ref` but no spec was given to resolve it against.
    MissingSpec,
    /// A `$ref` is not a local JSON pointer, or points at nothing.
    InvalidRef {
        /// The `$ref` as written.
        reference: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A `pattern` could not be sampled (only with [`Options::enable_patterns`]).
    InvalidPattern(String),
    /// A string longer than JavaScript allows (about 2^29 characters) was asked for,
    /// for example with a huge `minLength`.
    TooLong,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingSpec => f.write_str(
                "schema contains $ref; the full specification must be passed to resolve it",
            ),
            Error::InvalidRef { reference, reason } => {
                write!(f, "cannot resolve $ref {reference:?}: {reason}")
            }
            Error::InvalidPattern(reason) => write!(f, "cannot sample pattern: {reason}"),
            Error::TooLong => f.write_str("sample string would be too long"),
        }
    }
}

impl std::error::Error for Error {}

/// Generates an example value for `schema`.
///
/// `spec` is the whole document `$ref`s are resolved against (only local
/// references such as `#/components/schemas/Pet` are supported). It may be
/// `None` when the schema has no `$ref`.
///
/// Values come from `example`, then `const`, `examples[0]`, `enum[0]` and
/// `default`; otherwise one is made up from the type, format and constraints.
/// The result is deterministic.
///
/// ```
/// use openapi_sampler::{sample, Options};
/// use serde_json::json;
///
/// let schema = json!({ "type": "string", "format": "email" });
/// assert_eq!(sample(&schema, &Options::default(), None).unwrap(), json!("user@example.com"));
/// ```
pub fn sample(schema: &Value, options: &Options, spec: Option<&Value>) -> Result<Value, Error> {
    let mut sampler = traverse::Sampler::new(options, spec);
    let result = sampler.traverse(schema, None)?;
    Ok(result.value.into_json().unwrap_or(Value::Null))
}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
