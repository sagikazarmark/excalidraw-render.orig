//! Structural inventories matching tools/pdf-harness/structure/inspect.py.

use super::*;
use serde_json::{json, Value as V};

const MAX_INVENTORY_DEPTH: usize = 128;

pub(super) fn page_inventory(objects: &V, resources: &V, operations: &[V], page: &V) -> Result<V> {
    let (fonts, mappings) = font_inventory(objects, resources, operations)?;
    Ok(json!({
        "fonts": fonts,
        "mappings": mappings,
        "images": image_inventory(objects, resources, operations)?,
        "transparency": transparency_inventory(objects, resources)?,
        "forms": form_inventory(objects, resources, operations)?,
        "annotations": annotation_inventory(objects, page),
    }))
}

pub(super) fn document_inventory(source: &V, objects: &V, catalog: &V) -> Result<V> {
    Ok(json!({
        "actions": action_inventory(objects, catalog, list(&source["pages"]))?,
        "output_intents": output_intent_inventory(objects, catalog)?,
        "attachments": attachment_inventory(source, objects)?,
        "signatures": signature_inventory(source, objects),
    }))
}

pub(super) fn font_inventory(
    objects: &V,
    resources: &V,
    operations: &[V],
) -> Result<(Vec<V>, Vec<V>)> {
    let mut result = Vec::new();
    let mut mappings = Vec::new();
    for (scope, scoped_resources) in resource_scopes(objects, resources)? {
        let fonts = resolve_value(objects, &scoped_resources["/Font"]);
        for (resource, font_ref) in pairs(&fonts) {
            let full_resource = format!("{scope}{resource}");
            let font = resolve(objects, font_ref);
            let descendant = resolve(
                objects,
                list(&font["/DescendantFonts"]).first().unwrap_or(&V::Null),
            );
            let descriptor = resolve(
                objects,
                &get(
                    &descendant,
                    "/FontDescriptor",
                    font["/FontDescriptor"].clone(),
                ),
            );
            let embedded_ref = ["/FontFile", "/FontFile2", "/FontFile3"]
                .iter()
                .find_map(|name| descriptor.get(*name))
                .unwrap_or(&V::Null);
            let embedded = stream_value(objects, embedded_ref)?;
            let to_unicode = stream_value(objects, &font["/ToUnicode"])?;
            let unicode_descriptor = stream_descriptor(to_unicode.as_deref());
            result.push(json!({
                "base_font": normalize(&font["/BaseFont"]),
                "descendant_subtype": normalize(&descendant["/Subtype"]),
                "embedded": embedded.is_some(),
                "embedded_sha256": embedded.as_deref().map(sha256),
                "encoding": normalize(&font["/Encoding"]),
                "object": reference(font_ref),
                "resource": full_resource,
                "subtype": normalize(&font["/Subtype"]),
                "to_unicode": to_unicode.is_some(),
                "to_unicode_byte_length": unicode_descriptor["byte_length"],
                "to_unicode_sha256": unicode_descriptor["sha256"],
            }));
            mappings.push(json!({
                "byte_length": unicode_descriptor["byte_length"],
                "kind": "to-unicode",
                "operands": [],
                "resource": full_resource,
                "scope": scope,
                "sequence": null,
                "sha256": unicode_descriptor["sha256"],
            }));
        }
    }
    for operation in operations {
        if operation["operator"] == "BDC"
            && list(&operation["operands"])
                .iter()
                .any(|operand| operand == "/ActualText")
        {
            mappings.push(json!({
                "byte_length": null,
                "kind": "actual-text",
                "operands": operation["operands"],
                "resource": null,
                "scope": operation["scope"],
                "sequence": operation["sequence"],
                "sha256": null,
            }));
        }
    }
    Ok((result, mappings))
}

fn occurrences(operations: &[V], operator: &str, scope: &str, resource: &str) -> usize {
    operations
        .iter()
        .filter(|operation| {
            operation["operator"] == operator
                && operation["scope"] == scope
                && list(&operation["operands"])
                    .last()
                    .is_some_and(|operand| operand == resource)
        })
        .count()
}

