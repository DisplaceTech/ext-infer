//! JSON Schema → GBNF conversion for grammar-constrained generation.
//!
//! llama.cpp constrains sampling with GBNF grammars (a BNF dialect; see
//! `grammars/README.md` upstream). Callers can hand us a GBNF string
//! directly (`'grammar'` option), or — far more usefully — a JSON Schema
//! (`'schema'` option) that we compile to GBNF here.
//!
//! ## Supported schema subset
//!
//! Deliberately the *practical* subset, hard-failing on anything else so a
//! schema is never silently half-enforced:
//!
//! - `type: object` with `properties` — all properties are required and
//!   generated in declaration order. A `required` list must either be
//!   absent or name every property; optional properties are not supported
//!   (the combinatorial GBNF they need is v0.3 material).
//! - `type: array` with `items`, plus `minItems` of `0` (default) or `1`.
//! - `type: string` / `integer` / `number` / `boolean` / `null`.
//! - `enum` of strings, numbers, booleans, or null; `const` of the same.
//! - `anyOf` / `oneOf` — compiled as alternation (useful for nullable
//!   fields); multi-type arrays like `type: ["string", "null"]` likewise.
//!
//! Unsupported keywords (`$ref`, `additionalProperties: true` free-form
//! objects, `pattern`, numeric ranges, `minLength`, ...) raise
//! `InferException` naming the offending keyword. The error message is the
//! API contract: silently ignoring a constraint the caller asked for would
//! produce output that *looks* validated but isn't.
//!
//! The emitted grammar constrains the model to canonical JSON: minimal
//! whitespace freedom (`space ::= " "?` between tokens), no trailing
//! commas, strict JSON string escaping.

use serde_json::Value;

use crate::error::InferError;

/// Compile a JSON Schema document into a GBNF grammar string with a `root`
/// rule. Fails loudly on schema keywords outside the supported subset.
pub fn json_schema_to_gbnf(schema: &Value) -> Result<String, InferError> {
    let mut builder = Builder::default();
    let root = builder.visit(schema, "root")?;

    let mut out = String::new();
    // `root` must be a rule name, not an inline production; alias if needed.
    if root != "root" {
        out.push_str(&format!("root ::= {root}\n"));
    }
    for (name, production) in &builder.rules {
        out.push_str(&format!("{name} ::= {production}\n"));
    }
    Ok(out)
}

/// Schema keywords we accept on a node alongside the structural ones.
/// Anything not listed here (or handled structurally) is a hard error.
const TOLERATED_KEYWORDS: &[&str] = &[
    // Annotation-only keywords — no constraint semantics, safe to ignore.
    "title",
    "description",
    "default",
    "examples",
    "$schema",
    "$id",
    // Handled structurally in `visit`.
    "type",
    "properties",
    "required",
    "items",
    "minItems",
    "enum",
    "const",
    "anyOf",
    "oneOf",
    "additionalProperties",
];

#[derive(Default)]
struct Builder {
    /// Rule name → production, in insertion order (stable output makes the
    /// grammar diffable and the tests exact).
    rules: Vec<(String, String)>,
}

impl Builder {
    fn add_rule(&mut self, name: &str, production: &str) -> String {
        if let Some((existing, _)) = self.rules.iter().find(|(n, _)| n == name) {
            return existing.clone();
        }
        self.rules.push((name.to_string(), production.to_string()));
        name.to_string()
    }

    fn primitive(&mut self, name: &str) -> String {
        let production = match name {
            "space" => r#"" "?"#,
            "string" => {
                self.primitive("space");
                concat!(
                    r#""\"" ( [^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F]) )* "\"" space"#
                )
            }
            "integer" => {
                self.primitive("space");
                r#""-"? ("0" | [1-9] [0-9]*) space"#
            }
            "number" => {
                self.primitive("space");
                r#""-"? ("0" | [1-9] [0-9]*) ("." [0-9]+)? ([eE] [-+]? [0-9]+)? space"#
            }
            "boolean" => {
                self.primitive("space");
                r#"("true" | "false") space"#
            }
            "null" => {
                self.primitive("space");
                r#""null" space"#
            }
            other => unreachable!("unknown primitive rule {other}"),
        };
        self.add_rule(name, production)
    }

