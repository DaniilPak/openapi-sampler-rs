//! Behaviour not covered by openapi-sampler's own suite, checked against what
//! its JavaScript source does.

use openapi_sampler::{infer_type, sample, Error, Options};
use serde_json::{json, Value};

fn s(schema: Value) -> Value {
    sample(&schema, &Options::default(), None).unwrap()
}

#[test]
fn ref_without_spec_is_an_error() {
    let err = sample(
        &json!({"$ref": "#/definitions/A"}),
        &Options::default(),
        None,
    )
    .unwrap_err();
    assert_eq!(err, Error::MissingSpec);
}

#[test]
fn unresolvable_refs_are_errors() {
    let spec = json!({"definitions": {}});
    for reference in ["#/definitions/Missing", "other.yaml#/Foo", "#/a%zz"] {
        let err = sample(
            &json!({"$ref": reference}),
            &Options::default(),
            Some(&spec),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::InvalidRef { .. }),
            "{reference}: {err}"
        );
    }
}

#[test]
fn ref_pointers_are_unescaped() {
    let spec = json!({"paths": {"/pets/{id}": {"x": {"type": "integer", "example": 42}}}});
    let schema = json!({"$ref": "#/paths/~1pets~1%7Bid%7D/x"});
    assert_eq!(
        sample(&schema, &Options::default(), Some(&spec)).unwrap(),
        json!(42)
    );
}

#[test]
fn self_referencing_property_stops_after_one_level() {
    let spec = json!({
        "components": {"schemas": {"Tree": {
            "type": "object",
            "properties": {
                "value": {"type": "integer"},
                "children": {"type": "array", "items": {"$ref": "#/components/schemas/Tree"}}
            }
        }}}
    });
    assert_eq!(
        sample(
            &json!({"$ref": "#/components/schemas/Tree"}),
            &Options::default(),
            Some(&spec)
        )
        .unwrap(),
        json!({"value": 0, "children": [{}]})
    );
}

#[test]
fn max_sample_depth_cuts_off_nesting() {
    // Five nested objects; properties start at depth 2.
    let mut schema = json!({"type": "string"});
    for _ in 0..5 {
        schema = json!({"type": "object", "properties": {"n": schema}});
    }
    assert_eq!(
        sample(&schema, &Options::new().max_sample_depth(3), None).unwrap(),
        json!({"n": {"n": {"n": {}}}})
    );
    assert_eq!(
        s(schema),
        json!({"n": {"n": {"n": {"n": {"n": "string"}}}}})
    );
}

#[test]
fn patterns_are_ignored_unless_enabled() {
    let schema = json!({"type": "string", "pattern": "^[A-Z]{3}$"});
    assert_eq!(s(schema.clone()), json!("string"));
    assert_eq!(
        sample(&schema, &Options::new().enable_patterns(true), None).unwrap(),
        json!("AAA")
    );
}

#[test]
fn bad_patterns_are_errors() {
    let schema = json!({"type": "string", "pattern": "[z-a]"});
    let err = sample(&schema, &Options::new().enable_patterns(true), None).unwrap_err();
    assert!(matches!(err, Error::InvalidPattern(_)));
}

#[test]
fn huge_min_length_is_an_error_like_in_javascript() {
    let err = sample(
        &json!({"type": "string", "minLength": 1e12}),
        &Options::default(),
        None,
    )
    .unwrap_err();
    assert_eq!(err, Error::TooLong);
}

#[test]
fn min_and_max_length() {
    assert_eq!(
        s(json!({"type": "string", "minLength": 8})),
        json!("stringst")
    );
    assert_eq!(s(json!({"type": "string", "maxLength": 3})), json!("str"));
    // `maxLength: 0` is falsy in JavaScript and therefore ignored.
    assert_eq!(
        s(json!({"type": "string", "maxLength": 0})),
        json!("string")
    );
}

#[test]
fn nullable_is_not_special() {
    assert_eq!(
        s(json!({"type": "string", "nullable": true})),
        json!("string")
    );
    assert_eq!(s(json!({"type": ["null", "string"]})), json!(null));
}

#[test]
fn example_wins_over_everything() {
    assert_eq!(
        s(json!({"example": 1, "const": 2, "default": 3, "allOf": [{"type": "string"}]})),
        json!(1)
    );
    assert_eq!(s(json!({"type": "string", "example": null})), json!(null));
}

#[test]
fn boolean_and_empty_schemas_sample_as_null() {
    assert_eq!(s(json!({"type": "array", "items": true})), json!([null]));
    assert_eq!(
        s(json!({"type": "object", "additionalProperties": {}})),
        json!({"property1": null, "property2": null})
    );
    // `additionalProperties: true` is not an object, so nothing is generated.
    assert_eq!(
        s(json!({"type": "object", "additionalProperties": true})),
        json!({})
    );
}

#[test]
fn property_order_is_kept() {
    let value = s(json!({"type": "object", "properties": {
        "z": {"type": "string"}, "a": {"type": "string"}, "m": {"type": "string"}
    }}));
    let keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["z", "a", "m"]);
}

#[test]
fn skip_read_only_through_all_of_refs() {
    let spec = json!({
        "openapi": "3.0.0",
        "components": {"schemas": {
            "Base": {"type": "object", "properties": {
                "id": {"type": "integer", "readOnly": true},
                "secret": {"type": "string", "writeOnly": true}
            }},
            "User": {"allOf": [
                {"$ref": "#/components/schemas/Base"},
                {"type": "object", "properties": {"name": {"type": "string"}}}
            ]}
        }}
    });
    let schema = json!({"$ref": "#/components/schemas/User"});
    assert_eq!(
        sample(&schema, &Options::new().skip_read_only(true), Some(&spec)).unwrap(),
        json!({"secret": "string", "name": "string"})
    );
    assert_eq!(
        sample(&schema, &Options::new().skip_write_only(true), Some(&spec)).unwrap(),
        json!({"id": 0, "name": "string"})
    );
}

#[test]
fn infer_type_matches_javascript() {
    assert_eq!(infer_type(&json!({"type": "integer"})), Some("integer"));
    assert_eq!(infer_type(&json!({"type": []})), None);
    assert_eq!(infer_type(&json!({"maximum": 1})), Some("number"));
    assert_eq!(infer_type(&json!({"pattern": "x"})), Some("string"));
    assert_eq!(infer_type(&json!({"properties": {}})), Some("object"));
    assert_eq!(infer_type(&json!({"prefixItems": []})), None);
}

#[test]
fn swagger_2_document() {
    let spec = json!({
        "swagger": "2.0",
        "definitions": {
            "Order": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "format": "int64"},
                    "shipDate": {"type": "string", "format": "date-time"},
                    "status": {"type": "string", "enum": ["placed", "approved", "delivered"]},
                    "complete": {"type": "boolean", "default": false},
                    "attachment": {"type": "file"}
                }
            }
        }
    });
    assert_eq!(
        sample(
            &json!({"$ref": "#/definitions/Order"}),
            &Options::default(),
            Some(&spec)
        )
        .unwrap(),
        json!({
            "id": 0,
            "shipDate": "2019-08-24T14:15:22Z",
            "status": "placed",
            "complete": false,
            "attachment": null
        })
    );
}
