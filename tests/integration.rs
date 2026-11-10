// Port of openapi-sampler's test/integration.spec.js (all non-XML cases).
// Original tests: Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.

use openapi_sampler::{sample, Options};
use serde_json::{json, Value};

fn s(schema: Value) -> Value {
    sample(&schema, &Options::default(), None).unwrap()
}

fn s_opts(schema: Value, options: Options) -> Value {
    sample(&schema, &options, None).unwrap()
}

fn s_spec(schema: Value, options: Options, spec: &Value) -> Value {
    sample(&schema, &options, Some(spec)).unwrap()
}

mod primitives {
    use super::*;

    #[test]
    fn should_sample_string() {
        assert_eq!(s(json!({"type": "string"})), json!("string"));
    }

    #[test]
    fn should_sample_number() {
        assert_eq!(s(json!({"type": "number"})), json!(0));
    }

    #[test]
    fn should_sample_number_with_format_float() {
        assert_eq!(s(json!({"type": "number", "format": "float"})), json!(0.1));
    }

    #[test]
    fn should_sample_number_with_format_double() {
        assert_eq!(s(json!({"type": "number", "format": "double"})), json!(0.1));
    }

    #[test]
    fn should_sample_boolean() {
        assert_eq!(s(json!({"type": "boolean"})), json!(true));
    }

    #[test]
    fn should_use_default_property() {
        assert_eq!(s(json!({"type": "number", "default": 100})), json!(100));
    }

    #[test]
    fn should_support_type_array() {
        assert_eq!(s(json!({"type": ["string", "number"]})), json!("string"));
    }

    #[test]
    fn should_use_null_for_null() {
        assert_eq!(s(json!({"type": "null"})), json!(null));
    }

    #[test]
    fn should_use_null_if_type_is_not_specified() {
        assert_eq!(s(json!({})), json!(null));
    }

    #[test]
    fn should_use_null_if_type_array_is_empty() {
        assert_eq!(s(json!({"type": []})), json!(null));
    }
}

mod objects {
    use super::*;

    #[test]
    fn should_sample_object_without_properties() {
        assert_eq!(s(json!({"type": "object"})), json!({}));
    }

    #[test]
    fn should_sample_object_with_property() {
        assert_eq!(
            s(json!({"type": "object", "properties": {"title": {"type": "string"}}})),
            json!({"title": "string"})
        );
    }

    #[test]
    fn should_sample_object_with_property_with_default_value() {
        assert_eq!(
            s(json!({
                "type": "object",
                "properties": {"title": {"type": "string", "default": "Example"}}
            })),
            json!({"title": "Example"})
        );
    }

    #[test]
    fn should_sample_object_with_more_than_one_property() {
        assert_eq!(
            s(json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string", "default": "Example"},
                    "amount": {"type": "number", "default": 10}
                }
            })),
            json!({"title": "Example", "amount": 10})
        );
    }

    #[test]
    fn should_sample_both_properties_and_additional_properties() {
        assert_eq!(
            s(json!({
                "type": "object",
                "properties": {"test": {"type": "string"}},
                "additionalProperties": {"type": "number"}
            })),
            json!({"test": "string", "property1": 0, "property2": 0})
        );
    }
}

mod arrays {
    use super::*;

    #[test]
    fn should_sample_array() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}})),
            json!([0])
        );
    }

    #[test]
    fn should_sample_array_with_examples() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}, "examples": [[1, 2, 3]]})),
            json!([1, 2, 3])
        );
    }

    #[test]
    fn should_use_default_property() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}, "default": [5, 6, 7]})),
            json!([5, 6, 7])
        );
    }

    #[test]
    fn should_sample_array_with_min_items() {
        assert_eq!(
            s(json!({"type": "array", "items": {"type": "number"}, "minItems": 3})),
            json!([0, 0, 0])
        );
    }
}

mod all_of {
    use super::*;

    #[test]
    fn should_sample_schema_with_all_of() {
        assert_eq!(
            s(json!({
                "allOf": [
                    {"type": "object", "properties": {"title": {"type": "string"}}},
                    {"type": "object", "properties": {"amount": {"type": "number", "default": 1}}}
                ]
            })),
            json!({"title": "string", "amount": 1})
        );
    }

