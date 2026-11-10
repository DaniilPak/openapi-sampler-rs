// Port of openapi-sampler's test/unit/{array,number,object,string}.spec.js.
// Original tests: Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.
//
// The JavaScript tests call each sampler directly; here they go through
// `sample()` with the matching `type`, which is how the samplers are reached.

use openapi_sampler::{sample, Options};
use serde_json::{json, Value};

fn s(schema: Value) -> Value {
    sample(&schema, &Options::default(), None).unwrap()
}

fn s_opts(schema: Value, options: Options) -> Value {
    sample(&schema, &options, None).unwrap()
}

/// `type: "object"` plus the given keywords.
fn object(mut schema: Value) -> Value {
    schema["type"] = json!("object");
    schema
}

/// Samples a string schema as the value of a property called `name`.
fn string_in_property(schema: Value, name: &str, options: Options) -> Value {
    let wrapper = json!({"type": "object", "properties": {name: schema}});
    s_opts(wrapper, options)[name].clone()
}

mod sample_array {
    use super::*;

    #[test]
    fn should_return_empty_array_by_default() {
        assert_eq!(s(json!({"type": "array"})), json!([]));
    }

    #[test]
    fn should_return_elements_of_correct_type() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}})),
            json!([0])
        );
        assert_eq!(
            s(json!({"type": "array", "contains": {"type": "number"}})),
            json!([0])
        );
    }

    #[test]
    fn should_return_correct_number_of_elements_based_on_max_items() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}, "maxItems": 0})),
            json!([])
        );
    }

    #[test]
    fn should_return_correct_number_of_elements_based_on_min_items() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}, "minItems": 3})),
            json!([0, 0, 0])
        );
    }

    #[test]
    fn should_correctly_sample_tuples() {
        assert_eq!(
            s(json!({"type": "array", "items": [{"type": "number"}, {"type": "string"}, {}]})),
            json!([0, "string", null])
        );
    }

    #[test]
    fn should_correctly_sample_tuples_for_3_1() {
        assert_eq!(
            s(
                json!({"type": "array", "prefixItems": [{"type": "number"}, {"type": "string"}, {}]})
            ),
            json!([0, "string", null])
        );
    }
}

mod sample_number {
    use super::*;

    #[test]
    fn should_return_0_by_default() {
        assert_eq!(s(json!({"type": "number"})), json!(0));
    }

    #[test]
    fn should_return_0_1_for_float_type() {
        assert_eq!(s(json!({"type": "number", "format": "float"})), json!(0.1));
    }

    #[test]
    fn should_return_minimum_if_both_minimum_and_maximum_are_specified() {
        assert_eq!(s(json!({"maximum": 10, "minimum": 3})), json!(3));
    }

    #[test]
    fn should_return_exclusive_minimum_plus_1_for_draft_v7() {
        assert_eq!(s(json!({"exclusiveMinimum": 3})), json!(4));
    }

    #[test]
    fn should_return_exclusive_maximum_minus_1_for_draft_v7() {
        assert_eq!(s(json!({"exclusiveMaximum": -3})), json!(-4));
    }

    #[test]
    fn should_return_middle_point_if_boundary_integer_is_not_possible_for_draft_v7() {
        assert_eq!(
            s(json!({"exclusiveMinimum": 2, "exclusiveMaximum": 3})),
            json!(2.5)
        );
    }

    #[test]
    fn should_return_closer_to_minimum_possible_int_for_draft_v7() {
        assert_eq!(s(json!({"minimum": 2, "maximum": 3})), json!(2));
        assert_eq!(
            s(json!({"exclusiveMinimum": 8, "exclusiveMaximum": 13})),
            json!(9)
        );
    }

    #[test]
    fn should_return_a_value_above_exclusive_minimum_of_0_for_draft_v7() {
        assert_eq!(s(json!({"exclusiveMinimum": 0})), json!(1));
    }

    #[test]
    fn should_return_a_value_below_exclusive_maximum_of_0_for_draft_v7() {
        assert_eq!(s(json!({"exclusiveMaximum": 0})), json!(-1));
    }

