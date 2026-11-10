// Ported from openapi-sampler (src/traverse.js, src/allOf.js, src/samplers/array.js,
// src/samplers/object.js and parts of src/utils.js),
// Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.

use std::borrow::Cow;
use std::collections::HashMap;

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::infer::infer_type_value;
use crate::samplers::{sample_number, sample_string};
use crate::utils::{finite, js_to_string, percent_decode, resolve_pointer, truthy};
use crate::value::{merge_deep, merge_values, V};
use crate::{Error, Options};

/// What openapi-sampler passes down as `context`. Whether a context exists at
/// all matters too (it turns on JS-identity cycle detection), hence
/// `Option<&Ctx>` everywhere.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ctx {
    depth: Option<u32>,
    property_name: Option<String>,
    is_all_of_child: bool,
}

impl Ctx {
    fn child(depth: u32, property_name: Option<&str>) -> Ctx {
        Ctx {
            depth: Some(depth.saturating_add(1)),
            property_name: property_name.map(str::to_owned),
            is_all_of_child: false,
        }
    }
}

/// `context && context.depth || 1`
fn depth_of(ctx: Option<&Ctx>) -> u32 {
    ctx.and_then(|c| c.depth).filter(|&d| d != 0).unwrap_or(1)
}

/// The `{ value, readOnly, writeOnly, type }` that `traverse` returns.
#[derive(Debug, Clone)]
pub(crate) struct Res {
    pub(crate) value: V,
    read_only: bool,
    write_only: bool,
    /// `None` is `undefined`; `Some(Value::Null)` is `null`.
    ty: Option<Value>,
}

impl Res {
    /// `getResultForCircular`
    fn circular(ty: &Value) -> Res {
        let value = match ty.as_str() {
            Some("object") => V::empty_object(),
            Some("array") => V::Array(Vec::new()),
            _ => V::Undefined,
        };
        Res {
            value,
            read_only: false,
            write_only: false,
            ty: None,
        }
    }
}

fn flag(schema: &Value, key: &str) -> bool {
    truthy(schema.get(key))
}

/// `inferExample`: `const`, then `examples[0]`, then `enum[0]`, then `default`.
fn infer_example(schema: &Value) -> Option<V> {
    if let Some(v) = schema.get("const") {
        return Some(V::from_json(v));
    }
    let first = |key: &str| match schema.get(key) {
        Some(Value::Array(items)) => items.first().map(V::from_json),
        // `examples.length` and `examples[0]` also work on strings in JavaScript.
        Some(Value::String(s)) => s.chars().next().map(|c| V::String(c.to_string())),
        _ => None,
    };
    first("examples")
        .or_else(|| first("enum"))
        .or_else(|| schema.get("default").map(V::from_json))
}

/// `tryInferExample`
fn try_infer_example(schema: &Value) -> Option<Res> {
    infer_example(schema).map(|value| Res {
        value,
        read_only: flag(schema, "readOnly"),
        write_only: flag(schema, "writeOnly"),
        ty: Some(Value::Null),
    })
}

/// `{ ...schema, key: undefined }`
fn without(schema: &Value, keys: &[&str]) -> Value {
    let mut map = schema.as_object().cloned().unwrap_or_default();
    for key in keys {
        map.shift_remove(*key);
    }
    Value::Object(map)
}

/// The first `oneOf` / `anyOf` option, when the list is not empty.
fn first_option<'s>(schema: &'s Value, key: &str) -> Option<&'s Value> {
    schema.get(key)?.as_array()?.first()
}

/// `Object.assign({ readOnly: schema.readOnly, writeOnly: schema.writeOnly }, option)`
fn with_parent_annotations(parent: &Value, option: &Value) -> Value {
    let mut map = Map::new();
    for key in ["readOnly", "writeOnly"] {
        if let Some(v) = parent.get(key) {
            map.insert(key.to_owned(), v.clone());
        }
    }
    if let Some(option) = option.as_object() {
        for (k, v) in option {
            map.insert(k.clone(), v.clone());
        }
    }
    Value::Object(map)
}

/// Keywords next to a `$ref` are ignored in OpenAPI 3.0 and Swagger 2.0.
fn ignores_ref_siblings(spec: &Value) -> bool {
    let openapi = match spec.get("openapi") {
        None | Some(Value::Null) => String::new(),
        Some(v) => js_to_string(v),
    };
    openapi.starts_with("3.0") || spec.get("swagger").is_some()
}

