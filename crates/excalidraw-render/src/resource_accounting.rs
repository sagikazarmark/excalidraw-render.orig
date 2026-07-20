use crate::{
    ErrorCode, ErrorDetails, Measurement, OutputFormat, Recovery, ResourceLimits,
    ResourceMeasurement, ResourceValue, ValueKind,
};
#[cfg(test)]
use crate::{ReferenceProfileIdentity, RenderRequest, SceneDocument};
use serde::Serialize;
use std::collections::TryReserveError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct ResourceDelta {
    pub(crate) scene_bytes: u64,
    pub(crate) elements: u64,
    pub(crate) diagnostics: u64,
    pub(crate) working_bytes: u64,
    pub(crate) output_bytes: u64,
    pub(crate) total_work_units: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
struct ResourceCounters {
    scene_bytes: u64,
    elements: u64,
    diagnostics: u64,
    working_bytes: u64,
    output_bytes: u64,
    total_work_units: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct ResourceContext<'a> {
    phase: ResourcePhase,
    event: &'static str,
    json_pointer: &'a str,
    element_id: Option<&'a str>,
    semantic_identity: Option<&'a str>,
    local_owner: Option<&'a str>,
    sharing_key: Option<&'a str>,
    record_kind: Option<ResourceRecordKind>,
    fallback_parent: Option<u64>,
}

impl<'a> ResourceContext<'a> {
    pub(crate) const fn new(
        phase: ResourcePhase,
        event: &'static str,
        json_pointer: &'a str,
    ) -> Self {
        Self {
            phase,
            event,
            json_pointer,
            element_id: None,
            semantic_identity: None,
            local_owner: None,
            sharing_key: None,
            record_kind: None,
            fallback_parent: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn with_element_id(mut self, element_id: Option<&'a str>) -> Self {
        self.element_id = element_id;
        self
    }

    #[allow(dead_code)]
    pub(crate) const fn with_local_owner(mut self, local_owner: Option<&'a str>) -> Self {
        self.local_owner = local_owner;
        self
    }

    pub(crate) const fn with_record(mut self, record_kind: ResourceRecordKind) -> Self {
        self.record_kind = Some(record_kind);
        self
    }

    pub(crate) fn try_failure_location(self) -> Result<ResourceFailureLocation, TryReserveError> {
        Ok(ResourceFailureLocation {
            json_pointer: try_owned(self.json_pointer)?,
            element_id: self.element_id.map(try_owned).transpose()?,
        })
    }
}

pub(crate) struct ResourceFailureLocation {
    json_pointer: String,
    element_id: Option<String>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResourcePhase {
    Admission,
    JsonTraversalAndNormalization,
    FormatPlanning,
    ProjectionAndFallback,
    EncodingAndOutcomeAssembly,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResourceRecordKind {
    RequestSessionMeter,
    FormatPlanBase,
    FormatPlanOperation,
    RasterColorSurface,
    #[allow(dead_code)]
    Diagnostic,
    SvgSerializerWorkspace,
    PngEncoderWorkspace,
    PdfStreamWorkspace,
    OutputBuffer,
}

pub(crate) struct ResourceMeter {
    limits: ResourceLimits,
    counters: ResourceCounters,
    trace: Option<ResourceTrace>,
}

impl ResourceMeter {
    pub(crate) fn new(limits: &ResourceLimits) -> Self {
        Self {
            limits: limits.clone(),
            counters: ResourceCounters::default(),
            trace: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_trace(
        profile: &ReferenceProfileIdentity,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Self {
        Self {
            limits: request.resource_limits().clone(),
            counters: ResourceCounters::default(),
            trace: Some(ResourceTrace::new(profile, scene, request)),
        }
    }

    pub(crate) fn transact(
        &mut self,
        context: ResourceContext<'_>,
        delta: ResourceDelta,
    ) -> Result<(), ResourceCrossing> {
        let before = self.counters;
        let attempted = AttemptedCounters {
            scene_bytes: Attempted::new(self.counters.scene_bytes, delta.scene_bytes),
            elements: Attempted::new(self.counters.elements, delta.elements),
            diagnostics: Attempted::new(self.counters.diagnostics, delta.diagnostics),
            working_bytes: Attempted::new(self.counters.working_bytes, delta.working_bytes),
            output_bytes: Attempted::new(self.counters.output_bytes, delta.output_bytes),
            total_work_units: Attempted::new(
                self.counters.total_work_units,
                delta.total_work_units,
            ),
        };

        let crossing = [
            (LimitKind::SceneBytes, attempted.scene_bytes),
            (LimitKind::Elements, attempted.elements),
            (LimitKind::Diagnostics, attempted.diagnostics),
            (LimitKind::WorkingBytes, attempted.working_bytes),
            (LimitKind::OutputBytes, attempted.output_bytes),
            (LimitKind::TotalWork, attempted.total_work_units),
        ]
        .into_iter()
        .find(|(kind, attempted)| attempted.crosses(kind.configured(&self.limits)));

        if let Some((kind, crossing_attempt)) = crossing {
            let measurement = crossing_attempt.measurement(kind.configured(&self.limits));
            self.record_transaction(
                context,
                delta,
                before,
                attempted_counters_for_trace(attempted, &self.limits),
                before,
                measurement_form(measurement),
                TraceDecision::RejectedFatal,
                Some(kind.code()),
            );
            return Err(ResourceCrossing { kind, measurement });
        }

        if let Some(committed) = attempted.committed() {
            self.counters = committed;
            self.record_transaction(
                context,
                delta,
                before,
                committed,
                committed,
                TraceMeasurementForm::Exact,
                TraceDecision::Committed,
                None,
            );
            Ok(())
        } else {
            let measurement = Measurement::AtLeast {
                value: self.limits.max_total_work_units().saturating_add(1),
            };
            self.record_transaction(
                context,
                delta,
                before,
                attempted_counters_for_trace(attempted, &self.limits),
                before,
                TraceMeasurementForm::AtLeast,
                TraceDecision::RejectedFatal,
                Some(ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED),
            );
            Err(ResourceCrossing {
                kind: LimitKind::TotalWork,
                measurement,
            })
        }
    }

    pub(crate) fn release_working_bytes(
        &mut self,
        context: ResourceContext<'_>,
        bytes: u64,
    ) -> Result<(), ()> {
        let before = self.counters;
        let mut after = before;
        after.working_bytes = after.working_bytes.checked_sub(bytes).ok_or(())?;
        self.counters = after;
        self.record_release(context, bytes, before, after);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn record_transaction(
        &mut self,
        context: ResourceContext<'_>,
        delta: ResourceDelta,
        before: ResourceCounters,
        attempted: ResourceCounters,
        after: ResourceCounters,
        measurement: TraceMeasurementForm,
        decision: TraceDecision,
        code: Option<ErrorCode>,
    ) {
        let Some(trace) = &mut self.trace else {
            return;
        };
        let source_ordinal = trace.next_ordinal();
        trace.push(ResourceTraceEntry::transaction(
            source_ordinal,
            context,
            delta,
            before,
            attempted,
            after,
            measurement,
            decision,
            code,
        ));
    }

    fn record_release(
        &mut self,
        context: ResourceContext<'_>,
        bytes: u64,
        before: ResourceCounters,
        after: ResourceCounters,
    ) {
        let Some(trace) = &mut self.trace else {
            return;
        };
        let source_ordinal = trace.next_ordinal();
        trace.push(ResourceTraceEntry::release(
            source_ordinal,
            context,
            bytes,
            before,
            after,
        ));
    }

    #[cfg(test)]
    pub(crate) fn trace_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(
            self.trace
                .as_ref()
                .expect("test trace is enabled for this render session"),
        )
        .expect("resource trace contains only serializable values")
    }
}

#[derive(Clone, Copy)]
struct AttemptedCounters {
    scene_bytes: Attempted,
    elements: Attempted,
    diagnostics: Attempted,
    working_bytes: Attempted,
    output_bytes: Attempted,
    total_work_units: Attempted,
}

impl AttemptedCounters {
    fn committed(self) -> Option<ResourceCounters> {
        Some(ResourceCounters {
            scene_bytes: self.scene_bytes.exact()?,
            elements: self.elements.exact()?,
            diagnostics: self.diagnostics.exact()?,
            working_bytes: self.working_bytes.exact()?,
            output_bytes: self.output_bytes.exact()?,
            total_work_units: self.total_work_units.exact()?,
        })
    }
}

fn attempted_counters_for_trace(
    attempted: AttemptedCounters,
    limits: &ResourceLimits,
) -> ResourceCounters {
    ResourceCounters {
        scene_bytes: attempted.scene_bytes.trace_value(limits.max_scene_bytes()),
        elements: attempted
            .elements
            .trace_value(u64::from(limits.max_elements())),
        diagnostics: attempted
            .diagnostics
            .trace_value(u64::from(limits.max_diagnostics())),
        working_bytes: attempted
            .working_bytes
            .trace_value(limits.max_working_bytes()),
        output_bytes: attempted
            .output_bytes
            .trace_value(limits.max_output_bytes()),
        total_work_units: attempted
            .total_work_units
            .trace_value(limits.max_total_work_units()),
    }
}

const fn measurement_form(measurement: Measurement<u64>) -> TraceMeasurementForm {
    match measurement {
        Measurement::Attempted { .. } => TraceMeasurementForm::Exact,
        Measurement::AtLeast { .. } => TraceMeasurementForm::AtLeast,
    }
}

#[derive(Clone, Copy)]
enum Attempted {
    Exact(u64),
    Overflow,
}

impl Attempted {
    fn new(current: u64, delta: u64) -> Self {
        // Deltas that cannot be represented exactly use `u64::MAX` as the
        // transaction-local overflow sentinel.
        if delta == u64::MAX {
            return Self::Overflow;
        }
        current
            .checked_add(delta)
            .map_or(Self::Overflow, Self::Exact)
    }

    fn crosses(self, configured: u64) -> bool {
        match self {
            Self::Exact(value) => value > configured,
            Self::Overflow => true,
        }
    }

    fn exact(self) -> Option<u64> {
        match self {
            Self::Exact(value) => Some(value),
            Self::Overflow => None,
        }
    }

    fn measurement(self, configured: u64) -> Measurement<u64> {
        match self {
            Self::Exact(value) => Measurement::Attempted { value },
            Self::Overflow => Measurement::AtLeast {
                value: configured.saturating_add(1),
            },
        }
    }

    fn trace_value(self, configured: u64) -> u64 {
        match self {
            Self::Exact(value) => value,
            Self::Overflow => configured.saturating_add(1),
        }
    }
}

#[derive(Clone, Copy)]
enum LimitKind {
    SceneBytes,
    Elements,
    Diagnostics,
    WorkingBytes,
    OutputBytes,
    TotalWork,
}

impl LimitKind {
    fn configured(self, limits: &ResourceLimits) -> u64 {
        match self {
            Self::SceneBytes => limits.max_scene_bytes(),
            Self::Elements => u64::from(limits.max_elements()),
            Self::Diagnostics => u64::from(limits.max_diagnostics()),
            Self::WorkingBytes => limits.max_working_bytes(),
            Self::OutputBytes => limits.max_output_bytes(),
            Self::TotalWork => limits.max_total_work_units(),
        }
    }

    const fn code(self) -> ErrorCode {
        self.metadata().code
    }

    const fn field(self) -> &'static str {
        self.metadata().field
    }

    const fn value_kind(self) -> ValueKind {
        self.metadata().value_kind
    }

    const fn message(self) -> &'static str {
        self.metadata().message
    }

    const fn metadata(self) -> LimitMetadata {
        match self {
            Self::SceneBytes => LimitMetadata {
                code: ErrorCode::RESOURCE_SCENE_BYTES_EXCEEDED,
                field: "max_scene_bytes",
                value_kind: ValueKind::Bytes,
                message: "scene byte limit exceeded",
            },
            Self::Elements => LimitMetadata {
                code: ErrorCode::RESOURCE_ELEMENT_COUNT_EXCEEDED,
                field: "max_elements",
                value_kind: ValueKind::Count,
                message: "element count limit exceeded",
            },
            Self::Diagnostics => LimitMetadata {
                code: ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED,
                field: "max_diagnostics",
                value_kind: ValueKind::Count,
                message: "diagnostic count limit exceeded",
            },
            Self::WorkingBytes => LimitMetadata {
                code: ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED,
                field: "max_working_bytes",
                value_kind: ValueKind::Bytes,
                message: "working byte limit exceeded",
            },
            Self::OutputBytes => LimitMetadata {
                code: ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED,
                field: "max_output_bytes",
                value_kind: ValueKind::Bytes,
                message: "output byte limit exceeded",
            },
            Self::TotalWork => LimitMetadata {
                code: ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED,
                field: "max_total_work_units",
                value_kind: ValueKind::WorkUnits,
                message: "total work limit exceeded",
            },
        }
    }
}

struct LimitMetadata {
    code: ErrorCode,
    field: &'static str,
    value_kind: ValueKind,
    message: &'static str,
}

pub(crate) struct ResourceCrossing {
    kind: LimitKind,
    measurement: Measurement<u64>,
}

impl ResourceCrossing {
    fn details(&self, limits: &ResourceLimits) -> ErrorDetails {
        let measurement: ResourceMeasurement = match self.measurement {
            Measurement::Attempted { value } => Measurement::Attempted {
                value: ResourceValue::Integer(value),
            },
            Measurement::AtLeast { value } => Measurement::AtLeast {
                value: ResourceValue::Integer(value),
            },
        };
        ErrorDetails::ResourceLimit {
            field: self.kind.field(),
            configured: ResourceValue::Integer(self.kind.configured(limits)),
            value_kind: self.kind.value_kind(),
            measurement,
            recovery: None::<Recovery>,
        }
    }

    pub(crate) fn into_failure_parts(
        self,
        limits: &ResourceLimits,
        location: ResourceFailureLocation,
    ) -> (
        ErrorCode,
        &'static str,
        Option<String>,
        String,
        ErrorDetails,
    ) {
        let details = self.details(limits);
        (
            self.kind.code(),
            self.kind.message(),
            location.element_id,
            location.json_pointer,
            details,
        )
    }
}

fn try_owned(value: &str) -> Result<String, TryReserveError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len())?;
    owned.push_str(value);
    Ok(owned)
}

#[derive(Serialize)]
struct ResourceTrace {
    schema_version: u32,
    header: ResourceTraceHeader,
    entries: Vec<ResourceTraceEntry>,
}

impl ResourceTrace {
    #[cfg(test)]
    fn new(
        profile: &ReferenceProfileIdentity,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Self {
        let supplemental_assets = request
            .supplemental_assets()
            .iter()
            .map(|(file_id, asset)| SupplementalAssetDigest {
                file_id: file_id.to_owned(),
                payload_sha256: sha256_string(&sha256_digest(asset.bytes())),
            })
            .collect();
        Self {
            schema_version: 1,
            header: ResourceTraceHeader {
                compatibility_revision: profile.compatibility_revision().get(),
                reference_profile_digest: profile.profile_digest().to_owned(),
                format: request.format(),
                limits: ResourceTraceLimits::from(request.resource_limits()),
                input_digests: ResourceInputDigests {
                    scene_document_sha256: sha256_string(scene.source_sha256()),
                    supplemental_assets,
                },
            },
            entries: Vec::new(),
        }
    }

    fn next_ordinal(&self) -> u64 {
        u64::try_from(self.entries.len()).unwrap_or(u64::MAX)
    }

    fn push(&mut self, entry: ResourceTraceEntry) {
        self.entries.push(entry);
    }
}

#[derive(Serialize)]
struct ResourceTraceHeader {
    compatibility_revision: u64,
    reference_profile_digest: String,
    format: OutputFormat,
    limits: ResourceTraceLimits,
    input_digests: ResourceInputDigests,
}

#[derive(Serialize)]
struct ResourceInputDigests {
    scene_document_sha256: String,
    supplemental_assets: Vec<SupplementalAssetDigest>,
}

#[derive(Serialize)]
struct SupplementalAssetDigest {
    file_id: String,
    payload_sha256: String,
}

#[derive(Serialize)]
struct ResourceTraceLimits {
    max_scene_bytes: u64,
    max_elements: u32,
    max_asset_entries: u32,
    max_asset_bytes: u64,
    max_total_asset_bytes: u64,
    max_relationship_depth: u32,
    max_nested_asset_depth: u32,
    max_nested_svg_nodes: u32,
    max_geometry_segments_per_element: u32,
    max_total_geometry_segments: u64,
    max_text_scalars_per_element: u32,
    max_total_text_scalars: u64,
    max_raster_dimension: u32,
    max_decoded_pixels_per_asset: u64,
    max_total_decoded_pixels: u64,
    max_raster_surface_pixels: u64,
    max_total_rasterized_pixels: u64,
    max_svg_dimension: f64,
    max_pdf_page_points: f64,
    max_coordinate_magnitude: f64,
    max_diagnostics: u32,
    max_working_bytes: u64,
    max_output_bytes: u64,
    max_element_work_units: u64,
    max_asset_work_units: u64,
    max_total_work_units: u64,
}

impl From<&ResourceLimits> for ResourceTraceLimits {
    fn from(limits: &ResourceLimits) -> Self {
        Self {
            max_scene_bytes: limits.max_scene_bytes(),
            max_elements: limits.max_elements(),
            max_asset_entries: limits.max_asset_entries(),
            max_asset_bytes: limits.max_asset_bytes(),
            max_total_asset_bytes: limits.max_total_asset_bytes(),
            max_relationship_depth: limits.max_relationship_depth(),
            max_nested_asset_depth: limits.max_nested_asset_depth(),
            max_nested_svg_nodes: limits.max_nested_svg_nodes(),
            max_geometry_segments_per_element: limits.max_geometry_segments_per_element(),
            max_total_geometry_segments: limits.max_total_geometry_segments(),
            max_text_scalars_per_element: limits.max_text_scalars_per_element(),
            max_total_text_scalars: limits.max_total_text_scalars(),
            max_raster_dimension: limits.max_raster_dimension(),
            max_decoded_pixels_per_asset: limits.max_decoded_pixels_per_asset(),
            max_total_decoded_pixels: limits.max_total_decoded_pixels(),
            max_raster_surface_pixels: limits.max_raster_surface_pixels(),
            max_total_rasterized_pixels: limits.max_total_rasterized_pixels(),
            max_svg_dimension: limits.max_svg_dimension(),
            max_pdf_page_points: limits.max_pdf_page_points(),
            max_coordinate_magnitude: limits.max_coordinate_magnitude(),
            max_diagnostics: limits.max_diagnostics(),
            max_working_bytes: limits.max_working_bytes(),
            max_output_bytes: limits.max_output_bytes(),
            max_element_work_units: limits.max_element_work_units(),
            max_asset_work_units: limits.max_asset_work_units(),
            max_total_work_units: limits.max_total_work_units(),
        }
    }
}

#[derive(Serialize)]
struct ResourceTraceEntry {
    phase: ResourcePhase,
    source_ordinal: u64,
    json_pointer: String,
    element_id: Option<String>,
    semantic_identity: Option<String>,
    event: &'static str,
    local_owner: Option<String>,
    sharing_key: Option<String>,
    record_kind: Option<ResourceRecordKind>,
    deltas: ResourceDelta,
    before: ResourceCounters,
    attempted: ResourceCounters,
    after: ResourceCounters,
    measurement: TraceMeasurementForm,
    decision: TraceDecision,
    code: Option<ErrorCode>,
    fallback_parent: Option<u64>,
    record_action: Option<TraceRecordAction>,
}

impl ResourceTraceEntry {
    #[allow(clippy::too_many_arguments)]
    fn transaction(
        source_ordinal: u64,
        context: ResourceContext<'_>,
        deltas: ResourceDelta,
        before: ResourceCounters,
        attempted: ResourceCounters,
        after: ResourceCounters,
        measurement: TraceMeasurementForm,
        decision: TraceDecision,
        code: Option<ErrorCode>,
    ) -> Self {
        Self {
            phase: context.phase,
            source_ordinal,
            json_pointer: context.json_pointer.to_owned(),
            element_id: context.element_id.map(str::to_owned),
            semantic_identity: context.semantic_identity.map(str::to_owned),
            event: context.event,
            local_owner: context.local_owner.map(str::to_owned),
            sharing_key: context.sharing_key.map(str::to_owned),
            record_kind: context.record_kind,
            deltas,
            before,
            attempted,
            after,
            measurement,
            decision,
            code,
            fallback_parent: context.fallback_parent,
            record_action: context.record_kind.map(|_| TraceRecordAction::Acquire),
        }
    }

    fn release(
        source_ordinal: u64,
        context: ResourceContext<'_>,
        working_bytes: u64,
        before: ResourceCounters,
        after: ResourceCounters,
    ) -> Self {
        Self {
            phase: context.phase,
            source_ordinal,
            json_pointer: context.json_pointer.to_owned(),
            element_id: context.element_id.map(str::to_owned),
            semantic_identity: context.semantic_identity.map(str::to_owned),
            event: context.event,
            local_owner: context.local_owner.map(str::to_owned),
            sharing_key: context.sharing_key.map(str::to_owned),
            record_kind: context.record_kind,
            deltas: ResourceDelta {
                working_bytes,
                ..ResourceDelta::default()
            },
            before,
            attempted: after,
            after,
            measurement: TraceMeasurementForm::Exact,
            decision: TraceDecision::Released,
            code: None,
            fallback_parent: context.fallback_parent,
            record_action: Some(TraceRecordAction::Release),
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TraceMeasurementForm {
    Exact,
    AtLeast,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TraceDecision {
    Committed,
    RejectedLocal,
    RejectedFatal,
    SharedIntrinsic,
    Released,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TraceRecordAction {
    Acquire,
    Release,
}

#[cfg(test)]
pub(crate) fn sha256_digest(input: &[u8]) -> [u8; 32] {
    let mut state = [
        0x6a09_e667_u32,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut blocks = input.chunks_exact(64);
    for block in &mut blocks {
        sha256_compress(&mut state, block);
    }

    let remainder = blocks.remainder();
    let mut final_blocks = [0_u8; 128];
    final_blocks[..remainder.len()].copy_from_slice(remainder);
    final_blocks[remainder.len()] = 0x80;
    let final_len = if remainder.len() < 56 { 64 } else { 128 };
    let bit_len = u64::try_from(input.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    final_blocks[final_len - 8..final_len].copy_from_slice(&bit_len.to_be_bytes());
    for block in final_blocks[..final_len].chunks_exact(64) {
        sha256_compress(&mut state, block);
    }

    let mut digest = [0_u8; 32];
    for (word, output) in state.iter().zip(digest.chunks_exact_mut(4)) {
        output.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(test)]
fn sha256_compress(state: &mut [u32; 8], block: &[u8]) {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];

    let mut schedule = [0_u32; 64];
    for (index, word) in schedule[..16].iter_mut().enumerate() {
        let offset = index * 4;
        *word = u32::from_be_bytes([
            block[offset],
            block[offset + 1],
            block[offset + 2],
            block[offset + 3],
        ]);
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let choice = (e & f) ^ ((!e) & g);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let first = h
            .wrapping_add(sum1)
            .wrapping_add(choice)
            .wrapping_add(K[index])
            .wrapping_add(schedule[index]);
        let second = sum0.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (word, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(value);
    }
}

#[cfg(test)]
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex_bytes(&sha256_digest(bytes))
}

#[cfg(test)]
fn sha256_string(digest: &[u8]) -> String {
    let mut output = String::from("sha256:");
    output.push_str(&hex_bytes(digest));
    output
}

#[cfg(test)]
fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_sha256_matches_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn unrepresentable_delta_is_an_at_least_crossing() {
        let limits = ResourceLimits::default();
        let mut meter = ResourceMeter::new(&limits);
        let context = ResourceContext::new(ResourcePhase::Admission, "overflow_vector", "");
        let location = context
            .try_failure_location()
            .expect("empty failure location needs no allocation");

        let crossing = meter
            .transact(
                context,
                ResourceDelta {
                    total_work_units: u64::MAX,
                    ..ResourceDelta::default()
                },
            )
            .expect_err("overflow sentinel crosses total work");
        let (code, _, _, _, details) = crossing.into_failure_parts(&limits, location);

        assert_eq!(code, ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED);
        match details {
            ErrorDetails::ResourceLimit {
                measurement:
                    Measurement::AtLeast {
                        value: ResourceValue::Integer(value),
                    },
                ..
            } => assert_eq!(value, limits.max_total_work_units() + 1),
            _ => panic!("overflow uses an at-least resource measurement"),
        }
    }
}