    #[test]
    fn should_return_middle_point_if_exclusive_minimum_is_0_and_no_boundary_integer() {
        assert_eq!(
            s(json!({"exclusiveMinimum": 0, "exclusiveMaximum": 1})),
            json!(0.5)
        );
    }

    #[test]
    fn should_return_minimum_of_0_if_both_minimum_and_maximum_are_specified() {
        assert_eq!(s(json!({"minimum": 0, "maximum": 10})), json!(0));
    }

    #[test]
    fn should_respect_minimum_of_0_for_float_type() {
        assert_eq!(
            s(json!({"type": "number", "format": "float", "minimum": 0})),
            json!(0)
        );
    }

    #[test]
    fn should_respect_maximum_of_0_for_float_type() {
        assert_eq!(
            s(json!({"type": "number", "format": "float", "maximum": 0})),
            json!(0)
        );
    }

    #[test]
    fn should_return_minimum_plus_1_if_minimum_is_0_and_exclusive_minimum_for_draft_v4() {
        assert_eq!(s(json!({"minimum": 0, "exclusiveMinimum": true})), json!(1));
    }

    #[test]
    fn should_return_maximum_minus_1_if_maximum_is_0_and_exclusive_maximum_for_draft_v4() {
        assert_eq!(
            s(json!({"maximum": 0, "exclusiveMaximum": true})),
            json!(-1)
        );
    }

    #[test]
    fn should_exclude_minimum_of_0_if_maximum_is_also_specified_for_draft_v4() {
        assert_eq!(
            s(json!({"minimum": 0, "maximum": 10, "exclusiveMinimum": true})),
            json!(1)
        );
    }

    #[test]
    fn should_ignore_boundaries_that_are_not_finite_numbers() {
        // JSON has no NaN, so only the `null` cases carry over.
        assert_eq!(
            s(json!({"type": "number", "exclusiveMinimum": null})),
            json!(0)
        );
        assert_eq!(s(json!({"type": "number", "minimum": null})), json!(0));
    }
}

mod sample_object {
    use super::*;

    #[test]
    fn should_return_empty_object_by_default() {
        assert_eq!(s(json!({"type": "object"})), json!({}));
    }

    #[test]
    fn should_instantiate_all_properties() {
        assert_eq!(
            s(object(
                json!({"properties": {"a": {"type": "string"}, "b": {"type": "integer"}}})
            )),
            json!({"a": "string", "b": 0})
        );
    }

    #[test]
    fn should_skip_readonly_properties_if_skip_read_only() {
        assert_eq!(
            s_opts(
                object(json!({"properties": {
                    "a": {"type": "string"},
                    "b": {"type": "integer", "readOnly": true}
                }})),
                Options::new().skip_read_only(true)
            ),
            json!({"a": "string"})
        );
    }

    #[test]
    fn should_skip_readonly_properties_in_nested_objects_if_skip_read_only() {
        assert_eq!(
            s_opts(
                object(json!({"properties": {
                    "a": {"type": "string"},
                    "b": {"type": "object", "properties": {
                        "b1": {"type": "number", "readOnly": true},
                        "b2": {"type": "number"}
                    }}
                }})),
                Options::new().skip_read_only(true)
            ),
            json!({"a": "string", "b": {"b2": 0}})
        );
    }

    fn one_of_with(flag: &str) -> Value {
        let mut b = json!({
            "type": "object",
            "oneOf": [
                {"type": "object", "properties": {"c": {"type": "string"}}},
                {"type": "object", "properties": {"d": {"type": "string"}}}
            ]
        });
        b[flag] = json!(true);
        object(json!({"properties": {"a": {"type": "string"}, "b": b}}))
    }

    #[test]
    fn should_skip_readonly_properties_in_one_ofs_if_skip_read_only() {
        assert_eq!(
            s_opts(one_of_with("readOnly"), Options::new().skip_read_only(true)),
            json!({"a": "string"})
        );
    }