pub(crate) struct Sampler<'a> {
    opts: &'a Options,
    spec: Option<&'a Value>,
    /// `$refCache`: pointers currently being expanded.
    ref_cache: HashMap<String, bool>,
    /// `seenSchemasStack`: addresses of the schemas being traversed, standing in
    /// for JavaScript object identity.
    seen: Vec<usize>,
}

fn address(schema: &Value) -> usize {
    schema as *const Value as usize
}

impl<'a> Sampler<'a> {
    pub(crate) fn new(opts: &'a Options, spec: Option<&'a Value>) -> Self {
        Sampler {
            opts,
            spec: spec.filter(|s| truthy(Some(s))),
            ref_cache: HashMap::new(),
            seen: Vec::new(),
        }
    }

    fn warn(&self, message: &str) {
        if !self.opts.quiet {
            log::warn!("{message}");
        }
    }

    fn pop(&mut self, ctx: Option<&Ctx>) {
        if ctx.is_some() {
            self.seen.pop();
        }
    }

    pub(crate) fn traverse(&mut self, schema: &Value, ctx: Option<&Ctx>) -> Result<Res, Error> {
        if ctx.is_some() {
            if self.seen.contains(&address(schema)) {
                return Ok(Res::circular(&infer_type_value(schema)));
            }
            self.seen.push(address(schema));
        }

        if let Some(depth) = ctx.and_then(|c| c.depth) {
            if depth > self.opts.max_sample_depth {
                self.pop(ctx);
                return Ok(Res::circular(&infer_type_value(schema)));
            }
        }

        if let Some(reference) = schema.get("$ref").filter(|r| truthy(Some(r))) {
            let (pointer, referenced) = self.resolve_ref(schema, reference)?;
            let result = if self.ref_cache.get(&pointer) != Some(&true) {
                self.ref_cache.insert(pointer.clone(), true);
                let result = self.traverse(&referenced, ctx)?;
                self.ref_cache.insert(pointer, false);
                result
            } else {
                Res::circular(&infer_type_value(&referenced))
            };
            self.pop(ctx);
            return Ok(result);
        }

        if let Some(example) = schema.get("example") {
            self.pop(ctx);
            return Ok(Res {
                value: V::from_json(example),
                read_only: flag(schema, "readOnly"),
                write_only: flag(schema, "writeOnly"),
                ty: schema.get("type").cloned(),
            });
        }

        if let Some(all_of) = schema.get("allOf") {
            self.pop(ctx);
            if let Some(res) = try_infer_example(schema) {
                return Ok(res);
            }
            return self.all_of_sample(&without(schema, &["allOf"]), all_of, ctx);
        }

        if let Some(first) = first_option(schema, "oneOf") {
            if truthy(schema.get("anyOf")) {
                self.warn("oneOf and anyOf are not supported on the same level. Skipping anyOf");
            }
            self.pop(ctx);
            return self.one_or_any_of(schema, &with_parent_annotations(schema, first), ctx);
        }

        if let Some(first) = first_option(schema, "anyOf") {
            self.pop(ctx);
            return self.one_or_any_of(schema, &with_parent_annotations(schema, first), ctx);
        }

        if let (Some(if_schema), Some(then)) = (schema.get("if"), schema.get("then")) {
            if truthy(Some(if_schema)) && truthy(Some(then)) {
                self.pop(ctx);
                let rest = without(schema, &["if", "then"]);
                let merged = merge_values(&[&rest, if_schema, then]);
                return self.traverse(&merged, ctx);
            }
        }

        let (value, ty) = match infer_example(schema) {
            Some(example) => (example, Value::Null),
            None => {
                let mut ty = schema.get("type").cloned().unwrap_or(Value::Null);
                if let Some(first) = ty.as_array().and_then(|types| types.first()) {
                    ty = first.clone();
                }
                if !truthy(Some(&ty)) {
                    ty = infer_type_value(schema);
                }
                let value = match ty.as_str() {
                    Some("array") => self.sample_array(schema, ctx)?,
                    Some("boolean") => V::Bool(true),
                    Some("integer" | "number") => V::num(sample_number(schema)),
                    Some("object") => self.sample_object(schema, ctx)?,
                    Some("string") => V::String(sample_string(
                        schema,
                        self.opts,
                        ctx.and_then(|c| c.property_name.as_deref()),
                    )?),
                    _ => V::Null,
                };
                (value, ty)
            }
        };

        self.pop(ctx);
        Ok(Res {
            value,
            read_only: flag(schema, "readOnly"),
            write_only: flag(schema, "writeOnly"),
            ty: Some(ty),
        })
    }