    #[test]
    fn should_not_throw_for_schemas_with_all_of_with_different_types() {
        assert_eq!(
            s(json!({
                "allOf": [
                    {"type": "string"},
                    {"type": "object", "properties": {"amount": {"type": "number", "default": 1}}}
                ]
            })),
            json!({"amount": 1})
        );
    }

    #[test]
    fn deep_array() {
        assert_eq!(
            s(json!({
                "allOf": [
                    {
                        "type": "object",
                        "properties": {"arr": {"type": "array", "items": {"type": "object"}}}
                    },
                    {
                        "type": "object",
                        "properties": {
                            "arr": {
                                "type": "array",
                                "items": {"type": "object", "properties": {"name": {"type": "string"}}}
                            }
                        }
                    }
                ]
            })),
            json!({"arr": [{"name": "string"}]})
        );
    }

    #[test]
    fn should_not_be_confused_by_subschema_without_type() {
        assert_eq!(
            s(json!({"type": "string", "allOf": [{"description": "test"}]})),
            json!("string")
        );
    }

    #[test]
    fn should_not_throw_for_array_all_of() {
        let result = s(json!({
            "type": "array",
            "allOf": [{"type": "array", "items": {"type": "string"}}]
        }));
        assert!(result.is_array());
    }

    #[test]
    fn should_sample_schema_with_all_of_even_if_some_type_is_not_specified() {
        assert_eq!(
            s(json!({
                "properties": {"title": {"type": "string"}},
                "allOf": [
                    {"type": "object", "properties": {"amount": {"type": "number", "default": 1}}}
                ]
            })),
            json!({"title": "string", "amount": 1})
        );
        assert_eq!(
            s(json!({
                "type": "object",
                "properties": {"title": {"type": "string"}},
                "allOf": [{"properties": {"amount": {"type": "number", "default": 1}}}]
            })),
            json!({"title": "string", "amount": 1})
        );
    }

    #[test]
    fn should_merge_deep_properties() {
        assert_eq!(
            s(json!({
                "type": "object",
                "allOf": [
                    {
                        "type": "object",
                        "properties": {
                            "parent": {"type": "object", "properties": {"child1": {"type": "string"}}}
                        }
                    },
                    {
                        "type": "object",
                        "properties": {
                            "parent": {"type": "object", "properties": {"child2": {"type": "number"}}}
                        }
                    }
                ]
            })),
            json!({"parent": {"child1": "string", "child2": 0}})
        );
    }

    #[test]
    fn should_respect_read_only_from_schemas_in_all_of() {
        assert_eq!(
            s_opts(
                json!({
                    "allOf": [
                        {
                            "type": "object",
                            "properties": {"a": {"type": "string"}, "b": {"type": "integer"}}
                        },
                        {"properties": {"b": {"readOnly": true}}}
                    ]
                }),
                Options::new().skip_read_only(true)
            ),
            json!({"a": "string"})
        );
    }
}

mod inheritance {
    use super::*;

    #[test]
    fn should_support_basic_all_of() {
        assert_eq!(
            s(json!({"allOf": [{"type": "object", "properties": {"test": {"type": "string"}}}]})),
            json!({"test": "string"})
        );
    }

    #[test]
    fn should_merge_properties_in_all_of() {
        assert_eq!(
            s(json!({
                "allOf": [
                    {"type": "object", "properties": {"test1": {"type": "string"}}},
                    {"type": "object", "properties": {"test2": {"type": "number"}}}
                ]
            })),
            json!({"test1": "string", "test2": 0})
        );
    }
}

mod example {
    use super::*;

    #[test]
    fn should_use_example() {
        let obj = json!({"test": "test", "properties": {"test": {"type": "string"}}});
        assert_eq!(s(json!({"type": "object", "example": obj})), obj);
    }

    #[test]
    fn should_use_falsy_example() {
        assert_eq!(s(json!({"type": "string", "example": false})), json!(false));
    }

    #[test]
    fn should_use_enum() {
        assert_eq!(
            s(json!({"type": "string", "enum": ["test1", "test2"]})),
            json!("test1")
        );
    }
}

mod detection {
    use super::*;

