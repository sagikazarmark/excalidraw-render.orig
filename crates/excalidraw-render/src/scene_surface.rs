use std::collections::TryReserveError;

use crate::outcome::DiagnosticOrder;
use crate::scene::{
    finite_json_number, member_value, pointer_child, pointer_index, JsonMember, JsonValue,
};
use crate::{
    CompatibilityDiagnostic, DiagnosticCode, DiagnosticEffect, DiagnosticSeverity, ErrorDetails,
    OutputFormat, Recovery, SceneDocument,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneFieldDisposition {
    Supported,
    StaticExportIrrelevant,
    OpaqueMetadata,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct SceneFieldFallback {
    pub(crate) action: &'static str,
    pub(crate) code: Option<&'static str>,
    pub(crate) effects: &'static [&'static str],
    pub(crate) recovery: Option<&'static str>,
    pub(crate) recipe: Option<&'static str>,
    pub(crate) value_json: &'static str,
    pub(crate) format_rules_json: &'static str,
    pub(crate) format_recoveries: [Option<&'static str>; 3],
    pub(crate) when: &'static str,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct SceneFieldValueRule {
    pub(crate) code: Option<&'static str>,
    pub(crate) recovery: Option<&'static str>,
    pub(crate) value_json: &'static str,
    pub(crate) format_rules_json: &'static str,
    pub(crate) when: &'static str,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct SceneFieldPolicy {
    pub(crate) record_kind: &'static str,
    pub(crate) path: &'static str,
    pub(crate) accepted_domain: &'static str,
    pub(crate) disposition: SceneFieldDisposition,
    pub(crate) required: bool,
    pub(crate) normalization: &'static str,
    pub(crate) participation: &'static str,
    pub(crate) formats: &'static [&'static str],
    pub(crate) options: &'static [&'static str],
    pub(crate) states: &'static [&'static str],
    pub(crate) missing: SceneFieldFallback,
    pub(crate) invalid: SceneFieldFallback,
    pub(crate) value_rules: &'static [SceneFieldValueRule],
    pub(crate) ordinal: usize,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct SceneRecordPolicy {
    pub(crate) identity: &'static str,
    pub(crate) field_paths: &'static [&'static str],
    pub(crate) inherits: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct SceneElementPolicy {
    pub(crate) kind: &'static str,
    pub(crate) record_kind: &'static str,
    pub(crate) status: &'static str,
    pub(crate) ignored_inherited_paths: &'static [&'static str],
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct SceneSurfacePolicy {
    pub(crate) record_kinds: &'static [SceneRecordPolicy],
    pub(crate) fields: &'static [SceneFieldPolicy],
    pub(crate) element_kinds: &'static [SceneElementPolicy],
    pub(crate) diagnostic_codes: &'static [&'static str],
}

include!("generated/scene_surface_revision_1.rs");

pub(crate) struct SurfaceDiagnostic {
    pub(crate) source_order: u64,
    pub(crate) source_position: u64,
    pub(crate) local_owner: Option<String>,
    pub(crate) diagnostic: CompatibilityDiagnostic,
}

pub(crate) struct SurfaceClassification {
    pub(crate) diagnostics: Vec<SurfaceDiagnostic>,
    pub(crate) fallbacks: Vec<SurfaceFallback>,
}

pub(crate) struct SurfaceFallback {
    pub(crate) source_order: u64,
    pub(crate) element_id: Option<String>,
    pub(crate) json_pointer: String,
    pub(crate) label: String,
    pub(crate) link: Option<String>,
    pub(crate) frame_id: Option<String>,
    pub(crate) is_frame: bool,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) angle: f64,
    pub(crate) opacity: f64,
    pub(crate) paint: bool,
}

