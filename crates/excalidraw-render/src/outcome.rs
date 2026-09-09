use std::error::Error;
use std::fmt;

use serde::Serialize;

use crate::{
    CompatibilityRevision, ErrorCode, ErrorDescriptor, ErrorDetails, OutputFormat,
    ReferenceProfileIdentity,
};

/// String-backed stable compatibility diagnostic code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    pub(crate) const fn manifested(code: &'static str) -> Self {
        Self(code)
    }

    /// A missing source element ID was repaired.
    pub const NORMALIZATION_MISSING_ELEMENT_ID: Self = Self("normalization.missing-element-id");
    /// A later duplicate source element ID was repaired.
    pub const NORMALIZATION_DUPLICATE_ELEMENT_ID: Self = Self("normalization.duplicate-element-id");
    /// A missing required timestamp was repaired to canonical zero.
    pub const NORMALIZATION_MISSING_TIMESTAMP: Self = Self("normalization.missing-timestamp");
    /// A zero Rough seed was repaired deterministically.
    pub const NORMALIZATION_ZERO_ROUGH_SEED: Self = Self("normalization.zero-rough-seed");
    /// A cycle-closing relationship edge was removed.
    pub const NORMALIZATION_RELATIONSHIP_CYCLE: Self = Self("normalization.relationship-cycle");
    /// An unknown element type used the compatibility fallback.
    pub const COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE: Self =
        Self("compatibility.unsupported-element-type");
    /// A malformed element was replaced or omitted.
    pub const COMPATIBILITY_MALFORMED_ELEMENT: Self = Self("compatibility.malformed-element");
    /// An unsupported property was ignored.
    pub const COMPATIBILITY_UNSUPPORTED_PROPERTY: Self = Self("compatibility.unsupported-property");
    /// An unsupported property value used its declared recovery.
    pub const COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE: Self =
        Self("compatibility.unsupported-property-value");
    /// A known but unavailable element was replaced or omitted.
    pub const COMPATIBILITY_KNOWN_ELEMENT_REPLACED: Self =
        Self("compatibility.known-element-replaced");
    /// A scene-embedded asset remained authoritative over a supplement.
    pub const ASSET_SUPPLEMENT_CONFLICT: Self = Self("asset.supplement-conflict");
    /// Sniffed payload type replaced a mismatched advisory media type.
    pub const ASSET_MEDIA_TYPE_MISMATCH: Self = Self("asset.media-type-mismatch");
    /// A referenced scene file record was malformed.
    pub const ASSET_MALFORMED_FILE_RECORD: Self = Self("asset.malformed-file-record");
    /// A referring image had no authoritative file payload.
    pub const IMAGE_MISSING_FILE: Self = Self("image.missing-file");
    /// A forbidden image resource was removed or replaced.
    pub const IMAGE_FORBIDDEN_RESOURCE: Self = Self("image.forbidden-resource");
    /// A rendered animated image occurrence was flattened.
    pub const IMAGE_ANIMATION_FLATTENED: Self = Self("image.animation-flattened");
    /// An unsupported image format was omitted or replaced.
    pub const IMAGE_UNSUPPORTED_FORMAT: Self = Self("image.unsupported-format");
    /// Image decoding failed and used format-specific recovery.
    pub const IMAGE_DECODE_FAILED: Self = Self("image.decode-failed");
    /// Unsafe nested SVG content was removed.
    pub const IMAGE_UNSAFE_SVG_CONTENT_REMOVED: Self = Self("image.unsafe-svg-content-removed");
    /// An unsafe element link was omitted.
    pub const LINK_UNSAFE_OMITTED: Self = Self("link.unsafe-omitted");
    /// An invalid color was substituted deterministically.
    pub const COLOR_INVALID: Self = Self("color.invalid");
    /// A local font request used bundled Liberation Sans.
    pub const FONT_LIBERATION_SANS_SUBSTITUTED: Self = Self("font.liberation-sans-substituted");
    /// Emoji text used bundled Noto Emoji.
    pub const FONT_NOTO_EMOJI_SUBSTITUTED: Self = Self("font.noto-emoji-substituted");
    /// A missing glyph was replaced deterministically.
    pub const FONT_MISSING_GLYPH: Self = Self("font.missing-glyph");
    /// An over-complex element was replaced or omitted.
    pub const RESOURCE_ELEMENT_COMPLEXITY_REPLACED: Self =
        Self("resource.element-complexity-replaced");
    /// Element-local semantic work used local recovery.
    pub const RESOURCE_ELEMENT_WORK_EXCEEDED: Self = Self("resource.element-work-exceeded");
    /// One asset occurrence exceeded its byte ceiling.
    pub const RESOURCE_ASSET_BYTES_EXCEEDED: Self = Self("resource.asset-bytes-exceeded");
    /// One semantic asset exceeded its work ceiling.
    pub const RESOURCE_ASSET_WORK_EXCEEDED: Self = Self("resource.asset-work-exceeded");
    /// Raster dimensions exceeded the local image ceiling.
    pub const RESOURCE_IMAGE_DIMENSIONS_EXCEEDED: Self = Self("resource.image-dimensions-exceeded");
    /// Decoded image pixels exceeded the local asset ceiling.
    pub const RESOURCE_IMAGE_PIXELS_EXCEEDED: Self = Self("resource.image-pixels-exceeded");
    /// Nested SVG complexity exceeded its local node ceiling.
    pub const RESOURCE_NESTED_SVG_COMPLEXITY_EXCEEDED: Self =
        Self("resource.nested-svg-complexity-exceeded");
    /// Nested asset discovery exceeded its local depth ceiling.
    pub const RESOURCE_NESTED_ASSET_DEPTH_EXCEEDED: Self =
        Self("resource.nested-asset-depth-exceeded");
    /// A coordinate exceeded its local magnitude ceiling.
    pub const RESOURCE_COORDINATE_RANGE_EXCEEDED: Self = Self("resource.coordinate-range-exceeded");
    /// Optional metadata was omitted after a resource crossing.
    pub const RESOURCE_OPTIONAL_METADATA_OMITTED: Self = Self("resource.optional-metadata-omitted");
    /// Provenance serialization failed and was omitted.
    pub const METADATA_PROVENANCE_SERIALIZATION_FAILED: Self =
        Self("metadata.provenance-serialization-failed");
    /// Diagnostic-summary serialization failed and was omitted.
    pub const METADATA_DIAGNOSTIC_SUMMARY_SERIALIZATION_FAILED: Self =
        Self("metadata.diagnostic-summary-serialization-failed");
    /// Embedded-scene serialization failed and was omitted.
    pub const METADATA_SCENE_SERIALIZATION_FAILED: Self =
        Self("metadata.scene-serialization-failed");
    /// Embedded-scene compression failed and was omitted.
    pub const METADATA_SCENE_COMPRESSION_FAILED: Self = Self("metadata.scene-compression-failed");
    /// Embedded-scene carrier insertion failed and was omitted.
    pub const METADATA_SCENE_EMBEDDING_FAILED: Self = Self("metadata.scene-embedding-failed");
    /// Artifact-manifest serialization failed.
    pub const METADATA_MANIFEST_SERIALIZATION_FAILED: Self =
        Self("metadata.manifest-serialization-failed");
    /// Artifact-manifest carrier insertion failed.
    pub const METADATA_MANIFEST_EMBEDDING_FAILED: Self = Self("metadata.manifest-embedding-failed");
    /// A permitted isolated PDF closure was rasterized.
    pub const PDF_RASTER_FALLBACK: Self = Self("pdf.raster-fallback");

    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Severity of one compatibility diagnostic occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// Informational compatibility behavior.
    Info,
    /// Recoverable compatibility loss or repair.
    Warning,
}