pub(super) fn image_inventory(objects: &V, resources: &V, operations: &[V]) -> Result<Vec<V>> {
    let mut result = Vec::new();
    for (scope, scoped_resources) in resource_scopes(objects, resources)? {
        let xobjects = resolve_value(objects, &scoped_resources["/XObject"]);
        for (resource, image_ref) in pairs(&xobjects) {
            let image = stream_dict(objects, image_ref);
            if image["/Subtype"] != "/Image" {
                continue;
            }
            append_image(
                objects,
                image_ref,
                &format!("{scope}{resource}"),
                occurrences(operations, "Do", &scope, resource),
                &mut result,
                std::slice::from_ref(image_ref),
            )?;
        }
    }
    for (index, operation) in operations.iter().enumerate() {
        if operation["operator"] != "ID" {
            continue;
        }
        let inline_dict = inline_dictionary(list(&operation["operands"]));
        let inline_data = operations
            .get(index + 1)
            .filter(|next| next["operator"] == "EI")
            .and_then(|next| {
                list(&next["operands"])
                    .iter()
                    .filter_map(V::as_str)
                    .find(|operand| operand.starts_with("inline:"))
            });
        let (mut data_length, mut data_sha256) = (V::Null, V::Null);
        if let Some(data) = inline_data {
            let mut parts = data.splitn(3, ':');
            parts.next();
            data_length = integer_value(&json!(parts.next()));
            data_sha256 = parts.next().map_or(V::Null, |hash| json!(hash));
            if data_length.is_null() || data_sha256.is_null() {
                return Err(format!("invalid inline image descriptor: {data}"));
            }
        }
        let field = |short: &str, long: &str| get(&inline_dict, short, inline_dict[long].clone());
        let scope = operation["scope"].as_str().unwrap_or("");
        let sequence = &operation["sequence"];
        result.push(json!({
            "bits_per_component": integer_value(&field("/BPC", "/BitsPerComponent")),
            "classification": "undeclared",
            "color_space": field("/CS", "/ColorSpace"),
            "data_sha256": data_sha256,
            "filters": field("/F", "/Filter"),
            "height": integer_value(&field("/H", "/Height")),
            "interpolate": field("/I", "/Interpolate") == "true",
            "mask": field("/IM", "/ImageMask"),
            "object": null,
            "occurrences": 1,
            "resource": format!("{scope}/inline-image-{sequence}"),
            "soft_mask": null,
            "width": integer_value(&field("/W", "/Width")),
            "byte_length": data_length,
        }));
    }
    Ok(result)
}

pub(super) fn append_image(
    objects: &V,
    image_ref: &V,
    resource: &str,
    occurrences: usize,
    result: &mut Vec<V>,
    mask_stack: &[V],
) -> Result<()> {
    check_depth(mask_stack.len(), "image masks")?;
    let image = stream_dict(objects, image_ref);
    let image_data = stream_value(objects, image_ref)?;
    let descriptor = stream_descriptor(image_data.as_deref());
    result.push(json!({
        "bits_per_component": image["/BitsPerComponent"],
        "byte_length": descriptor["byte_length"],
        "classification": "undeclared",
        "color_space": normalize(&image["/ColorSpace"]),
        "data_sha256": descriptor["sha256"],
        "filters": normalize(&image["/Filter"]),
        "height": image["/Height"],
        "interpolate": get(&image, "/Interpolate", json!(false)),
        "mask": normalize(&image["/Mask"]),
        "object": reference(image_ref),
        "occurrences": occurrences,
        "resource": resource,
        "soft_mask": reference(&image["/SMask"]),
        "width": image["/Width"],
    }));
    for name in ["Mask", "SMask"] {
        let mask_ref = &image[format!("/{name}")];
        if !mask_ref.is_string() || mask_stack.contains(mask_ref) {
            continue;
        }
        let mask = stream_dict(objects, mask_ref);
        if mask["/Subtype"] != "/Image" {
            continue;
        }
        let mut stack = mask_stack.to_vec();
        stack.push(mask_ref.clone());
        append_image(
            objects,
            mask_ref,
            &format!("{resource}/{name}"),
            occurrences,
            result,
            &stack,
        )?;
    }
    Ok(())
}

pub(super) fn inline_dictionary(operands: &[V]) -> V {
    let mut result = json!({});
    let mut index = 0;
    while index + 1 < operands.len() {
        if let Some(key) = operands[index].as_str().filter(|key| key.starts_with('/')) {
            result[key] = operands[index + 1].clone();
            index += 2;
        } else {
            index += 1;
        }
    }
    result
}