pub(crate) fn classify(
    scene: &SceneDocument,
    policy: &'static SceneSurfacePolicy,
    format: OutputFormat,
    frame_names: bool,
    diagnostic_limit: usize,
) -> Result<SurfaceClassification, TryReserveError> {
    let mut diagnostics = Vec::new();
    let mut fallbacks = Vec::new();
    classify_record(
        scene,
        policy,
        "scene",
        scene.root_members(),
        "",
        None,
        format,
        diagnostic_limit,
        &mut diagnostics,
    )?;
    classify_record(
        scene,
        policy,
        "app_state",
        scene.app_state_members(),
        "/appState",
        Some("appState"),
        format,
        diagnostic_limit,
        &mut diagnostics,
    )?;
    for index in 0..scene.element_occurrence_count() {
        if diagnostics.len() >= diagnostic_limit {
            break;
        }
        if scene.element_is_silent(index) {
            continue;
        }
        let pointer = crate::scene::pointer_index("/elements", index)?;
        let source_order = scene.source_owner_order("elements", index);
        let Some(value) = scene.element_value(index) else {
            continue;
        };
        let JsonValue::Object(members) = value else {
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: 0,
                local_owner: None,
                diagnostic: element_boundary_diagnostic(
                    DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT,
                    format,
                    &[DiagnosticEffect::Visual],
                    None,
                    &pointer,
                    Recovery::Omitted,
                    None,
                    "malformed element was omitted by the revision-1 scene-surface policy",
                )?,
            });
            continue;
        };
        let kind = match member_value(members, "type") {
            Some(JsonValue::String(kind)) if !kind.is_empty() => Some(kind.as_ref()),
            _ => None,
        };
        let element_id = match member_value(members, "id") {
            Some(JsonValue::String(id)) if !id.is_empty() => Some(id.as_ref()),
            _ => None,
        };
        let element_policy = kind.and_then(|kind| {
            policy
                .element_kinds
                .iter()
                .find(|element| element.kind == kind)
        });
        if let Some(element_policy) = element_policy {
            if element_policy.status == "static-export-irrelevant" {
                continue;
            }
            let has_unrendered_element = scene.element_at(index).is_none();
            if has_unrendered_element {
                let bounds = recoverable_bounds(members);
                diagnostics.try_reserve(1)?;
                diagnostics.push(SurfaceDiagnostic {
                    source_order,
                    source_position: 0,
                    local_owner: element_id.map(try_owned).transpose()?,
                    diagnostic: element_boundary_diagnostic(
                        DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED,
                        format,
                        &[DiagnosticEffect::Visual, DiagnosticEffect::Semantic],
                        element_id,
                        &pointer,
                        if bounds.is_some() {
                            Recovery::Placeholder
                        } else {
                            Recovery::Omitted
                        },
                        bounds
                            .is_some()
                            .then_some("renderer.known-element-unavailable.v1"),
                        "known element unavailable to this renderer used the revision-1 fallback",
                    )?,
                });
                if let Some(bounds) = bounds {
                    let label = fallback_label("Unavailable: ", element_policy.kind)?;
                    fallbacks.try_reserve(1)?;
                    fallbacks.push(surface_fallback(
                        source_order,
                        element_id,
                        &pointer,
                        &label,
                        fallback_link(members)?,
                        fallback_frame_id(members),
                        matches!(element_policy.kind, "frame" | "magicframe"),
                        bounds,
                    )?);
                }
            }
            classify_element_record(
                scene,
                policy,
                element_policy,
                members,
                &pointer,
                source_order,
                element_id,
                format,
                frame_names,
                has_unrendered_element,
                diagnostic_limit,
                &mut diagnostics,
            )?;
            continue;
        }
        if kind == Some("draw") {
            classify_element_record(
                scene,
                policy,
                &SceneElementPolicy {
                    kind: "draw",
                    record_kind: "element.legacy_draw",
                    status: "supported",
                    ignored_inherited_paths: &[],
                },
                members,
                &pointer,
                source_order,
                element_id,
                format,
                frame_names,
                false,
                diagnostic_limit,
                &mut diagnostics,
            )?;
            continue;
        }

        let bounds = recoverable_bounds(members);
        let (code, effects, recipe, message, label) = if let Some(kind) = kind {
            (
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE,
                &[DiagnosticEffect::Visual, DiagnosticEffect::Semantic][..],
                "renderer.unknown-element.v1",
                "unsupported element type used the revision-1 element fallback",
                fallback_label("Unsupported: ", kind)?,
            )
        } else {
            (
                DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT,
                &[DiagnosticEffect::Visual][..],
                "renderer.malformed-element.v1",
                "malformed element used the revision-1 element fallback",
                try_owned("Malformed element")?,
            )
        };
        diagnostics.try_reserve(1)?;
        diagnostics.push(SurfaceDiagnostic {
            source_order,
            source_position: 0,
            local_owner: element_id.map(try_owned).transpose()?,
            diagnostic: element_boundary_diagnostic(
                code,
                format,
                effects,
                element_id,
                &pointer,
                if bounds.is_some() {
                    Recovery::Placeholder
                } else {
                    Recovery::Omitted
                },
                bounds.is_some().then_some(recipe),
                message,
            )?,
        });
        if format != OutputFormat::Png
            && diagnostics.len() < diagnostic_limit
            && matches!(member_value(members, "link"), Some(JsonValue::String(link)) if !safe_link(link))
        {
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: u64::try_from(
                    members
                        .iter()
                        .position(|member| &*member.key == "link")
                        .unwrap_or(usize::MAX),
                )
                .unwrap_or(u64::MAX)
                .saturating_mul(u64::from(u32::MAX) + 1),
                local_owner: element_id.map(try_owned).transpose()?,
                diagnostic: value_rule_diagnostic(
                    DiagnosticCode::LINK_UNSAFE_OMITTED,
                    format,
                    &[DiagnosticEffect::Semantic],
                    element_id,
                    &pointer_child(&pointer, "link")?,
                    Recovery::Omitted,
                    "unsafe link was omitted by the revision-1 sanitizer",
                )?,
            });
        }
        if let Some(bounds) = bounds {
            fallbacks.try_reserve(1)?;
            fallbacks.push(surface_fallback(
                source_order,
                element_id,
                &pointer,
                &label,
                fallback_link(members)?,
                fallback_frame_id(members),
                false,
                bounds,
            )?);
        }
    }
    Ok(SurfaceClassification {
        diagnostics,
        fallbacks,
    })
}

#[allow(clippy::too_many_arguments)]
fn classify_record(
    scene: &SceneDocument,
    policy: &'static SceneSurfacePolicy,
    record_kind: &str,
    members: &[JsonMember],
    pointer: &str,
    owner: Option<&str>,
    format: OutputFormat,
    diagnostic_limit: usize,
    diagnostics: &mut Vec<SurfaceDiagnostic>,
) -> Result<(), TryReserveError> {
    for (position, member) in members.iter().enumerate() {
        if diagnostics.len() >= diagnostic_limit {
            break;
        }
        let semantic_path = pointer_child("", &member.key)?;
        if let Some(field) = field_policy(policy, record_kind, &semantic_path) {
            if field.disposition == SceneFieldDisposition::Supported
                && !value_matches_domain(&member.value, field.accepted_domain)
            {
                if let Some(diagnostic) = manifested_field_diagnostic(
                    &field.invalid,
                    format,
                    &pointer_child(pointer, &member.key)?,
                    None,
                )? {
                    diagnostics.try_reserve(1)?;
                    diagnostics.push(SurfaceDiagnostic {
                        source_order: scene.source_owner_order(owner.unwrap_or(&member.key), 0),
                        source_position: u64::try_from(position).unwrap_or(u64::MAX),
                        local_owner: None,
                        diagnostic,
                    });
                }
            }
            continue;
        }
        let json_pointer = pointer_child(pointer, &member.key)?;
        diagnostics.try_reserve(1)?;
        diagnostics.push(SurfaceDiagnostic {
            source_order: scene.source_owner_order(owner.unwrap_or(&member.key), 0),
            source_position: u64::try_from(position).unwrap_or(u64::MAX),
            local_owner: None,
            diagnostic: CompatibilityDiagnostic {
                code: DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                severity: DiagnosticSeverity::Warning,
                format,
                effects: owned_effects(&[
                    DiagnosticEffect::Visual,
                    DiagnosticEffect::Semantic,
                    DiagnosticEffect::Metadata,
                ])?,
                element_id: None,
                json_pointer,
                message: try_owned(
                    "unmatched property was ignored by the revision-1 scene-surface policy",
                )?,
                details: Some(Box::new(ErrorDetails::Compatibility {
                    recovery: Recovery::Ignored,
                    recipe_id: None,
                })),
                order: DiagnosticOrder::default(),
            },
        });
    }
    Ok(())
}