    /// Compile one schema node. Returns the GBNF *rule name* that matches it.
    fn visit(&mut self, schema: &Value, name: &str) -> Result<String, InferError> {
        let obj = schema.as_object().ok_or_else(|| {
            schema_error(format!(
                "schema node at {name:?} must be an object, got {schema}"
            ))
        })?;

        for key in obj.keys() {
            if !TOLERATED_KEYWORDS.contains(&key.as_str()) {
                return Err(schema_error(format!(
                    "unsupported JSON Schema keyword {key:?} at {name:?} — \
                     supported: type/properties/required/items/minItems/enum/const/anyOf/oneOf"
                )));
            }
        }

        // enum / const take precedence over type: they pin exact values.
        if let Some(values) = obj.get("enum") {
            let list = values
                .as_array()
                .ok_or_else(|| schema_error(format!("\"enum\" at {name:?} must be an array")))?;
            if list.is_empty() {
                return Err(schema_error(format!("\"enum\" at {name:?} is empty")));
            }
            let alternation = list
                .iter()
                .map(|v| literal_terminal(v, name))
                .collect::<Result<Vec<_>, _>>()?
                .join(" | ");
            self.primitive("space");
            return Ok(self.add_rule(name, &format!("({alternation}) space")));
        }
        if let Some(value) = obj.get("const") {
            let terminal = literal_terminal(value, name)?;
            self.primitive("space");
            return Ok(self.add_rule(name, &format!("{terminal} space")));
        }

        // anyOf / oneOf — alternation over sub-schemas.
        for combinator in ["anyOf", "oneOf"] {
            if let Some(variants) = obj.get(combinator) {
                let list = variants.as_array().ok_or_else(|| {
                    schema_error(format!("{combinator:?} at {name:?} must be an array"))
                })?;
                if list.is_empty() {
                    return Err(schema_error(format!("{combinator:?} at {name:?} is empty")));
                }
                let mut alternatives = Vec::with_capacity(list.len());
                for (i, variant) in list.iter().enumerate() {
                    alternatives.push(self.visit(variant, &format!("{name}-{i}"))?);
                }
                return Ok(self.add_rule(name, &format!("({})", alternatives.join(" | "))));
            }
        }

        match obj.get("type") {
            Some(Value::String(t)) => self.visit_typed(obj, t, name),
            // Multi-type shorthand: ["string", "null"] → alternation.
            Some(Value::Array(types)) => {
                if types.is_empty() {
                    return Err(schema_error(format!("\"type\" array at {name:?} is empty")));
                }
                let mut alternatives = Vec::with_capacity(types.len());
                for t in types {
                    let t = t.as_str().ok_or_else(|| {
                        schema_error(format!("\"type\" array at {name:?} must hold strings"))
                    })?;
                    alternatives.push(self.visit_typed(obj, t, &format!("{name}-{t}"))?);
                }
                Ok(self.add_rule(name, &format!("({})", alternatives.join(" | "))))
            }
            Some(other) => Err(schema_error(format!(
                "\"type\" at {name:?} must be a string or array of strings, got {other}"
            ))),
            None => Err(schema_error(format!(
                "schema node at {name:?} needs one of: type, enum, const, anyOf, oneOf"
            ))),
        }
    }

    fn visit_typed(
        &mut self,
        obj: &serde_json::Map<String, Value>,
        type_name: &str,
        name: &str,
    ) -> Result<String, InferError> {
        match type_name {
            "string" => Ok(self.primitive("string")),
            "integer" => Ok(self.primitive("integer")),
            "number" => Ok(self.primitive("number")),
            "boolean" => Ok(self.primitive("boolean")),
            "null" => Ok(self.primitive("null")),
            "object" => self.visit_object(obj, name),
            "array" => self.visit_array(obj, name),
            other => Err(schema_error(format!(
                "unsupported \"type\" {other:?} at {name:?}"
            ))),
        }
    }