// Inline operands are strings. Python's int accepts signs, leading zeroes and
// underscores, but rejects a decimal point; retain arbitrary-size JSON integers.
fn integer_value(value: &V) -> V {
    let Some(text) = value.as_str() else {
        return V::Null;
    };
    let text = text.trim();
    let negative = text.starts_with('-');
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty()
        || digits
            .split('_')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return V::Null;
    }
    let digits = digits.replace('_', "");
    let digits = digits.trim_start_matches('0');
    let number = if digits.is_empty() {
        "0".to_owned()
    } else if negative {
        format!("-{digits}")
    } else {
        digits.to_owned()
    };
    number
        .parse::<serde_json::Number>()
        .map_or(V::Null, V::Number)
}

pub(super) fn transparency_inventory(objects: &V, resources: &V) -> Result<Vec<V>> {
    let mut result = Vec::new();
    for (scope, scoped_resources) in resource_scopes(objects, resources)? {
        let states = resolve_value(objects, &scoped_resources["/ExtGState"]);
        for (resource, state_ref) in pairs(&states) {
            let state = resolve(objects, state_ref);
            let soft_mask = &state["/SMask"];
            result.push(json!({
                "alpha_is_shape": get(&state, "/AIS", json!(false)),
                "blend_mode": normalize(&get(&state, "/BM", json!("/Normal"))),
                "fill_alpha": get(&state, "/ca", json!(1)),
                "object": reference(state_ref),
                "resource": format!("{scope}{resource}"),
                "soft_mask": if soft_mask.is_null() || soft_mask == "/None" {
                    V::Null
                } else {
                    normalize(&resolve_value(objects, soft_mask))
                },
                "stroke_alpha": get(&state, "/CA", json!(1)),
            }));
        }
    }
    Ok(result)
}

pub(super) fn form_inventory(objects: &V, resources: &V, operations: &[V]) -> Result<Vec<V>> {
    let mut result = Vec::new();
    for (scope, scoped_resources) in resource_scopes(objects, resources)? {
        let xobjects = resolve_value(objects, &scoped_resources["/XObject"]);
        for (resource, form_ref) in pairs(&xobjects) {
            let form = stream_dict(objects, form_ref);
            if form["/Subtype"] != "/Form" {
                continue;
            }
            let group = resolve_value(objects, &form["/Group"]);
            result.push(form_record(
                &form,
                form_ref,
                &format!("{scope}{resource}"),
                occurrences(operations, "Do", &scope, resource),
                &group,
            ));
        }
        let states = resolve_value(objects, &scoped_resources["/ExtGState"]);
        for (resource, state_ref) in pairs(&states) {
            let state = resolve(objects, state_ref);
            let soft_mask = resolve_value(objects, &state["/SMask"]);
            let form_ref = &soft_mask["/G"];
            let form = stream_dict(objects, form_ref);
            if form["/Subtype"] != "/Form" {
                continue;
            }
            let group = resolve_value(objects, &form["/Group"]);
            result.push(form_record(
                &form,
                form_ref,
                &format!("{scope}{resource}/SMask"),
                occurrences(operations, "gs", &scope, resource),
                &group,
            ));
        }
    }
    Ok(result)
}

pub(super) fn form_record(
    form: &V,
    form_ref: &V,
    resource: &str,
    occurrences: usize,
    group: &V,
) -> V {
    json!({
        "bbox": normalize(&form["/BBox"]),
        "group": if group.is_object() {
            json!({
                "color_space": normalize(&group["/CS"]),
                "isolated": get(group, "/I", json!(false)),
                "knockout": get(group, "/K", json!(false)),
                "subtype": normalize(&group["/S"]),
            })
        } else { V::Null },
        "matrix": normalize(&get(form, "/Matrix", json!([1, 0, 0, 1, 0, 0]))),
        "object": reference(form_ref),
        "occurrences": occurrences,
        "resource": resource,
    })
}