fn value_matches_domain(value: &JsonValue, domain: &str) -> bool {
    let string_is = |allowed: &[&str]| matches!(value, JsonValue::String(string) if allowed.contains(&string.as_ref()));
    let integer = || match value {
        JsonValue::Number(token) => {
            !token.contains(['.', 'e', 'E']) && finite_json_number(value).is_some()
        }
        _ => false,
    };
    let number_array = |accepts: fn(f64) -> bool| match value {
        JsonValue::Array(values) => values
            .iter()
            .all(|value| finite_json_number(value).is_some_and(accepts)),
        _ => false,
    };
    let point = |value: &JsonValue| match value {
        JsonValue::Array(values) => {
            values.len() == 2
                && values
                    .iter()
                    .all(|value| finite_json_number(value).is_some())
        }
        _ => false,
    };
    match domain {
        "any_json" => true,
        "arrow_type" => string_is(&["sharp", "round", "elbow"]),
        "arrowhead" => {
            matches!(value, JsonValue::Null)
                || string_is(&[
                    "arrow",
                    "bar",
                    "dot",
                    "circle",
                    "circle_outline",
                    "triangle",
                    "triangle_outline",
                    "diamond",
                    "diamond_outline",
                    "crowfoot_one",
                    "crowfoot_many",
                    "crowfoot_one_or_many",
                ])
        }
        "boolean" => matches!(value, JsonValue::Bool(_)),
        "bound_element_array" => match value {
            JsonValue::Null => true,
            JsonValue::Array(values) => values.iter().all(|value| {
                let JsonValue::Object(members) = value else {
                    return false;
                };
                matches!(member_value(members, "id"), Some(JsonValue::String(id)) if !id.is_empty())
                    && matches!(member_value(members, "type"), Some(JsonValue::String(kind)) if matches!(&**kind, "arrow" | "text"))
            }),
            _ => false,
        },
        "bound_element_type" => string_is(&["arrow", "text"]),
        "chart_type" => string_is(&["bar", "line"]),
        "css_color" => matches!(value, JsonValue::String(color) if css_color_is_admitted(color)),
        "cursor_button" => string_is(&["up", "down"]),
        "data_url" => matches!(value, JsonValue::String(url) if url.starts_with("data:")),
        "element_type" => string_is(&[
            "selection",
            "rectangle",
            "diamond",
            "ellipse",
            "text",
            "line",
            "arrow",
            "freedraw",
            "image",
            "frame",
            "magicframe",
            "iframe",
            "embeddable",
        ]),
        "fill_style" => string_is(&["hachure", "cross-hatch", "solid", "zigzag"]),
        "finite_number" => finite_json_number(value).is_some(),
        "finite_number_array" => number_array(|_| true),
        "fixed_segment_array" => matches!(value, JsonValue::Null)
            || matches!(value, JsonValue::Array(values) if values.iter().all(|value| matches!(value, JsonValue::Object(_)))),
        "font_family" => integer() && matches!(finite_json_number(value), Some(1.0 | 2.0 | 3.0 | 5.0 | 6.0 | 7.0 | 8.0 | 9.0)),
        "freedraw_points" => {
            matches!(value, JsonValue::Array(values) if values.iter().all(&point))
        }
        "hyperlink_popup" => matches!(value, JsonValue::Bool(false))
            || string_is(&["info", "editor"]),
        "image_crop" => {
            matches!(value, JsonValue::Null | JsonValue::Object(_))
        }
        "image_mime" => string_is(&[
            "image/svg+xml",
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/bmp",
            "image/x-icon",
            "image/avif",
            "image/jfif",
            "application/octet-stream",
        ]),
        "image_scale" => match value {
            JsonValue::Array(values) => {
                values.len() == 2
                    && values.iter().all(|value| {
                        matches!(finite_json_number(value), Some(-1.0 | 1.0))
                    })
            }
            _ => false,
        },
        "image_status" => string_is(&["pending", "saved", "error"]),
        "integer" => integer(),
        "nonempty_string" => {
            matches!(value, JsonValue::String(string) if !string.is_empty())
        }
        "nonnegative_integer" => {
            integer() && finite_json_number(value).is_some_and(|value| value >= 0.0)
        }
        "nonnegative_number" => finite_json_number(value).is_some_and(|value| value >= 0.0),
        "nonzero_integer" => {
            integer() && finite_json_number(value).is_some_and(|value| value != 0.0)
        }
        "nullable_boolean" => matches!(value, JsonValue::Null | JsonValue::Bool(_)),
        "nullable_font_family" => {
            matches!(value, JsonValue::Null)
                || (integer()
                    && matches!(finite_json_number(value), Some(1.0 | 2.0 | 3.0 | 5.0 | 6.0 | 7.0 | 8.0 | 9.0)))
        }
        "nullable_point" => matches!(value, JsonValue::Null) || point(value),
        "nullable_record" => matches!(value, JsonValue::Null | JsonValue::Object(_)),
        "nullable_string" => matches!(value, JsonValue::Null | JsonValue::String(_)),
        "opacity" => {
            finite_json_number(value).is_some_and(|value| (0.0..=100.0).contains(&value))
        }
        "open_menu" => matches!(value, JsonValue::Null) || string_is(&["canvas", "shape"]),
        "open_popup" => {
            matches!(value, JsonValue::Null)
                || string_is(&[
                    "canvasBackground",
                    "elementBackground",
                    "elementStroke",
                    "fontFamily",
                ])
        }
        "point" => point(value),
        "point_array" => {
            matches!(value, JsonValue::Array(values) if values.len() >= 2 && values.iter().all(point))
        }
        "point_binding" => matches!(value, JsonValue::Null | JsonValue::Object(_)),
        "pointer_type" => string_is(&["mouse", "pen", "touch"]),
        "pressure_array" => number_array(|value| (0.0..=1.0).contains(&value)),
        "roundness" => matches!(value, JsonValue::Null | JsonValue::Object(_)),
        "roundness_type" => {
            integer() && matches!(finite_json_number(value), Some(1.0 | 2.0 | 3.0))
        }
        "string" => matches!(value, JsonValue::String(_)),
        "string_array" => {
            matches!(value, JsonValue::Array(values) if values.iter().all(|value| matches!(value, JsonValue::String(_))))
        }
        "stroke_roundness" => string_is(&["round", "sharp"]),
        "stroke_style" => string_is(&["solid", "dashed", "dotted"]),
        "text_align" => string_is(&["left", "center", "right"]),
        "theme" => string_is(&["light", "dark"]),
        "vertical_align" => string_is(&["top", "middle", "bottom"]),
        "literal_excalidraw" => {
            matches!(value, JsonValue::String(value) if &**value == "excalidraw")
        }
        "literal_v2" => matches!(value, JsonValue::Number(value) if &**value == "2"),
        "scene_elements" => matches!(value, JsonValue::Array(_)),
        "positive_number" => finite_json_number(value).is_some_and(|value| value > 0.0),
        "frame_rendering" | "object" => matches!(value, JsonValue::Object(_)),
        _ => false,
    }
}