    /// `resolveRef`: the referenced schema, with the `$ref`'s sibling keywords
    /// merged in (except in OpenAPI 3.0 and Swagger 2.0).
    fn resolve_ref(
        &self,
        schema: &Value,
        reference: &Value,
    ) -> Result<(String, Cow<'a, Value>), Error> {
        let spec = self.spec.ok_or(Error::MissingSpec)?;
        let raw = match reference {
            Value::String(s) => s.clone(),
            other => js_to_string(other),
        };
        let invalid = |reason: String| Error::InvalidRef {
            reference: raw.clone(),
            reason,
        };
        let decoded = percent_decode(&raw).map_err(invalid)?;
        let pointer = decoded.strip_prefix('#').unwrap_or(&decoded).to_owned();
        let referenced = resolve_pointer(spec, &pointer).map_err(invalid)?;

        let mut siblings = schema.as_object().cloned().unwrap_or_default();
        siblings.shift_remove("$ref");
        if siblings.is_empty() || ignores_ref_siblings(spec) {
            return Ok((pointer, Cow::Borrowed(referenced)));
        }
        let siblings = Value::Object(siblings);
        let mut merged = merge_values(&[referenced, &siblings]);
        // mergeDeep merges arrays by index, which would drop required names.
        if let (Some(Value::Array(a)), Some(Value::Array(b))) =
            (referenced.get("required"), siblings.get("required"))
        {
            let mut required: Vec<Value> = Vec::new();
            for name in a.iter().chain(b) {
                if !required.contains(name) {
                    required.push(name.clone());
                }
            }
            if let Some(map) = merged.as_object_mut() {
                map.insert("required".to_owned(), Value::Array(required));
            }
        }
        Ok((pointer, Cow::Owned(merged)))
    }

    /// `traverseOneOrAnyOf`
    fn one_or_any_of(
        &mut self,
        schema: &Value,
        selected: &Value,
        ctx: Option<&Ctx>,
    ) -> Result<Res, Error> {
        if let Some(res) = try_infer_example(schema) {
            return Ok(res);
        }
        let local = self.traverse(&without(schema, &["oneOf", "anyOf"]), ctx)?;
        let mut sub = self.traverse(selected, ctx)?;
        if local.value.is_typeof_object() && sub.value.is_typeof_object() {
            sub.value = merge_deep(vec![local.value, sub.value]);
        }
        Ok(sub)
    }

    /// `allOfSample`
    fn all_of_sample(
        &mut self,
        into: &Value,
        children: &Value,
        ctx: Option<&Ctx>,
    ) -> Result<Res, Error> {
        let mut res = self.traverse(into, None)?;
        let mut sub_samples = Vec::new();
        let child_ctx = Ctx {
            is_all_of_child: true,
            ..ctx.cloned().unwrap_or_default()
        };

        for sub_schema in children.as_array().into_iter().flatten() {
            // `subSchema.$ref ? { ...subSchema } : { type: res.type, ...subSchema }`
            let child = if truthy(sub_schema.get("$ref")) {
                sub_schema.clone()
            } else {
                let mut map = Map::new();
                if let Some(ty) = &res.ty {
                    map.insert("type".to_owned(), ty.clone());
                }
                if let Some(sub) = sub_schema.as_object() {
                    for (k, v) in sub {
                        map.insert(k.clone(), v.clone());
                    }
                }
                Value::Object(map)
            };
            let sub = self.traverse(&child, Some(&child_ctx))?;
            if truthy(res.ty.as_ref()) && truthy(sub.ty.as_ref()) && sub.ty != res.ty {
                // Not gated by `quiet` upstream; it is here.
                self.warn("allOf: schemas with different types can't be merged");
                res.ty.clone_from(&sub.ty);
            }
            if !truthy(res.ty.as_ref()) {
                res.ty = sub.ty;
            }
            res.read_only = res.read_only || sub.read_only;
            res.write_only = res.write_only || sub.write_only;
            if !sub.value.is_nullish() {
                sub_samples.push(sub.value);
            }
        }

        if res.ty.as_ref().and_then(Value::as_str) == Some("object") {
            let base = if res.value.truthy() {
                res.value
            } else {
                V::empty_object()
            };
            let mut objects = vec![base];
            objects.extend(sub_samples.into_iter().filter(V::is_typeof_object));
            res.value = match merge_deep(objects) {
                V::Object(mut map) => {
                    map.retain(|_, v| *v != V::Skip);
                    V::Object(map)
                }
                V::Array(items) => V::Array(
                    items
                        .into_iter()
                        .map(|v| if v == V::Skip { V::Undefined } else { v })
                        .collect(),
                ),
                other => other,
            };
        } else {
            if res.ty.as_ref().and_then(Value::as_str) == Some("array") {
                self.warn(
                    "OpenAPI Sampler: found allOf with \"array\" type. Result may be incorrect",
                );
            }
            if let Some(last) = sub_samples.pop() {
                res.value = last;
            }
        }
        Ok(res)
    }

