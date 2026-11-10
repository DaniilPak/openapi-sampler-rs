//! Internal sample value with the two JavaScript states that `serde_json::Value`
//! cannot express: `undefined` and the "skip this property" marker that
//! openapi-sampler uses while merging `allOf` children.

use indexmap::IndexMap;
use serde_json::{Map, Number, Value};

/// Largest integer a JavaScript number holds exactly (2^53).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum V {
    /// JavaScript `undefined`: dropped from objects, `null` inside arrays.
    Undefined,
    /// openapi-sampler's `SKIP_SYMBOL`: a property removed after `allOf` merging.
    Skip,
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<V>),
    Object(IndexMap<String, V>),
}

impl V {
    pub(crate) fn from_json(value: &Value) -> V {
        match value {
            Value::Null => V::Null,
            Value::Bool(b) => V::Bool(*b),
            Value::Number(n) => V::Number(n.clone()),
            Value::String(s) => V::String(s.clone()),
            Value::Array(items) => V::Array(items.iter().map(V::from_json).collect()),
            Value::Object(map) => V::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), V::from_json(v)))
                    .collect(),
            ),
        }
    }

    /// Converts to JSON the way `JSON.stringify` would: `undefined` and skipped
    /// properties disappear from objects and become `null` inside arrays.
    pub(crate) fn into_json(self) -> Option<Value> {
        match self {
            V::Undefined | V::Skip => None,
            V::Null => Some(Value::Null),
            V::Bool(b) => Some(Value::Bool(b)),
            V::Number(n) => Some(Value::Number(n)),
            V::String(s) => Some(Value::String(s)),
            V::Array(items) => Some(Value::Array(
                items
                    .into_iter()
                    .map(|v| v.into_json().unwrap_or(Value::Null))
                    .collect(),
            )),
            V::Object(map) => {
                let mut out = Map::new();
                for (k, v) in map {
                    if let Some(v) = v.into_json() {
                        out.insert(k, v);
                    }
                }
                Some(Value::Object(out))
            }
        }
    }

    /// A JavaScript number: integral values become JSON integers, like `JSON.stringify`.
    pub(crate) fn num(f: f64) -> V {
        if f.is_finite() && f.fract() == 0.0 && f.abs() < MAX_SAFE_INTEGER {
            V::Number(Number::from(f as i64))
        } else {
            Number::from_f64(f).map_or(V::Null, V::Number)
        }
    }

    pub(crate) fn empty_object() -> V {
        V::Object(IndexMap::new())
    }

    /// JavaScript truthiness.
    pub(crate) fn truthy(&self) -> bool {
        match self {
            V::Undefined | V::Null => false,
            V::Skip | V::Array(_) | V::Object(_) => true,
            V::Bool(b) => *b,
            V::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
            V::String(s) => !s.is_empty(),
        }
    }

    /// `value == null` in JavaScript.
    pub(crate) fn is_nullish(&self) -> bool {
        matches!(self, V::Undefined | V::Null)
    }

    /// `typeof value === 'object'` in JavaScript (includes `null`).
    pub(crate) fn is_typeof_object(&self) -> bool {
        matches!(self, V::Null | V::Array(_) | V::Object(_))
    }

    /// `value && typeof value === 'object'`.
    fn is_object_like(&self) -> bool {
        matches!(self, V::Array(_) | V::Object(_))
    }
}

/// Canonical array index, as JavaScript understands property keys on arrays.
fn array_index(key: &str) -> Option<usize> {
    let idx: usize = key.parse().ok()?;
    (idx.to_string() == key && idx < u32::MAX as usize).then_some(idx)
}

/// `Object.keys(value)` paired with the values.
fn js_entries(value: V) -> Vec<(String, V)> {
    match value {
        V::Object(map) => map.into_iter().collect(),
        V::Array(items) => items
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        V::String(s) => s
            .chars()
            .enumerate()
            .map(|(i, c)| (i.to_string(), V::String(c.to_string())))
            .collect(),
        _ => Vec::new(),
    }
}

fn get_key<'v>(target: &'v V, key: &str) -> Option<&'v V> {
    match target {
        V::Object(map) => map.get(key),
        V::Array(items) => array_index(key).and_then(|i| items.get(i)),
        _ => None,
    }
}

fn set_key(target: &mut V, key: String, value: V) {
    match target {
        V::Object(map) => {
            map.insert(key, value);
        }
        V::Array(items) => {
            if let Some(i) = array_index(&key) {
                if i >= items.len() {
                    items.resize(i + 1, V::Undefined);
                }
                items[i] = value;
            }
            // Non-index keys on a JavaScript array never reach JSON output.
        }
        _ => {}
    }
}

/// Port of openapi-sampler's `mergeDeep`: a fresh object (or array, when the
/// last argument is an array) with every argument deep-merged in order.
pub(crate) fn merge_deep(objects: Vec<V>) -> V {
    let mut target = match objects.last() {
        Some(V::Array(_)) => V::Array(Vec::new()),
        _ => V::empty_object(),
    };
    for object in objects {
        for (key, value) in js_entries(object) {
            let merged = match get_key(&target, &key) {
                Some(prev) if prev.is_object_like() && value.is_object_like() => {
                    merge_deep(vec![prev.clone(), value])
                }
                _ => value,
            };
            set_key(&mut target, key, merged);
        }
    }
    target
}

/// `mergeDeep` over schemas.
pub(crate) fn merge_values(objects: &[&Value]) -> Value {
    merge_deep(objects.iter().map(|v| V::from_json(v)).collect())
        .into_json()
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn merge(values: &[Value]) -> Value {
        merge_values(&values.iter().collect::<Vec<_>>())
    }

    #[test]
    fn merges_nested_objects() {
        assert_eq!(
            merge(&[json!({"a": {"x": 1}}), json!({"a": {"y": 2}, "b": 3})]),
            json!({"a": {"x": 1, "y": 2}, "b": 3})
        );
    }

    #[test]
    fn merges_arrays_by_index() {
        assert_eq!(merge(&[json!(["a", "b"]), json!(["c"])]), json!(["c", "b"]));
        assert_eq!(
            merge(&[json!({"arr": [{}]}), json!({"arr": [{"name": "x"}]})]),
            json!({"arr": [{"name": "x"}]})
        );
    }

    #[test]
    fn does_not_merge_scalars() {
        assert_eq!(merge(&[json!(null), json!(null)]), json!({}));
        assert_eq!(
            merge(&[json!({"a": 1}), json!({"a": {"b": 1}})]),
            json!({"a": {"b": 1}})
        );
    }

    #[test]
    fn numbers_follow_json_stringify() {
        assert_eq!(V::num(0.0).into_json(), Some(json!(0)));
        assert_eq!(V::num(-0.0).into_json(), Some(json!(0)));
        assert_eq!(V::num(2.5).into_json(), Some(json!(2.5)));
        assert_eq!(V::num(f64::NAN).into_json(), Some(json!(null)));
    }
}