pub(crate) fn css_color_is_admitted(color: &str) -> bool {
    crate::color::parse_css(color).is_some()
}

fn manifested_field_diagnostic(
    fallback: &SceneFieldFallback,
    format: OutputFormat,
    json_pointer: &str,
    element_id: Option<&str>,
) -> Result<Option<CompatibilityDiagnostic>, TryReserveError> {
    let Some(code) = fallback.code else {
        return Ok(None);
    };
    let recovery = fallback.recovery.or(match format {
        OutputFormat::Svg => fallback.format_recoveries[0],
        OutputFormat::Png => fallback.format_recoveries[1],
        OutputFormat::Pdf => fallback.format_recoveries[2],
    });
    let Some(recovery) = recovery else {
        return Ok(None);
    };
    let Some(recovery) = recovery_from_manifest(recovery) else {
        return Ok(None);
    };
    let mut effects = Vec::new();
    effects.try_reserve_exact(fallback.effects.len())?;
    for effect in fallback.effects {
        if let Some(effect) = effect_from_manifest(effect) {
            effects.push(effect);
        }
    }
    Ok(Some(CompatibilityDiagnostic {
        code: DiagnosticCode::manifested(code),
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("manifested field value used its revision-1 recovery")?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery,
            recipe_id: fallback.recipe.map(try_owned).transpose()?,
        })),
        order: DiagnosticOrder::default(),
    }))
}

fn effect_from_manifest(effect: &str) -> Option<DiagnosticEffect> {
    match effect {
        "input_repair" => Some(DiagnosticEffect::InputRepair),
        "visual" => Some(DiagnosticEffect::Visual),
        "semantic" => Some(DiagnosticEffect::Semantic),
        "metadata" => Some(DiagnosticEffect::Metadata),
        _ => None,
    }
}

fn recovery_from_manifest(recovery: &str) -> Option<Recovery> {
    match recovery {
        "repaired" => Some(Recovery::Repaired),
        "ignored" => Some(Recovery::Ignored),
        "placeholder" => Some(Recovery::Placeholder),
        "omitted" => Some(Recovery::Omitted),
        "removed" => Some(Recovery::Removed),
        "substituted" => Some(Recovery::Substituted),
        "flattened" => Some(Recovery::Flattened),
        "rasterized" => Some(Recovery::Rasterized),
        "metadata_omitted" => Some(Recovery::MetadataOmitted),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_element_record(
    scene: &SceneDocument,
    policy: &'static SceneSurfacePolicy,
    element_policy: &SceneElementPolicy,
    members: &[JsonMember],
    pointer: &str,
    source_order: u64,
    element_id: Option<&str>,
    format: OutputFormat,
    frame_names: bool,
    validate_values: bool,
    diagnostic_limit: usize,
    diagnostics: &mut Vec<SurfaceDiagnostic>,
) -> Result<(), TryReserveError> {
    for (position, member) in members.iter().enumerate() {
        if diagnostics.len() >= diagnostic_limit {
            break;
        }
        let semantic_path = pointer_child("", &member.key)?;
        if element_policy
            .ignored_inherited_paths
            .contains(&semantic_path.as_str())
        {
            continue;
        }
        if let Some(field) = field_policy(policy, element_policy.record_kind, &semantic_path) {
            if semantic_path == "/boundElements" {
                classify_bound_elements(
                    scene,
                    policy,
                    &member.value,
                    pointer,
                    position,
                    source_order,
                    element_id,
                    format,
                    diagnostic_limit,
                    diagnostics,
                )?;
            }
            if field_participates(field, format, frame_names)
                && semantic_path == "/link"
                && matches!(&member.value, JsonValue::String(link) if !safe_link(link))
            {
                diagnostics.try_reserve(1)?;
                diagnostics.push(SurfaceDiagnostic {
                    source_order,
                    source_position: u64::try_from(position)
                        .unwrap_or(u64::MAX)
                        .saturating_mul(u64::from(u32::MAX) + 1),
                    local_owner: element_id.map(try_owned).transpose()?,
                    diagnostic: value_rule_diagnostic(
                        DiagnosticCode::LINK_UNSAFE_OMITTED,
                        format,
                        &[DiagnosticEffect::Semantic],
                        element_id,
                        &pointer_child(pointer, &member.key)?,
                        Recovery::Omitted,
                        "unsafe link was omitted by the revision-1 sanitizer",
                    )?,
                });
                continue;
            }
            if validate_values
                && semantic_path == "/seed"
                && matches!(finite_json_number(&member.value), Some(0.0))
            {
                diagnostics.try_reserve(1)?;
                diagnostics.push(SurfaceDiagnostic {
                    source_order,
                    source_position: u64::try_from(position)
                        .unwrap_or(u64::MAX)
                        .saturating_mul(u64::from(u32::MAX) + 1),
                    local_owner: element_id.map(try_owned).transpose()?,
                    diagnostic: value_rule_diagnostic(
                        DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                        format,
                        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                        element_id,
                        &pointer_child(pointer, &member.key)?,
                        Recovery::Repaired,
                        "zero Rough seed was repaired deterministically",
                    )?,
                });
                continue;
            }
            if (validate_values || surface_validates_element_field(field))
                && field.disposition != SceneFieldDisposition::OpaqueMetadata
                && field_participates(field, format, frame_names)
                && field.invalid.code.is_some()
                && !value_matches_domain(&member.value, field.accepted_domain)
            {
                if let Some(diagnostic) = manifested_field_diagnostic(
                    &field.invalid,
                    format,
                    &pointer_child(pointer, &member.key)?,
                    element_id,
                )? {
                    diagnostics.try_reserve(1)?;
                    diagnostics.push(SurfaceDiagnostic {
                        source_order,
                        source_position: u64::try_from(position)
                            .unwrap_or(u64::MAX)
                            .saturating_mul(u64::from(u32::MAX) + 1),
                        local_owner: element_id.map(try_owned).transpose()?,
                        diagnostic,
                    });
                }
            }
            continue;
        }
        diagnostics.try_reserve(1)?;
        diagnostics.push(SurfaceDiagnostic {
            source_order,
            source_position: u64::try_from(position)
                .unwrap_or(u64::MAX)
                .saturating_mul(u64::from(u32::MAX) + 1),
            local_owner: element_id.map(try_owned).transpose()?,
            diagnostic: CompatibilityDiagnostic {
                code: DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                severity: DiagnosticSeverity::Warning,
                format,
                effects: owned_effects(&[
                    DiagnosticEffect::Visual,
                    DiagnosticEffect::Semantic,
                    DiagnosticEffect::Metadata,
                ])?,
                element_id: element_id.map(try_owned).transpose()?,
                json_pointer: pointer_child(pointer, &member.key)?,
                message: try_owned(
                    "unmatched element property was ignored by the revision-1 scene-surface policy",
                )?,
                details: Some(Box::new(ErrorDetails::Compatibility {
                    recovery: Recovery::Ignored,
                    recipe_id: None,
                })),
                order: DiagnosticOrder::default(),
            },
        });
    }
    Ok(())
}

fn surface_validates_element_field(field: &SceneFieldPolicy) -> bool {
    matches!(
        field.accepted_domain,
        "bound_element_array" | "css_color" | "string_array"
    ) || matches!(
        field.path,
        "/link" | "/strokeSharpness" | "/boundElementIds"
    )
}

fn field_participates(field: &SceneFieldPolicy, format: OutputFormat, frame_names: bool) -> bool {
    field.formats.contains(&format.as_str())
        && (field.participation != "participating-frame-when-frame-name-rendering-is-enabled"
            || frame_names)
}

pub(crate) fn safe_link(link: &str) -> bool {
    matches!(sanitized_link(link), Ok(Some(_)))
}

pub(crate) fn sanitized_link(link: &str) -> Result<Option<String>, TryReserveError> {
    let decoded = decode_numeric_html_entities(link)?;
    let mut sanitized = String::new();
    sanitized.try_reserve_exact(decoded.len())?;
    let mut index = 0;
    while index < decoded.len() {
        let remaining = &decoded[index..];
        if remaining
            .get(..9)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("&newline;"))
        {
            index += 9;
            continue;
        }
        if remaining
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("&tab;"))
        {
            index += 5;
            continue;
        }
        let Some(character) = remaining.chars().next() else {
            break;
        };
        if !matches!(
            u32::from(character),
            0x0000..=0x001f | 0x007f..=0x009f | 0x2000..=0x200d | 0xfeff
        ) {
            sanitized.push(character);
        }
        index += character.len_utf8();
    }
    let sanitized = sanitized.trim();
    if sanitized.is_empty() {
        return Ok(None);
    }
    if matches!(sanitized.as_bytes().first(), Some(b'.' | b'/')) {
        return try_owned(sanitized).map(Some);
    }
    if sanitized.contains(':') || contains_ascii_case_insensitive(sanitized, b"&colon;") {
        let protocol = sanitized.trim_start_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '_'
        });
        if starts_with_ascii_case_insensitive(protocol, b"javascript")
            || starts_with_ascii_case_insensitive(protocol, b"data")
            || starts_with_ascii_case_insensitive(protocol, b"vbscript")
        {
            return Ok(None);
        }
    }
    try_owned(sanitized).map(Some)
}