    #[test]
    fn should_detect_autodetect_types_based_on_props() {
        assert_eq!(
            s(json!({"properties": {"a": {"minimum": 10}, "b": {"minLength": 1}}})),
            json!({"a": 10, "b": "string"})
        );
    }
}

mod compound_keywords {
    use super::*;

    #[test]
    fn should_support_basic_if_then_else_usage() {
        assert_eq!(
            s(json!({
                "type": "object",
                "properties": {"top": {"type": "number"}},
                "if": {"properties": {"foo": {"type": "string", "format": "email"}}},
                "then": {"properties": {"bar": {"type": "string"}}},
                "else": {"properties": {"baz": {"type": "number"}}}
            })),
            json!({"foo": "user@example.com", "bar": "string", "top": 0})
        );
    }

    mod one_of_and_any_of {
        use super::*;

        #[test]
        fn should_support_one_of() {
            assert_eq!(
                s(json!({"oneOf": [{"type": "string"}, {"type": "number"}]})),
                json!("string")
            );
        }

        #[test]
        fn should_support_any_of() {
            assert_eq!(
                s(json!({"anyOf": [{"type": "string"}, {"type": "number"}]})),
                json!("string")
            );
        }

        #[test]
        fn should_prefer_one_of_if_any_of_and_one_of_are_on_the_same_level() {
            assert_eq!(
                s_opts(
                    json!({"anyOf": [{"type": "string"}], "oneOf": [{"type": "number"}]}),
                    Options::new().quiet(true)
                ),
                json!(0)
            );
        }

        #[test]
        fn should_work_with_nested_circular_one_of() {
            let spec = json!({
                "definitions": {
                    "A": {
                        "properties": {"a": {"type": "string"}},
                        "oneOf": [{"$ref": "#/definitions/A"}]
                    }
                }
            });
            assert_eq!(
                s_spec(
                    json!({"$ref": "#/definitions/A"}),
                    Options::default(),
                    &spec
                ),
                json!({"a": "string"})
            );
        }

        #[test]
        fn should_work_with_nested_circular_any_of() {
            let spec = json!({
                "definitions": {
                    "A": {
                        "properties": {"a": {"type": "string"}},
                        "anyOf": [{"$ref": "#/definitions/A"}]
                    }
                }
            });
            assert_eq!(
                s_spec(
                    json!({"$ref": "#/definitions/A"}),
                    Options::default(),
                    &spec
                ),
                json!({"a": "string"})
            );
        }
    }

    mod inferring_type_from_root_schema {
        use super::*;

        fn basic_schema(extra: Value) -> Value {
            let mut schema = json!({
                "oneOf": [{"type": "string"}, {"type": "number"}],
                "anyOf": [{"type": "string"}, {"type": "number"}],
                "allOf": [
                    {"type": "object", "properties": {"title": {"type": "string"}}},
                    {"type": "object", "properties": {"amount": {"type": "number", "default": 1}}}
                ],
                "if": {"properties": {"foo": {"type": "string", "format": "email"}}},
                "then": {"properties": {"bar": {"type": "string"}}},
                "else": {"properties": {"baz": {"type": "number"}}}
            });
            for (k, v) in extra.as_object().unwrap() {
                schema[k] = v.clone();
            }
            schema
        }

        #[test]
        fn const_keyword() {
            assert_eq!(s(basic_schema(json!({"const": "foobar"}))), json!("foobar"));
        }

        #[test]
        fn examples_keyword() {
            assert_eq!(
                s(basic_schema(json!({"examples": ["foobar"]}))),
                json!("foobar")
            );
        }

        #[test]
        fn default_keyword() {
            assert_eq!(
                s(basic_schema(json!({"default": "foobar"}))),
                json!("foobar")
            );
        }

        #[test]
        fn enum_keyword() {
            assert_eq!(
                s(basic_schema(json!({"enum": ["foobar"]}))),
                json!("foobar")
            );
        }

        #[test]
        fn const_has_priority_over_examples() {
            assert_eq!(
                s(basic_schema(
                    json!({"const": "foobar", "examples": ["barfoo"]})
                )),
                json!("foobar")
            );
        }

