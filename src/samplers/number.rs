// Ported from openapi-sampler (src/samplers/number.js),
// Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.

use serde_json::Value;

use crate::utils::{finite, truthy};

pub(crate) fn sample_number(schema: &Value) -> f64 {
    let mut res = 0.0;
    let is_number = schema.get("type").and_then(Value::as_str) == Some("number");
    let format = schema.get("format").and_then(Value::as_str);
    if is_number && matches!(format, Some("float" | "double")) {
        res = 0.1;
    }

    let minimum = finite(schema.get("minimum"));
    let maximum = finite(schema.get("maximum"));
    let exclusive_minimum = schema.get("exclusiveMinimum");
    let exclusive_maximum = schema.get("exclusiveMaximum");

    if exclusive_minimum.is_some_and(Value::is_boolean)
        || exclusive_maximum.is_some_and(Value::is_boolean)
    {
        // JSON Schema draft 4: exclusiveMinimum / exclusiveMaximum are booleans.
        let excl_min = truthy(exclusive_minimum);
        let excl_max = truthy(exclusive_maximum);
        if let (Some(max), Some(min)) = (maximum, minimum) {
            res = if excl_min { min.floor() + 1.0 } else { min };
            if (excl_max && res >= max) || (!excl_max && res > max) {
                res = (max + min) / 2.0;
            }
            return res;
        }
        if let Some(min) = minimum {
            return if excl_min { min.floor() + 1.0 } else { min };
        }
        if let Some(max) = maximum {
            return match (excl_max, max > 0.0) {
                (_, true) => 0.0,
                (true, false) => max.floor() - 1.0,
                (false, false) => max,
            };
        }
    } else {
        if let Some(min) = minimum {
            return min;
        }
        if let Some(excl_min) = finite(exclusive_minimum) {
            res = excl_min.floor() + 1.0;
            if let Some(excl_max) = exclusive_maximum.and_then(Value::as_f64) {
                if res == excl_max {
                    res = (res + excl_max.floor() - 1.0) / 2.0;
                }
            }
        } else if let Some(excl_max) = finite(exclusive_maximum) {
            res = excl_max.floor() - 1.0;
        } else if let Some(max) = maximum {
            res = max;
        }
    }
    res
}
