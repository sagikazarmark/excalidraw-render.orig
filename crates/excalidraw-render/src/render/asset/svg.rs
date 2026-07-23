use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;

use super::{IntrinsicTransaction, ResolutionFailure, ResolveError};
use crate::{CompatibilityRevision, OutputFormat, RenderError, ResourceLimits};

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
    pub(super) nested_admissions: Vec<NestedAdmission>,
    pub(super) tree: Arc<usvg::Tree>,
}

#[derive(Clone)]
pub(super) enum NestedAdmission {
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
                shared_rasters,
                pending_rasters,
                removals,
                failures,
                effects,
                transact,
                nested_accounting,
            ) {
                Ok(value) => value,
                Err(ResolutionFailure::Asset(error)) => {
                    failures.push(Failure {
                        path: vec![(
                            ordinal,
                            u64::try_from(attribute_ordinal).unwrap_or(u64::MAX),
                        )],
                        error,
                    });
                    continue;
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
    let (decoded_prefix, valid_base64) = super::decoded_prefix(encoded);
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
        removals.extend(nested.removals.iter().cloned());
        for mut failure in nested.failures.iter().cloned() {
            failure
                .path
                .insert(0, (slot_node_ordinal, slot_attribute_ordinal));
            failures.push(failure);
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
        removals.push(Removal {
            node_ordinal: 0,
            attribute_ordinal: None,
            name: "media-type".to_owned(),
            changes_appearance: false,
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