/// Observable effect class of a compatibility diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticEffect {
    /// Submitted input was repaired, selected, or ignored.
    InputRepair,
    /// Appearance may differ.
    Visual,
    /// Links, text, animation, or vector semantics may differ.
    Semantic,
    /// Carried metadata may differ.
    Metadata,
}

/// Stable serializable view of one compatibility diagnostic occurrence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompatibilityDiagnostic {
    pub(crate) code: DiagnosticCode,
    pub(crate) severity: DiagnosticSeverity,
    pub(crate) format: OutputFormat,
    pub(crate) effects: Vec<DiagnosticEffect>,
    pub(crate) element_id: Option<String>,
    pub(crate) json_pointer: String,
    pub(crate) message: String,
    pub(crate) details: Option<Box<ErrorDetails>>,
    #[serde(skip)]
    pub(crate) order: DiagnosticOrder,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DiagnosticOrder {
    pub(crate) owner_index: u64,
    pub(crate) source_position: u64,
    pub(crate) retention_ordinal: u64,
    pub(crate) attribution: DiagnosticAttribution,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DiagnosticAttribution {
    SceneGlobal,
    Target,
}

impl Default for DiagnosticOrder {
    fn default() -> Self {
        Self {
            owner_index: u64::MAX,
            source_position: u64::MAX,
            retention_ordinal: u64::MAX,
            attribution: DiagnosticAttribution::SceneGlobal,
        }
    }
}

impl DiagnosticOrder {
    /// The order the session gives a diagnostic it retains as the
    /// `retention_ordinal`th of its report: the source owner and member
    /// position the emitting stage attributed it to, or last among the owners
    /// or positions when the stage knew neither.
    pub(crate) fn retained(
        owner_index: Option<u64>,
        source_position: Option<u64>,
        retention_ordinal: u64,
        attribution: DiagnosticAttribution,
    ) -> Self {
        Self {
            owner_index: owner_index.unwrap_or(u64::MAX),
            source_position: source_position.unwrap_or(u64::MAX),
            retention_ordinal,
            attribution,
        }
    }
}

impl CompatibilityDiagnostic {
    /// Returns the stable machine-readable diagnostic code.
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the diagnostic severity.
    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Returns the format whose projection produced this diagnostic.
    pub const fn format(&self) -> OutputFormat {
        self.format
    }

    /// Returns the nonempty canonical effect list.
    pub fn effects(&self) -> &[DiagnosticEffect] {
        &self.effects
    }

    /// Returns the normalized source element ID, when applicable.
    pub fn element_id(&self) -> Option<&str> {
        self.element_id.as_deref()
    }

    /// Returns the required RFC 6901 scene pointer.
    pub fn json_pointer(&self) -> &str {
        &self.json_pointer
    }

    /// Returns the human-readable explanation.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns typed details required by the diagnostic code.
    pub fn details(&self) -> Option<&ErrorDetails> {
        self.details.as_deref()
    }
}

/// Complete ordered compatibility diagnostics for one render transaction.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompatibilityReport {
    pub(crate) schema_version: u32,
    pub(crate) diagnostics: Vec<CompatibilityDiagnostic>,
}