    /// `sampleArray`
    fn sample_array(&mut self, schema: &Value, ctx: Option<&Ctx>) -> Result<V, Error> {
        let depth = depth_of(ctx);
        // Math.min(maxItems ?? Infinity, minItems || 1)
        let max_items = finite(schema.get("maxItems")).unwrap_or(f64::INFINITY);
        let min_items = finite(schema.get("minItems"))
            .filter(|&n| n != 0.0)
            .unwrap_or(1.0);
        let mut length = max_items.min(min_items);

        // `contains` is treated like `items`, for the sake of simplicity.
        let items = ["prefixItems", "items", "contains"]
            .iter()
            .filter_map(|key| schema.get(*key))
            .find(|v| truthy(Some(v)));
        let Some(items) = items else {
            return Ok(V::Array(Vec::new()));
        };
        if let Value::Array(tuple) = items {
            length = length.max(tuple.len() as f64);
        }

        let empty = Value::Object(Map::new());
        let mut res = Vec::new();
        let mut i = 0usize;
        while (i as f64) < length {
            let item_schema = match items {
                Value::Array(tuple) => tuple.get(i).filter(|v| truthy(Some(v))).unwrap_or(&empty),
                single => single,
            };
            let ctx = Ctx::child(depth, None);
            res.push(self.traverse(item_schema, Some(&ctx))?.value);
            i += 1;
        }
        Ok(V::Array(res))
    }

    /// `sampleObject`
    fn sample_object(&mut self, schema: &Value, ctx: Option<&Ctx>) -> Result<V, Error> {
        let depth = depth_of(ctx);
        let is_all_of_child = ctx.is_some_and(|c| c.is_all_of_child);
        let mut res: IndexMap<String, V> = IndexMap::new();
        let required: Vec<&str> = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let properties = schema.get("properties").and_then(Value::as_object);

        if let Some(properties) = properties {
            for (name, property) in properties {
                // Skip before traversing, which could be costly.
                if self.opts.skip_non_required && !required.contains(&name.as_str()) {
                    continue;
                }
                let sample = self.traverse(property, Some(&Ctx::child(depth, Some(name))))?;
                if (self.opts.skip_read_only && sample.read_only)
                    || (self.opts.skip_write_only && sample.write_only)
                {
                    if is_all_of_child {
                        res.insert(name.clone(), V::Skip);
                    }
                    continue;
                }
                res.insert(name.clone(), sample.value);
            }
        }

        if let Some(additional) = schema.get("additionalProperties").filter(|v| v.is_object()) {
            let base = additional
                .get("x-additionalPropertiesName")
                .filter(|v| truthy(Some(v)))
                .map_or_else(|| "property".to_owned(), js_to_string);
            for n in 1..=2 {
                let value = self
                    .traverse(additional, Some(&Ctx::child(depth, None)))?
                    .value;
                res.insert(format!("{base}{n}"), value);
            }
        }

        // Strictly enforce maxProperties: required properties first, then the rest in order.
        let max_properties = schema.get("maxProperties").and_then(Value::as_f64);
        if let (Some(_), Some(max)) = (properties, max_properties) {
            if res.len() as f64 > max {
                let mut filtered = IndexMap::new();
                let mut added = 0usize;
                for name in &required {
                    if let Some(value) = res.get(*name).filter(|v| **v != V::Undefined) {
                        filtered.insert((*name).to_owned(), value.clone());
                        added += 1;
                    }
                }
                for (name, value) in res {
                    if (added as f64) < max && !filtered.contains_key(&name) {
                        filtered.insert(name, value);
                        added += 1;
                    }
                }
                res = filtered;
            }
        }

        Ok(V::Object(res))
    }
}
