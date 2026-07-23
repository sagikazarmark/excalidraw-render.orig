use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;

use super::ResolveError;
use crate::{CompatibilityRevision, OutputFormat, ResourceLimits};

#[derive(Clone)]
pub(crate) struct Removal {
    pub(crate) node_ordinal: u64,
    pub(crate) attribute_ordinal: Option<u64>,
    pub(crate) name: String,
    pub(crate) changes_appearance: bool,
}

#[derive(Clone)]
pub(crate) struct Failure {
    pub(crate) path: Vec<(u64, u64)>,
    pub(crate) error: ResolveError,
}

#[derive(Clone)]
pub(crate) struct EffectCause {
    pub(crate) slots: Vec<(u64, u64)>,
    pub(crate) node_ordinal: u64,
    pub(crate) attribute: Option<(u64, String)>,
    pub(crate) effect_input: bool,
}

#[derive(Clone)]
pub(crate) struct Effect {
    pub(crate) node_ordinal: u64,
    pub(crate) filter_id: String,
    pub(crate) usage_id: String,
    pub(crate) marker: String,
    pub(crate) causes: Vec<EffectCause>,
    pub(crate) unisolatable: bool,
}

pub(super) struct SanitizedSvg {
    pub(super) bytes: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) removals: Vec<Removal>,
    pub(super) failures: Vec<Failure>,
    pub(super) effects: Vec<Effect>,
    pub(super) work_units: u64,
    pub(super) working_bytes: u64,
    pub(super) nested_asset_entries: u64,
    pub(super) nested_asset_bytes: u64,
    pub(super) nested_decoded_pixels: u64,
    pub(super) tree: Arc<usvg::Tree>,
}

pub(super) fn looks_like_svg(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<')
}

pub(super) fn sanitize(
    bytes: &[u8],
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    format: OutputFormat,
    normalized_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
) -> Result<SanitizedSvg, ResolveError> {
    sanitize_at(bytes, limits, revision, format, 0, normalized_rasters)
}

fn sanitize_at(
    bytes: &[u8],
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    format: OutputFormat,
    depth: u32,
    normalized_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
) -> Result<SanitizedSvg, ResolveError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ResolveError::SvgDecodeFailed)?;
    let (text, mut removals) = remove_dtd_and_entities(text)?;
    let mut failures = Vec::new();
    let document = roxmltree::Document::parse(&text).map_err(|_| ResolveError::SvgDecodeFailed)?;
    for instruction in document.descendants().filter_map(|node| node.pi()) {
        if instruction.target != "xml" {
            removals.push(Removal {
                node_ordinal: 0,
                attribute_ordinal: None,
                name: instruction.target.to_owned(),
                changes_appearance: true,
            });
        }
    }
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err(ResolveError::SvgDecodeFailed);
    }
    let mut effects = discover_effects(&document);
    let nodes = document
        .descendants()
        .filter(|node| node.is_element() || node.is_text())
        .count();
    if nodes > usize::try_from(limits.max_nested_svg_nodes()).unwrap_or(usize::MAX) {
        return Err(ResolveError::SvgComplexity {
            attempted: u64::try_from(nodes).unwrap_or(u64::MAX),
        });
    }
    let mut serialized = String::new();
    let mut node_ordinal = 0;
    let mut nested_asset_entries = 0;
    let mut nested_asset_bytes = 0;
    let mut nested_decoded_pixels = 0;
    serialize_node(
        root,
        true,
        &mut node_ordinal,
        &mut serialized,
        &mut removals,
        &mut failures,
        &mut effects,
        limits,
        revision,
        format,
        depth,
        normalized_rasters,
        &mut nested_asset_entries,
        &mut nested_asset_bytes,
        &mut nested_decoded_pixels,
    )?;
    let options = viewer_options()?;
    let tree =
        usvg::Tree::from_str(&serialized, &options).map_err(|_| ResolveError::SvgDecodeFailed)?;
    let size = tree.size().to_int_size();
    let width = size.width();
    let height = size.height();
    let attributes = document
        .descendants()
        .filter(|node| node.is_element())
        .map(|node| node.attributes().len())
        .sum::<usize>()
        .saturating_add(removals.len());
    let tokens = nodes.saturating_add(attributes);
    let local_references = document
        .descendants()
        .filter(|node| node.is_element())
        .flat_map(|node| node.attributes())
        .filter(|attribute| {
            matches!(attribute.name(), "href" | "xlink:href") || attribute.value().contains("url(")
        })
        .count();
    let semantic_records = u64::try_from(tokens)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(nodes).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(attributes).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(local_references).unwrap_or(u64::MAX));
    let work_units = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .saturating_add(semantic_records.saturating_mul(16));
    let working_bytes = 512_u64
        .saturating_add(u64::try_from(nodes).unwrap_or(u64::MAX).saturating_mul(128))
        .saturating_add(
            u64::try_from(
                attributes
                    .saturating_add(tokens)
                    .saturating_add(local_references),
            )
            .unwrap_or(u64::MAX)
            .saturating_mul(64),
        );
    Ok(SanitizedSvg {
        bytes: serialized.into_bytes(),
        width,
        height,
        removals,
        failures,
        effects,
        work_units,
        working_bytes,
        nested_asset_entries,
        nested_asset_bytes,
        nested_decoded_pixels,
        tree: Arc::new(tree),
    })
}

