// Portions ported from openapi-sampler (src/utils.js),
// Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.

use serde_json::Value;

use crate::Error;

/// Longest string a JavaScript engine (V8) can build; openapi-sampler throws a
/// `RangeError` past it, and so do we (as [`Error::TooLong`]).
pub(crate) const MAX_STRING_LENGTH: f64 = 536_870_888.0;

/// JavaScript truthiness of an optional (possibly `undefined`) JSON value.
pub(crate) fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}

/// `Number.isFinite(value)`: only JSON numbers qualify.
pub(crate) fn finite(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|f| f.is_finite())
}

/// `String(value)` in JavaScript.
pub(crate) fn js_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => i.to_string(),
            (_, Some(u), _) => u.to_string(),
            (_, _, Some(f)) if f.fract() == 0.0 && f.abs() < 1e21 => format!("{f:.0}"),
            (_, _, Some(f)) => f.to_string(),
            _ => n.to_string(),
        },
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|v| match v {
                Value::Null => String::new(),
                other => js_to_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

/// `str.substring(0, end)` (counting characters).
pub(crate) fn substring(s: &str, end: f64) -> String {
    let end = if end.is_nan() {
        0.0
    } else {
        end.trunc().max(0.0)
    };
    s.chars()
        .take(end.min(usize::MAX as f64) as usize)
        .collect()
}

/// openapi-sampler's `ensureMinLength`: repeats `sample` up to `min` characters.
pub(crate) fn ensure_min_length(sample: &str, min: f64) -> Result<String, Error> {
    let len = sample.chars().count();
    if min > len as f64 {
        if min > MAX_STRING_LENGTH {
            return Err(Error::TooLong);
        }
        let times = (min / len as f64).trunc() as usize + 1;
        return Ok(substring(&sample.repeat(times), min));
    }
    Ok(sample.to_owned())
}

fn hash_code(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit))
    })
}

/// The `jsf32` PRNG from openapi-sampler, returning numbers in `[0, 1)`.
struct Jsf32 {
    a: i32,
    b: i32,
    c: i32,
    d: i32,
}

impl Jsf32 {
    fn next(&mut self) -> f64 {
        let t = self.a.wrapping_sub(self.b.rotate_left(27));
        self.a = self.b ^ self.c.rotate_left(17);
        self.b = self.c.wrapping_add(self.d);
        self.c = self.d.wrapping_add(t);
        self.d = self.a.wrapping_add(t);
        f64::from(self.d as u32) / 4_294_967_296.0
    }
}

/// openapi-sampler's deterministic UUID, seeded by `seed` (usually the property name).
pub(crate) fn uuid(seed: &str) -> String {
    let hash = hash_code(seed);
    let mut random = Jsf32 {
        a: hash,
        b: hash,
        c: hash,
        d: hash,
    };
    "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"
        .chars()
        .map(|c| match c {
            'x' | 'y' => {
                let r = ((random.next() * 16.0) % 16.0) as u32;
                let digit = if c == 'x' { r } else { (r & 0x3) | 0x8 };
                char::from_digit(digit, 16).unwrap_or('0')
            }
            other => other,
        })
        .collect()
}

/// `decodeURIComponent`.
pub(crate) fn percent_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| "URI malformed".to_owned())?;
            out.push(hex);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "URI malformed".to_owned())
}

/// `JsonPointer.get(spec, pointer)` from the `json-pointer` package.
pub(crate) fn resolve_pointer<'a>(spec: &'a Value, pointer: &str) -> Result<&'a Value, String> {
    if pointer.is_empty() {
        return Ok(spec);
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return Err(format!("Invalid JSON pointer: {pointer}"));
    };
    let mut current = spec;
    for raw in rest.split('/') {
        let token = raw.replace("~1", "/").replace("~0", "~");
        let next = match current {
            Value::Object(map) => map.get(&token),
            Value::Array(items) => token
                .parse::<usize>()
                .ok()
                .filter(|i| i.to_string() == token)
                .and_then(|i| items.get(i)),
            _ => None,
        };
        current = next.ok_or_else(|| format!("Invalid reference token: {token}"))?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uuid_is_deterministic() {
        assert_eq!(uuid("id"), "497f6eca-6276-4993-bfeb-53cbbbba6f08");
        assert_eq!(uuid("fooId"), "fb4274c7-4fcd-4035-8958-a680548957ff");
        assert_eq!(uuid("barId"), "3c966637-4898-4972-9a9d-baefa6cd6c89");
    }

    #[test]
    fn pointer_resolution() {
        let spec = json!({"a": {"b/c": [1, {"~d": 2}]}});
        assert_eq!(resolve_pointer(&spec, "").unwrap(), &spec);
        assert_eq!(resolve_pointer(&spec, "/a/b~1c/1/~0d").unwrap(), &json!(2));
        assert!(resolve_pointer(&spec, "/a/missing").is_err());
        assert!(resolve_pointer(&spec, "a").is_err());
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("#/a%20b").unwrap(), "#/a b");
        assert_eq!(percent_decode("%D0%B9").unwrap(), "й");
        assert!(percent_decode("%zz").is_err());
        assert!(percent_decode("%D0").is_err());
    }

    #[test]
    fn js_string_conversion() {
        assert_eq!(js_to_string(&json!(3.0)), "3");
        assert_eq!(js_to_string(&json!(3.1)), "3.1");
        assert_eq!(js_to_string(&json!("3.0.1")), "3.0.1");
    }
}
