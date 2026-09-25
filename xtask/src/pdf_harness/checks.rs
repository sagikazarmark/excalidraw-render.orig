use super::*;
use crate::representative::json::{compare_numbers, decode, number};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::cmp::Ordering;
use std::fmt;

/// Validated before inspecting a PDF, including fields whose checks are
/// conditional on pages or output intents being present.
pub(super) struct Expectation {
    pub(super) value: Value,
    operation_order: Vec<String>,
}

impl Expectation {
    pub(super) fn load(bytes: &[u8]) -> Result<Self> {
        let value = decode(bytes).map_err(|e| format!("invalid expectation: {e}"))?;
        validate(&value)?;
        // Value's sorted maps are important to report hashes. Preserve only the
        // check-producing object's input order, without global preserve_order.
        #[derive(Deserialize)]
        struct Order {
            #[serde(deserialize_with = "operation_order")]
            operation_minimums: Vec<String>,
        }
        let order: Order = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        Ok(Self {
            value,
            operation_order: order.operation_minimums,
        })
    }
}

fn operation_order<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error> {
    struct Keys;
    impl<'de> Visitor<'de> for Keys {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an operation minimum object")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut keys = Vec::new();
            while let Some((key, _)) = map.next_entry::<String, &RawValue>()? {
                keys.push(key);
            }
            Ok(keys)
        }
    }
    deserializer.deserialize_map(Keys)
}

fn field<'a>(
    value: &'a Value,
    key: &str,
    valid: impl FnOnce(&Value) -> bool,
    kind: &str,
) -> Result<&'a Value> {
    let field = value
        .get(key)
        .ok_or_else(|| format!("invalid expectation: missing {key}"))?;
    if !valid(field) {
        return Err(format!("invalid expectation: {key} must be {kind}"));
    }
    Ok(field)
}

fn nonnegative(value: &Value) -> bool {
    value
        .as_number()
        .is_some_and(|n| compare_numbers(n, &0.into()) != Ordering::Less)
}

fn validate(value: &Value) -> Result<()> {
    if !value.is_object() {
        return Err("invalid expectation: expected an object".into());
    }
    for key in ["pdf_version", "operation_sha256", "inventory_sha256"] {
        field(value, key, Value::is_string, "a string")?;
    }
    for key in [
        "page_count",
        "font_count",
        "form_count",
        "mapping_count",
        "annotation_count",
        "clip_count",
        "mask_count",
        "image_count",
        "operation_count",
        "attachment_count",
        "output_intent_count",
    ] {
        field(
            value,
            key,
            |v| {
                nonnegative(v)
                    && v.as_number().is_some_and(|n| {
                        !n.is_f64() || n.as_f64().is_some_and(|f| f.fract() == 0.0)
                    })
            },
            "a nonnegative integer or whole-valued float",
        )?;
    }
    for key in ["media_box", "crop_box"] {
        field(
            value,
            key,
            |v| {
                v.as_array()
                    .is_some_and(|a| a.len() == 4 && a.iter().all(Value::is_number))
            },
            "an array of four finite numbers",
        )?;
    }
    field(value, "xmp_present", Value::is_boolean, "a boolean")?;
    for key in ["action_kinds", "required_icc_tags"] {
        field(
            value,
            key,
            |v| v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
            "an array of strings",
        )?;
    }
    let images = field(value, "images", Value::is_array, "an array")?;
    for image in images.as_array().expect("validated images") {
        for key in ["resource", "classification"] {
            field(
                image,
                key,
                Value::is_string,
                "a string in each images entry",
            )?;
        }
    }
    let minimums = field(value, "operation_minimums", Value::is_object, "an object")?;
    for (operator, minimum) in minimums.as_object().expect("validated minimums") {
        if !nonnegative(minimum) {
            return Err(format!("invalid expectation: operation_minimums.{operator} must be a nonnegative finite number"));
        }
    }
    Ok(())
}

fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => compare_numbers(a, b) == Ordering::Equal,
        // Python treats bool as an int in report equality. Expectation fields
        // still enforce their declared types at the input boundary.
        (Value::Bool(a), Value::Number(b)) | (Value::Number(b), Value::Bool(a)) => {
            compare_numbers(&u8::from(*a).into(), b) == Ordering::Equal
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| equal(a, b)))
        }
        _ => a == b,
    }
}
pub(super) fn evaluate(report: &Value, loaded: &Expectation) -> Result<Value> {
    let expectation = &loaded.value;
    let first = &report["pages"][0];
    let document = &report["document"];
    let mut checks = Vec::new();
    let mut check = |name: &str, actual: Value, expected: Value| {
        checks.push(json!({"actual":actual,"expected":expected,"id":name,"passed":equal(&actual,&expected)}));
    };
    check(
        "parser-warnings",
        report["parser"]["warnings"].clone(),
        json!([]),
    );
    check("parser-errors", report["errors"].clone(), json!([]));
    for (id, key) in [("pdf-version", "pdf_version"), ("page-count", "page_count")] {
        check(id, document[key].clone(), expectation[key].clone());
    }
    if !list(&report["pages"]).is_empty() {
        for (id, key) in [("media-box", "media_box"), ("crop-box", "crop_box")] {
            check(id, first[key].clone(), expectation[key].clone());
        }
        for (id, key, value) in [
            ("other-page-boxes", "other_boxes", json!({})),
            ("rotation", "rotate", json!(0)),
            ("user-unit", "user_unit", json!(1)),
        ] {
            check(id, first[key].clone(), value);
        }
        for (id, key, e) in [
            ("font-count", "fonts", "font_count"),
            ("form-count", "forms", "form_count"),
            ("mapping-count", "mappings", "mapping_count"),
            ("annotation-count", "annotations", "annotation_count"),
            ("clip-count", "clips", "clip_count"),
            ("mask-count", "masks", "mask_count"),
            ("image-count", "images", "image_count"),
        ] {
            check(id, json!(list(&first[key]).len()), expectation[e].clone());
        }
        check(
            "image-inventory",
            json!(list(&first["images"])
                .iter()
                .map(|i| json!({"classification":i["classification"],"resource":i["resource"]}))
                .collect::<Vec<_>>()),
            expectation["images"].clone(),
        );
        check(
            "operation-count",
            json!(list(&first["operations"]).len()),
            expectation["operation_count"].clone(),
        );
        check(
            "operation-sequence-sha256",
            json!(sha256(&canonical(&first["operations"])?)),
            expectation["operation_sha256"].clone(),
        );
        let page = Value::Object(
            pairs(first)
                .into_iter()
                .filter(|(k, _)| {
                    !matches!(*k, "operations" | "text_operations" | "vector_operations")
                })
                .map(|(k, v)| (k.to_owned(), v.clone()))
                .collect(),
        );
        check(
            "structural-inventory-sha256",
            json!(sha256(&canonical(
                &json!({"document":document,"page":page})
            )?)),
            expectation["inventory_sha256"].clone(),
        );
    }
    // End the closure borrow so minimum checks preserve the protocol order.
    if !list(&report["pages"]).is_empty() {
        for operator in &loaded.operation_order {
            let minimum = &expectation["operation_minimums"][operator];
            let actual = list(&first["operations"])
                .iter()
                .filter(|o| o["operator"] == operator.as_str())
                .count();
            let passed = compare_numbers(
                &actual.into(),
                minimum.as_number().expect("validated minimum"),
            ) != Ordering::Less;
            checks.push(json!({"actual":actual,"expected":format!(">={}",number(minimum)?),"id":format!("operation-{operator}"),"passed":passed}));
        }
    }
    let mut check = |name: &str, actual: Value, expected: Value| {
        checks.push(json!({"actual":actual,"expected":expected,"id":name,"passed":equal(&actual,&expected)}));
    };
    check("encrypted", document["encrypted"].clone(), json!(false));
    check(
        "action-kinds",
        json!(list(&document["actions"])
            .iter()
            .map(|a| &a["kind"])
            .collect::<Vec<_>>()),
        expectation["action_kinds"].clone(),
    );
    check(
        "prohibited-actions",
        json!(list(&document["actions"])
            .iter()
            .filter(|a| a["classification"] == "prohibited")
            .map(|a| &a["location"])
            .collect::<Vec<_>>()),
        json!([]),
    );
    check(
        "signature-count",
        json!(list(&document["signatures"]).len()),
        json!(0),
    );
    for (id, key, value) in [
        ("incremental-revisions", "incremental_revisions", json!(0)),
        ("linearized", "linearized", json!(false)),
        ("info-dictionary", "info_dictionary", Value::Null),
    ] {
        check(id, document[key].clone(), value);
    }
    check(
        "attachment-count",
        json!(pairs(&document["attachments"]).len()),
        expectation["attachment_count"].clone(),
    );
    check(
        "xmp-present",
        json!(!document["xmp"].is_null()),
        expectation["xmp_present"].clone(),
    );
    let intents = list(&document["output_intents"]);
    check(
        "output-intent-count",
        json!(intents.len()),
        expectation["output_intent_count"].clone(),
    );
    if let Some(intent) = intents.first() {
        let profile = &intent["profile"];
        for (id, key, value) in [
            ("output-profile-version", "version", json!("2.1")),
            ("output-profile-class", "device_class", json!("mntr")),
            ("output-profile-space", "color_space", json!("RGB")),
            ("output-profile-pcs", "pcs", json!("XYZ")),
            ("output-profile-signature", "signature", json!("acsp")),
            ("output-profile-header", "valid_header", json!(true)),
        ] {
            check(id, profile[key].clone(), value);
        }
        let missing = list(&expectation["required_icc_tags"])
            .iter()
            .filter(|t| !list(&profile["tags"]).contains(t))
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        check("output-profile-tags", json!(missing), json!([]));
    }
    Ok(json!(checks))
}