fn decode_numeric_html_entities(link: &str) -> Result<String, TryReserveError> {
    let mut decoded = String::new();
    decoded.try_reserve_exact(link.len())?;
    let mut remaining = link;
    while let Some(start) = remaining.find("&#") {
        decoded.push_str(&remaining[..start]);
        let entity = &remaining[start + 2..];
        let word_bytes = entity
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            .count();
        if word_bytes == 0 {
            decoded.push_str("&#");
            remaining = entity;
            continue;
        }
        let token = &entity[..word_bytes];
        let number = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .map_or_else(
                || token.parse::<f64>().unwrap_or(f64::NAN),
                |hex| {
                    hex.bytes().fold(0.0, |value, digit| {
                        value * 16.0
                            + f64::from(match digit {
                                b'0'..=b'9' => digit - b'0',
                                b'a'..=b'f' => digit - b'a' + 10,
                                b'A'..=b'F' => digit - b'A' + 10,
                                _ => 0,
                            })
                    })
                },
            );
        let value = if number.is_finite() {
            number.trunc().rem_euclid(65_536.0) as u32
        } else {
            0
        };
        decoded.push(char::from_u32(value).unwrap_or('\0'));
        let suffix = usize::from(entity.as_bytes().get(word_bytes) == Some(&b';'));
        remaining = &entity[word_bytes + suffix..];
    }
    decoded.push_str(remaining);
    Ok(decoded)
}

fn starts_with_ascii_case_insensitive(value: &str, prefix: &[u8]) -> bool {
    value
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

fn contains_ascii_case_insensitive(value: &str, needle: &[u8]) -> bool {
    value
        .as_bytes()
        .windows(needle.len())
        .any(|candidate| candidate.eq_ignore_ascii_case(needle))
}

#[allow(clippy::too_many_arguments)]
fn value_rule_diagnostic(
    code: DiagnosticCode,
    format: OutputFormat,
    effects: &[DiagnosticEffect],
    element_id: Option<&str>,
    json_pointer: &str,
    recovery: Recovery,
    message: &str,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects(effects)?,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

#[allow(clippy::too_many_arguments)]
fn classify_bound_elements(
    scene: &SceneDocument,
    policy: &'static SceneSurfacePolicy,
    value: &JsonValue,
    element_pointer: &str,
    member_position: usize,
    source_order: u64,
    element_id: Option<&str>,
    format: OutputFormat,
    diagnostic_limit: usize,
    diagnostics: &mut Vec<SurfaceDiagnostic>,
) -> Result<(), TryReserveError> {
    let JsonValue::Array(values) = value else {
        return Ok(());
    };
    for (index, value) in values.iter().enumerate() {
        if diagnostics.len() >= diagnostic_limit {
            break;
        }
        let JsonValue::Object(members) = value else {
            continue;
        };
        let target_id = match member_value(members, "id") {
            Some(JsonValue::String(id)) if !id.is_empty() => Some(id.as_ref()),
            _ => None,
        };
        if target_id.is_some_and(|target| !participating_element_id_exists(scene, target)) {
            continue;
        }
        let record_pointer =
            pointer_index(&pointer_child(element_pointer, "boundElements")?, index)?;
        for (nested_position, member) in members.iter().enumerate() {
            if diagnostics.len() >= diagnostic_limit {
                break;
            }
            let path = pointer_child("", &member.key)?;
            let pointer = pointer_child(&record_pointer, &member.key)?;
            let diagnostic = match field_policy(policy, "nested.bound_element", &path) {
                Some(field) if !value_matches_domain(&member.value, field.accepted_domain) => {
                    manifested_field_diagnostic(&field.invalid, format, &pointer, element_id)?
                }
                Some(_) => None,
                None => Some(unsupported_property_diagnostic(
                    format,
                    element_id,
                    &pointer,
                    "unmatched bound-element property was ignored",
                )?),
            };
            let Some(diagnostic) = diagnostic else {
                continue;
            };
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: u64::try_from(member_position)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(u64::from(u32::MAX) + 1)
                    .saturating_add(
                        u64::try_from(index.saturating_add(1))
                            .unwrap_or(u64::MAX)
                            .saturating_mul(u64::from(u16::MAX) + 1),
                    )
                    .saturating_add(
                        u64::try_from(nested_position.saturating_add(1)).unwrap_or(u64::MAX),
                    ),
                local_owner: element_id.map(try_owned).transpose()?,
                diagnostic,
            });
        }
    }
    Ok(())
}