pub(super) fn resource_scopes(objects: &V, resources: &V) -> Result<Vec<(String, V)>> {
    fn visit(
        objects: &V,
        scope: String,
        current: &V,
        stack: &mut Vec<V>,
        result: &mut Vec<(String, V)>,
    ) -> Result<()> {
        check_depth(stack.len(), "resource scopes")?;
        result.push((scope.clone(), current.clone()));
        // Preserve Python's depth-first ordering: XObjects, then soft-mask forms.
        for (key, suffix) in [("/XObject", ""), ("/ExtGState", "/SMask")] {
            let entries = resolve_value(objects, &current[key]);
            for (resource, entry) in pairs(&entries) {
                let form_ref = if key == "/XObject" {
                    entry.clone()
                } else {
                    let state = resolve(objects, entry);
                    let mask = resolve_value(objects, &state["/SMask"]);
                    mask["/G"].clone()
                };
                let form = stream_dict(objects, &form_ref);
                if form["/Subtype"] != "/Form" || stack.contains(&form_ref) {
                    continue;
                }
                let nested = resolve_value(objects, &get(&form, "/Resources", json!({})));
                if nested.is_object() {
                    stack.push(form_ref);
                    visit(
                        objects,
                        format!("{scope}{resource}{suffix}"),
                        &nested,
                        stack,
                        result,
                    )?;
                    stack.pop();
                }
            }
        }
        Ok(())
    }
    let mut result = Vec::new();
    visit(
        objects,
        "page".to_owned(),
        resources,
        &mut Vec::new(),
        &mut result,
    )?;
    Ok(result)
}

fn check_depth(depth: usize, inventory: &str) -> Result<()> {
    if depth > MAX_INVENTORY_DEPTH {
        Err(format!(
            "{inventory} exceed inventory depth limit ({MAX_INVENTORY_DEPTH})"
        ))
    } else {
        Ok(())
    }
}

pub(super) fn annotation_inventory(objects: &V, page: &V) -> Vec<V> {
    list(&page["/Annots"])
        .iter()
        .map(|annotation_ref| {
            let annotation = resolve(objects, annotation_ref);
            let action = resolve_value(objects, &annotation["/A"]);
            json!({
                "action": if is_truthy(&action) {
                    json!({
                        "kind": normalize(&action["/S"]),
                        "uri": normalize(&action["/URI"]),
                    })
                } else { V::Null },
                "additional_actions": normalize(&resolve_value(objects, &annotation["/AA"])),
                "border": normalize(&annotation["/Border"]),
                "flags": get(&annotation, "/F", json!(0)),
                "object": reference(annotation_ref),
                "quad_points": normalize(&annotation["/QuadPoints"]),
                "rect": normalize(&annotation["/Rect"]),
                "subtype": normalize(&annotation["/Subtype"]),
            })
        })
        .collect()
}

fn is_truthy(value: &V) -> bool {
    match value {
        V::Null => false,
        V::Bool(value) => *value,
        V::Number(value) => value.as_f64() != Some(0.0),
        V::String(value) => !value.is_empty(),
        V::Array(value) => !value.is_empty(),
        V::Object(value) => !value.is_empty(),
    }
}

pub(super) fn action_inventory(objects: &V, catalog: &V, page_records: &[V]) -> Result<Vec<V>> {
    let mut result = Vec::new();
    walk_action(
        objects,
        &catalog["/OpenAction"],
        "catalog/OpenAction",
        &[],
        &mut result,
        0,
    )?;
    walk_additional_actions(objects, &catalog["/AA"], "catalog/AA", &mut result)?;
    for (page_index, page_record) in page_records.iter().enumerate() {
        let page = resolve(objects, &page_record["object"]);
        walk_additional_actions(
            objects,
            &page["/AA"],
            &format!("page[{page_index}]/AA"),
            &mut result,
        )?;
        for (annotation_index, annotation_ref) in list(&page["/Annots"]).iter().enumerate() {
            let annotation = resolve(objects, annotation_ref);
            let location = format!("page[{page_index}]/annotation[{annotation_index}]");
            walk_action(
                objects,
                &annotation["/A"],
                &format!("{location}/A"),
                &[],
                &mut result,
                0,
            )?;
            walk_additional_actions(
                objects,
                &annotation["/AA"],
                &format!("{location}/AA"),
                &mut result,
            )?;
        }
    }
    Ok(result)
}

pub(super) fn walk_additional_actions(
    objects: &V,
    value: &V,
    location: &str,
    result: &mut Vec<V>,
) -> Result<()> {
    let actions = resolve_value(objects, value);
    for (event, action) in pairs(&actions) {
        walk_action(
            objects,
            action,
            &format!("{location}{event}"),
            &[],
            result,
            0,
        )?;
    }
    Ok(())
}

