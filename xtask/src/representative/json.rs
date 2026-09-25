//! Strict JSON ingestion and Python-compatible tool-owned JSON. Source scene
//! bytes are always retained directly, including number and escape spelling.
use super::*;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{value::RawValue, Number};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

// Never deserialize an input object as Number/Value: arbitrary_precision's
// private Number sentinel is a legal user key, not a trusted numeric wrapper.
struct Container(usize);
impl<'de> Visitor<'de> for Container {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON with unique members and finite numbers")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            values.push(decode_raw(raw, self.0 + 1).map_err(de::Error::custom)?);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some((key, raw)) = map.next_entry::<String, &RawValue>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON member: {key}")));
            }
            values.insert(key, decode_raw(raw, self.0 + 1).map_err(de::Error::custom)?);
        }
        Ok(Value::Object(values))
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Value> {
    let raw =
        serde_json::from_slice::<&RawValue>(bytes).map_err(|e| format!("invalid JSON: {e}"))?;
    decode_raw(raw, 0).map_err(|e| format!("invalid JSON: {e}"))
}

fn decode_raw(raw: &RawValue, depth: usize) -> Result<Value> {
    require(depth < 128, "JSON nesting limit exceeded")?;
    let token = raw.get();
    let mut deserializer = serde_json::Deserializer::from_str(token);
    match token.as_bytes()[0] {
        b'{' => deserializer.deserialize_map(Container(depth)),
        b'[' => deserializer.deserialize_seq(Container(depth)),
        b'-' | b'0'..=b'9' => {
            let number = if token.contains(['.', 'e', 'E']) {
                // Python parses decimal/exponent tokens as binary64, including
                // signed underflow; integer tokens never pass through a float.
                let float = token.parse::<f64>().map_err(|e| e.to_string())?;
                Number::from_f64(float).ok_or("non-finite JSON number")?
            } else {
                Number::from_str(if token == "-0" { "0" } else { token })
                    .map_err(|e| e.to_string())?
            };
            return Ok(Value::Number(number));
        }
        _ => serde_json::from_str(token),
    }
    .map_err(|e| e.to_string())
}

// Compare Python int/float values without rounding an integer to binary64.
// Fixed precision zero expands the float's *exact* truncated integer, rather
// than its shortest round-trip decimal spelling (which can be a different int).
pub(crate) fn compare_numbers(a: &Number, b: &Number) -> Ordering {
    fn parts(n: &Number) -> (String, f64) {
        if let Some(f) = n.as_f64().filter(|_| n.is_f64()) {
            let integer = if f.trunc() == 0.0 {
                "0".into()
            } else {
                format!("{:.0}", f.trunc())
            };
            (integer, f.fract())
        } else {
            (n.to_string(), 0.0)
        }
    }
    let (a, af) = parts(a);
    let (b, bf) = parts(b);
    let negative_a = a.starts_with('-');
    let negative_b = b.starts_with('-');
    let integers = match (negative_a, negative_b) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => {
            let magnitude = a.len().cmp(&b.len()).then_with(|| a.cmp(&b));
            if negative_a {
                magnitude.reverse()
            } else {
                magnitude
            }
        }
    };
    integers.then_with(|| af.partial_cmp(&bf).unwrap_or(Ordering::Equal))
}

pub(crate) fn number(value: &Value) -> Result<String> {
    let n = value.as_number().ok_or("expected number")?;
    if !n.is_f64() {
        return Ok(n.to_string());
    }
    python_float(n.as_f64().ok_or("invalid float")?)
}