    fn visit_object(
        &mut self,
        obj: &serde_json::Map<String, Value>,
        name: &str,
    ) -> Result<String, InferError> {
        let properties = obj
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                schema_error(format!(
                    "object at {name:?} needs a \"properties\" map — \
                     free-form objects are not supported"
                ))
            })?;
        if properties.is_empty() {
            return Err(schema_error(format!(
                "object at {name:?} has no properties"
            )));
        }
        if let Some(additional) = obj.get("additionalProperties") {
            if additional != &Value::Bool(false) {
                return Err(schema_error(format!(
                    "\"additionalProperties\" at {name:?} must be false when present — \
                     the grammar emits exactly the declared properties"
                )));
            }
        }
        // Every property is generated; a `required` list must agree.
        if let Some(required) = obj.get("required") {
            let required = required.as_array().ok_or_else(|| {
                schema_error(format!("\"required\" at {name:?} must be an array"))
            })?;
            let listed: Vec<&str> = required.iter().filter_map(Value::as_str).collect();
            for key in properties.keys() {
                if !listed.contains(&key.as_str()) {
                    return Err(schema_error(format!(
                        "optional properties are not supported: {key:?} at {name:?} is \
                         missing from \"required\" — list every property or drop \"required\""
                    )));
                }
            }
        }

        self.primitive("space");
        let mut parts: Vec<String> = vec![r#""{" space"#.into()];
        for (i, (key, sub_schema)) in properties.iter().enumerate() {
            let value_rule = self.visit(sub_schema, &format!("{name}-{}", sanitize(key)))?;
            if i > 0 {
                parts.push(r#""," space"#.into());
            }
            // The grammar must match the JSON-encoded key bytes, so encode
            // the key as a JSON string first, then escape that for GBNF.
            let key_terminal = gbnf_string_literal(&Value::String(key.clone()).to_string());
            parts.push(format!(r#"{key_terminal} space ":" space {value_rule}"#));
        }
        parts.push(r#""}" space"#.into());
        Ok(self.add_rule(name, &parts.join(" ")))
    }

    fn visit_array(
        &mut self,
        obj: &serde_json::Map<String, Value>,
        name: &str,
    ) -> Result<String, InferError> {
        let items = obj.get("items").ok_or_else(|| {
            schema_error(format!(
                "array at {name:?} needs \"items\" — untyped arrays are not supported"
            ))
        })?;
        let min_items = match obj.get("minItems") {
            None => 0,
            Some(v) => v.as_u64().filter(|n| *n <= 1).ok_or_else(|| {
                schema_error(format!(
                    "\"minItems\" at {name:?} must be 0 or 1 — larger minimums are not supported"
                ))
            })?,
        };

        let item_rule = self.visit(items, &format!("{name}-item"))?;
        self.primitive("space");
        let production = if min_items == 0 {
            format!(r#""[" space ({item_rule} ("," space {item_rule})*)? "]" space"#)
        } else {
            format!(r#""[" space {item_rule} ("," space {item_rule})* "]" space"#)
        };
        Ok(self.add_rule(name, &production))
    }
}

/// GBNF terminal for an exact JSON literal (enum/const member).
fn literal_terminal(value: &Value, name: &str) -> Result<String, InferError> {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
            // `to_string()` JSON-encodes the literal (quotes strings, etc.);
            // the grammar must match those exact bytes.
            Ok(gbnf_string_literal(&value.to_string()))
        }
        _ => Err(schema_error(format!(
            "enum/const at {name:?} supports only strings, numbers, booleans, and null"
        ))),
    }
}

/// Quote arbitrary text as a GBNF string terminal (`"..."` with `\` and `"`
/// escaped, control characters as `\xNN`).
fn gbnf_string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!(r"\x{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// GBNF rule names allow `[a-zA-Z0-9-]`; anything else becomes `-`.
fn sanitize(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn schema_error(message: String) -> InferError {
    InferError::InvalidOption {
        name: "schema".into(),
        reason: message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gbnf(schema: serde_json::Value) -> String {
        json_schema_to_gbnf(&schema).expect("schema should compile")
    }

    fn gbnf_err(schema: serde_json::Value) -> String {
        json_schema_to_gbnf(&schema)
            .expect_err("schema should be rejected")
            .to_string()
    }

    #[test]
    fn object_with_scalar_properties() {
        let g = gbnf(json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name", "age"]
        }));
        assert!(g.contains(
            r#"root ::= "{" space "\"name\"" space ":" space string "," space "\"age\"" space ":" space integer "}" space"#
        ));
        assert!(g.contains("string ::="));
        assert!(g.contains("integer ::="));
        assert!(g.contains(r#"space ::= " "?"#));
    }

    #[test]
    fn nested_objects_and_arrays() {
        let g = gbnf(json!({
            "type": "object",
            "properties": {
                "tags": {"type": "array", "items": {"type": "string"}, "minItems": 1},
                "author": {
                    "type": "object",
                    "properties": {"email": {"type": "string"}}
                }
            }
        }));
        assert!(g.contains(r#"root-tags ::= "[" space string ("," space string)* "]" space"#));
        assert!(
            g.contains(r#"root-author ::= "{" space "\"email\"" space ":" space string "}" space"#)
        );
    }

    #[test]
    fn enum_of_strings() {
        let g = gbnf(json!({"enum": ["draft", "published"]}));
        assert!(g.contains(r#"root ::= ("\"draft\"" | "\"published\"") space"#));
    }

    #[test]
    fn const_and_mixed_enum() {
        let g = gbnf(json!({"enum": ["yes", 42, true, null]}));
        assert!(g.contains(r#"("\"yes\"" | "42" | "true" | "null") space"#));

        let g = gbnf(json!({"const": "fixed"}));
        assert!(g.contains(r#"root ::= "\"fixed\"" space"#));
    }

    #[test]
    fn nullable_via_anyof_and_type_array() {
        let g = gbnf(json!({"anyOf": [{"type": "string"}, {"type": "null"}]}));
        assert!(g.contains("root ::= (string | null)"));

        let g = gbnf(json!({"type": ["integer", "null"]}));
        assert!(g.contains("root ::= (integer | null)"));
    }

    #[test]
    fn bare_scalar_root_gets_alias() {
        let g = gbnf(json!({"type": "string"}));
        assert!(g.starts_with("root ::= string\n"));
    }

    #[test]
    fn unsupported_keyword_is_named_in_the_error() {
        let err = gbnf_err(json!({"type": "string", "pattern": "^a+$"}));
        assert!(err.contains("pattern"), "got: {err}");
    }

    #[test]
    fn optional_properties_are_rejected() {
        let err = gbnf_err(json!({
            "type": "object",
            "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
            "required": ["a"]
        }));
        assert!(err.contains("optional properties"), "got: {err}");
    }

    #[test]
    fn ref_is_rejected() {
        let err = gbnf_err(json!({"$ref": "#/$defs/thing"}));
        assert!(err.contains("$ref"), "got: {err}");
    }

    #[test]
    fn free_form_object_is_rejected() {
        let err = gbnf_err(json!({"type": "object"}));
        assert!(err.contains("properties"), "got: {err}");
    }

    #[test]
    fn min_items_above_one_is_rejected() {
        let err = gbnf_err(json!({"type": "array", "items": {"type": "integer"}, "minItems": 3}));
        assert!(err.contains("minItems"), "got: {err}");
    }

    #[test]
    fn property_keys_are_escaped_in_terminals() {
        let g = gbnf(json!({
            "type": "object",
            "properties": {"weird \"key\"": {"type": "boolean"}}
        }));
        assert!(g.contains(r#""\"weird \\\"key\\\"\"" space"#), "got: {g}");
    }
}
