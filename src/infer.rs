// Ported from openapi-sampler (src/infer.js),
// Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.

use serde_json::Value;

/// Keywords that imply a type when `type` is missing, checked in this order.
const SCHEMA_KEYWORD_TYPES: [(&str, &str); 20] = [
    ("multipleOf", "number"),
    ("maximum", "number"),
    ("exclusiveMaximum", "number"),
    ("minimum", "number"),
    ("exclusiveMinimum", "number"),
    ("maxLength", "string"),
    ("minLength", "string"),
    ("pattern", "string"),
    ("items", "array"),
    ("maxItems", "array"),
    ("minItems", "array"),
    ("uniqueItems", "array"),
    ("additionalItems", "array"),
    ("maxProperties", "object"),
    ("minProperties", "object"),
    ("required", "object"),
    ("additionalProperties", "object"),
    ("properties", "object"),
    ("patternProperties", "object"),
    ("dependencies", "object"),
];

/// openapi-sampler's `inferType`, returning the raw value it would return
/// (`null` when nothing is inferred).
pub(crate) fn infer_type_value(schema: &Value) -> Value {
    if let Some(ty) = schema.get("type") {
        return match ty {
            Value::Array(types) => types.first().cloned().unwrap_or(Value::Null),
            other => other.clone(),
        };
    }
    SCHEMA_KEYWORD_TYPES
        .iter()
        .find(|(keyword, _)| schema.get(*keyword).is_some())
        .map_or(Value::Null, |(_, ty)| Value::String((*ty).to_owned()))
}

/// The type openapi-sampler samples `schema` as: its `type` (the first one when
/// `type` is a list), or the type implied by its keywords (`minimum` → `"number"`,
/// `properties` → `"object"`, …).
///
/// ```
/// use serde_json::json;
/// use openapi_sampler::infer_type;
///
/// assert_eq!(infer_type(&json!({"type": ["string", "null"]})), Some("string"));
/// assert_eq!(infer_type(&json!({"minItems": 1})), Some("array"));
/// assert_eq!(infer_type(&json!({})), None);
/// ```
pub fn infer_type(schema: &Value) -> Option<&str> {
    if let Some(ty) = schema.get("type") {
        return match ty {
            Value::Array(types) => types.first().and_then(Value::as_str),
            other => other.as_str(),
        };
    }
    SCHEMA_KEYWORD_TYPES
        .iter()
        .find(|(keyword, _)| schema.get(*keyword).is_some())
        .map(|(_, ty)| *ty)
}