/// Python's `repr(float)`: shortest round-trip digits, positional for
/// decimal exponents -4..16, otherwise `d.ddde±XX`.
pub(crate) fn python_float(f: f64) -> Result<String> {
    if !f.is_finite() {
        return Err("non-finite JSON number".into());
    }
    if f == 0.0 {
        return Ok(if f.is_sign_negative() { "-0.0" } else { "0.0" }.into());
    }
    let formatted = format!("{:e}", f.abs());
    let (mantissa, exponent) = formatted.split_once('e').ok_or("float format")?;
    let exp: i32 = exponent.parse().map_err(|_| "float exponent")?;
    let digits = mantissa.replace('.', "");
    let n = digits.len() as i32;
    let body = if (-4..16).contains(&exp) {
        if exp < 0 {
            format!("0.{}{digits}", "0".repeat((-exp - 1) as usize))
        } else if n <= exp + 1 {
            format!("{digits}{}.0", "0".repeat((exp + 1 - n) as usize))
        } else {
            let (a, b) = digits.split_at((exp + 1) as usize);
            format!("{a}.{b}")
        }
    } else {
        let (first, rest) = digits.split_at(1);
        let m = if rest.is_empty() {
            first.to_owned()
        } else {
            format!("{first}.{rest}")
        };
        format!("{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    };
    Ok(format!("{}{body}", if f < 0.0 { "-" } else { "" }))
}

pub(super) fn encode(value: &Value) -> Result<Vec<u8>> {
    fn pretty(v: &Value, depth: usize, out: &mut String) -> Result<()> {
        let indent = |n| "  ".repeat(n);
        match v {
            Value::Array(a) if !a.is_empty() => {
                out.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    out.push_str(&indent(depth + 1));
                    pretty(v, depth + 1, out)?;
                    out.push_str(if i + 1 == a.len() { "\n" } else { ",\n" });
                }
                out.push_str(&indent(depth));
                out.push(']');
            }
            Value::Object(o) if !o.is_empty() => {
                out.push_str("{\n");
                for (i, (key, v)) in o.iter().enumerate() {
                    out.push_str(&indent(depth + 1));
                    out.push_str(&serde_json::to_string(key).map_err(|e| e.to_string())?);
                    out.push_str(": ");
                    pretty(v, depth + 1, out)?;
                    out.push_str(if i + 1 == o.len() { "\n" } else { ",\n" });
                }
                out.push_str(&indent(depth));
                out.push('}');
            }
            Value::Number(_) => out.push_str(&number(v)?),
            _ => out.push_str(&serde_json::to_string(v).map_err(|e| e.to_string())?),
        }
        Ok(())
    }
    let mut out = String::new();
    pretty(value, 0, &mut out)?;
    out.push('\n');
    Ok(out.into_bytes())
}

/// Python's sorted compact JSON used by legacy representative case subjects.
pub(crate) fn compact(value: &Value) -> Result<Vec<u8>> {
    let mut out = String::new();
    compact_into(value, &mut out)?;
    Ok(out.into_bytes())
}

/// Append `json.dumps(value, sort_keys=True, separators=(",", ":"),
/// ensure_ascii=False)`, preserving arbitrary-size integer tokens. Shared with
/// coverage so both paths compute the same representative case subjects.
pub(crate) fn compact_into(value: &Value, out: &mut String) -> Result<()> {
    match value {
        Value::Number(_) => out.push_str(&number(value)?),
        Value::Array(values) => {
            out.push('[');
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                compact_into(value, out)?;
            }
            out.push(']');
        }
        Value::Object(values) => {
            out.push('{');
            let mut entries: Vec<_> = values.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).map_err(|e| e.to_string())?);
                out.push(':');
                compact_into(value, out)?;
            }
            out.push('}');
        }
        _ => out.push_str(&serde_json::to_string(value).map_err(|e| e.to_string())?),
    }
    Ok(())
}

// Input has already passed strict decoding. Stream offsets let us insert at
// object boundaries without reserializing unrelated fields or numeric tokens.
pub(super) fn element_offsets(data: &[u8]) -> Result<Vec<usize>> {
    fn whitespace(data: &[u8], pos: &mut usize) {
        while data.get(*pos).is_some_and(u8::is_ascii_whitespace) {
            *pos += 1;
        }
    }
    fn value(data: &[u8], pos: &mut usize) -> Result<Value> {
        let mut stream =
            serde_json::Deserializer::from_slice(&data[*pos..]).into_iter::<&RawValue>();
        let value = stream
            .next()
            .ok_or("missing JSON value")?
            .map_err(|e| e.to_string())?;
        *pos += stream.byte_offset();
        decode_raw(value, 0)
    }
    let mut pos = 0;
    whitespace(data, &mut pos);
    require(data.get(pos) == Some(&b'{'), "scene is not a JSON object")?;
    pos += 1;
    loop {
        whitespace(data, &mut pos);
        let key = value(data, &mut pos)?;
        whitespace(data, &mut pos);
        require(data.get(pos) == Some(&b':'), "missing colon")?;
        pos += 1;
        whitespace(data, &mut pos);
        if key == "elements" {
            require(data.get(pos) == Some(&b'['), "elements is not an array")?;
            pos += 1;
            let mut offsets = Vec::new();
            loop {
                whitespace(data, &mut pos);
                if data.get(pos) == Some(&b']') {
                    return Ok(offsets);
                }
                require(data.get(pos) == Some(&b'{'), "element is not an object")?;
                offsets.push(pos);
                value(data, &mut pos)?;
                whitespace(data, &mut pos);
                if data.get(pos) == Some(&b',') {
                    pos += 1;
                }
            }
        }
        value(data, &mut pos)?;
        whitespace(data, &mut pos);
        require(data.get(pos) == Some(&b','), "scene has no elements array")?;
        pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_preserves_python_integer_and_float_semantics() {
        let huge = "9".repeat(400);
        let input = format!(
            r#"{{"z":[18446744073709551617,-18446744073709551617,{huge},-{huge},-0,-0.0,1e-999,-1e-999,5e-324,1e16],"a":"é"}}"#
        );
        // CPython json.loads followed by sorted, compact, ensure_ascii=False
        // json.dumps: integers never round through binary64, floats do.
        let expected = format!(
            r#"{{"a":"é","z":[18446744073709551617,-18446744073709551617,{huge},-{huge},0,-0.0,0.0,-0.0,5e-324,1e+16]}}"#
        );
        let value = decode(input.as_bytes()).unwrap();
        assert_eq!(compact(&value).unwrap(), expected.as_bytes());
        let mut appended = String::from("prefix:");
        compact_into(&value, &mut appended).unwrap();
        assert_eq!(appended, format!("prefix:{expected}"));
        for non_finite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                python_float(non_finite).unwrap_err(),
                "non-finite JSON number"
            );
        }
    }
}
