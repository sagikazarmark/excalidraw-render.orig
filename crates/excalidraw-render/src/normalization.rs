use std::collections::TryReserveError;

use sha2::{Digest, Sha256};

use crate::outcome::DiagnosticOrder;
use crate::scene::{pointer_child, pointer_index, SourceField};
use crate::{
    CompatibilityDiagnostic, CompatibilityRevision, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, ErrorDetails, OutputFormat, Recovery, SceneDocument,
};

const MISSING_ID_DOMAIN: &[u8] = b"excalidraw-render:missing-element-id:v1\0";
const DUPLICATE_ID_DOMAIN: &[u8] = b"excalidraw-render:duplicate-element-id:v1\0";
const ID_COLLISION_DOMAIN: &[u8] = b"excalidraw-render:element-id-collision:v1\0";
const ROUGH_SEED_DOMAIN: &[u8] = b"excalidraw-render:rough-seed:v1\0";
const NORMALIZED_FIELD_COUNT: u64 = 15;
const SCALAR_REFERENCE_FIELD_COUNT: u64 = 10;
const GENERATED_ID_BYTES: usize = "generated-".len() + 64;
pub(crate) const NORMALIZATION_WORK_UNITS: u64 = NORMALIZED_FIELD_COUNT * 8;

pub(crate) struct NormalizedRectangle {
    pub(crate) source: NormalizedElementSource,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) angle: f64,
    pub(crate) stroke_color: String,
    pub(crate) background_color: String,
    pub(crate) fill_style: String,
    pub(crate) stroke_width: f64,
    pub(crate) stroke_style: String,
    pub(crate) roughness: f64,
    pub(crate) opacity: f64,
    pub(crate) seed: i32,
    pub(crate) view_background_color: String,
    pub(crate) normalized_working_bytes: u64,
    pub(crate) normalization_work_units: u64,
    pub(crate) repair_work_units: u64,
}

pub(crate) struct NormalizedElementSource {
    id: String,
    pointer: String,
}

impl NormalizedElementSource {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn pointer(&self) -> &str {
        &self.pointer
    }
}

pub(crate) struct NormalizationOutcome {
    pub(crate) rectangles: Vec<NormalizedRectangle>,
}

pub(crate) enum NormalizationEvent<'a> {
    PreflightRecord {
        json_pointer: &'a str,
        working_bytes: u64,
        work_units: u64,
    },
    CommitRecord {
        element_id: &'a str,
        json_pointer: &'a str,
        working_bytes: u64,
        work_units: u64,
    },
    Diagnostic(CompatibilityDiagnostic),
}

pub(crate) enum NormalizationError<E> {
    Observer(E),
    MemoryUnavailable { source_index: Option<usize> },
}

impl<E> From<TryReserveError> for NormalizationError<E> {
    fn from(_: TryReserveError) -> Self {
        Self::MemoryUnavailable { source_index: None }
    }
}

struct RectangleFields<'a> {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    stroke_color: &'a str,
    background_color: &'a str,
    fill_style: &'a str,
    stroke_width: f64,
    stroke_style: &'a str,
    roughness: f64,
    opacity: f64,
    source_seed: i32,
    view_background_color: &'a str,
}