        #[test]
        fn examples_have_priority_over_enum() {
            assert_eq!(
                s(basic_schema(
                    json!({"enum": ["barfoo"], "examples": ["foobar"]})
                )),
                json!("foobar")
            );
        }

        #[test]
        fn enum_has_priority_over_default() {
            assert_eq!(
                s(basic_schema(
                    json!({"default": "barfoo", "enum": ["foobar"]})
                )),
                json!("foobar")
            );
        }
    }
}

mod refs {
    use super::*;

    #[test]
    fn should_follow_ref() {
        let spec = json!({
            "definitions": {
                "Pet": {"type": "object", "properties": {"name": {"type": "string"}}}
            }
        });
        assert_eq!(
            s_spec(
                json!({"$ref": "#/definitions/Pet"}),
                Options::default(),
                &spec
            ),
            json!({"name": "string"})
        );
    }

    mod keywords_that_are_siblings_of_ref {
        use super::*;

        fn ref_siblings_schema() -> Value {
            json!({
                "type": "object",
                "properties": {
                    "example": {"$ref": "#/components/schemas/Num", "example": 10},
                    "examples": {"$ref": "#/components/schemas/Num", "examples": [55]},
                    "minimum": {"$ref": "#/components/schemas/Num", "minimum": 51},
                    "override": {"$ref": "#/components/schemas/NumWithExample", "example": 99},
                    "plain": {"$ref": "#/components/schemas/Num"},
                    "extended": {
                        "$ref": "#/components/schemas/Pet",
                        "properties": {"age": {"type": "integer"}}
                    }
                }
            })
        }

        fn ref_siblings_spec(version: Value) -> Value {
            let mut spec = json!({
                "components": {
                    "schemas": {
                        "Num": {"type": "number"},
                        "NumWithExample": {"type": "number", "example": 7},
                        "Pet": {"type": "object", "properties": {"name": {"type": "string"}}}
                    }
                }
            });
            for (k, v) in version.as_object().unwrap() {
                spec[k] = v.clone();
            }
            spec
        }

        #[test]
        fn should_ignore_them_in_swagger_2_and_openapi_3_0() {
            for spec in [
                ref_siblings_spec(json!({"swagger": "2.0"})),
                ref_siblings_spec(json!({"openapi": "3.0.3"})),
            ] {
                assert_eq!(
                    s_spec(ref_siblings_schema(), Options::default(), &spec),
                    json!({
                        "example": 0,
                        "examples": 0,
                        "minimum": 0,
                        "override": 7,
                        "plain": 0,
                        "extended": {"name": "string"}
                    })
                );
            }
        }

        #[test]
        fn should_keep_required_names_of_both_the_referenced_schema_and_its_siblings() {
            let spec = json!({
                "openapi": "3.1.0",
                "components": {
                    "schemas": {
                        "Base": {
                            "type": "object",
                            "required": ["a", "b"],
                            "properties": {
                                "a": {"type": "string"},
                                "b": {"type": "string"},
                                "c": {"type": "string"}
                            }
                        }
                    }
                }
            });
            assert_eq!(
                s_spec(
                    json!({"$ref": "#/components/schemas/Base", "required": ["c"]}),
                    Options::new().skip_non_required(true),
                    &spec
                ),
                json!({"a": "string", "b": "string", "c": "string"})
            );
        }

        #[test]
        fn should_sample_a_circular_ref_that_has_siblings() {
            let spec = json!({
                "openapi": "3.1.0",
                "components": {
                    "schemas": {
                        "Node": {
                            "type": "object",
                            "properties": {
                                "child": {"$ref": "#/components/schemas/Node", "description": "next"}
                            }
                        }
                    }
                }
            });
            assert_eq!(
                s_spec(
                    json!({"$ref": "#/components/schemas/Node"}),
                    Options::default(),
                    &spec
                ),
                json!({"child": {}})
            );
        }

        #[test]
        fn should_apply_them_in_openapi_3_1_and_when_no_version_is_given() {
            for spec in [
                ref_siblings_spec(json!({"openapi": "3.1.0"})),
                ref_siblings_spec(json!({"openapi": "3.2.0"})),
                ref_siblings_spec(json!({})),
            ] {
                let before = spec.clone();
                assert_eq!(
                    s_spec(ref_siblings_schema(), Options::default(), &spec),
                    json!({
                        "example": 10,
                        "examples": 55,
                        "minimum": 51,
                        "override": 99,
                        "plain": 0,
                        "extended": {"name": "string", "age": 0}
                    })
                );
                assert_eq!(spec, before);
                assert_eq!(
                    spec["components"]["schemas"]["Num"],
                    json!({"type": "number"})
                );
            }
        }