impl CompatibilityReport {
    /// Creates a standalone empty schema-1 report.
    pub const fn empty() -> Self {
        Self {
            schema_version: 1,
            diagnostics: Vec::new(),
        }
    }

    /// Returns the stable report schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the complete diagnostics in canonical order.
    pub fn diagnostics(&self) -> &[CompatibilityDiagnostic] {
        &self.diagnostics
    }

    /// Returns whether the report contains no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the number of diagnostic occurrences without coalescing.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }
}

impl Default for CompatibilityReport {
    fn default() -> Self {
        Self::empty()
    }
}

/// One complete renderer-owned static export.
#[derive(Debug, Eq, PartialEq)]
pub struct Artifact {
    pub(crate) format: OutputFormat,
    pub(crate) bytes: Vec<u8>,
}

impl Artifact {
    /// Returns the artifact's output format.
    pub const fn format(&self) -> OutputFormat {
        self.format
    }

    /// Returns the canonical media type for the artifact format.
    pub const fn media_type(&self) -> &'static str {
        self.format.media_type()
    }

    /// Borrows the complete artifact bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the artifact and returns its complete owned bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Successful result of one complete render transaction.
#[derive(Debug)]
pub struct RenderOutcome {
    pub(crate) artifact: Artifact,
    pub(crate) report: CompatibilityReport,
    pub(crate) profile: &'static ReferenceProfileIdentity,
}