fn participating_element_id_exists(scene: &SceneDocument, expected: &str) -> bool {
    (0..scene.element_occurrence_count()).any(|index| {
        if scene.element_is_silent(index) {
            return false;
        }
        let Some(JsonValue::Object(element)) = scene.element_value(index) else {
            return false;
        };
        matches!(member_value(element, "id"), Some(JsonValue::String(id)) if &**id == expected)
    })
}

fn unsupported_property_diagnostic(
    format: OutputFormat,
    element_id: Option<&str>,
    json_pointer: &str,
    message: &str,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects(&[
            DiagnosticEffect::Visual,
            DiagnosticEffect::Semantic,
            DiagnosticEffect::Metadata,
        ])?,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: Recovery::Ignored,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(crate) fn classify_referenced_files(
    scene: &SceneDocument,
    policy: &'static SceneSurfacePolicy,
    referenced_file_ids: &[&str],
    format: OutputFormat,
    diagnostic_limit: usize,
) -> Result<Vec<SurfaceDiagnostic>, TryReserveError> {
    let mut diagnostics = Vec::new();
    for index in 0..scene.file_record_count() {
        if diagnostics.len() >= diagnostic_limit {
            break;
        }
        let Some(file) = scene.file_record_at(index) else {
            continue;
        };
        if referenced_file_ids.binary_search(&file.key()).is_err() {
            continue;
        }
        let pointer = pointer_child("/files", file.key())?;
        let source_order = scene.source_owner_order("files", index);
        if file.key().is_empty() {
            let (recovery, recipe_id) = malformed_file_recovery(scene, file.key(), format);
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: 0,
                local_owner: Some(String::new()),
                diagnostic: asset_diagnostic(
                    DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
                    format,
                    file.key(),
                    &pointer,
                    recovery,
                    recipe_id,
                    "referenced authoritative file key was malformed",
                )?,
            });
            continue;
        }
        let Some(members) = file.members() else {
            let (recovery, recipe_id) = malformed_file_recovery(scene, file.key(), format);
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: 0,
                local_owner: Some(try_owned(file.key())?),
                diagnostic: asset_diagnostic(
                    DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
                    format,
                    file.key(),
                    &pointer,
                    recovery,
                    recipe_id,
                    "referenced authoritative file record was malformed",
                )?,
            });
            continue;
        };
        if member_value(members, "dataURL").is_none() {
            let (recovery, recipe_id) = match format {
                OutputFormat::Png => {
                    let recipe = if referenced_image_has_error_status(scene, file.key()) {
                        "official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9"
                    } else {
                        "official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d"
                    };
                    (Recovery::Placeholder, Some(recipe))
                }
                OutputFormat::Svg | OutputFormat::Pdf => (Recovery::Omitted, None),
            };
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: field_policy(policy, "file_record", "/dataURL")
                    .map_or(u64::MAX - 1, |field| {
                        u64::try_from(field.ordinal).unwrap_or(u64::MAX - 1)
                    }),
                local_owner: Some(try_owned(file.key())?),
                diagnostic: asset_diagnostic(
                    DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
                    format,
                    file.key(),
                    &pointer_child(&pointer, "dataURL")?,
                    recovery,
                    recipe_id,
                    "referenced authoritative file record had no payload",
                )?,
            });
        }
        for (position, member) in members.iter().enumerate() {
            if diagnostics.len() >= diagnostic_limit {
                break;
            }
            let semantic_path = pointer_child("", &member.key)?;
            if let Some(field) = field_policy(policy, "file_record", &semantic_path) {
                if field.invalid.code.is_some()
                    && !value_matches_domain(&member.value, field.accepted_domain)
                {
                    let member_pointer = pointer_child(&pointer, &member.key)?;
                    let diagnostic = if semantic_path == "/dataURL"
                        && matches!(&member.value, JsonValue::String(url) if !url.starts_with("data:"))
                    {
                        forbidden_resource_diagnostic(
                            format,
                            file.key(),
                            &member_pointer,
                            referenced_image_has_safe_bounds(scene, file.key()),
                        )?
                    } else if field.invalid.code
                        == Some(DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH.as_str())
                    {
                        asset_media_type_diagnostic(
                            format,
                            file.key(),
                            &member_pointer,
                            match &member.value {
                                JsonValue::String(media_type) => Some(media_type.as_ref()),
                                _ => None,
                            },
                            None,
                        )?
                    } else if let Some(diagnostic) =
                        manifested_field_diagnostic(&field.invalid, format, &member_pointer, None)?
                    {
                        diagnostic
                    } else {
                        continue;
                    };
                    diagnostics.try_reserve(1)?;
                    diagnostics.push(SurfaceDiagnostic {
                        source_order,
                        source_position: u64::try_from(position).unwrap_or(u64::MAX),
                        local_owner: Some(try_owned(file.key())?),
                        diagnostic,
                    });
                }
                continue;
            }
            diagnostics.try_reserve(1)?;
            diagnostics.push(SurfaceDiagnostic {
                source_order,
                source_position: u64::try_from(position).unwrap_or(u64::MAX),
                local_owner: Some(try_owned(file.key())?),
                diagnostic: CompatibilityDiagnostic {
                    code: DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                    severity: DiagnosticSeverity::Warning,
                    format,
                    effects: owned_effects(&[
                        DiagnosticEffect::Visual,
                        DiagnosticEffect::Semantic,
                        DiagnosticEffect::Metadata,
                    ])?,
                    element_id: None,
                    json_pointer: pointer_child(&pointer, &member.key)?,
                    message: try_owned(
                        "unmatched file property was ignored by the revision-1 scene-surface policy",
                    )?,
                    details: Some(Box::new(ErrorDetails::Compatibility {
                        recovery: Recovery::Ignored,
                        recipe_id: None,
                    })),
                    order: DiagnosticOrder::default(),
                },
            });
        }
    }
    Ok(diagnostics)
}