        #[test]
        fn should_apply_a_null_sibling_written_in_the_document() {
            assert_eq!(
                s_spec(
                    json!({"$ref": "#/components/schemas/Num", "default": null}),
                    Options::default(),
                    &ref_siblings_spec(json!({"openapi": "3.1.0"}))
                ),
                json!(null)
            );
        }

        #[test]
        fn should_not_apply_the_type_that_all_of_passes_to_its_items_to_a_ref() {
            for spec in [
                ref_siblings_spec(json!({"openapi": "3.1.0"})),
                ref_siblings_spec(json!({})),
            ] {
                assert_eq!(
                    s_spec(
                        json!({"allOf": [{"$ref": "#/components/schemas/Pet"}]}),
                        Options::default(),
                        &spec
                    ),
                    json!({"name": "string"})
                );
                assert_eq!(
                    s_spec(
                        json!({
                            "allOf": [
                                {"$ref": "#/components/schemas/Pet"},
                                {"properties": {"age": {"type": "integer"}}}
                            ]
                        }),
                        Options::default(),
                        &spec
                    ),
                    json!({"name": "string", "age": 0})
                );
            }
        }

        #[test]
        fn should_sample_a_recursive_ref_inside_all_of_as_an_empty_object() {
            for version in ["3.0.0", "3.1.0"] {
                let spec = json!({
                    "openapi": version,
                    "components": {
                        "schemas": {
                            "Node": {
                                "type": "object",
                                "properties": {
                                    "name": {"type": "string"},
                                    "child": {"allOf": [{"$ref": "#/components/schemas/Node"}]}
                                }
                            }
                        }
                    }
                });
                let node = &spec["components"]["schemas"]["Node"];
                assert_eq!(
                    sample(node, &Options::default(), Some(&spec)).unwrap(),
                    json!({"name": "string", "child": {"name": "string", "child": {}}})
                );
            }
        }

        #[test]
        fn should_keep_read_only_and_write_only_of_a_ref_in_one_of_and_any_of() {
            for version in [json!({"openapi": "3.1.0"}), json!({})] {
                let mut spec = json!({
                    "components": {
                        "schemas": {
                            "Id": {"type": "string", "readOnly": true},
                            "Password": {"type": "string", "writeOnly": true}
                        }
                    }
                });
                for (k, v) in version.as_object().unwrap() {
                    spec[k] = v.clone();
                }
                let schema = json!({
                    "type": "object",
                    "properties": {
                        "id": {"oneOf": [{"$ref": "#/components/schemas/Id"}]},
                        "password": {"anyOf": [{"$ref": "#/components/schemas/Password"}]},
                        "name": {"type": "string"}
                    }
                });
                assert_eq!(
                    s_spec(schema.clone(), Options::new().skip_read_only(true), &spec),
                    json!({"password": "string", "name": "string"})
                );
                assert_eq!(
                    s_spec(schema, Options::new().skip_write_only(true), &spec),
                    json!({"id": "string", "name": "string"})
                );
            }
        }
    }
}

mod discriminator {
    use super::*;

    #[test]
    fn should_support_discriminator() {
        assert_eq!(
            s(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "required": ["pet_type"],
                        "properties": {
                            "pet_type": {"type": "string", "enum": ["Cat"]},
                            "name": {"type": "string"},
                            "huntingSkill": {"type": "string"}
                        }
                    },
                    {
                        "type": "object",
                        "required": ["pet_type"],
                        "properties": {
                            "pet_type": {"type": "string", "enum": ["Dog"]},
                            "name": {"type": "string"},
                            "packSize": {"type": "number"}
                        }
                    }
                ],
                "discriminator": {"propertyName": "pet_type"}
            })),
            json!({"pet_type": "Cat", "name": "string", "huntingSkill": "string"})
        );
    }
}