impl RenderOutcome {
    /// Returns the single complete artifact.
    pub const fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    /// Returns the complete ordered compatibility report.
    pub const fn report(&self) -> &CompatibilityReport {
        &self.report
    }

    /// Consumes the outcome without discarding either complete owned value.
    pub fn into_parts(self) -> (Artifact, CompatibilityReport) {
        (self.artifact, self.report)
    }

    /// Returns the selected compatibility revision.
    pub const fn compatibility_revision(&self) -> CompatibilityRevision {
        self.profile.compatibility_revision()
    }

    /// Returns the pinned Excalidraw release.
    pub const fn excalidraw_release(&self) -> &'static str {
        self.profile.excalidraw_release()
    }

    /// Returns the canonical reference-profile digest.
    pub const fn profile_digest(&self) -> &'static str {
        self.profile.profile_digest()
    }
}

/// Stable category of a fatal render failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RenderErrorCategory {
    /// Admitted scene input could not produce bounded global semantics.
    Input,
    /// A configured semantic resource ceiling was crossed.
    ResourceLimit,
    /// Rendering could not retain a meaningful complete artifact.
    NoMeaningfulArtifact,
    /// Complete artifact encoding failed.
    Encoding,
    /// A trusted renderer invariant failed.
    Internal,
}

impl RenderErrorCategory {
    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::ResourceLimit => "resource_limit",
            Self::NoMeaningfulArtifact => "no_meaningful_artifact",
            Self::Encoding => "encoding",
            Self::Internal => "internal",
        }
    }
}

/// Fatal render failure with the complete report accumulated before failure.
#[derive(Debug, PartialEq)]
pub struct RenderError {
    pub(crate) category: RenderErrorCategory,
    pub(crate) code: ErrorCode,
    pub(crate) message: Box<str>,
    pub(crate) format: OutputFormat,
    pub(crate) element_id: Option<String>,
    pub(crate) json_pointer: Box<str>,
    pub(crate) details: Option<Box<ErrorDetails>>,
    pub(crate) report: CompatibilityReport,
}

impl RenderError {
    /// Returns the stable fatal failure category.
    pub const fn category(&self) -> RenderErrorCategory {
        self.category
    }

    /// Returns the stable machine-readable error code.
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the selected output format.
    pub const fn format(&self) -> OutputFormat {
        self.format
    }

    /// Returns the normalized source element ID, when applicable.
    pub fn element_id(&self) -> Option<&str> {
        self.element_id.as_deref()
    }

    /// Returns the required RFC 6901 scene pointer.
    pub fn json_pointer(&self) -> &str {
        &self.json_pointer
    }

    /// Returns typed details required by the stable error code.
    pub fn details(&self) -> Option<&ErrorDetails> {
        self.details.as_deref()
    }

    /// Returns the complete ordered report accumulated before failure.
    pub const fn report(&self) -> &CompatibilityReport {
        &self.report
    }

    /// Returns this failure's stable serializable descriptor.
    pub fn descriptor(&self) -> ErrorDescriptor {
        ErrorDescriptor::render(
            self.category,
            self.code,
            self.message.to_string(),
            self.format,
            self.element_id.clone(),
            self.json_pointer.to_string(),
            self.details.as_deref().cloned(),
        )
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RenderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recovery, Renderer, ResourceMeasurement, ResourceValue, ValueKind};

