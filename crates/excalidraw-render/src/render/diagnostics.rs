use super::*;

pub(super) fn diagnostic_replaces_element(diagnostic: &CompatibilityDiagnostic) -> bool {
    if !diagnostic.effects().contains(&DiagnosticEffect::Visual) {
        return false;
    }
    let recovery = match diagnostic.details() {
        Some(ErrorDetails::Compatibility { recovery, .. })
        | Some(ErrorDetails::Asset { recovery, .. })
        | Some(ErrorDetails::MissingGlyph { recovery, .. })
        | Some(ErrorDetails::Metadata { recovery, .. })
        | Some(ErrorDetails::NestedSource { recovery, .. }) => Some(*recovery),
        Some(ErrorDetails::ResourceLimit { recovery, .. }) => *recovery,
        Some(ErrorDetails::PdfRasterFallback { recovery, .. }) => *recovery,
        _ => None,
    };
    matches!(recovery, Some(Recovery::Placeholder | Recovery::Omitted))
}

pub(super) fn diagnostic_uses_non_generic_recovery(
    source: &NormalizedElement,
    diagnostic: &CompatibilityDiagnostic,
) -> bool {
    if !diagnostic_replaces_element(diagnostic) {
        return false;
    }
    let applies = diagnostic.element_id() == Some(source.source.id())
        || matches!(
            (&source.kind, diagnostic.details()),
            (
                NormalizedElementKind::Image(image),
                Some(ErrorDetails::Asset { asset_id, .. })
            ) if image.file_id.as_deref() == Some(asset_id.as_str())
        );
    if !applies {
        return false;
    }
    let recipe = match diagnostic.details() {
        Some(ErrorDetails::Compatibility { recipe_id, .. })
        | Some(ErrorDetails::Asset { recipe_id, .. }) => recipe_id.as_deref(),
        _ => None,
    };
    recipe != Some("renderer.known-element-unavailable.v1")
}