fn remove_dtd_and_entities(text: &str) -> Result<(String, Vec<Removal>), ResolveError> {
    let mut without_dtd = String::new();
    let mut removals = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = text[cursor..].find("<!DOCTYPE") {
        let start = cursor + relative;
        without_dtd.push_str(&text[cursor..start]);
        let end = doctype_end(&text[start..]).ok_or(ResolveError::SvgDecodeFailed)? + start;
        removals.push(Removal {
            node_ordinal: 0,
            attribute_ordinal: None,
            name: "DOCTYPE".to_owned(),
            changes_appearance: false,
        });
        cursor = end;
    }
    without_dtd.push_str(&text[cursor..]);

    let mut sanitized = String::new();
    let mut rest = without_dtd.as_str();
    while let Some(start) = rest.find('&') {
        sanitized.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';') else {
            sanitized.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        if matches!(entity, "amp" | "lt" | "gt" | "quot" | "apos")
            || entity.strip_prefix('#').is_some_and(valid_numeric_entity)
        {
            sanitized.push_str(&rest[..=end]);
        } else {
            removals.push(Removal {
                node_ordinal: 0,
                attribute_ordinal: None,
                name: entity.to_owned(),
                changes_appearance: true,
            });
        }
        rest = &rest[end + 1..];
    }
    sanitized.push_str(rest);
    Ok((sanitized, removals))
}

fn doctype_end(doctype: &str) -> Option<usize> {
    let mut quote = None;
    let mut subset_depth = 0_u32;
    for (index, character) in doctype.char_indices() {
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' => subset_depth = subset_depth.saturating_add(1),
            ']' => subset_depth = subset_depth.saturating_sub(1),
            '>' if subset_depth == 0 => return Some(index + 1),
            _ => {}
        }
    }
    None
}

fn valid_numeric_entity(number: &str) -> bool {
    number.strip_prefix(['x', 'X']).map_or_else(
        || !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()),
        |hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
    )
}

