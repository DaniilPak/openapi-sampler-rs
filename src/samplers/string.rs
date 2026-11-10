// Ported from openapi-sampler (src/samplers/string.js),
// Copyright (c) 2017 Roman Hotsiy, MIT License. See LICENSE-MIT.

use serde_json::Value;

use super::string_regex::regex_sample;
use crate::utils::{ensure_min_length, substring, truthy, uuid};
use crate::{Error, Options};

const PASSWORD_SYMBOLS: &str = "qwerty!@#$%^123456";
const DATE_TIME: &str = "2019-08-24T14:15:22Z";
const DATE: &str = "2019-08-24";
const TIME: &str = "14:15:22Z";

fn password_sample(min: f64) -> Result<String, Error> {
    let mut res = String::from("pa$$word");
    if min > 8.0 {
        res.push('_');
        let missing = min - 9.0;
        res += &substring(&ensure_min_length(PASSWORD_SYMBOLS, missing)?, missing);
    }
    Ok(res)
}

fn date_time_sample(
    sample: &str,
    format: &str,
    min: f64,
    max: Option<f64>,
    opts: &Options,
) -> String {
    let len = sample.len() as f64;
    if len < min && !opts.quiet {
        log::warn!("Using minLength = {min} is incorrect with format \"{format}\"");
    }
    if max.is_some_and(|max| max != 0.0 && len > max) && !opts.quiet {
        log::warn!(
            "Using maxLength = {} is incorrect with format \"{format}\"",
            max.unwrap_or_default()
        );
    }
    sample.to_owned()
}

fn default_sample(
    min: f64,
    max: Option<f64>,
    pattern: Option<&str>,
    opts: &Options,
) -> Result<String, Error> {
    if let Some(pattern) = pattern.filter(|_| opts.enable_patterns) {
        return regex_sample(pattern);
    }
    let res = ensure_min_length("string", min)?;
    match max {
        Some(max) if max != 0.0 && res.chars().count() as f64 > max => Ok(substring(&res, max)),
        _ => Ok(res),
    }
}

pub(crate) fn sample_string(
    schema: &Value,
    opts: &Options,
    property_name: Option<&str>,
) -> Result<String, Error> {
    let format = schema
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("default");
    // `schema.minLength || 0`
    let min = schema
        .get("minLength")
        .filter(|v| truthy(Some(v)))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let max = schema.get("maxLength").and_then(Value::as_f64);
    let pattern = schema
        .get("pattern")
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty());

    let fixed = |s: &str| Ok(s.to_owned());
    match format {
        "email" => fixed("user@example.com"),
        "idn-email" => fixed("пошта@укр.нет"),
        "password" => password_sample(min),
        "date-time" => Ok(date_time_sample(DATE_TIME, format, min, max, opts)),
        "date" => Ok(date_time_sample(DATE, format, min, max, opts)),
        // The original checks the length of "T14:15:22Z" before dropping the "T".
        "time" => Ok(date_time_sample(&format!("T{TIME}"), format, min, max, opts)[1..].to_owned()),
        "ipv4" => fixed("192.168.0.1"),
        "ipv6" => fixed("2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
        "hostname" => fixed("example.com"),
        "idn-hostname" => fixed("приклад.укр"),
        "iri" => fixed("http://example.com/entity/1"),
        "iri-reference" => fixed("/entity/1"),
        "uri" => fixed("http://example.com"),
        "uri-reference" => fixed("../dictionary"),
        "uri-template" => fixed("http://example.com/{endpoint}"),
        "uuid" => Ok(uuid(
            property_name.filter(|n| !n.is_empty()).unwrap_or("id"),
        )),
        "json-pointer" => fixed("/json/pointer"),
        "relative-json-pointer" => fixed("1/relative/json/pointer"),
        "regex" => fixed("/regex/"),
        _ => default_sample(min, max, pattern, opts),
    }
}
