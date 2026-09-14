use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;

use super::{IntrinsicTransaction, ResolutionFailure, ResolveError};
use crate::{CompatibilityRevision, OutputFormat, RenderError, ResourceLimits};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Removal {
    pub(crate) slots: Vec<(u64, u64)>,
    pub(crate) node_ordinal: u64,
    pub(crate) attribute_ordinal: Option<u64>,
    pub(crate) name: String,
    pub(crate) changes_appearance: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failure {
    pub(crate) path: Vec<(u64, u64)>,
    pub(crate) error: ResolveError,
    pub(crate) placeholder: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaClaim {
    pub(crate) path: Vec<(u64, u64)>,
    pub(crate) claimed: String,
    pub(crate) validated: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectCause {
    pub(crate) slots: Vec<(u64, u64)>,
    pub(crate) node_ordinal: u64,
    pub(crate) attribute: Option<(u64, String)>,
    pub(crate) effect_input: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Effect {
    pub(crate) node_ordinal: u64,
    pub(crate) filter_id: String,
    pub(crate) usage_id: String,
    pub(crate) marker: String,
    pub(crate) causes: Vec<EffectCause>,
    pub(crate) unisolatable: bool,
}

#[derive(Debug)]
pub(super) struct SanitizedSvg {
    pub(super) bytes: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) removals: Vec<Removal>,
    pub(super) failures: Vec<Failure>,
    pub(super) media_claims: Vec<MediaClaim>,
    pub(super) effects: Vec<Effect>,
    pub(super) work_units: u64,
    pub(super) working_bytes: u64,
    pub(super) nested_admissions: Vec<NestedAdmission>,
    pub(super) tree: Arc<usvg::Tree>,
}

#[derive(Clone, Debug)]
pub(super) enum NestedAdmission {
    Recovery {
        identity: [u8; 32],
        retained_bytes: u64,
    },
    Occurrence {
        asset_bytes: u64,
    },
    DecodedPixels {
        decoded_pixels: u64,
    },
    Work {
        identity: [u8; 32],
        total_work_units: u64,
    },
}

#[derive(Default)]
struct NestedAccounting {
    admissions: Vec<NestedAdmission>,
    sanitized_svgs: BTreeMap<[u8; 32], Arc<SanitizedSvg>>,
    asset_entries: u64,
    asset_bytes: u64,
    decoded_pixels: u64,
    total_work_units: u64,
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
    shared_rasters: &BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    pending_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
) -> Result<SanitizedSvg, ResolutionFailure> {
    let mut nested_accounting = NestedAccounting::default();
    let mut sanitized = sanitize_at(
        bytes,
        limits,
        revision,
        format,
        0,
        shared_rasters,
        pending_rasters,
        transact,
        &mut nested_accounting,
    )?;
    sanitized.nested_admissions = nested_accounting.admissions;
    Ok(sanitized)
}

#[allow(clippy::too_many_arguments)]
fn sanitize_at(
    bytes: &[u8],
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    format: OutputFormat,
    depth: u32,
    shared_rasters: &BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    pending_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
    nested_accounting: &mut NestedAccounting,
) -> Result<SanitizedSvg, ResolutionFailure> {
    let text = std::str::from_utf8(bytes).map_err(|_| ResolveError::SvgDecodeFailed)?;
    let (text, mut removals) = remove_dtd_and_entities(text)?;
    let mut failures = Vec::new();
    let mut media_claims = Vec::new();
    let document = roxmltree::Document::parse(&text).map_err(|_| ResolveError::SvgDecodeFailed)?;
    for instruction in document.descendants().filter_map(|node| node.pi()) {
        if instruction.target != "xml" {
            removals.push(Removal {
                slots: Vec::new(),
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
        return Err(ResolveError::SvgDecodeFailed.into());
    }
    let mut effects = discover_effects(&document);
    let nodes = document
        .descendants()
        .filter(|node| node.is_element() || node.is_text())
        .count();
    if nodes > usize::try_from(limits.max_nested_svg_nodes()).unwrap_or(usize::MAX) {
        return Err(ResolveError::SvgComplexity {
            attempted: u64::try_from(nodes).unwrap_or(u64::MAX),
        }
        .into());
    }
    let attributes = document
        .descendants()
        .filter(|node| node.is_element())
        .map(|node| node.attributes().len())
        .sum::<usize>()
        .saturating_add(removals.len())
        .saturating_add(local_removal_count(&document));
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
    let source_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let work_units = source_len.saturating_add(semantic_records.saturating_mul(16));
    if work_units > limits.max_asset_work_units() {
        if depth != 0 {
            transact(IntrinsicTransaction::PreflightNested {
                asset_entries: nested_accounting.asset_entries,
                asset_bytes: nested_accounting.asset_bytes,
                decoded_pixels: nested_accounting.decoded_pixels,
                total_work_units: nested_accounting
                    .total_work_units
                    .saturating_add(nested_accounting.asset_bytes)
                    .saturating_add(work_units),
            })
            .map_err(ResolutionFailure::Transaction)?;
        }
        let identity = super::semantic_identity(revision, super::MediaKind::Svg, bytes);
        transact(IntrinsicTransaction::RejectLocalWork {
            event: "derived_sanitized_svg",
            identity,
            total_work_units: if depth == 0 {
                work_units.saturating_sub(source_len)
            } else {
                work_units
            },
        })
        .map_err(ResolutionFailure::Transaction)?;
        return Err(ResolveError::AssetWork {
            attempted: work_units,
        }
        .into());
    }
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
    let mut serialized = String::new();
    let mut node_ordinal = 0;
    serialize_node(
        root,
        true,
        &mut node_ordinal,
        &mut serialized,
        &mut removals,
        &mut failures,
        &mut media_claims,
        &mut effects,
        limits,
        revision,
        format,
        depth,
        shared_rasters,
        pending_rasters,
        transact,
        nested_accounting,
    )?;
    let options = viewer_options()?;
    let tree =
        usvg::Tree::from_str(&serialized, &options).map_err(|_| ResolveError::SvgDecodeFailed)?;
    let size = tree.size().to_int_size();
    let width = size.width();
    let height = size.height();
    Ok(SanitizedSvg {
        bytes: serialized.into_bytes(),
        width,
        height,
        removals,
        failures,
        media_claims,
        effects,
        work_units,
        working_bytes,
        nested_admissions: Vec::new(),
        tree: Arc::new(tree),
    })
}

fn local_removal_count(document: &roxmltree::Document<'_>) -> usize {
    document
        .descendants()
        .filter(|node| node.is_element())
        .filter(|node| {
            !node
                .ancestors()
                .skip(1)
                .filter(|ancestor| ancestor.is_element())
                .any(|ancestor| !is_allowed_svg_node(&ancestor))
        })
        .map(|node| {
            let name = node.tag_name().name();
            if !is_allowed_svg_node(&node) {
                return 1;
            }
            node.attributes()
                .filter(|attribute| {
                    unsafe_attribute(
                        name,
                        attribute.name(),
                        attribute.namespace(),
                        attribute.value(),
                    )
                })
                .count()
        })
        .sum()
}

fn is_allowed_svg_node(node: &roxmltree::Node<'_, '_>) -> bool {
    node.tag_name().namespace() == Some("http://www.w3.org/2000/svg")
        && allowed_element(node.tag_name().name())
        && (node.tag_name().name() != "style" || safe_static_style_node(*node))
}

fn safe_static_style_node(node: roxmltree::Node<'_, '_>) -> bool {
    let mut text = None;
    for child in node.children() {
        if !child.is_text() || text.is_some() {
            return false;
        }
        text = child.text();
    }
    text.is_none_or(safe_static_stylesheet)
}

fn safe_static_stylesheet(stylesheet: &str) -> bool {
    if stylesheet.contains(['@', '\\'])
        || stylesheet.contains("/*")
        || stylesheet.contains("*/")
        || unsafe_attribute("style", "style", None, stylesheet)
    {
        return false;
    }

    let mut rest = stylesheet.trim();
    while !rest.is_empty() {
        let Some((selector, after_selector)) = rest.split_once('{') else {
            return false;
        };
        let Some((declarations, after_declarations)) = after_selector.split_once('}') else {
            return false;
        };
        if selector.trim().is_empty() || selector.contains([':', '}']) || declarations.contains('{')
        {
            return false;
        }
        for declaration in declarations.split(';') {
            let declaration = declaration.trim();
            if declaration.is_empty() {
                continue;
            }
            let Some((property, value)) = declaration.split_once(':') else {
                return false;
            };
            let property = property.trim();
            if property.is_empty()
                || !property
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                || !allowed_static_style_property(property)
                || value.trim().is_empty()
            {
                return false;
            }
        }
        rest = after_declarations.trim();
    }
    true
}

fn allowed_static_style_property(property: &str) -> bool {
    matches!(
        property.to_ascii_lowercase().as_str(),
        "alignment-baseline"
            | "baseline-shift"
            | "clip-path"
            | "clip-rule"
            | "color"
            | "color-interpolation"
            | "color-interpolation-filters"
            | "direction"
            | "display"
            | "dominant-baseline"
            | "fill"
            | "fill-opacity"
            | "fill-rule"
            | "filter"
            | "flood-color"
            | "flood-opacity"
            | "font"
            | "font-family"
            | "font-size"
            | "font-size-adjust"
            | "font-stretch"
            | "font-style"
            | "font-variant"
            | "font-weight"
            | "image-rendering"
            | "isolation"
            | "letter-spacing"
            | "lighting-color"
            | "marker"
            | "marker-end"
            | "marker-mid"
            | "marker-start"
            | "mask"
            | "mix-blend-mode"
            | "opacity"
            | "overflow"
            | "paint-order"
            | "shape-rendering"
            | "stop-color"
            | "stop-opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-opacity"
            | "stroke-width"
            | "text-anchor"
            | "text-decoration"
            | "text-rendering"
            | "unicode-bidi"
            | "vector-effect"
            | "visibility"
            | "word-spacing"
            | "writing-mode"
    )
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
            slots: Vec::new(),
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
                slots: Vec::new(),
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
    media_claims: &mut Vec<MediaClaim>,
    effects: &mut Vec<Effect>,
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    format: OutputFormat,
    depth: u32,
    shared_rasters: &BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    pending_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
    nested_accounting: &mut NestedAccounting,
) -> Result<(), ResolutionFailure> {
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
    if !is_allowed_svg_node(&node) {
        removals.push(Removal {
            slots: Vec::new(),
            node_ordinal: ordinal,
            attribute_ordinal: None,
            name: name.to_owned(),
            changes_appearance: name != "script",
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
                slots: Vec::new(),
                node_ordinal: ordinal,
                attribute_ordinal: Some(u64::try_from(attribute_ordinal).unwrap_or(u64::MAX)),
                name: attribute_name.to_owned(),
                changes_appearance: !attribute_name.to_ascii_lowercase().starts_with("on")
                    && (!matches!(attribute_name, "href" | "xlink:href") || name != "a"),
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
                shared_rasters,
                pending_rasters,
                removals,
                failures,
                media_claims,
                effects,
                transact,
                nested_accounting,
            ) {
                Ok(value) => value,
                Err(ResolutionFailure::Asset(error)) => {
                    let resource_limit = matches!(
                        error,
                        ResolveError::AssetBytes { .. }
                            | ResolveError::AssetWork { .. }
                            | ResolveError::Dimensions { .. }
                            | ResolveError::Pixels { .. }
                            | ResolveError::SvgComplexity { .. }
                            | ResolveError::SvgDepth { .. }
                    );
                    let replacement = if resource_limit && name == "image" {
                        let replacement =
                            nested_limit_placeholder(node, limits, revision, ordinal, transact)?;
                        if let Some(url) = &replacement {
                            nested_accounting
                                .admissions
                                .try_reserve(1)
                                .map_err(|_| ResolutionFailure::Memory)?;
                            nested_accounting
                                .admissions
                                .push(NestedAdmission::Recovery {
                                    identity: super::semantic_identity(
                                        revision,
                                        super::MediaKind::Svg,
                                        &ordinal.to_le_bytes(),
                                    ),
                                    retained_bytes: 256 + (url.len() as u64).div_ceil(64) * 64,
                                });
                        }
                        replacement
                    } else {
                        None
                    };
                    failures.push(Failure {
                        path: vec![(
                            ordinal,
                            u64::try_from(attribute_ordinal).unwrap_or(u64::MAX),
                        )],
                        error,
                        placeholder: replacement.is_some(),
                    });
                    if replacement.is_none() {
                        continue;
                    }
                    replacement
                }
                Err(ResolutionFailure::Transaction(error)) => {
                    return Err(ResolutionFailure::Transaction(error));
                }
                Err(ResolutionFailure::Memory) => return Err(ResolutionFailure::Memory),
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
                media_claims,
                effects,
                limits,
                revision,
                format,
                depth,
                shared_rasters,
                pending_rasters,
                transact,
                nested_accounting,
            )?;
        }
    }
    output.push_str("</");
    output.push_str(name);
    output.push('>');
    Ok(())
}

/// A stopped nested image retains only its own established finite viewport.
/// The data SVG contains no resources, so recovery cannot recurse again.
fn nested_limit_placeholder(
    node: roxmltree::Node<'_, '_>,
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    ordinal: u64,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
) -> Result<Option<String>, ResolutionFailure> {
    fn length(value: &str, basis: Option<f64>) -> Option<f64> {
        let value = value.trim();
        let number = if let Some(percent) = value.strip_suffix('%') {
            percent.parse::<f64>().ok()? * basis? / 100.0
        } else {
            let (raw, multiplier) = [
                ("px", 1.0),
                ("in", 96.0),
                ("cm", 96.0 / 2.54),
                ("mm", 96.0 / 25.4),
                ("pt", 96.0 / 72.0),
                ("pc", 16.0),
            ]
            .into_iter()
            .find_map(|(unit, factor)| value.strip_suffix(unit).map(|raw| (raw, factor)))
            .unwrap_or((value, 1.0));
            raw.parse::<f64>().ok()? * multiplier
        };
        (number.is_finite() && number.abs() <= f64::from(f32::MAX)).then_some(number)
    }
    let mut ancestors = Vec::new();
    for ancestor in node.ancestors().skip(1).filter(|n| n.has_tag_name("svg")) {
        ancestors
            .try_reserve(1)
            .map_err(|_| ResolutionFailure::Memory)?;
        ancestors.push(ancestor);
    }
    let mut viewport = None;
    for ancestor in ancestors.into_iter().rev() {
        let parent = viewport;
        viewport = None;
        if let Some(view_box) = ancestor.attribute("viewBox") {
            let mut values = view_box
                .split(|c: char| c.is_ascii_whitespace() || c == ',')
                .filter(|s| !s.is_empty());
            let _ = values.next();
            let _ = values.next();
            viewport = values
                .next()
                .and_then(|v| length(v, None))
                .zip(values.next().and_then(|v| length(v, None)));
        }
        if viewport.is_none() {
            viewport = Some(ancestor.attribute("width").unwrap_or("100%"))
                .and_then(|v| length(v, parent.map(|v: (f64, f64)| v.0)))
                .zip(
                    Some(ancestor.attribute("height").unwrap_or("100%"))
                        .and_then(|v| length(v, parent.map(|v| v.1))),
                );
        }
    }
    let x = length(node.attribute("x").unwrap_or("0"), viewport.map(|v| v.0));
    let y = length(node.attribute("y").unwrap_or("0"), viewport.map(|v| v.1));
    let width = node
        .attribute("width")
        .and_then(|v| length(v, viewport.map(|v| v.0)));
    let height = node
        .attribute("height")
        .and_then(|v| length(v, viewport.map(|v| v.1)));
    let (Some(x), Some(y), Some(width), Some(height)) = (x, y, width, height) else {
        return Ok(None);
    };
    if width <= 0.0
        || height <= 0.0
        || [x, y, width, height, x + width, y + height]
            .iter()
            .any(|v| v.abs() > limits.max_coordinate_magnitude())
    {
        return Ok(None);
    }
    resource_placeholder_url(width, height, limits, revision, ordinal, transact)
}

pub(super) fn resource_placeholder_url(
    width: f64,
    height: f64,
    limits: &ResourceLimits,
    revision: CompatibilityRevision,
    ordinal: u64,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
) -> Result<Option<String>, ResolutionFailure> {
    use std::fmt::Write;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > limits.max_coordinate_magnitude()
        || height > limits.max_coordinate_magnitude()
        || width as f32 <= 0.0
        || height as f32 <= 0.0
        || !(width as f32).is_finite()
        || !(height as f32).is_finite()
    {
        return Ok(None);
    }
    // The manifested two-glyph fallback has a fixed bounded construction
    // envelope, shared by no other rejected occurrence.
    const WORK: u64 = 8192;
    if WORK > limits.max_asset_work_units() {
        return Ok(None);
    }
    let identity =
        super::semantic_identity(revision, super::MediaKind::Svg, &ordinal.to_le_bytes());
    transact(IntrinsicTransaction::Acquire {
        event: "nested_resource_placeholder",
        identity,
        persistent_working_bytes: 0,
        temporary_working_bytes: 8192,
        decoded_pixels: 0,
        total_work_units: WORK,
    })
    .map_err(ResolutionFailure::Transaction)?;
    let size = width.min(height).min((width.min(height) * 0.4).min(100.0));
    // Exact official image-error recipe; transforms are preserved by the
    // enclosing source image, and its existing viewport clips this content.
    let mut svg = String::new();
    svg.try_reserve_exact(8192)
        .map_err(|_| ResolutionFailure::Memory)?;
    write!(svg,r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><rect width="{width}" height="{height}" fill="#E7E7E7"/><svg x="{}" y="{}" width="{size}" height="{size}" viewBox="0 0 668 668"><path fill="#888" transform="matrix(0.81709 0 0 0.81709 124.825 145.825)" d="M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48ZM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56ZM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48Z"/><path fill="#888" transform="matrix(0.30366 0 0 0.30366 506.822 60.065)" d="M256 8C119.034 8 8 119.033 8 256c0 136.967 111.034 248 248 248s248-111.034 248-248S392.967 8 256 8Zm130.108 117.892c65.448 65.448 70 165.481 20.677 235.637L150.47 105.216c70.204-49.356 170.226-44.735 235.638 20.676ZM125.892 386.108c-65.448-65.448-70-165.481-20.677-235.637L361.53 406.784c-70.203 49.356-170.226 44.736-235.638-20.676Z"/></svg></svg>"##, (width-size)/2.0, (height-size)/2.0).map_err(|_| ResolutionFailure::Memory)?;
    let mut url = String::new();
    url.try_reserve_exact(32 + svg.len().div_ceil(3) * 4)
        .map_err(|_| ResolutionFailure::Memory)?;
    url.push_str("data:image/svg+xml;base64,");
    base64::engine::general_purpose::STANDARD.encode_string(svg, &mut url);
    transact(IntrinsicTransaction::Acquire {
        event: "nested_resource_placeholder_bytes",
        identity,
        persistent_working_bytes: 256 + (url.len() as u64).div_ceil(64) * 64,
        temporary_working_bytes: 0,
        decoded_pixels: 0,
        total_work_units: 0,
    })
    .map_err(ResolutionFailure::Transaction)?;
    transact(IntrinsicTransaction::ReleaseTemporary {
        event: "nested_resource_placeholder",
        identity,
        bytes: 8192,
    })
    .map_err(ResolutionFailure::Transaction)?;
    Ok(Some(url))
}

pub(super) fn replay_recovery(
    admission: &NestedAdmission,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
) -> Result<(), ResolutionFailure> {
    if let NestedAdmission::Recovery {
        identity,
        retained_bytes,
    } = admission
    {
        transact(IntrinsicTransaction::Acquire {
            event: "nested_resource_placeholder",
            identity: *identity,
            persistent_working_bytes: 0,
            temporary_working_bytes: 8192,
            decoded_pixels: 0,
            total_work_units: 8192,
        })
        .map_err(ResolutionFailure::Transaction)?;
        transact(IntrinsicTransaction::Acquire {
            event: "nested_resource_placeholder_bytes",
            identity: *identity,
            persistent_working_bytes: *retained_bytes,
            temporary_working_bytes: 0,
            decoded_pixels: 0,
            total_work_units: 0,
        })
        .map_err(ResolutionFailure::Transaction)?;
        transact(IntrinsicTransaction::ReleaseTemporary {
            event: "nested_resource_placeholder",
            identity: *identity,
            bytes: 8192,
        })
        .map_err(ResolutionFailure::Transaction)?;
    }
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
    shared_rasters: &BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    pending_rasters: &mut BTreeMap<[u8; 32], Arc<super::NormalizedNestedRaster>>,
    removals: &mut Vec<Removal>,
    failures: &mut Vec<Failure>,
    media_claims: &mut Vec<MediaClaim>,
    effects: &mut Vec<Effect>,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
    nested_accounting: &mut NestedAccounting,
) -> Result<Option<String>, ResolutionFailure> {
    let Some(data) = value.strip_prefix("data:") else {
        return Ok(None);
    };
    let Some((claimed_media_type, encoded)) = data.split_once(";base64,") else {
        return Err(ResolveError::SvgDecodeFailed.into());
    };
    let (decoded_prefix, valid_base64) = super::payload::decoded_prefix(encoded);
    admit_nested_occurrence(decoded_prefix, transact, nested_accounting)?;
    if !valid_base64 {
        return Err(ResolveError::SvgDecodeFailed.into());
    }
    let attempted_depth = depth.saturating_add(1);
    if attempted_depth > limits.max_nested_asset_depth() {
        return Err(ResolveError::SvgDepth {
            attempted: u64::from(attempted_depth),
        }
        .into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| ResolveError::SvgDecodeFailed)?;
    let decoded_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if decoded_bytes > limits.max_asset_bytes() {
        return Err(ResolveError::AssetBytes {
            attempted: decoded_bytes,
        }
        .into());
    }
    let validated_media_type = if looks_like_svg(&bytes) {
        "image/svg+xml"
    } else {
        super::sniff_media_kind(&bytes)?.media_type()
    };
    let (media_type, canonical_bytes) = if looks_like_svg(&bytes) {
        let identity = super::semantic_identity(revision, super::MediaKind::Svg, &bytes);
        let cached = nested_accounting.sanitized_svgs.get(&identity).cloned();
        let (nested, shared) = if let Some(nested) = cached {
            for admission in &nested.nested_admissions {
                replay_recovery(admission, transact)?;
                if matches!(admission, NestedAdmission::Recovery { .. }) {
                    nested_accounting
                        .admissions
                        .try_reserve(1)
                        .map_err(|_| ResolutionFailure::Memory)?;
                    nested_accounting.admissions.push(admission.clone());
                }
                if let NestedAdmission::Occurrence { asset_bytes } = admission {
                    admit_nested_occurrence(*asset_bytes, transact, nested_accounting)?;
                }
            }
            (nested, true)
        } else {
            let admission_start = nested_accounting.admissions.len();
            let mut nested = sanitize_at(
                &bytes,
                limits,
                revision,
                format,
                attempted_depth,
                shared_rasters,
                pending_rasters,
                transact,
                nested_accounting,
            )?;
            let child_admissions = &nested_accounting.admissions[admission_start..];
            nested
                .nested_admissions
                .try_reserve_exact(child_admissions.len())
                .map_err(|_| ResolutionFailure::Memory)?;
            nested
                .nested_admissions
                .extend(child_admissions.iter().cloned());
            let nested = Arc::new(nested);
            nested_accounting
                .sanitized_svgs
                .insert(identity, Arc::clone(&nested));
            (nested, false)
        };
        if !shared {
            let total_work_units = nested_accounting
                .total_work_units
                .saturating_add(nested.work_units);
            transact(IntrinsicTransaction::PreflightNested {
                asset_entries: nested_accounting.asset_entries,
                asset_bytes: nested_accounting.asset_bytes,
                decoded_pixels: nested_accounting.decoded_pixels,
                total_work_units: total_work_units.saturating_add(nested_accounting.asset_bytes),
            })
            .map_err(ResolutionFailure::Transaction)?;
            nested_accounting
                .admissions
                .try_reserve(1)
                .map_err(|_| ResolutionFailure::Memory)?;
            nested_accounting.admissions.push(NestedAdmission::Work {
                identity,
                total_work_units: nested.work_units,
            });
            nested_accounting.total_work_units = total_work_units;
        }
        for mut removal in nested.removals.iter().cloned() {
            removal
                .slots
                .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
            removals.push(removal);
        }
        for mut failure in nested.failures.iter().cloned() {
            failure
                .path
                .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
            failures.push(failure);
        }
        for mut claim in nested.media_claims.iter().cloned() {
            claim
                .path
                .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
            media_claims.push(claim);
        }
        for mut effect in nested.effects.iter().cloned() {
            for cause in &mut effect.causes {
                cause
                    .slots
                    .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
            }
            effects.push(effect);
        }
        ("image/svg+xml", nested.bytes.clone())
    } else {
        let (kind, canonical_bytes) = if format == OutputFormat::Svg {
            let (kind, _, validation_work) = super::validate_nested_raster(&bytes, limits)?;
            let identity = super::semantic_identity(revision, kind, &bytes);
            let total_work_units = nested_accounting
                .total_work_units
                .saturating_add(validation_work);
            transact(IntrinsicTransaction::PreflightNested {
                asset_entries: nested_accounting.asset_entries,
                asset_bytes: nested_accounting.asset_bytes,
                decoded_pixels: nested_accounting.decoded_pixels,
                total_work_units: total_work_units.saturating_add(nested_accounting.asset_bytes),
            })
            .map_err(ResolutionFailure::Transaction)?;
            nested_accounting
                .admissions
                .try_reserve(1)
                .map_err(|_| ResolutionFailure::Memory)?;
            nested_accounting.admissions.push(NestedAdmission::Work {
                identity,
                total_work_units: validation_work,
            });
            nested_accounting.total_work_units = total_work_units;
            (kind, Arc::new(bytes.clone()))
        } else {
            let kind = super::sniff_media_kind(&bytes)?;
            let identity = super::semantic_identity(revision, kind, &bytes);
            let normalized = if let Some(normalized) = pending_rasters
                .get(&identity)
                .or_else(|| shared_rasters.get(&identity))
            {
                Arc::clone(normalized)
            } else {
                let validated = super::validate_nested_raster_semantics(&bytes, limits)?;
                let work = u64::try_from(bytes.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(5)
                    .saturating_add(validated.pixels.saturating_mul(4))
                    .saturating_add(validated.validation_work);
                let decoded_pixels = nested_accounting
                    .decoded_pixels
                    .saturating_add(validated.pixels);
                transact(IntrinsicTransaction::PreflightNested {
                    asset_entries: nested_accounting.asset_entries,
                    asset_bytes: nested_accounting.asset_bytes,
                    decoded_pixels,
                    total_work_units: nested_accounting
                        .total_work_units
                        .saturating_add(nested_accounting.asset_bytes)
                        .saturating_add(work),
                })
                .map_err(ResolutionFailure::Transaction)?;
                let largest = validated.source_width.max(validated.source_height);
                if largest > limits.max_raster_dimension() {
                    return Err(ResolveError::Dimensions {
                        attempted: u64::from(largest),
                    }
                    .into());
                }
                if validated.pixels > limits.max_decoded_pixels_per_asset() {
                    return Err(ResolveError::Pixels {
                        attempted: validated.pixels,
                    }
                    .into());
                }
                if work > limits.max_asset_work_units() {
                    transact(IntrinsicTransaction::RejectLocalWork {
                        event: "nested_raster_normalization",
                        identity,
                        total_work_units: work,
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(ResolveError::AssetWork { attempted: work }.into());
                }
                nested_accounting
                    .admissions
                    .try_reserve(1)
                    .map_err(|_| ResolutionFailure::Memory)?;
                nested_accounting
                    .admissions
                    .push(NestedAdmission::DecodedPixels {
                        decoded_pixels: validated.pixels,
                    });
                nested_accounting.decoded_pixels = decoded_pixels;
                nested_accounting
                    .admissions
                    .try_reserve(1)
                    .map_err(|_| ResolutionFailure::Memory)?;
                nested_accounting.admissions.push(NestedAdmission::Work {
                    identity,
                    total_work_units: work,
                });
                nested_accounting.total_work_units =
                    nested_accounting.total_work_units.saturating_add(work);
                let normalized = Arc::new(super::normalize_nested_raster(&bytes, validated)?);
                pending_rasters.insert(identity, Arc::clone(&normalized));
                normalized
            };
            (
                normalized.source_media_kind,
                Arc::new(normalized.normalized_png.clone()),
            )
        };
        let emitted_type = if format == OutputFormat::Svg {
            kind.media_type()
        } else {
            "image/png"
        };
        (emitted_type, canonical_bytes.as_ref().clone())
    };
    if claimed_media_type != validated_media_type {
        media_claims.push(MediaClaim {
            path: vec![(slot_node_ordinal, slot_attribute_ordinal)],
            claimed: claimed_media_type.to_owned(),
            validated: validated_media_type.to_owned(),
        });
    }
    let mut canonical = format!("data:{media_type};base64,");
    base64::engine::general_purpose::STANDARD.encode_string(canonical_bytes, &mut canonical);
    Ok(Some(canonical))
}

fn admit_nested_occurrence(
    decoded_bytes: u64,
    transact: &mut dyn FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
    accounting: &mut NestedAccounting,
) -> Result<(), ResolutionFailure> {
    let asset_entries = accounting.asset_entries.saturating_add(1);
    let asset_bytes = accounting.asset_bytes.saturating_add(decoded_bytes);
    transact(IntrinsicTransaction::PreflightNested {
        asset_entries,
        asset_bytes,
        decoded_pixels: accounting.decoded_pixels,
        total_work_units: accounting.total_work_units.saturating_add(asset_bytes),
    })
    .map_err(ResolutionFailure::Transaction)?;
    accounting
        .admissions
        .try_reserve(1)
        .map_err(|_| ResolutionFailure::Memory)?;
    accounting.admissions.push(NestedAdmission::Occurrence {
        asset_bytes: decoded_bytes,
    });
    accounting.asset_entries = asset_entries;
    accounting.asset_bytes = asset_bytes;
    Ok(())
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
            | "style"
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

pub(super) fn viewer_options() -> Result<usvg::Options<'static>, ResolveError> {
    let excalifont = excalidraw_render_assets_base::load_excalifont_basic_latin_sfnt()
        .map_err(|_| ResolveError::SvgDecodeFailed)?;
    let (_, liberation_sans) = crate::render::text::FallbackFont::load()
        .map_err(|_| ResolveError::SvgDecodeFailed)?
        .into_parts();
    let mut options = usvg::Options {
        resources_dir: None,
        font_family: "Liberation Sans".to_owned(),
        ..usvg::Options::default()
    };
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(excalifont.bytes().to_vec());
    fontdb.load_font_data(liberation_sans);
    fontdb.set_serif_family("Liberation Sans");
    fontdb.set_sans_serif_family("Liberation Sans");
    fontdb.set_cursive_family("Liberation Sans");
    fontdb.set_fantasy_family("Liberation Sans");
    fontdb.set_monospace_family("Liberation Sans");
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

#[cfg(test)]
mod tests {
    //! The nested-SVG sanitizer at its own seam: what it removes, what it
    //! refuses, and where it stops recursing, read off `SanitizedSvg` rather
    //! than off the diagnostics a session later maps them to.

    use super::*;

    const PNG_BASE64: &str =
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    /// Sanitizes `bytes` under `limits` for the SVG format with a transaction
    /// sink that admits everything: nothing here tests the meter.
    fn sanitize_with(
        bytes: &[u8],
        limits: &ResourceLimits,
        format: OutputFormat,
    ) -> Result<SanitizedSvg, ResolutionFailure> {
        let shared_rasters = BTreeMap::new();
        let mut pending_rasters = BTreeMap::new();
        sanitize(
            bytes,
            limits,
            CompatibilityRevision::REVISION_1,
            format,
            &shared_rasters,
            &mut pending_rasters,
            &mut |_| Ok(()),
        )
    }

    fn sanitize_default(bytes: &[u8]) -> SanitizedSvg {
        sanitize_with(bytes, &ResourceLimits::default(), OutputFormat::Svg)
            .expect("the SVG sanitizes")
    }

    fn sanitized_text(sanitized: &SanitizedSvg) -> &str {
        std::str::from_utf8(&sanitized.bytes).expect("sanitized SVG is UTF-8")
    }

    fn removal_names(sanitized: &SanitizedSvg) -> Vec<&str> {
        sanitized
            .removals
            .iter()
            .map(|removal| removal.name.as_str())
            .collect()
    }

    fn svg_with_nested_data_image(child: &[u8]) -> String {
        let child = base64::engine::general_purpose::STANDARD.encode(child);
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/svg+xml;base64,{child}"/></svg>"#
        )
    }

    fn asset_error(result: Result<SanitizedSvg, ResolutionFailure>) -> ResolveError {
        match result {
            Err(ResolutionFailure::Asset(error)) => error,
            other => panic!("the sanitizer refuses the asset: {other:?}"),
        }
    }

    #[test]
    fn a_safe_svg_is_reserialized_with_its_root_namespace_and_size() {
        let sanitized = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10"><rect width="10" height="10" fill="#ff0000"/><rect x="10" width="10" height="10" fill="#0000ff"/></svg>"##,
        );
        assert_eq!((sanitized.width, sanitized.height), (20, 10));
        assert!(sanitized.removals.is_empty());
        assert!(sanitized.failures.is_empty());
        assert!(sanitized.effects.is_empty());
        assert!(sanitized.nested_admissions.is_empty());
        let text = sanitized_text(&sanitized);
        assert!(text.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
        assert_eq!(text.matches("<rect").count(), 2);
        assert!(sanitized.work_units > 0 && sanitized.working_bytes > 512);
    }

    #[test]
    fn scripts_handlers_animation_and_foreign_content_are_removed() {
        let sanitized = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" onload="alert(1)">
            <rect width="20" height="10" fill="#00ff00" onclick="alert(2)"/>
            <script>alert(3)</script>
            <animate attributeName="opacity" values="0;1"/>
            <foreignObject><div xmlns="http://www.w3.org/1999/xhtml">unsafe</div></foreignObject>
            <image href="https://example.com/tracker.png" width="20" height="10"/>
            <style>@import url(https://example.com/theme.css); rect { fill: red }</style>
        </svg>"##,
        );

        let text = sanitized_text(&sanitized);
        for forbidden in [
            "onload",
            "onclick",
            "script",
            "animate",
            "foreignObject",
            "example.com",
            "@import",
        ] {
            assert!(!text.contains(forbidden), "removed {forbidden}: {text}");
        }
        assert!(text.contains("<rect"));
        assert_eq!(
            removal_names(&sanitized),
            [
                "onload",
                "onclick",
                "script",
                "animate",
                "foreignObject",
                "href",
                "style"
            ]
        );
        // Scripts and handlers never execute in static rendering; an image's
        // removed href does affect appearance, unlike an action link on `a`.
        assert_eq!(
            sanitized
                .removals
                .iter()
                .filter(|removal| !removal.changes_appearance)
                .map(|removal| removal.name.as_str())
                .collect::<Vec<_>>(),
            ["onload", "onclick", "script"]
        );
        // Attribute removals name their slot; element removals do not.
        assert!(sanitized.removals[0].attribute_ordinal.is_some());
        assert!(sanitized.removals[2].attribute_ordinal.is_none());
    }

    #[test]
    fn dtds_and_declared_entities_are_removed_without_expansion() {
        let sanitized = sanitize_default(
            br##"<?xml version="1.0"?><!DOCTYPE svg [<!ENTITY secret "EXPANDED">]><svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><text>&secret;</text><rect width="20" height="10" fill="#00ff00"/></svg>"##,
        );
        let text = sanitized_text(&sanitized);
        assert!(!text.contains("DOCTYPE"));
        assert!(!text.contains("ENTITY"));
        assert!(!text.contains("EXPANDED"));
        assert!(text.contains("<rect"));
        // The DTD itself paints nothing; the reference to the declared entity
        // did, and its removal changes what the text shows.
        assert_eq!(
            sanitized
                .removals
                .iter()
                .map(|removal| (removal.name.as_str(), removal.changes_appearance))
                .collect::<Vec<_>>(),
            [("DOCTYPE", false), ("secret", true)]
        );
    }

    #[test]
    fn processing_instructions_other_than_the_xml_declaration_are_removed() {
        let sanitized = sanitize_default(
            br##"<?xml-stylesheet href="https://example.com/theme.css"?><svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##,
        );
        let text = sanitized_text(&sanitized);
        assert!(!text.contains("xml-stylesheet"));
        assert!(!text.contains("example.com"));
        assert_eq!(removal_names(&sanitized), ["xml-stylesheet"]);

        let declared = sanitize_default(
            br##"<?xml version="1.0" encoding="UTF-8"?><svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10"/></svg>"##,
        );
        assert!(
            declared.removals.is_empty(),
            "the XML declaration is not a removal"
        );
    }

    #[test]
    fn external_references_are_removed_and_same_document_fragments_kept() {
        let sanitized = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="10">
            <defs><linearGradient id="paint"><stop stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></linearGradient><rect id="tile" width="10" height="10" fill="url(#paint)"/></defs>
            <use href="#tile"/><use xlink:href="#tile" x="10"/>
            <use href="https://example.com/shapes.svg#tile"/>
            <image xlink:href="//example.com/tracker.png" width="1" height="1"/>
            <a href="javascript:alert(1)"><rect width="1" height="1"/></a>
        </svg>"##,
        );
        let text = sanitized_text(&sanitized);
        assert!(!text.contains("example.com"));
        assert!(!text.contains("javascript:"));
        assert_eq!(text.matches(r##"href="#tile""##).count(), 2, "{text}");
        assert!(text.contains(r##"fill="url(#paint)""##));
        assert_eq!(removal_names(&sanitized), ["href", "href", "href"]);
        // xlink:href is lowered to a plain href on the way out.
        assert!(!text.contains("xlink:href"));
    }

    #[test]
    fn data_urls_are_removed_from_link_actions() {
        let sanitized = sanitize_default(
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><a href="data:image/png;base64,{PNG_BASE64}"><rect width="20" height="10" fill="#00ff00"/></a></svg>"##
            )
            .as_bytes(),
        );
        assert!(!sanitized_text(&sanitized).contains("data:image/png"));
        assert_eq!(removal_names(&sanitized), ["href"]);
    }

    #[test]
    fn nested_raster_data_is_validated_and_kept_byte_for_byte() {
        let sanitized = sanitize_default(
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,{PNG_BASE64}"/></svg>"#
            )
            .as_bytes(),
        );
        assert!(sanitized_text(&sanitized).contains(&format!("data:image/png;base64,{PNG_BASE64}")));
        assert!(sanitized.removals.is_empty());
        assert!(sanitized.failures.is_empty());
        // The nested payload is one asset occurrence of its decoded size.
        assert!(matches!(
            sanitized.nested_admissions.as_slice(),
            [NestedAdmission::Occurrence { asset_bytes }, ..] if *asset_bytes > 0
        ));
    }

    #[test]
    fn nested_data_svgs_are_recursively_sanitized() {
        let child = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/><script>alert(1)</script></svg>"##;
        let sanitized = sanitize_default(svg_with_nested_data_image(child).as_bytes());

        let text = sanitized_text(&sanitized);
        let (_, nested) = text
            .split_once("data:image/svg+xml;base64,")
            .expect("the nested SVG is re-embedded as a data URL");
        let nested = base64::engine::general_purpose::STANDARD
            .decode(nested.split('"').next().expect("terminated data URL"))
            .expect("re-embedded nested SVG is base64");
        let nested = std::str::from_utf8(&nested).expect("nested SVG is UTF-8");
        assert!(nested.contains("<rect"));
        assert!(!nested.contains("script"));
        // The child's removal surfaces on the parent.
        assert_eq!(removal_names(&sanitized), ["script"]);
        assert!(sanitized
            .nested_admissions
            .iter()
            .any(|admission| matches!(admission, NestedAdmission::Work { .. })));
    }

    #[test]
    fn a_malformed_nested_image_fails_locally_without_discarding_its_siblings() {
        let sanitized = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000"/><image width="20" height="10" href="data:image/png;base64,AA=="/></svg>"##,
        );
        assert!(sanitized_text(&sanitized).contains("<rect"));
        let [failure] = sanitized.failures.as_slice() else {
            panic!("one nested failure: {:#?}", sanitized.failures);
        };
        assert_eq!(failure.error, ResolveError::Unsupported);
        // The failing slot: the image is the third node (root, rect, image),
        // and href its third attribute.
        assert_eq!(failure.path, [(2, 2)]);
    }

    #[test]
    fn the_depth_ceiling_stops_recursive_resolution_as_a_local_failure() {
        let leaf = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
        let child = svg_with_nested_data_image(leaf);
        let parent = svg_with_nested_data_image(child.as_bytes());
        let limits = ResourceLimits::builder()
            .max_nested_asset_depth(1)
            .build()
            .expect("depth limit is valid");

        let sanitized = sanitize_with(parent.as_bytes(), &limits, OutputFormat::Svg)
            .expect("the parent survives its child's refusal");
        let [failure] = sanitized.failures.as_slice() else {
            panic!("one depth failure: {:#?}", sanitized.failures);
        };
        assert_eq!(failure.error, ResolveError::SvgDepth { attempted: 2 });
        assert_eq!(failure.path.len(), 2, "the failing slot is two levels down");

        let permitted = sanitize_with(
            parent.as_bytes(),
            &ResourceLimits::default(),
            OutputFormat::Svg,
        )
        .expect("the default depth admits two levels");
        assert!(permitted.failures.is_empty());
    }

    #[test]
    fn the_node_ceiling_refuses_the_document_before_viewer_parsing() {
        let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
        let limits = ResourceLimits::builder()
            .max_nested_svg_nodes(1)
            .build()
            .expect("node limit is valid");
        assert_eq!(
            asset_error(sanitize_with(nested, &limits, OutputFormat::Svg)),
            ResolveError::SvgComplexity { attempted: 2 }
        );
        let limits = ResourceLimits::builder()
            .max_nested_svg_nodes(2)
            .build()
            .expect("node limit is valid");
        sanitize_with(nested, &limits, OutputFormat::Svg).expect("two nodes fit the ceiling");
    }

    #[test]
    fn the_asset_work_ceiling_refuses_the_document() {
        let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#00ff00"/></svg>"##;
        let work = sanitize_default(nested).work_units;
        let limits = ResourceLimits::builder()
            .max_asset_work_units(work - 1)
            .build()
            .expect("work limit is valid");
        assert_eq!(
            asset_error(sanitize_with(nested, &limits, OutputFormat::Svg)),
            ResolveError::AssetWork { attempted: work }
        );
    }

    #[test]
    fn documents_that_are_not_svg_are_refused_as_decode_failures() {
        for bytes in [
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect>"#.as_slice(),
            br#"<svg width="20" height="10"><rect width="1" height="1"/></svg>"#.as_slice(),
            br#"<html xmlns="http://www.w3.org/1999/xhtml"><body/></html>"#.as_slice(),
            b"\xff\xfe not text".as_slice(),
        ] {
            assert_eq!(
                asset_error(sanitize_with(
                    bytes,
                    &ResourceLimits::default(),
                    OutputFormat::Svg
                )),
                ResolveError::SvgDecodeFailed,
                "{}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    #[test]
    fn effects_are_discovered_with_their_backdrop_dependence() {
        let isolated = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" filter="url(#blur)"/></svg>"##,
        );
        let [effect] = isolated.effects.as_slice() else {
            panic!("one effect: {:#?}", isolated.effects);
        };
        assert_eq!(effect.filter_id, "blur");
        assert!(!effect.unisolatable);
        assert!(!effect.causes.is_empty());

        let backdrop = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blend"><feBlend in="SourceGraphic" in2="BackgroundImage" mode="multiply"/></filter></defs><rect width="20" height="10" filter="url(#blend)"/></svg>"##,
        );
        assert!(backdrop.effects[0].unisolatable);

        let noop = sanitize_default(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="effect"><feGaussianBlur stdDeviation="0"/></filter></defs><rect width="20" height="10" filter="url(#effect)"/></svg>"##,
        );
        assert!(
            noop.effects.is_empty(),
            "a no-op filter is culled: {:#?}",
            noop.effects
        );
        assert!(
            !sanitized_text(&noop).contains("filter="),
            "and its reference dropped from the rect"
        );
    }

    #[test]
    fn the_pdf_format_labels_effect_nodes_for_closure_isolation() {
        let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs><rect width="20" height="10" filter="url(#blur)"/></svg>"##;
        let pdf = sanitize_with(nested, &ResourceLimits::default(), OutputFormat::Pdf)
            .expect("the SVG sanitizes for PDF");
        let text = sanitized_text(&pdf);
        assert!(text.contains(r#"id="__excalidraw_effect_"#), "{text}");
        assert!(
            text.contains(r#"result="__excalidraw_filter_result_"#),
            "{text}"
        );
        let svg = sanitize_default(nested);
        assert!(!sanitized_text(&svg).contains("__excalidraw_"));
    }
}
