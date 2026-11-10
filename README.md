# openapi-sampler (Rust)

[![CI](https://github.com/DaniilPak/openapi-sampler-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/DaniilPak/openapi-sampler-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/openapi-sampler.svg)](https://crates.io/crates/openapi-sampler)
[![docs.rs](https://docs.rs/openapi-sampler/badge.svg)](https://docs.rs/openapi-sampler)

Generate deterministic example values from **OpenAPI 3.x**, **Swagger 2.0** and
**JSON Schema** schemas, in pure Rust.

This is an independent Rust port of Redocly's
[openapi-sampler](https://github.com/Redocly/openapi-sampler) (the JavaScript
library behind the request and response samples in Redoc). It aims to give the
same output as openapi-sampler for the same schema. **It is not affiliated with
or endorsed by Redocly.**

```toml
[dependencies]
openapi-sampler = "0.1"
serde_json = "1"
```

## Usage

```rust
use openapi_sampler::{sample, Options};
use serde_json::json;

let spec = json!({
    "openapi": "3.1.0",
    "components": { "schemas": {
        "Pet": {
            "type": "object",
            "required": ["name"],
            "properties": {
                "id":   { "type": "string", "format": "uuid", "readOnly": true },
                "name": { "type": "string", "example": "Rex" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "born": { "type": "string", "format": "date" }
            }
        }
    }}
});

let schema = json!({ "$ref": "#/components/schemas/Pet" });

// A response body
let response = sample(&schema, &Options::default(), Some(&spec)).unwrap();
assert_eq!(response, json!({
    "id": "497f6eca-6276-4993-bfeb-53cbbbba6f08",
    "name": "Rex",
    "tags": ["string"],
    "born": "2019-08-24"
}));

// A request body: leave out what the server fills in
let request = sample(&schema, &Options::new().skip_read_only(true), Some(&spec)).unwrap();
assert_eq!(request, json!({ "name": "Rex", "tags": ["string"], "born": "2019-08-24" }));
```

`sample(schema, options, spec)`:

- `schema` — the schema to sample (`&serde_json::Value`).
- `options` — see below.
- `spec` — the whole document that `$ref`s are resolved against, or `None`
  when the schema has no `$ref`. Only local references (`#/...`) are supported.

The result is a `serde_json::Value`, or an [`Error`](https://docs.rs/openapi-sampler/latest/openapi_sampler/enum.Error.html)
when a `$ref` cannot be resolved (or no spec was given), or a `pattern` cannot be
sampled.

`infer_type(schema)` is also exported: the type a schema is sampled as.

### How values are chosen

1. `example`, then `const`, `examples[0]`, `enum[0]`, `default`.
2. `$ref` is followed (circular references become `{}` / `[]` / nothing).
   In OpenAPI 3.1 (or when the spec has no version), keywords next to `$ref`
   are merged in; in OpenAPI 3.0 and Swagger 2.0 they are ignored.
3. `allOf` children are deep-merged; `oneOf` / `anyOf` use the first option
   (merged with the keywords next to it); `if` + `then` are merged into the schema.
4. Otherwise a value is made up from `type` (the first one when it is a list), or
   the type implied by the keywords (`minimum` → number, `properties` → object, …):

| Type | Sample |
| --- | --- |
| `string` | `"string"`, padded to `minLength` and cut to `maxLength` |
| `string` + `format` | `email` → `user@example.com`, `uuid` → deterministic UUID seeded by the property name, `date` → `2019-08-24`, `date-time` → `2019-08-24T14:15:22Z`, `time` → `14:15:22Z`, `password` → `pa$$word`, `ipv4` → `192.168.0.1`, `ipv6`, `hostname`, `uri`, `uri-reference`, `uri-template`, `iri`, `iri-reference`, `idn-email`, `idn-hostname`, `json-pointer`, `relative-json-pointer`, `regex` |
| `string` + `pattern` | a matching string, when `enable_patterns` is on |
| `number` / `integer` | `0` (`0.1` for `float` / `double`), moved inside `minimum` / `maximum` / `exclusiveMinimum` / `exclusiveMaximum` (draft 4 booleans too) |
| `boolean` | `true` |
| `array` | `minItems` items (at least 1, at most `maxItems`); tuples via `items: [...]` or `prefixItems`; `contains` is used like `items` |
| `object` | every property, plus two `additionalProperties` (`property1`, `property2`, or `x-additionalPropertiesName`); `maxProperties` keeps required ones first |
| no type | `null` |

## Options

| Field (builder method) | openapi-sampler | Default | Effect |
| --- | --- | --- | --- |
| `skip_read_only` | `skipReadOnly` | `false` | Leave out `readOnly` properties (for request bodies). |
| `skip_write_only` | `skipWriteOnly` | `false` | Leave out `writeOnly` properties (for responses). |
| `skip_non_required` | `skipNonRequired` | `false` | Leave out properties not listed in `required`. |
| `max_sample_depth` | `maxSampleDepth` | `15` | Nesting depth after which objects become `{}` and arrays `[]`. |
| `enable_patterns` | `enablePatterns` | `false` | Generate strings matching `pattern` (otherwise `pattern` is ignored). |
| `quiet` | `quiet` | `false` | Do not emit warnings. Warnings go through the [`log`](https://docs.rs/log) crate. |

```rust
let options = openapi_sampler::Options::new()
    .skip_write_only(true)
    .max_sample_depth(5);
```

## Parity with openapi-sampler

Ported from openapi-sampler 1.7.10. Its test suite was translated to Rust tests
(`tests/integration.rs`, `tests/samplers.rs`): every case of
`integration.spec.js` except the XML ones, and `unit/{array,number,object,string}.spec.js`.
All pass, including the exact deterministic UUIDs.

openapi-sampler's own quirks are kept on purpose, so samples match:

- `nullable` is ignored; a `type` list is sampled as its first type (`["null", "string"]` → `null`).
- `minItems: 0` still gives one item (unless `maxItems: 0`); `maxLength: 0` is ignored.
- `additionalProperties: {}` adds `property1: null, property2: null`.
- `byte` and `binary` formats have no special sample (`"string"`).
- `pattern` sampling covers the same regex subset as openapi-sampler (character
  classes, `{n}`, `{n,m}`, `?`, `*`, `+`, anchors), always picking the first
  character of a class and the lower bound of a quantifier.
- `discriminator` is not used; the first `oneOf` option is sampled.

Differences:

- **XML output (`format: "xml"`) is not implemented.** JSON-shaped samples only.
- Errors are returned as `Result` instead of thrown. Inputs that make the
  JavaScript library crash with a `TypeError` (for example `properties: null` or a
  `null` schema) are treated as empty schemas.
- All warnings respect `quiet` (openapi-sampler prints a few unconditionally).
- Object keys keep schema order. JavaScript moves integer-like keys (`"1"`, `"2"`)
  to the front of objects; this crate does not.
- String lengths are counted in Unicode characters, JavaScript counts UTF-16
  code units; this only differs for characters outside the Basic Multilingual Plane.
- Large integer examples are kept exactly; JavaScript would round them past 2^53.
- Custom samplers (`_registerSampler`) are not supported.

The crate enables `serde_json`'s `preserve_order` feature so that samples keep
property order; through feature unification this applies to `serde_json` across
your build.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option.

The code and tests ported from openapi-sampler remain under its MIT license,
Copyright (c) 2017 Roman Hotsiy; the pattern sampler is based on faker.js (MIT).
The full notices are in [LICENSE-MIT](LICENSE-MIT).

Thanks to [Redocly](https://redocly.com) and the openapi-sampler contributors for
the original library.