    #[test]
    fn should_skip_writeonly_properties_if_skip_write_only() {
        assert_eq!(
            s_opts(
                object(json!({"properties": {
                    "a": {"type": "string"},
                    "b": {"type": "integer", "writeOnly": true}
                }})),
                Options::new().skip_write_only(true)
            ),
            json!({"a": "string"})
        );
    }

    #[test]
    fn should_skip_writeonly_properties_in_nested_objects_if_skip_write_only() {
        assert_eq!(
            s_opts(
                object(json!({"properties": {
                    "a": {"type": "string"},
                    "b": {"type": "object", "properties": {
                        "b1": {"type": "number", "writeOnly": true},
                        "b2": {"type": "number"}
                    }}
                }})),
                Options::new().skip_write_only(true)
            ),
            json!({"a": "string", "b": {"b2": 0}})
        );
    }

    #[test]
    fn should_skip_writeonly_properties_in_one_ofs_if_skip_write_only() {
        assert_eq!(
            s_opts(
                one_of_with("writeOnly"),
                Options::new().skip_write_only(true)
            ),
            json!({"a": "string"})
        );
    }

    #[test]
    fn should_instantiate_2_additional_properties() {
        assert_eq!(
            s(object(json!({"additionalProperties": {"type": "string"}}))),
            json!({"property1": "string", "property2": "string"})
        );
    }

    #[test]
    fn should_use_custom_property_name_when_x_additional_properties_name_is_defined() {
        assert_eq!(
            s(object(json!({"additionalProperties": {
                "type": "string",
                "x-additionalPropertiesName": "attribute-name"
            }}))),
            json!({"attribute-name1": "string", "attribute-name2": "string"})
        );
    }

    #[test]
    fn should_skip_non_required_properties_if_skip_non_required() {
        assert_eq!(
            s_opts(
                object(json!({
                    "properties": {"a": {"type": "string"}, "b": {"type": "integer"}},
                    "required": ["a"]
                })),
                Options::new().skip_non_required(true)
            ),
            json!({"a": "string"})
        );
    }

    #[test]
    fn should_pass_property_name_context_to_samplers() {
        assert_eq!(
            s(object(json!({"properties": {
                "fooId": {"type": "string", "format": "uuid"},
                "barId": {"type": "string", "format": "uuid"}
            }}))),
            json!({
                "fooId": "fb4274c7-4fcd-4035-8958-a680548957ff",
                "barId": "3c966637-4898-4972-9a9d-baefa6cd6c89"
            })
        );
    }

    #[test]
    fn respects_max_properties() {
        let result = s(json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "email": {"type": "string", "format": "email"},
                "age": {"type": "integer", "minimum": 0},
                "phone": {"type": "string"}
            },
            "required": ["name", "email"],
            "maxProperties": 2
        }));
        let result = result.as_object().unwrap();
        assert!(result.len() <= 2);
        assert!(result.contains_key("name"));
        assert!(result.contains_key("email"));
    }
}

mod sample_string {
    use super::*;

    fn string(mut schema: Value) -> Value {
        schema["type"] = json!("string");
        s(schema)
    }

    fn len(value: &Value) -> usize {
        value.as_str().unwrap().chars().count()
    }

    #[test]
    fn should_return_string_by_default() {
        assert_eq!(string(json!({})), json!("string"));
    }

    #[test]
    fn should_return_string_of_appropriate_length_if_min_length_specified() {
        assert_eq!(len(&string(json!({"minLength": 20}))), 20);
    }

    #[test]
    fn should_return_string_of_appropriate_length_if_max_length_specified() {
        assert_eq!(len(&string(json!({"maxLength": 2}))), 2);
    }

    #[test]
    fn should_return_email_for_format_email() {
        assert_eq!(
            string(json!({"format": "email"})),
            json!("user@example.com")
        );
    }

    #[test]
    fn should_return_password_for_format_password() {
        assert_eq!(string(json!({"format": "password"})), json!("pa$$word"));
    }