fn malformed_file_recovery(
    scene: &SceneDocument,
    file_id: &str,
    format: OutputFormat,
) -> (Recovery, Option<&'static str>) {
    match format {
        OutputFormat::Png => {
            let recipe = if referenced_image_has_error_status(scene, file_id) {
                "official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9"
            } else {
                "official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d"
            };
            (Recovery::Placeholder, Some(recipe))
        }
        OutputFormat::Svg | OutputFormat::Pdf => (Recovery::Omitted, None),
    }
}

pub(crate) fn supplement_conflict(
    scene: &SceneDocument,
    file_id: &str,
    format: OutputFormat,
) -> Result<Option<SurfaceDiagnostic>, TryReserveError> {
    for index in 0..scene.file_record_count() {
        let Some(file) = scene.file_record_at(index) else {
            continue;
        };
        if file.key() != file_id {
            continue;
        }
        let pointer = pointer_child("/files", file_id)?;
        return Ok(Some(SurfaceDiagnostic {
            source_order: scene.source_owner_order("files", index),
            source_position: 0,
            local_owner: Some(try_owned(file_id)?),
            diagnostic: CompatibilityDiagnostic {
                code: DiagnosticCode::ASSET_SUPPLEMENT_CONFLICT,
                severity: DiagnosticSeverity::Warning,
                format,
                effects: owned_effects(&[DiagnosticEffect::InputRepair])?,
                element_id: None,
                json_pointer: pointer,
                message: try_owned("scene file remained authoritative over supplemental asset")?,
                details: Some(Box::new(ErrorDetails::Asset {
                    asset_id: try_owned(file_id)?,
                    claimed_media_type: None,
                    validated_media_type: None,
                    recovery: Recovery::Ignored,
                    recipe_id: None,
                })),
                order: DiagnosticOrder::default(),
            },
        }));
    }
    Ok(None)
}

fn referenced_image_has_error_status(scene: &SceneDocument, file_id: &str) -> bool {
    (0..scene.element_occurrence_count()).any(|index| {
        let Some(JsonValue::Object(element)) = scene.element_value(index) else {
            return false;
        };
        matches!(member_value(element, "type"), Some(JsonValue::String(kind)) if &**kind == "image")
            && matches!(member_value(element, "fileId"), Some(JsonValue::String(id)) if &**id == file_id)
            && matches!(member_value(element, "status"), Some(JsonValue::String(status)) if &**status == "error")
    })
}

fn referenced_image_has_safe_bounds(scene: &SceneDocument, file_id: &str) -> bool {
    (0..scene.element_occurrence_count()).any(|index| {
        let Some(JsonValue::Object(element)) = scene.element_value(index) else {
            return false;
        };
        matches!(member_value(element, "type"), Some(JsonValue::String(kind)) if &**kind == "image")
            && matches!(member_value(element, "fileId"), Some(JsonValue::String(id)) if &**id == file_id)
            && ["width", "height"].into_iter().all(|field| {
                member_value(element, field)
                    .and_then(finite_json_number)
                    .is_some_and(|value| value != 0.0)
            })
    })
}