pub(super) fn known_element_replaced_diagnostic(
    format: OutputFormat,
    element_id: &str,
    json_pointer: &str,
    has_bounds: bool,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(2)?;
    effects.extend_from_slice(&[DiagnosticEffect::Visual, DiagnosticEffect::Semantic]);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(
            "known element unavailable to this renderer used the revision-1 fallback",
        )?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: if has_bounds {
                Recovery::Placeholder
            } else {
                Recovery::Omitted
            },
            recipe_id: has_bounds
                .then(|| try_owned("renderer.known-element-unavailable.v1"))
                .transpose()?,
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn image_asset_failure_diagnostic(
    failure: asset::ResolveError,
    format: OutputFormat,
    asset_id: &str,
    element_id: &str,
    json_pointer: &str,
    status: crate::normalization::NormalizedImageStatus,
    limits: &crate::ResourceLimits,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let (code, message, resource) = match failure {
        asset::ResolveError::Malformed => (
            DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
            "referenced authoritative file payload was malformed",
            None,
        ),
        asset::ResolveError::Unsupported => (
            DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT,
            "referenced image payload was not a supported raster format",
            None,
        ),
        asset::ResolveError::DecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced static PNG payload could not be decoded",
            None,
        ),
        asset::ResolveError::JpegDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced JPEG/JFIF payload could not be decoded",
            None,
        ),
        asset::ResolveError::GifDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced GIF payload could not be decoded",
            None,
        ),
        asset::ResolveError::WebPDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced WebP payload could not be decoded",
            None,
        ),
        asset::ResolveError::BmpDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced BMP payload could not be decoded",
            None,
        ),
        asset::ResolveError::IcoDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced ICO payload could not be decoded",
            None,
        ),
        asset::ResolveError::SvgDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced nested SVG payload could not be parsed and sanitized",
            None,
        ),
        asset::ResolveError::Missing => {
            unreachable!("missing assets are diagnosed by normalization")
        }
        asset::ResolveError::AssetBytes { attempted } => (
            DiagnosticCode::RESOURCE_ASSET_BYTES_EXCEEDED,
            "asset payload occurrence exceeded its byte ceiling",
            Some((
                "max_asset_bytes",
                limits.max_asset_bytes(),
                attempted,
                ValueKind::Bytes,
            )),
        ),
        asset::ResolveError::Dimensions { attempted } => (
            DiagnosticCode::RESOURCE_IMAGE_DIMENSIONS_EXCEEDED,
            "raster dimensions exceeded the local image ceiling",
            Some((
                "max_raster_dimension",
                u64::from(limits.max_raster_dimension()),
                attempted,
                ValueKind::Dimension,
            )),
        ),
        asset::ResolveError::Pixels { attempted } => (
            DiagnosticCode::RESOURCE_IMAGE_PIXELS_EXCEEDED,
            "raster pixels exceeded the local asset ceiling",
            Some((
                "max_decoded_pixels_per_asset",
                limits.max_decoded_pixels_per_asset(),
                attempted,
                ValueKind::Pixels,
            )),
        ),
        asset::ResolveError::AssetWork { attempted } => (
            DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED,
            "raster intrinsic work exceeded the local asset ceiling",
            Some((
                "max_asset_work_units",
                limits.max_asset_work_units(),
                attempted,
                ValueKind::WorkUnits,
            )),
        ),
        asset::ResolveError::SvgComplexity { attempted } => (
            DiagnosticCode::RESOURCE_NESTED_SVG_COMPLEXITY_EXCEEDED,
            "nested SVG exceeded the local node ceiling",
            Some((
                "max_nested_svg_nodes",
                u64::from(limits.max_nested_svg_nodes()),
                attempted,
                ValueKind::Count,
            )),
        ),
        asset::ResolveError::SvgDepth { attempted } => (
            DiagnosticCode::RESOURCE_NESTED_ASSET_DEPTH_EXCEEDED,
            "nested SVG resource exceeded the local asset-depth ceiling",
            Some((
                "max_nested_asset_depth",
                u64::from(limits.max_nested_asset_depth()),
                attempted,
                ValueKind::Depth,
            )),
        ),
        asset::ResolveError::Memory => unreachable!("memory failure is fatal before diagnostics"),
    };
    let (recovery, recipe_id) = match format {
        OutputFormat::Png => (
            Recovery::Placeholder,
            Some(match status {
                crate::normalization::NormalizedImageStatus::Error => "official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9",
                crate::normalization::NormalizedImageStatus::Pending
                | crate::normalization::NormalizedImageStatus::Saved => "official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d",
            }),
        ),
        OutputFormat::Svg | OutputFormat::Pdf => (Recovery::Omitted, None),
    };
    let mut effects = Vec::new();
    effects.try_reserve_exact(1)?;
    effects.push(DiagnosticEffect::Visual);
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(
            if let Some((field, configured, attempted, value_kind)) = resource {
                ErrorDetails::ResourceLimit {
                    field,
                    configured: ResourceValue::Integer(configured),
                    value_kind,
                    measurement: Measurement::Attempted {
                        value: ResourceValue::Integer(attempted),
                    },
                    recovery: Some(recovery),
                }
            } else {
                ErrorDetails::Asset {
                    asset_id: try_owned(asset_id)?,
                    claimed_media_type: None,
                    validated_media_type: match failure {
                        asset::ResolveError::DecodeFailed => Some(try_owned("image/png")?),
                        asset::ResolveError::JpegDecodeFailed => Some(try_owned("image/jpeg")?),
                        asset::ResolveError::GifDecodeFailed => Some(try_owned("image/gif")?),
                        asset::ResolveError::WebPDecodeFailed => Some(try_owned("image/webp")?),
                        asset::ResolveError::BmpDecodeFailed => Some(try_owned("image/bmp")?),
                        asset::ResolveError::IcoDecodeFailed => Some(try_owned("image/x-icon")?),
                        asset::ResolveError::SvgDecodeFailed => Some(try_owned("image/svg+xml")?),
                        _ => None,
                    },
                    recovery,
                    recipe_id: recipe_id.map(try_owned).transpose()?,
                }
            },
        )),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn image_animation_flattened_diagnostic(
    format: OutputFormat,
    asset_id: &str,
    element_id: &str,
    json_pointer: &str,
    media_type: &str,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(1)?;
    effects.push(DiagnosticEffect::Visual);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::IMAGE_ANIMATION_FLATTENED,
        severity: DiagnosticSeverity::Info,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("animated image was flattened to its first animation frame")?,
        details: Some(Box::new(ErrorDetails::Asset {
            asset_id: try_owned(asset_id)?,
            claimed_media_type: None,
            validated_media_type: Some(try_owned(media_type)?),
            recovery: Recovery::Flattened,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn pdf_raster_fallback_diagnostic(
    closure: &pdf::PdfRasterClosure,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(2)?;
    effects.extend_from_slice(&[DiagnosticEffect::Visual, DiagnosticEffect::Semantic]);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::PDF_RASTER_FALLBACK,
        severity: DiagnosticSeverity::Warning,
        format: OutputFormat::Pdf,
        effects,
        element_id: Some(try_owned(&closure.element_id)?),
        json_pointer: try_owned(&closure.json_pointer)?,
        message: try_owned("unsupported nested-SVG effect closure was rasterized for PDF")?,
        details: Some(Box::new(ErrorDetails::PdfRasterFallback {
            recovery: Some(Recovery::Rasterized),
            logical_bounds: Some(closure.logical_bounds),
            pixel_width: Some(closure.pixel_width),
            pixel_height: Some(closure.pixel_height),
            causes: closure.causes.clone(),
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn unsafe_svg_removal_diagnostic(
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    removal: &asset::svg::Removal,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(if removal.changes_appearance { 2 } else { 1 })?;
    effects.push(DiagnosticEffect::Semantic);
    if removal.changes_appearance {
        effects.push(DiagnosticEffect::Visual);
    }
    let mut asset_chain = Vec::new();
    asset_chain.try_reserve_exact(1)?;
    asset_chain.push(crate::AssetChainEntry {
        asset_id: Some(try_owned(asset_id)?),
        occurrence: 0,
    });
    let mut segments = Vec::new();
    segments.try_reserve_exact(if removal.attribute_ordinal.is_some() {
        2
    } else {
        1
    })?;
    segments.push(crate::NestedPathSegment {
        kind: crate::NestedSegmentKind::Node,
        ordinal: removal.node_ordinal,
        name: None,
    });
    if let Some(ordinal) = removal.attribute_ordinal {
        segments.push(crate::NestedPathSegment {
            kind: crate::NestedSegmentKind::Attribute,
            ordinal,
            name: Some(try_owned(&removal.name)?),
        });
    }
    let mut paths = Vec::new();
    paths.try_reserve_exact(1)?;
    paths.push(crate::NestedPath { segments });
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: None,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("unsafe nested SVG content was removed before rendering")?,
        details: Some(Box::new(ErrorDetails::NestedSource {
            asset_chain,
            paths,
            recovery: Recovery::Removed,
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn nested_image_failure_diagnostic(
    failure: &asset::svg::Failure,
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    limits: &crate::ResourceLimits,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut diagnostic = image_asset_failure_diagnostic(
        failure.error,
        format,
        asset_id,
        "",
        json_pointer,
        crate::normalization::NormalizedImageStatus::Saved,
        limits,
    )?;
    let mut asset_chain = Vec::new();
    asset_chain.try_reserve_exact(1)?;
    asset_chain.push(crate::AssetChainEntry {
        asset_id: Some(try_owned(asset_id)?),
        occurrence: 0,
    });
    let mut segments = Vec::new();
    segments.try_reserve_exact(failure.path.len().saturating_mul(2))?;
    for (node_ordinal, attribute_ordinal) in &failure.path {
        segments.push(crate::NestedPathSegment {
            kind: crate::NestedSegmentKind::Node,
            ordinal: *node_ordinal,
            name: None,
        });
        segments.push(crate::NestedPathSegment {
            kind: crate::NestedSegmentKind::Attribute,
            ordinal: *attribute_ordinal,
            name: Some(try_owned("href")?),
        });
    }
    let mut paths = Vec::new();
    paths.try_reserve_exact(1)?;
    paths.push(crate::NestedPath { segments });
    diagnostic.element_id = None;
    diagnostic.message = try_owned("nested data image was removed after local recovery")?;
    diagnostic.details = Some(Box::new(ErrorDetails::NestedSource {
        asset_chain,
        paths,
        recovery: Recovery::Removed,
    }));
    Ok(diagnostic)
}

pub(super) fn relationship_cycle_diagnostic(
    format: OutputFormat,
    element_id: &str,
    json_pointer: &str,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(2)?;
    effects.extend_from_slice(&[DiagnosticEffect::InputRepair, DiagnosticEffect::Semantic]);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::NORMALIZATION_RELATIONSHIP_CYCLE,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("relationship cycle-closing edge was removed deterministically")?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: Recovery::Removed,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn fallback_font_diagnostic(
    code: DiagnosticCode,
    severity: DiagnosticSeverity,
    format: OutputFormat,
    element_id: Option<&str>,
    json_pointer: &str,
    missing_codepoint: Option<u32>,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(1)?;
    effects.push(DiagnosticEffect::Visual);
    Ok(CompatibilityDiagnostic {
        code,
        severity,
        format,
        effects,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        // Non-normative wording (spec 11.1). The two Liberation Sans strings are
        // bound by approved expected reports and must not move; the Noto Emoji
        // one names the face that substitution actually used.
        message: try_owned(if missing_codepoint.is_some() {
            "generated fallback label used the deterministic missing-glyph replacement"
        } else if code == DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED {
            "emoji text used bundled Noto Emoji"
        } else {
            "generated fallback label used bundled Liberation Sans"
        })?,
        details: Some(Box::new(match missing_codepoint {
            Some(codepoint) => ErrorDetails::MissingGlyph {
                codepoint,
                recovery: Recovery::Substituted,
            },
            None => ErrorDetails::Compatibility {
                recovery: Recovery::Substituted,
                recipe_id: None,
            },
        })),
        order: DiagnosticOrder::default(),
    })
}

pub(super) fn official_missing_glyph_diagnostic(
    format: OutputFormat,
    element_id: &str,
    json_pointer: &str,
    codepoint: u32,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut diagnostic = fallback_font_diagnostic(
        DiagnosticCode::FONT_MISSING_GLYPH,
        DiagnosticSeverity::Warning,
        format,
        Some(element_id),
        json_pointer,
        Some(codepoint),
    )?;
    diagnostic.message =
        try_owned("official text used the deterministic basic-Latin replacement glyph")?;
    Ok(diagnostic)
}

pub(super) fn known_element_fallback(
    source: &NormalizedElement,
) -> Result<crate::scene_surface::SurfaceFallback, std::collections::TryReserveError> {
    let prefix = "Unavailable: ";
    let mut label = String::new();
    label.try_reserve_exact(prefix.len().saturating_add(source.kind_name.len()))?;
    label.push_str(prefix);
    label.push_str(source.kind_name);
    Ok(crate::scene_surface::SurfaceFallback {
        source_index: source.source.source_index(),
        source_order: source.source_order,
        element_id: Some(try_owned(source.source.id())?),
        json_pointer: try_owned(source.source.pointer())?,
        label,
        link: source.link.as_deref().map(try_owned).transpose()?,
        frame_id: source.frame_id.as_deref().map(try_owned).transpose()?,
        is_frame: matches!(source.kind_name, "frame" | "magicframe"),
        x: source.x,
        y: source.y,
        width: source.width,
        height: source.height,
        angle: source.angle,
        opacity: source.opacity,
        paint: true,
    })
}

pub(super) fn optional_metadata_omitted_diagnostic(
    format: OutputFormat,
    crossing: crate::resource_accounting::OptionalMetadataCrossing,
) -> CompatibilityDiagnostic {
    let measurement = match crossing.measurement {
        Measurement::Attempted { value } => Measurement::Attempted {
            value: ResourceValue::Integer(value),
        },
        Measurement::AtLeast { value } => Measurement::AtLeast {
            value: ResourceValue::Integer(value),
        },
    };
    CompatibilityDiagnostic {
        code: DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: vec![DiagnosticEffect::Metadata],
        element_id: None,
        json_pointer: String::new(),
        message: "optional artifact metadata was omitted because it exceeds a resource limit"
            .to_owned(),
        details: Some(Box::new(ErrorDetails::ResourceLimit {
            field: crossing.field,
            configured: ResourceValue::Integer(crossing.configured),
            value_kind: crossing.value_kind,
            measurement,
            recovery: Some(Recovery::MetadataOmitted),
        })),
        order: DiagnosticOrder::default(),
    }
}

pub(super) fn metadata_failure_diagnostic(
    format: OutputFormat,
    failure: MetadataFailure,
) -> CompatibilityDiagnostic {
    CompatibilityDiagnostic {
        code: failure.code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: vec![DiagnosticEffect::Metadata],
        element_id: None,
        json_pointer: String::new(),
        message: "optional artifact metadata component could not be retained".to_owned(),
        details: Some(Box::new(ErrorDetails::Metadata {
            component: failure.component,
            operation: failure.operation,
            recovery: Recovery::MetadataOmitted,
        })),
        order: DiagnosticOrder::default(),
    }
}