#[allow(clippy::too_many_arguments)]
fn serialize_node(
    node: roxmltree::Node<'_, '_>,
    is_root: bool,
    node_ordinal: &mut u64,
    output: &mut String,
    removals: &mut Vec<Removal>,
    failures: &mut Vec<Failure>,
    effects: &mut Vec<Effect>,
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    format: OutputFormat,
    depth: u32,
    normalized_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    nested_asset_entries: &mut u64,
    nested_asset_bytes: &mut u64,
    nested_decoded_pixels: &mut u64,
) -> Result<(), ResolveError> {
    let ordinal = *node_ordinal;
    *node_ordinal = node_ordinal.saturating_add(1);
    if node.is_text() {
        escape_text(output, node.text().unwrap_or_default());
        return Ok(());
    }
    if !node.is_element() {
        return Ok(());
    }
    let name = node.tag_name().name();
    let svg_namespace = node.tag_name().namespace() == Some("http://www.w3.org/2000/svg");
    if !svg_namespace || !allowed_element(name) {
        removals.push(Removal {
            node_ordinal: ordinal,
            attribute_ordinal: None,
            name: name.to_owned(),
            changes_appearance: true,
        });
        count_descendants(node, node_ordinal);
        return Ok(());
    }

    output.push('<');
    output.push_str(name);
    if is_root {
        output.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    if format == OutputFormat::Pdf
        && filter_reference(node).is_some()
        && node.attribute("id").is_none()
    {
        output.push_str(" id=\"__excalidraw_effect_");
        output.push_str(&ordinal.to_string());
        output.push('"');
    }
    if format == OutputFormat::Pdf && name.starts_with("fe") && node.attribute("result").is_none() {
        output.push_str(" result=\"__excalidraw_filter_result_");
        output.push_str(&ordinal.to_string());
        output.push('"');
    }
    for (attribute_ordinal, attribute) in node.attributes().enumerate() {
        let attribute_name = attribute.name();
        let value = attribute.value();
        if attribute_name == "filter" && filter_reference_is_noop(node, value) {
            continue;
        }
        if unsafe_attribute(name, attribute_name, attribute.namespace(), value) {
            removals.push(Removal {
                node_ordinal: ordinal,
                attribute_ordinal: Some(u64::try_from(attribute_ordinal).unwrap_or(u64::MAX)),
                name: attribute_name.to_owned(),
                changes_appearance: attribute_name != "href" && attribute_name != "xlink:href",
            });
            continue;
        }
        let nested_value = if matches!(attribute_name, "href" | "xlink:href") {
            match sanitize_nested_data_image(
                value,
                limits,
                revision,
                format,
                depth,
                ordinal,
                u64::try_from(attribute_ordinal).unwrap_or(u64::MAX),
                normalized_rasters,
                removals,
                failures,
                effects,
                nested_asset_entries,
                nested_asset_bytes,
                nested_decoded_pixels,
            ) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(Failure {
                        path: vec![(
                            ordinal,
                            u64::try_from(attribute_ordinal).unwrap_or(u64::MAX),
                        )],
                        error,
                    });
                    continue;
                }
            }
        } else if attribute_name == "style" {
            style_without_noop_filter(node, value)
        } else {
            None
        };
        let value = nested_value.as_deref().unwrap_or(value);
        output.push(' ');
        if attribute.namespace() == Some("http://www.w3.org/1999/xlink") {
            output.push_str("href");
        } else {
            output.push_str(attribute_name);
        }
        output.push_str("=\"");
        escape_attribute(output, value);
        output.push('"');
    }
    output.push('>');
    for child in node.children() {
        if child.is_element() || child.is_text() {
            serialize_node(
                child,
                false,
                node_ordinal,
                output,
                removals,
                failures,
                effects,
                limits,
                revision,
                format,
                depth,
                normalized_rasters,
                nested_asset_entries,
                nested_asset_bytes,
                nested_decoded_pixels,
            )?;
        }
    }
    output.push_str("</");
    output.push_str(name);
    output.push('>');
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn sanitize_nested_data_image(
    value: &str,
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    format: OutputFormat,
    depth: u32,
    slot_node_ordinal: u64,
    slot_attribute_ordinal: u64,
    normalized_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    removals: &mut Vec<Removal>,
    failures: &mut Vec<Failure>,
    effects: &mut Vec<Effect>,
    nested_asset_entries: &mut u64,
    nested_asset_bytes: &mut u64,
    nested_decoded_pixels: &mut u64,
) -> Result<Option<String>, ResolveError> {
    let Some(data) = value.strip_prefix("data:") else {
        return Ok(None);
    };
    let Some((claimed_media_type, encoded)) = data.split_once(";base64,") else {
        return Err(ResolveError::SvgDecodeFailed);
    };
    let (decoded_prefix, valid_base64) = super::decoded_prefix(encoded);
    *nested_asset_entries = nested_asset_entries.saturating_add(1);
    *nested_asset_bytes = nested_asset_bytes.saturating_add(decoded_prefix);
    if !valid_base64 {
        return Err(ResolveError::SvgDecodeFailed);
    }
    let attempted_depth = depth.saturating_add(1);
    if attempted_depth > limits.max_nested_asset_depth() {
        return Err(ResolveError::SvgDepth {
            attempted: u64::from(attempted_depth),
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| ResolveError::SvgDecodeFailed)?;
    let decoded_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if decoded_bytes > limits.max_asset_bytes() {
        return Err(ResolveError::AssetBytes {
            attempted: decoded_bytes,
        });
    }
    let validated_media_type = if looks_like_svg(&bytes) {
        "image/svg+xml"
    } else {
        super::sniff_media_kind(&bytes)?.media_type()
    };
    let (media_type, canonical_bytes, child_entries, child_bytes, child_pixels) =
        if looks_like_svg(&bytes) {
            let nested = sanitize_at(
                &bytes,
                limits,
                revision,
                format,
                attempted_depth,
                normalized_rasters,
            )?;
            removals.extend(nested.removals);
            for mut failure in nested.failures {
                failure
                    .path
                    .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
                failures.push(failure);
            }
            for mut effect in nested.effects {
                for cause in &mut effect.causes {
                    cause
                        .slots
                        .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
                }
                effects.push(effect);
            }
            (
                "image/svg+xml",
                nested.bytes,
                nested.nested_asset_entries,
                nested.nested_asset_bytes,
                nested.nested_decoded_pixels,
            )
        } else {
            let (kind, pixels, canonical_bytes) = if format == OutputFormat::Svg {
                let (kind, _) = super::validate_nested_raster(&bytes, limits)?;
                (kind, 0, Arc::new(bytes.clone()))
            } else {
                let kind = super::sniff_media_kind(&bytes)?;
                let identity = super::semantic_identity(revision, kind, &bytes);
                let (normalized, decoded_pixels) =
                    if let Some(normalized) = normalized_rasters.get(&identity) {
                        (Arc::clone(normalized), 0)
                    } else {
                        let normalized = Arc::new(super::normalize_nested_raster(&bytes, limits)?);
                        normalized_rasters.insert(identity, Arc::clone(&normalized));
                        let decoded_pixels = normalized.pixels;
                        (normalized, decoded_pixels)
                    };
                (
                    normalized.source_media_kind,
                    decoded_pixels,
                    Arc::new(normalized.normalized_png.clone()),
                )
            };
            let emitted_type = if format == OutputFormat::Svg {
                kind.media_type()
            } else {
                "image/png"
            };
            (emitted_type, canonical_bytes.as_ref().clone(), 0, 0, pixels)
        };
    if claimed_media_type != validated_media_type {
        removals.push(Removal {
            node_ordinal: 0,
            attribute_ordinal: None,
            name: "media-type".to_owned(),
            changes_appearance: false,
        });
    }
    *nested_asset_entries = nested_asset_entries.saturating_add(child_entries);
    *nested_asset_bytes = nested_asset_bytes.saturating_add(child_bytes);
    *nested_decoded_pixels = nested_decoded_pixels.saturating_add(child_pixels);
    let mut canonical = format!("data:{media_type};base64,");
    base64::engine::general_purpose::STANDARD.encode_string(canonical_bytes, &mut canonical);
    Ok(Some(canonical))
}

fn count_descendants(node: roxmltree::Node<'_, '_>, node_ordinal: &mut u64) {
    let descendants = node
        .descendants()
        .skip(1)
        .filter(|node| node.is_element() || node.is_text())
        .count();
    *node_ordinal = node_ordinal.saturating_add(u64::try_from(descendants).unwrap_or(u64::MAX));
}

fn allowed_element(name: &str) -> bool {
    matches!(
        name,
        "svg"
            | "g"
            | "defs"
            | "symbol"
            | "use"
            | "title"
            | "desc"
            | "metadata"
            | "path"
            | "rect"
            | "circle"
            | "ellipse"
            | "line"
            | "polyline"
            | "polygon"
            | "linearGradient"
            | "radialGradient"
            | "stop"
            | "pattern"
            | "clipPath"
            | "mask"
            | "marker"
            | "image"
            | "text"
            | "tspan"
            | "textPath"
            | "a"
            | "filter"
            | "feBlend"
            | "feColorMatrix"
            | "feComponentTransfer"
            | "feFuncR"
            | "feFuncG"
            | "feFuncB"
            | "feFuncA"
            | "feComposite"
            | "feConvolveMatrix"
            | "feDiffuseLighting"
            | "feDisplacementMap"
            | "feDistantLight"
            | "feDropShadow"
            | "feFlood"
            | "feGaussianBlur"
            | "feImage"
            | "feMerge"
            | "feMergeNode"
            | "feMorphology"
            | "feOffset"
            | "fePointLight"
            | "feSpecularLighting"
            | "feSpotLight"
            | "feTile"
            | "feTurbulence"
    )
}

fn unsafe_attribute(element_name: &str, name: &str, namespace: Option<&str>, value: &str) -> bool {
    if name.len() >= 2 && name[..2].eq_ignore_ascii_case("on") {
        return true;
    }
    if namespace.is_some_and(|namespace| {
        namespace != "http://www.w3.org/1999/xlink"
            && namespace != "http://www.w3.org/XML/1998/namespace"
    }) {
        return true;
    }
    if matches!(
        name,
        "href" | "xlink:href" | "src" | "action" | "formaction"
    ) {
        return !safe_href(element_name, value);
    }
    let lowercase = value.to_ascii_lowercase();
    lowercase.contains("@import")
        || lowercase.contains("javascript:")
        || lowercase.contains("expression(")
        || lowercase.contains("-moz-binding")
        || lowercase.contains("behavior:")
        || contains_nonlocal_url(&lowercase)
}

fn discover_effects(document: &roxmltree::Document<'_>) -> Vec<Effect> {
    let mut effects = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        let Some((attribute_ordinal, attribute_name, filter_value)) = filter_reference(node) else {
            continue;
        };
        let Some(filter_id) = local_url_target(&filter_value) else {
            continue;
        };
        let Some(filter) = referenced_filter(node, &filter_value) else {
            continue;
        };
        let primitives = filter
            .children()
            .filter(|child| child.is_element() && child.tag_name().name().starts_with("fe"))
            .collect::<Vec<_>>();
        if effect_fully_clipped(node)
            || primitives.is_empty()
            || primitives
                .iter()
                .all(|primitive| primitive_is_noop(*primitive))
        {
            continue;
        }
        let node_ordinal = source_node_ordinal(document, node);
        let mut causes = Vec::new();
        causes.push(EffectCause {
            slots: Vec::new(),
            node_ordinal,
            attribute: Some((
                u64::try_from(attribute_ordinal).unwrap_or(u64::MAX),
                attribute_name,
            )),
            effect_input: false,
        });
        for primitive in &primitives {
            let primitive_ordinal = source_node_ordinal(document, *primitive);
            causes.push(EffectCause {
                slots: Vec::new(),
                node_ordinal: primitive_ordinal,
                attribute: None,
                effect_input: false,
            });
            for (attribute_ordinal, attribute) in primitive.attributes().enumerate() {
                if matches!(attribute.name(), "in" | "in2") {
                    causes.push(EffectCause {
                        slots: Vec::new(),
                        node_ordinal: primitive_ordinal,
                        attribute: Some((
                            u64::try_from(attribute_ordinal).unwrap_or(u64::MAX),
                            attribute.name().to_owned(),
                        )),
                        effect_input: true,
                    });
                }
            }
        }
        let unisolatable = primitives.iter().any(|primitive| {
            primitive.attributes().any(|attribute| {
                matches!(attribute.name(), "in" | "in2")
                    && matches!(attribute.value(), "BackgroundImage" | "BackgroundAlpha")
            })
        });
        effects.push(Effect {
            node_ordinal,
            filter_id: filter_id.to_owned(),
            usage_id: node
                .attribute("id")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("__excalidraw_effect_{node_ordinal}")),
            marker: primitives[0]
                .attribute("result")
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "__excalidraw_filter_result_{}",
                        source_node_ordinal(document, primitives[0])
                    )
                }),
            causes,
            unisolatable,
        });
    }
    effects
}