    #[test]
    fn should_return_password_of_appropriate_length_if_min_length_specified() {
        let res = string(json!({"format": "password", "minLength": 20}));
        assert!(res.as_str().unwrap().starts_with("pa$$word_"));
        assert_eq!(len(&res), 20);
    }

    #[test]
    fn should_return_deterministic_date_string_for_format_date() {
        assert_eq!(string(json!({"format": "date"})), json!("2019-08-24"));
    }

    #[test]
    fn should_return_deterministic_date_string_for_format_date_time() {
        assert_eq!(
            string(json!({"format": "date-time"})),
            json!("2019-08-24T14:15:22Z")
        );
    }

    #[test]
    fn should_not_throw_if_incorrect_max_length_applied_to_date_time() {
        assert_eq!(
            string(json!({"format": "date-time", "maxLength": 5})),
            json!("2019-08-24T14:15:22Z")
        );
    }

    #[test]
    fn should_not_throw_if_incorrect_min_length_applied_to_date_time() {
        assert_eq!(
            string(json!({"format": "date-time", "minLength": 100})),
            json!("2019-08-24T14:15:22Z")
        );
    }

    #[test]
    fn should_return_deterministic_time_string_for_format_time() {
        assert_eq!(string(json!({"format": "time"})), json!("14:15:22Z"));
    }

    #[test]
    fn should_not_throw_if_incorrect_max_length_applied_to_time() {
        assert_eq!(
            string(json!({"format": "time", "maxLength": 5})),
            json!("14:15:22Z")
        );
    }

    #[test]
    fn should_not_throw_if_incorrect_min_length_applied_to_time() {
        assert_eq!(
            string(json!({"format": "time", "minLength": 100})),
            json!("14:15:22Z")
        );
    }

    // The JavaScript suite checks these formats with regular expressions and
    // ajv-formats; the samples are fixed, so exact values are checked here.
    #[test]
    fn should_return_fixed_values_for_formats() {
        let cases = [
            ("email", "user@example.com"),
            ("idn-email", "пошта@укр.нет"),
            ("ipv4", "192.168.0.1"),
            ("ipv6", "2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
            ("hostname", "example.com"),
            ("idn-hostname", "приклад.укр"),
            ("uri", "http://example.com"),
            ("uri-reference", "../dictionary"),
            ("uri-template", "http://example.com/{endpoint}"),
            ("iri", "http://example.com/entity/1"),
            ("iri-reference", "/entity/1"),
            ("json-pointer", "/json/pointer"),
            ("relative-json-pointer", "1/relative/json/pointer"),
            ("regex", "/regex/"),
            // Not special-cased by openapi-sampler: plain strings.
            ("byte", "string"),
            ("binary", "string"),
            ("unknown-format", "string"),
        ];
        for (format, expected) in cases {
            assert_eq!(
                string(json!({"format": format})),
                json!(expected),
                "format {format}"
            );
        }
    }

    #[test]
    fn should_return_valid_uuid_for_uuid_format_without_property_name_context() {
        assert_eq!(
            string(json!({"format": "uuid"})),
            json!("497f6eca-6276-4993-bfeb-53cbbbba6f08")
        );
    }

    #[test]
    fn should_return_valid_uuid_for_uuid_format_with_property_name_context() {
        assert_eq!(
            string_in_property(
                json!({"type": "string", "format": "uuid"}),
                "fooId",
                Options::default()
            ),
            json!("fb4274c7-4fcd-4035-8958-a680548957ff")
        );
    }

    #[test]
    fn should_generate_valid_text_for_basic_regexes() {
        for pattern in [
            "#{3}test[1-5]",
            "[500-15000]",
            "#{2,9}",
            "#{5}",
            "0x[0-9a-f]{40}",
            "^[0-9]$",
        ] {
            let res = string_in_property(
                json!({"type": "string", "pattern": pattern}),
                "fooId",
                Options::new().enable_patterns(true),
            );
            let regex = regex::Regex::new(pattern).unwrap();
            assert!(
                regex.is_match(res.as_str().unwrap()),
                "{res} does not match {pattern}"
            );
        }
    }
}