fn forbidden_resource_diagnostic(
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    has_safe_bounds: bool,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    let recovery = if has_safe_bounds {
        Recovery::Placeholder
    } else {
        Recovery::Omitted
    };
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::IMAGE_FORBIDDEN_RESOURCE,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects(&[DiagnosticEffect::Visual, DiagnosticEffect::Semantic])?,
        element_id: None,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("non-local image resource was rejected")?,
        details: Some(Box::new(ErrorDetails::Asset {
            asset_id: try_owned(asset_id)?,
            claimed_media_type: None,
            validated_media_type: None,
            recovery,
            recipe_id: has_safe_bounds
                .then(|| {
                    try_owned("official.forbidden-resource@sha256:4452f99158233e3703e46dbbb83f6900a894fbceacd3e98b141332d552e22827")
                })
                .transpose()?,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn asset_diagnostic(
    code: DiagnosticCode,
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    recovery: Recovery,
    recipe_id: Option<&str>,
    message: &str,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects(&[DiagnosticEffect::Visual])?,
        element_id: None,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(ErrorDetails::Asset {
            asset_id: try_owned(asset_id)?,
            claimed_media_type: None,
            validated_media_type: None,
            recovery,
            recipe_id: recipe_id.map(try_owned).transpose()?,
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(crate) fn asset_media_type_diagnostic(
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    claimed_media_type: Option<&str>,
    validated_media_type: Option<&str>,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects(&[DiagnosticEffect::InputRepair])?,
        element_id: None,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(if validated_media_type.is_some() {
            "asset media type did not match the validated payload type"
        } else {
            "asset media type was outside the revision-1 admitted domain"
        })?,
        details: Some(Box::new(ErrorDetails::Asset {
            asset_id: try_owned(asset_id)?,
            claimed_media_type: claimed_media_type.map(try_owned).transpose()?,
            validated_media_type: validated_media_type.map(try_owned).transpose()?,
            recovery: Recovery::Repaired,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

#[derive(Clone, Copy)]
struct RecoverableBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    opacity: f64,
}

fn recoverable_bounds(members: &[JsonMember]) -> Option<RecoverableBounds> {
    let optional_coordinate = |field| match member_value(members, field) {
        Some(value) => finite_json_number(value),
        None => Some(0.0),
    };
    let mut x = optional_coordinate("x")?;
    let mut y = optional_coordinate("y")?;
    let source_width = member_value(members, "width").and_then(finite_json_number)?;
    let source_height = member_value(members, "height").and_then(finite_json_number)?;
    if source_width == 0.0 || source_height == 0.0 {
        return None;
    }
    if source_width < 0.0 {
        x += source_width;
    }
    if source_height < 0.0 {
        y += source_height;
    }
    let angle = member_value(members, "angle")
        .and_then(finite_json_number)
        .unwrap_or(0.0);
    let opacity = member_value(members, "opacity")
        .and_then(finite_json_number)
        .filter(|opacity| (0.0..=100.0).contains(opacity))
        .unwrap_or(100.0);
    Some(RecoverableBounds {
        x: canonical_zero(x),
        y: canonical_zero(y),
        width: canonical_zero(source_width.abs()),
        height: canonical_zero(source_height.abs()),
        angle: canonical_zero(angle),
        opacity: canonical_zero(opacity),
    })
}

#[allow(clippy::too_many_arguments)]
fn surface_fallback(
    source_order: u64,
    element_id: Option<&str>,
    json_pointer: &str,
    label: &str,
    link: Option<String>,
    frame_id: Option<&str>,
    is_frame: bool,
    bounds: RecoverableBounds,
) -> Result<SurfaceFallback, TryReserveError> {
    Ok(SurfaceFallback {
        source_order,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        label: try_owned(label)?,
        link,
        frame_id: frame_id.map(try_owned).transpose()?,
        is_frame,
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        angle: bounds.angle,
        opacity: bounds.opacity,
        paint: true,
    })
}

fn fallback_link(members: &[JsonMember]) -> Result<Option<String>, TryReserveError> {
    match member_value(members, "link") {
        Some(JsonValue::String(link)) => sanitized_link(link),
        _ => Ok(None),
    }
}

fn fallback_frame_id(members: &[JsonMember]) -> Option<&str> {
    match member_value(members, "frameId") {
        Some(JsonValue::String(frame_id)) if !frame_id.is_empty() => Some(frame_id),
        _ => None,
    }
}

fn fallback_label(prefix: &str, kind: &str) -> Result<String, TryReserveError> {
    let mut label = String::new();
    label.try_reserve_exact(prefix.len().saturating_add(kind.len()))?;
    label.push_str(prefix);
    label.push_str(kind);
    Ok(label)
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[allow(clippy::too_many_arguments)]
fn element_boundary_diagnostic(
    code: DiagnosticCode,
    format: OutputFormat,
    effects: &[DiagnosticEffect],
    element_id: Option<&str>,
    json_pointer: &str,
    recovery: Recovery,
    recipe_id: Option<&str>,
    message: &str,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects(effects)?,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery,
            recipe_id: recipe_id.map(try_owned).transpose()?,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn owned_effects(effects: &[DiagnosticEffect]) -> Result<Vec<DiagnosticEffect>, TryReserveError> {
    let mut owned = Vec::new();
    owned.try_reserve_exact(effects.len())?;
    owned.extend_from_slice(effects);
    Ok(owned)
}

fn try_owned(value: &str) -> Result<String, TryReserveError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len())?;
    owned.push_str(value);
    Ok(owned)
}

fn field_policy<'a>(
    policy: &'a SceneSurfacePolicy,
    record_kind: &str,
    path: &str,
) -> Option<&'a SceneFieldPolicy> {
    let record = policy
        .record_kinds
        .iter()
        .find(|record| record.identity == record_kind)?;
    policy
        .fields
        .iter()
        .find(|field| field.record_kind == record_kind && field.path == path)
        .or_else(|| {
            record
                .inherits
                .iter()
                .find_map(|inherited| field_policy(policy, inherited, path))
        })
}

pub(crate) fn manifested_element_field<'a>(
    policy: &'a SceneSurfacePolicy,
    kind: &str,
    path: &str,
) -> Option<&'a SceneFieldPolicy> {
    let record_kind = if kind == "draw" {
        "element.legacy_draw"
    } else {
        policy
            .element_kinds
            .iter()
            .find(|element| element.kind == kind)?
            .record_kind
    };
    field_policy(policy, record_kind, path)
}

pub(crate) fn is_manifested_path(
    policy: &SceneSurfacePolicy,
    record_kind: &str,
    path: &str,
) -> bool {
    field_policy(policy, record_kind, path).is_some()
}

pub(crate) fn element_ignores_inherited_path(
    policy: &SceneSurfacePolicy,
    kind: &str,
    path: &str,
) -> bool {
    policy
        .element_kinds
        .iter()
        .find(|element| element.kind == kind)
        .is_some_and(|element| element.ignored_inherited_paths.contains(&path))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{css_color_is_admitted, SceneFieldDisposition, SCENE_SURFACE_POLICY};

    #[test]
    fn css_color_domain_rejects_malformed_functions_and_accepts_case_insensitively() {
        assert!(css_color_is_admitted("RGB(30 30 30)"));
        assert!(!css_color_is_admitted("rgb(future)"));
    }

    #[test]
    fn generated_policy_has_exact_unique_manifest_ownership() {
        let policy = &SCENE_SURFACE_POLICY;
        assert_eq!(policy.record_kinds.len(), 29);
        assert_eq!(policy.fields.len(), 202);
        assert_eq!(policy.element_kinds.len(), 13);
        assert_eq!(policy.diagnostic_codes.len(), 42);

        let fields = policy
            .fields
            .iter()
            .enumerate()
            .map(|(ordinal, field)| {
                assert_eq!(field.ordinal, ordinal);
                (field.record_kind, field.path)
            })
            .collect::<BTreeSet<_>>();
        let declared = policy
            .record_kinds
            .iter()
            .flat_map(|record| {
                record
                    .field_paths
                    .iter()
                    .map(move |path| (record.identity, *path))
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(fields, declared);
        assert_eq!(
            policy
                .fields
                .iter()
                .filter(|field| field.disposition == SceneFieldDisposition::Supported)
                .count(),
            88
        );
        assert_eq!(
            policy
                .fields
                .iter()
                .filter(|field| {
                    field.disposition == SceneFieldDisposition::StaticExportIrrelevant
                })
                .count(),
            75
        );
        assert_eq!(
            policy
                .fields
                .iter()
                .filter(|field| field.disposition == SceneFieldDisposition::OpaqueMetadata)
                .count(),
            39
        );
    }
}