fn filter_reference(node: roxmltree::Node<'_, '_>) -> Option<(usize, String, String)> {
    for (attribute_ordinal, attribute) in node.attributes().enumerate() {
        if attribute.name() == "filter" {
            return Some((
                attribute_ordinal,
                "filter".to_owned(),
                attribute.value().to_owned(),
            ));
        }
        if attribute.name() == "style" {
            for declaration in attribute.value().split(';') {
                let Some((name, value)) = declaration.split_once(':') else {
                    continue;
                };
                if name.trim().eq_ignore_ascii_case("filter") {
                    return Some((
                        attribute_ordinal,
                        "style".to_owned(),
                        value.trim().to_owned(),
                    ));
                }
            }
        }
    }
    None
}

fn local_url_target(value: &str) -> Option<&str> {
    value
        .trim()
        .strip_prefix("url(")?
        .strip_suffix(')')?
        .trim()
        .trim_matches(['\'', '"'])
        .strip_prefix('#')
}

fn filter_reference_is_noop(node: roxmltree::Node<'_, '_>, value: &str) -> bool {
    effect_fully_clipped(node) || filter_value_is_noop(node, value)
}

fn filter_value_is_noop(node: roxmltree::Node<'_, '_>, value: &str) -> bool {
    let Some(filter) = referenced_filter(node, value) else {
        return false;
    };
    let primitives = filter
        .children()
        .filter(|child| child.is_element() && child.tag_name().name().starts_with("fe"))
        .collect::<Vec<_>>();
    primitives.is_empty()
        || primitives
            .iter()
            .all(|primitive| primitive_is_noop(*primitive))
}