enum IdentityPlan<'a> {
    Retained(&'a str),
    Duplicate(&'a str),
    Missing,
}

pub(crate) fn normalize_rectangles<E>(
    scene: &SceneDocument,
    revision: CompatibilityRevision,
    format: OutputFormat,
    mut observe: impl FnMut(NormalizationEvent<'_>) -> Result<(), E>,
) -> Result<Option<NormalizationOutcome>, NormalizationError<E>> {
    let element_count = scene.element_occurrence_count();
    let mut rectangles = Vec::new();
    let mut used_ids = Vec::<String>::new();

    for index in 0..element_count {
        macro_rules! memory_at_source {
            ($result:expr) => {
                $result.map_err(|_| NormalizationError::MemoryUnavailable {
                    source_index: Some(index),
                })?
            };
        }
        if scene.element_is_silent(index) {
            continue;
        }
        let Some(source) = scene.rectangle_at(index) else {
            return Ok(None);
        };
        let element_pointer = memory_at_source!(pointer_index("/elements", index));

        let Some(is_deleted) = defaulted(source.is_deleted, false) else {
            return Ok(None);
        };
        if is_deleted {
            continue;
        }
        let Some(fields) = rectangle_fields(&source) else {
            return Ok(None);
        };

        let identity_plan = match source.id {
            SourceField::Value("") | SourceField::Invalid => return Ok(None),
            SourceField::Value(id) if used_ids.iter().any(|used| used == id) => {
                IdentityPlan::Duplicate(id)
            }
            SourceField::Value(id) => IdentityPlan::Retained(id),
            SourceField::Missing => IdentityPlan::Missing,
        };
        let normalized_id_bytes = match identity_plan {
            IdentityPlan::Retained(id) => id.len(),
            IdentityPlan::Duplicate(_) | IdentityPlan::Missing => GENERATED_ID_BYTES,
        };
        let normalized_working_bytes = normalized_working_bytes(normalized_id_bytes, &fields);
        observe(NormalizationEvent::PreflightRecord {
            json_pointer: &element_pointer,
            working_bytes: normalized_working_bytes,
            work_units: NORMALIZATION_WORK_UNITS,
        })
        .map_err(NormalizationError::Observer)?;

        let mut id_diagnostic = None;
        let id = match identity_plan {
            IdentityPlan::Retained(id) => memory_at_source!(try_owned(id)),
            IdentityPlan::Duplicate(original_id) => {
                let canonical_element = memory_at_source!(source.canonical_element_bytes());
                let id = memory_at_source!(generated_duplicate_element_id(
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    original_id,
                    &canonical_element,
                ));
                let id = memory_at_source!(unique_generated_id(
                    id,
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &used_ids,
                ));
                id_diagnostic = Some((
                    DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID,
                    "duplicate element identity was rekeyed deterministically",
                ));
                id
            }
            IdentityPlan::Missing => {
                let canonical_element = memory_at_source!(source.canonical_element_bytes());
                let id = memory_at_source!(generated_element_id(
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &canonical_element,
                ));
                let id = memory_at_source!(unique_generated_id(
                    id,
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &used_ids,
                ));
                id_diagnostic = Some((
                    DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
                    "missing element identity was repaired deterministically",
                ));
                id
            }
        };
        observe(NormalizationEvent::CommitRecord {
            element_id: &id,
            json_pointer: &element_pointer,
            working_bytes: normalized_working_bytes,
            work_units: NORMALIZATION_WORK_UNITS,
        })
        .map_err(NormalizationError::Observer)?;
        memory_at_source!(used_ids.try_reserve(1));
        used_ids.push(memory_at_source!(try_owned(&id)));

        let mut element_diagnostics = Vec::new();
        memory_at_source!(element_diagnostics.try_reserve_exact(3));
        if let Some((code, message)) = id_diagnostic {
            let pointer = memory_at_source!(pointer_child(&element_pointer, "id"));
            let diagnostic = memory_at_source!(repair_diagnostic(
                code,
                format,
                &[DiagnosticEffect::InputRepair],
                &id,
                &pointer,
                message,
            ));
            element_diagnostics.push((source.member_position("id").unwrap_or(0), diagnostic));
        }

        let seed = if fields.source_seed == 0 {
            let seed = generated_rough_seed(revision, &id);
            let pointer = memory_at_source!(pointer_child(&element_pointer, "seed"));
            let diagnostic = memory_at_source!(repair_diagnostic(
                DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                format,
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                &id,
                &pointer,
                "zero Rough seed was repaired deterministically",
            ));
            element_diagnostics.push((
                source.member_position("seed").unwrap_or(usize::MAX - 1),
                diagnostic,
            ));
            seed
        } else {
            fields.source_seed
        };

        if source.updated_is_missing {
            let pointer = memory_at_source!(pointer_child(&element_pointer, "updated"));
            let diagnostic = memory_at_source!(repair_diagnostic(
                DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                format,
                &[DiagnosticEffect::InputRepair],
                &id,
                &pointer,
                "missing required timestamp was repaired to canonical zero",
            ));
            element_diagnostics.push((usize::MAX, diagnostic));
        }

        element_diagnostics.sort_by(|(left_position, left), (right_position, right)| {
            left_position
                .cmp(right_position)
                .then_with(|| left.json_pointer.cmp(&right.json_pointer))
                .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        });
        let repair_work_units = u64::try_from(element_diagnostics.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(8);
        for (_, diagnostic) in element_diagnostics {
            observe(NormalizationEvent::Diagnostic(diagnostic))
                .map_err(NormalizationError::Observer)?;
        }

        memory_at_source!(rectangles.try_reserve(1));
        rectangles.push(NormalizedRectangle {
            source: NormalizedElementSource {
                id,
                pointer: element_pointer,
            },
            x: fields.x,
            y: fields.y,
            width: fields.width,
            height: fields.height,
            angle: fields.angle,
            stroke_color: memory_at_source!(try_owned(fields.stroke_color)),
            background_color: memory_at_source!(try_owned(fields.background_color)),
            fill_style: memory_at_source!(try_owned(fields.fill_style)),
            stroke_width: fields.stroke_width,
            stroke_style: memory_at_source!(try_owned(fields.stroke_style)),
            roughness: fields.roughness,
            opacity: fields.opacity,
            seed,
            view_background_color: memory_at_source!(try_owned(fields.view_background_color)),
            normalized_working_bytes,
            normalization_work_units: NORMALIZATION_WORK_UNITS,
            repair_work_units,
        });
    }

    Ok(Some(NormalizationOutcome { rectangles }))
}

fn rectangle_fields<'a>(source: &crate::scene::RectangleSource<'a>) -> Option<RectangleFields<'a>> {
    if !matches!(
        source.roundness,
        SourceField::Missing | SourceField::Value(())
    ) || !matches!(defaulted(source.group_ids, 0), Some(0))
        || !matches!(defaulted(source.frame_id, None), Some(None))
        || !matches!(defaulted(source.bound_elements, 0), Some(0))
        || !matches!(defaulted(source.link, None), Some(None))
    {
        return None;
    }
    let mut x = defaulted(source.x, 0.0)?;
    let mut y = defaulted(source.y, 0.0)?;
    let mut width = defaulted(source.width, 0.0)?;
    let mut height = defaulted(source.height, 0.0)?;
    if width < 0.0 {
        x += width;
    }
    width = canonical_zero(width.abs());
    if height < 0.0 {
        y += height;
    }
    height = canonical_zero(height.abs());
    Some(RectangleFields {
        x: canonical_zero(x),
        y: canonical_zero(y),
        width,
        height,
        angle: canonical_zero(defaulted(source.angle, 0.0)?),
        stroke_color: defaulted(source.stroke_color, "#1e1e1e")?,
        background_color: defaulted(source.background_color, "transparent")?,
        fill_style: defaulted(source.fill_style, "solid")?,
        stroke_width: defaulted(source.stroke_width, 2.0)?,
        stroke_style: defaulted(source.stroke_style, "solid")?,
        roughness: defaulted(source.roughness, 1.0)?,
        opacity: defaulted(source.opacity, 100.0)?,
        source_seed: defaulted(source.seed, 1)?,
        view_background_color: defaulted(source.view_background_color, "#ffffff")?,
    })
}

fn normalized_working_bytes(id_bytes: usize, fields: &RectangleFields<'_>) -> u64 {
    256 + SCALAR_REFERENCE_FIELD_COUNT * 64
        + owned_field_bytes(id_bytes)
        + owned_field_bytes(fields.stroke_color.len())
        + owned_field_bytes(fields.background_color.len())
        + owned_field_bytes(fields.fill_style.len())
        + owned_field_bytes(fields.stroke_style.len())
        + owned_field_bytes(fields.view_background_color.len())
}

fn owned_field_bytes(bytes: usize) -> u64 {
    64 + u64::try_from(bytes).unwrap_or(u64::MAX).saturating_add(63) / 64 * 64
}

fn generated_element_id(
    revision: CompatibilityRevision,
    source_index: u64,
    canonical_element: &[u8],
) -> Result<String, TryReserveError> {
    let mut hasher = Sha256::new();
    hasher.update(MISSING_ID_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(source_index.to_le_bytes());
    hasher.update(
        u64::try_from(canonical_element.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(canonical_element);
    generated_id_from_digest(&hasher.finalize())
}

fn generated_rough_seed(revision: CompatibilityRevision, normalized_id: &str) -> i32 {
    let mut hasher = Sha256::new();
    hasher.update(ROUGH_SEED_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(
        u64::try_from(normalized_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(normalized_id.as_bytes());
    let digest = hasher.finalize();
    let seed = i32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) & i32::MAX;
    if seed == 0 {
        1
    } else {
        seed
    }
}

fn generated_duplicate_element_id(
    revision: CompatibilityRevision,
    source_index: u64,
    original_id: &str,
    canonical_element: &[u8],
) -> Result<String, TryReserveError> {
    let mut hasher = Sha256::new();
    hasher.update(DUPLICATE_ID_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(source_index.to_le_bytes());
    hasher.update(
        u64::try_from(original_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(original_id.as_bytes());
    hasher.update(
        u64::try_from(canonical_element.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(canonical_element);
    generated_id_from_digest(&hasher.finalize())
}

fn generated_id_from_digest(digest: &[u8]) -> Result<String, TryReserveError> {
    let mut id = String::new();
    id.try_reserve_exact("generated-".len() + digest.len() * 2)?;
    id.push_str("generated-");
    push_lower_hex(&mut id, digest);
    Ok(id)
}

fn unique_generated_id(
    mut candidate: String,
    revision: CompatibilityRevision,
    source_index: u64,
    used_ids: &[String],
) -> Result<String, TryReserveError> {
    let base = try_owned(&candidate)?;
    let mut collision_ordinal = 0_u64;
    while used_ids.iter().any(|used| used == &candidate) {
        collision_ordinal = collision_ordinal.saturating_add(1);
        let mut hasher = Sha256::new();
        hasher.update(ID_COLLISION_DOMAIN);
        hasher.update(revision.get().to_le_bytes());
        hasher.update(source_index.to_le_bytes());
        hasher.update(collision_ordinal.to_le_bytes());
        hasher.update(u64::try_from(base.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(base.as_bytes());
        candidate = generated_id_from_digest(&hasher.finalize())?;
    }
    Ok(candidate)
}

fn defaulted<T: Copy>(field: SourceField<T>, default: T) -> Option<T> {
    match field {
        SourceField::Missing => Some(default),
        SourceField::Value(value) => Some(value),
        SourceField::Invalid => None,
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn repair_diagnostic(
    code: DiagnosticCode,
    format: OutputFormat,
    effects: &[DiagnosticEffect],
    element_id: &str,
    json_pointer: &str,
    message: &str,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    let mut owned_effects = Vec::new();
    owned_effects.try_reserve_exact(effects.len())?;
    owned_effects.extend_from_slice(effects);
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: Recovery::Repaired,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn try_owned(value: &str) -> Result<String, TryReserveError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len())?;
    owned.push_str(value);
    Ok(owned)
}

fn push_lower_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}