pub(super) fn walk_action(
    objects: &V,
    value: &V,
    location: &str,
    seen: &[V],
    result: &mut Vec<V>,
    depth: usize,
) -> Result<()> {
    check_depth(depth, "actions")?;
    if let Some(actions) = value.as_array() {
        for (index, action) in actions.iter().enumerate() {
            walk_action(
                objects,
                action,
                &format!("{location}[{index}]"),
                seen,
                result,
                depth + 1,
            )?;
        }
        return Ok(());
    }
    let action_ref = reference(value);
    let mut seen = seen.to_vec();
    if !action_ref.is_null() {
        if seen.contains(&action_ref) {
            result.push(json!({
                "classification": "prohibited",
                "details": {},
                "kind": "cycle",
                "location": location,
                "object": action_ref,
            }));
            return Ok(());
        }
        seen.push(action_ref.clone());
    }
    let action = resolve_value(objects, value);
    if !action.is_object() || action.get("/S").is_none() {
        return Ok(());
    }
    let kind = normalize(&action["/S"]);
    let uri = normalize(&action["/URI"]);
    let scheme = uri.as_str().map(uri_scheme).transpose()?;
    let safe_uri =
        kind == "/URI" && matches!(scheme.as_deref(), Some("" | "http" | "https" | "mailto"));
    let details: serde_json::Map<String, V> = pairs(&action)
        .into_iter()
        .filter(|(key, _)| !matches!(*key, "/S" | "/Next"))
        .map(|(key, value)| (key.chars().skip(1).collect(), value.clone()))
        .collect();
    result.push(json!({
        "classification": if safe_uri { "uri" } else { "prohibited" },
        "details": normalize(&V::Object(details)),
        "kind": kind,
        "location": location,
        "object": action_ref,
    }));
    walk_action(
        objects,
        &action["/Next"],
        &format!("{location}/Next"),
        &seen,
        result,
        depth + 1,
    )
}

// urllib.parse.urlsplit strips leading C0 controls/spaces and removes ASCII
// tab/newline/carriage-return before recognizing an ASCII URL scheme.
fn uri_scheme(uri: &str) -> Result<String> {
    let uri: String = uri
        .trim_start_matches(|character| character <= '\u{20}')
        .chars()
        .filter(|character| !matches!(character, '\t' | '\n' | '\r'))
        .collect();
    let (scheme, remainder) = uri
        .split_once(':')
        .filter(|(scheme, _)| {
            scheme.starts_with(|character: char| character.is_ascii_alphabetic())
                && scheme
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte))
        })
        .unwrap_or(("", &uri));
    if let Some(authority) = remainder.strip_prefix("//") {
        let authority = authority.split(['/', '?', '#']).next().unwrap_or("");
        validate_uri_authority(authority)?;
    }
    Ok(scheme.to_ascii_lowercase())
}

// urlsplit validates bracketed hosts even when only its scheme is requested.
// Preserve that failure boundary rather than classifying malformed URLs as URI
// actions. Ports are deliberately not parsed: urlsplit does not inspect them.
fn validate_uri_authority(authority: &str) -> Result<()> {
    if authority.contains('[') != authority.contains(']') {
        return Err("Invalid IPv6 URL".into());
    }
    if authority.contains('[') {
        let host_port = authority.rsplit('@').next().unwrap_or("");
        let host = if let Some((before, bracketed)) = host_port.split_once('[') {
            let (host, after) = bracketed.split_once(']').ok_or("Invalid IPv6 URL")?;
            if !before.is_empty() || (!after.is_empty() && !after.starts_with(':')) {
                return Err("Invalid IPv6 URL".into());
            }
            host
        } else {
            host_port.split(':').next().unwrap_or("")
        };
        if let Some(future) = host.strip_prefix('v') {
            let valid = future.split_once('.').is_some_and(|(version, address)| {
                !version.is_empty()
                    && version.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && !address.is_empty()
            });
            if !valid {
                return Err("IPvFuture address is invalid".into());
            }
        } else {
            // Python accepts IPv6 zone IDs; std's address parser excludes them.
            let address = match host.split_once('%') {
                Some((address, zone)) if !zone.is_empty() && !zone.contains('%') => address,
                Some(_) => return Err("Invalid IPv6 URL".into()),
                None => host,
            };
            if address.parse::<std::net::Ipv6Addr>().is_err() {
                return Err("Invalid IPv6 URL".into());
            }
        }
    }
    // Complete non-ASCII compatibility decompositions containing / ? # @ :.
    // These stable Unicode mappings implement urlsplit's narrow NFKC delimiter
    // check without normalizing (and thereby changing) the reported URI.
    if authority.chars().any(|ch| {
        matches!(
            ch,
            '\u{2047}'..='\u{2049}'
                | '\u{2100}'..='\u{2101}'
                | '\u{2105}'..='\u{2106}'
                | '\u{2a74}'
                | '\u{fe13}'
                | '\u{fe16}'
                | '\u{fe55}'..='\u{fe56}'
                | '\u{fe5f}'
                | '\u{fe6b}'
                | '\u{ff03}'
                | '\u{ff0f}'
                | '\u{ff1a}'
                | '\u{ff1f}'..='\u{ff20}'
        )
    }) {
        return Err("URI authority contains invalid characters under NFKC normalization".into());
    }
    Ok(())
}