fn referenced_filter<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    value: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    let filter_id = local_url_target(value)?;
    node.document().descendants().find(|candidate| {
        candidate.is_element()
            && candidate.tag_name().name() == "filter"
            && candidate.attribute("id") == Some(filter_id)
    })
}

fn style_without_noop_filter(node: roxmltree::Node<'_, '_>, style: &str) -> Option<String> {
    let mut removed = false;
    let mut retained = Vec::new();
    for declaration in style.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        if let Some((name, value)) = declaration.split_once(':') {
            if name.trim().eq_ignore_ascii_case("filter")
                && filter_reference_is_noop(node, value.trim())
            {
                removed = true;
                continue;
            }
        }
        retained.push(declaration);
    }
    removed.then(|| retained.join(";"))
}

fn effect_fully_clipped(node: roxmltree::Node<'_, '_>) -> bool {
    let Some(clip_id) = node.attribute("clip-path").and_then(local_url_target) else {
        return false;
    };
    let Some(subject) = source_shape_bounds(node) else {
        return false;
    };
    let Some(clip) = node.document().descendants().find(|candidate| {
        candidate.is_element()
            && candidate.tag_name().name() == "clipPath"
            && candidate.attribute("id") == Some(clip_id)
    }) else {
        return false;
    };
    let clip = clip
        .descendants()
        .filter(|candidate| candidate.is_element())
        .filter_map(source_shape_bounds)
        .reduce(|left, right| {
            (
                left.0.min(right.0),
                left.1.min(right.1),
                left.2.max(right.2),
                left.3.max(right.3),
            )
        });
    clip.is_none_or(|clip| {
        subject.2 <= clip.0 || subject.0 >= clip.2 || subject.3 <= clip.1 || subject.1 >= clip.3
    })
}