    #[test]
    fn owned_outcome_and_render_failure_expose_only_complete_contract_values() {
        let diagnostic = CompatibilityDiagnostic {
            code: DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT,
            severity: DiagnosticSeverity::Warning,
            format: OutputFormat::Svg,
            effects: vec![DiagnosticEffect::Visual],
            element_id: Some("element-1".to_owned()),
            json_pointer: "/elements/0".to_owned(),
            message: "malformed element replaced".to_owned(),
            details: Some(Box::new(ErrorDetails::Compatibility {
                recovery: Recovery::Placeholder,
                recipe_id: Some("renderer.malformed-element.v1".to_owned()),
            })),
            order: DiagnosticOrder::default(),
        };
        let report = CompatibilityReport {
            schema_version: 1,
            diagnostics: vec![diagnostic],
        };
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 profile exists");
        let outcome = RenderOutcome {
            artifact: Artifact {
                format: OutputFormat::Svg,
                bytes: b"<svg/>".to_vec(),
            },
            report: report.clone(),
            profile,
        };

        assert_eq!(outcome.artifact().bytes(), b"<svg/>");
        assert_eq!(outcome.artifact().media_type(), "image/svg+xml");
        assert_eq!(outcome.report(), &report);
        assert_eq!(
            outcome.compatibility_revision(),
            CompatibilityRevision::REVISION_1
        );
        assert_eq!(outcome.excalidraw_release(), "v0.18.1");
        assert_eq!(outcome.profile_digest(), profile.profile_digest());
        assert_eq!(
            serde_json::to_string(outcome.report()).expect("serialize diagnosed report"),
            concat!(
                "{\"schema_version\":1,\"diagnostics\":[{",
                "\"code\":\"compatibility.malformed-element\",",
                "\"severity\":\"warning\",",
                "\"format\":\"svg\",",
                "\"effects\":[\"visual\"],",
                "\"element_id\":\"element-1\",",
                "\"json_pointer\":\"/elements/0\",",
                "\"message\":\"malformed element replaced\",",
                "\"details\":{\"kind\":\"compatibility\",",
                "\"recovery\":\"placeholder\",",
                "\"recipe_id\":\"renderer.malformed-element.v1\"}}]}"
            )
        );
        let (artifact, returned_report) = outcome.into_parts();
        assert_eq!(artifact.into_bytes(), b"<svg/>");
        assert_eq!(returned_report, report);

        let error = RenderError {
            category: RenderErrorCategory::ResourceLimit,
            code: ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED,
            message: "output byte limit exceeded".into(),
            format: OutputFormat::Svg,
            element_id: None,
            json_pointer: Box::from(""),
            details: Some(Box::new(ErrorDetails::ResourceLimit {
                field: "max_output_bytes",
                configured: ResourceValue::Integer(134_217_728),
                value_kind: ValueKind::Bytes,
                measurement: ResourceMeasurement::Attempted {
                    value: ResourceValue::Integer(134_217_729),
                },
                recovery: None,
            })),
            report: returned_report,
        };
        assert_eq!(error.report().len(), 1);
        assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
        assert_eq!(
            serde_json::to_string(&error.descriptor()).expect("serialize render descriptor"),
            concat!(
                "{\"family\":\"render\",",
                "\"category\":\"resource_limit\",",
                "\"code\":\"resource.output-bytes-exceeded\",",
                "\"message\":\"output byte limit exceeded\",",
                "\"format\":\"svg\",",
                "\"element_id\":null,",
                "\"json_pointer\":\"\",",
                "\"details\":{\"kind\":\"resource_limit\",",
                "\"field\":\"max_output_bytes\",",
                "\"configured\":134217728,",
                "\"value_kind\":\"bytes\",",
                "\"measurement\":{\"kind\":\"attempted\",\"value\":134217729},",
                "\"recovery\":null}}"
            )
        );
    }

    #[test]
    fn report_preserves_every_diagnostic_in_supplied_canonical_order() {
        let diagnostic = |code, pointer: &str| CompatibilityDiagnostic {
            code,
            severity: DiagnosticSeverity::Warning,
            format: OutputFormat::Png,
            effects: vec![DiagnosticEffect::Visual],
            element_id: None,
            json_pointer: pointer.to_owned(),
            message: "diagnosed".to_owned(),
            details: None,
            order: DiagnosticOrder::default(),
        };
        let report = CompatibilityReport {
            schema_version: 1,
            diagnostics: vec![
                diagnostic(DiagnosticCode::COLOR_INVALID, "/elements/0/strokeColor"),
                diagnostic(DiagnosticCode::IMAGE_DECODE_FAILED, "/files/image/dataURL"),
            ],
        };

        assert_eq!(report.len(), 2);
        assert_eq!(
            report.diagnostics()[0].code(),
            DiagnosticCode::COLOR_INVALID
        );
        assert_eq!(
            report.diagnostics()[1].code(),
            DiagnosticCode::IMAGE_DECODE_FAILED
        );
    }
}