pub(super) fn output_intent_inventory(objects: &V, catalog: &V) -> Result<Vec<V>> {
    let mut result = Vec::new();
    for intent_ref in list(&catalog["/OutputIntents"]) {
        let intent = resolve(objects, intent_ref);
        let profile_ref = &intent["/DestOutputProfile"];
        let profile = stream_value(objects, profile_ref)?;
        result.push(json!({
            "condition_identifier": normalize(&intent["/OutputConditionIdentifier"]),
            "info": normalize(&intent["/Info"]),
            "object": reference(intent_ref),
            "profile": icc_descriptor(profile.as_deref()),
            "profile_object": reference(profile_ref),
            "subtype": normalize(&intent["/S"]),
        }));
    }
    Ok(result)
}

pub(super) fn attachment_inventory(source: &V, objects: &V) -> Result<V> {
    let mut result = json!({});
    for (name, attachment) in pairs(&source["attachments"]) {
        let mut record = normalize(attachment);
        let data = stream_value(objects, &attachment["preferredcontents"])?;
        let descriptor = stream_descriptor(data.as_deref());
        if !record.is_object() {
            return Err(format!("attachment {name} is not an object"));
        }
        record["byte_length"] = descriptor["byte_length"].clone();
        record["sha256"] = descriptor["sha256"].clone();
        result[name] = record;
    }
    Ok(result)
}

pub(super) fn icc_descriptor(data: Option<&[u8]>) -> V {
    let Some(data) = data else {
        return V::Null;
    };
    let mut descriptor = stream_descriptor(Some(data));
    if data.len() < 132 {
        descriptor["valid_header"] = json!(false);
        return descriptor;
    }
    let count = u32::from_be_bytes(data[128..132].try_into().unwrap()) as usize;
    let tags: Vec<String> = data[132..]
        .as_chunks::<12>()
        .0
        .iter()
        .take(count)
        .map(|entry| ascii_replacement(&entry[..4]))
        .collect();
    let rstrip =
        |character: char| character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character);
    descriptor["color_space"] = json!(ascii_replacement(&data[16..20]).trim_end_matches(rstrip));
    descriptor["device_class"] = json!(ascii_replacement(&data[12..16]));
    descriptor["pcs"] = json!(ascii_replacement(&data[20..24]).trim_end_matches(rstrip));
    descriptor["signature"] = json!(ascii_replacement(&data[36..40]));
    descriptor["tags"] = json!(tags);
    descriptor["valid_header"] =
        json!(u32::from_be_bytes(data[..4].try_into().unwrap()) as usize == data.len());
    descriptor["version"] = json!(format!("{}.{}", data[8], data[9] >> 4));
    descriptor
}

fn ascii_replacement(data: &[u8]) -> String {
    data.iter()
        .map(|&byte| {
            if byte.is_ascii() {
                char::from(byte)
            } else {
                '\u{fffd}'
            }
        })
        .collect()
}

pub(super) fn signature_inventory(source: &V, objects: &V) -> Vec<V> {
    let mut result = Vec::new();
    for field in list(&source["acroform"]["fields"]) {
        if field["fieldtype"] == "/Sig" {
            result.push(normalize(field));
        }
    }
    for (object_name, record) in pairs(objects) {
        // object_value's precedence includes explicit null "value" entries.
        let value = record.get("value").cloned().unwrap_or_else(|| {
            if let Some(stream) = record.get("stream") {
                get(stream, "dict", json!({}))
            } else {
                record.clone()
            }
        });
        if let Some(byte_range) = value.as_object().and_then(|value| value.get("/ByteRange")) {
            result.push(json!({
                "byte_range": normalize(byte_range),
                "object": object_name.chars().skip(4).collect::<String>(),
            }));
        }
    }
    result
}