fn source_shape_bounds(node: roxmltree::Node<'_, '_>) -> Option<(f64, f64, f64, f64)> {
    let number = |name: &str, default: f64| {
        node.attribute(name)
            .map_or(Some(default), |value| value.parse::<f64>().ok())
    };
    match node.tag_name().name() {
        "rect" | "image" => {
            let x = number("x", 0.0)?;
            let y = number("y", 0.0)?;
            let width = number("width", 0.0)?.max(0.0);
            let height = number("height", 0.0)?.max(0.0);
            Some((x, y, x + width, y + height))
        }
        "circle" => {
            let cx = number("cx", 0.0)?;
            let cy = number("cy", 0.0)?;
            let radius = number("r", 0.0)?.max(0.0);
            Some((cx - radius, cy - radius, cx + radius, cy + radius))
        }
        "ellipse" => {
            let cx = number("cx", 0.0)?;
            let cy = number("cy", 0.0)?;
            let radius_x = number("rx", 0.0)?.max(0.0);
            let radius_y = number("ry", 0.0)?.max(0.0);
            Some((cx - radius_x, cy - radius_y, cx + radius_x, cy + radius_y))
        }
        _ => None,
    }
}

fn source_node_ordinal(document: &roxmltree::Document<'_>, target: roxmltree::Node<'_, '_>) -> u64 {
    document
        .descendants()
        .filter(|node| node.is_element() || node.is_text())
        .position(|node| node == target)
        .and_then(|ordinal| u64::try_from(ordinal).ok())
        .unwrap_or(u64::MAX)
}

fn primitive_is_noop(node: roxmltree::Node<'_, '_>) -> bool {
    match node.tag_name().name() {
        "feGaussianBlur" => number_pair_is_zero(node.attribute("stdDeviation").unwrap_or("0")),
        "feOffset" => {
            number_is_zero(node.attribute("dx").unwrap_or("0"))
                && number_is_zero(node.attribute("dy").unwrap_or("0"))
        }
        "feColorMatrix" => color_matrix_is_identity(node),
        "feComponentTransfer" => node
            .children()
            .filter(|child| child.is_element() && child.tag_name().name().starts_with("feFunc"))
            .all(|function| function.attribute("type").unwrap_or("identity") == "identity"),
        _ => false,
    }
}

fn color_matrix_is_identity(node: roxmltree::Node<'_, '_>) -> bool {
    match node.attribute("type").unwrap_or("matrix") {
        "saturate" => node
            .attribute("values")
            .is_none_or(|value| value.trim() == "1"),
        "hueRotate" => node.attribute("values").is_none_or(number_is_zero),
        "matrix" => {
            let identity = [
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ];
            node.attribute("values").is_none_or(|values| {
                values
                    .split(|character: char| character.is_ascii_whitespace() || character == ',')
                    .filter(|value| !value.is_empty())
                    .map(str::parse::<f64>)
                    .collect::<Result<Vec<_>, _>>()
                    .is_ok_and(|values| values == identity)
            })
        }
        "luminanceToAlpha" => false,
        _ => false,
    }
}

fn number_pair_is_zero(value: &str) -> bool {
    let mut values = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty());
    values.next().is_none_or(number_is_zero) && values.next().is_none_or(number_is_zero)
}

fn number_is_zero(value: &str) -> bool {
    value.parse::<f64>().is_ok_and(|value| value == 0.0)
}

fn safe_href(element_name: &str, value: &str) -> bool {
    let value = value.trim();
    value.starts_with('#')
        || matches!(element_name, "image" | "feImage")
            && value.starts_with("data:image/")
            && value.contains(";base64,")
}

fn contains_nonlocal_url(value: &str) -> bool {
    let mut rest = value;
    while let Some(index) = rest.find("url(") {
        rest = &rest[index + 4..];
        let Some(end) = rest.find(')') else {
            return true;
        };
        let target = rest[..end].trim().trim_matches(['\'', '"']);
        if !target.starts_with('#') {
            return true;
        }
        rest = &rest[end + 1..];
    }
    false
}

fn escape_text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

fn escape_attribute(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '"' => output.push_str("&quot;"),
            '\r' => output.push_str("&#13;"),
            '\n' => output.push_str("&#10;"),
            '\t' => output.push_str("&#9;"),
            _ => output.push(character),
        }
    }
}

fn viewer_options() -> Result<usvg::Options<'static>, ResolveError> {
    let font = excalidraw_render_assets_base::load_excalifont_basic_latin_sfnt()
        .map_err(|_| ResolveError::SvgDecodeFailed)?;
    let mut options = usvg::Options {
        resources_dir: None,
        font_family: "Excalifont".to_owned(),
        ..usvg::Options::default()
    };
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(font.bytes().to_vec());
    options.fontdb = Arc::new(fontdb);
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(|media_type, bytes, options| match media_type {
            "image/png" => Some(usvg::ImageKind::PNG(bytes)),
            "image/svg+xml" => usvg::Tree::from_data(&bytes, options)
                .ok()
                .map(usvg::ImageKind::SVG),
            _ => None,
        }),
        resolve_string: Box::new(|_, _| None),
    };
    Ok(options)
}
