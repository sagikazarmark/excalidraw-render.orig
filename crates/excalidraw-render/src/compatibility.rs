use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use serde::ser::SerializeStruct;
use serde::Serialize;

use crate::outcome::{RenderError, RenderErrorCategory, RenderOutcome};
use crate::request::{OutputFormat, RenderRequest};
use crate::scene::SceneDocument;

/// Identifies a retained static-export compatibility promise.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CompatibilityRevision(u64);

impl CompatibilityRevision {
    /// Compatibility revision 1, bound to Excalidraw v0.18.1.
    pub const REVISION_1: Self = Self(1);

    /// Creates a compatibility revision identifier.
    pub const fn new(revision: u64) -> Self {
        Self(revision)
    }

    /// Returns the numeric compatibility revision.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for CompatibilityRevision {
    fn from(revision: u64) -> Self {
        Self::new(revision)
    }
}

impl FromStr for CompatibilityRevision {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self::new)
    }
}

impl fmt::Display for CompatibilityRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The compatibility revision selected by [`Renderer::new`].
pub const DEFAULT_COMPATIBILITY_REVISION: CompatibilityRevision = CompatibilityRevision::REVISION_1;

/// Immutable identity of the reference profile used by a compatibility revision.
#[derive(Debug, Serialize)]
pub struct ReferenceProfileIdentity {
    compatibility_revision: CompatibilityRevision,
    excalidraw_release: &'static str,
    reference_profile_digest: &'static str,
}

impl ReferenceProfileIdentity {
    pub(crate) const fn new(
        compatibility_revision: CompatibilityRevision,
        excalidraw_release: &'static str,
        reference_profile_digest: &'static str,
    ) -> Self {
        Self {
            compatibility_revision,
            excalidraw_release,
            reference_profile_digest,
        }
    }

    /// Returns the compatibility revision bound to this reference profile.
    pub const fn compatibility_revision(&self) -> CompatibilityRevision {
        self.compatibility_revision
    }

    /// Returns the pinned official Excalidraw release.
    pub const fn excalidraw_release(&self) -> &'static str {
        self.excalidraw_release
    }

    /// Returns the canonical SHA-256 digest of the reference profile.
    pub const fn profile_digest(&self) -> &'static str {
        self.reference_profile_digest
    }
}

/// Immutable renderer configured for one compatibility revision.
#[derive(Clone, Debug)]
pub struct Renderer {
    catalog: &'static crate::catalog::RevisionCatalog,
}

impl Renderer {
    /// Constructs a renderer for [`DEFAULT_COMPATIBILITY_REVISION`].
    pub const fn new() -> Self {
        Self {
            catalog: &crate::catalog::REVISION_1,
        }
    }

    /// Constructs a renderer for a retained compatibility revision.
    pub fn for_revision(revision: CompatibilityRevision) -> Result<Self, UnsupportedRevisionError> {
        crate::catalog::for_revision(revision)
            .map(|catalog| Self { catalog })
            .ok_or(UnsupportedRevisionError { revision })
    }

    /// Returns retained compatibility revisions in increasing order.
    pub const fn supported_revisions() -> &'static [CompatibilityRevision] {
        crate::catalog::SUPPORTED_REVISIONS
    }

    /// Returns the reference profile identity for a retained revision.
    pub fn profile_identity(
        revision: CompatibilityRevision,
    ) -> Result<&'static ReferenceProfileIdentity, UnsupportedRevisionError> {
        crate::catalog::for_revision(revision)
            .map(crate::catalog::RevisionCatalog::profile)
            .ok_or(UnsupportedRevisionError { revision })
    }

    /// Returns this renderer's selected reference profile identity.
    pub const fn selected_profile_identity(&self) -> &'static ReferenceProfileIdentity {
        self.catalog.profile()
    }

    /// Renders one admitted scene into the request's selected output format.
    pub fn render(
        &self,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Result<RenderOutcome, RenderError> {
        crate::render::render(self.catalog, scene, request)
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Indicates that a requested compatibility revision is not retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedRevisionError {
    revision: CompatibilityRevision,
}

impl UnsupportedRevisionError {
    /// Returns the unsupported revision requested by the caller.
    pub const fn requested_revision(&self) -> CompatibilityRevision {
        self.revision
    }
}

impl fmt::Display for UnsupportedRevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compatibility revision {} is not supported",
            self.revision
        )
    }
}

impl Error for UnsupportedRevisionError {}

impl UnsupportedRevisionError {
    /// Returns this failure's stable serializable descriptor.
    pub fn descriptor(&self) -> ErrorDescriptor {
        ErrorDescriptor {
            family: ErrorFamily::UnsupportedRevision,
            category: None,
            code: ErrorCode::REVISION_UNSUPPORTED,
            message: self.to_string(),
            format: None,
            element_id: None,
            json_pointer: None,
            details: Some(ErrorDetails::Revision {
                requested: self.revision,
            }),
        }
    }
}

/// Stable family of a typed renderer failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ErrorFamily {
    /// Scene-document bytes could not be admitted.
    SceneParse,
    /// A render request is invalid.
    Request,
    /// Rendering failed after request validation.
    Render,
    /// A requested compatibility revision is not retained.
    UnsupportedRevision,
}

impl ErrorFamily {
    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SceneParse => "scene_parse",
            Self::Request => "request",
            Self::Render => "render",
            Self::UnsupportedRevision => "unsupported_revision",
        }
    }
}

impl fmt::Display for ErrorFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// String-backed stable error code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ErrorCode(&'static str);

impl ErrorCode {
    /// Request padding is negative or non-finite.
    pub const REQUEST_INVALID_PADDING: Self = Self("request.invalid-padding");

    /// Request scale is non-positive or non-finite.
    pub const REQUEST_INVALID_SCALE: Self = Self("request.invalid-scale");

    /// Request maximum dimension is non-positive or non-finite.
    pub const REQUEST_INVALID_MAX_DIMENSION: Self = Self("request.invalid-max-dimension");

    /// Format-specific options were supplied for another output format.
    pub const REQUEST_INCOMPATIBLE_FORMAT_OPTIONS: Self =
        Self("request.incompatible-format-options");

    /// A supplemental asset has an empty scene file ID.
    pub const REQUEST_EMPTY_ASSET_ID: Self = Self("request.empty-asset-id");

    /// A supplemental scene file ID occurs more than once.
    pub const REQUEST_DUPLICATE_ASSET_ID: Self = Self("request.duplicate-asset-id");

    /// A supplemental asset advisory media type is malformed.
    pub const REQUEST_INVALID_ASSET_MEDIA_TYPE: Self = Self("request.invalid-asset-media-type");

    /// A semantic resource ceiling is zero, invalid, inconsistent, or too large.
    pub const REQUEST_INVALID_RESOURCE_LIMIT: Self = Self("request.invalid-resource-limit");

    /// Finite whole-scene geometry could not be established.
    pub const SCENE_UNBOUNDED_GEOMETRY: Self = Self("scene.unbounded-geometry");
    /// A required PDF raster fallback was forbidden by the request.
    pub const PDF_RASTER_FALLBACK_FORBIDDEN: Self = Self("pdf.raster-fallback-forbidden");
    /// A PDF raster fallback could not be isolated safely.
    pub const PDF_RASTER_FALLBACK_UNISOLATABLE: Self = Self("pdf.raster-fallback-unisolatable");
    /// No meaningful complete artifact could be produced.
    pub const RENDER_NO_MEANINGFUL_ARTIFACT: Self = Self("render.no-meaningful-artifact");
    /// SVG encoding failed.
    pub const ENCODE_SVG_FAILED: Self = Self("encode.svg-failed");
    /// PNG encoding failed.
    pub const ENCODE_PNG_FAILED: Self = Self("encode.png-failed");
    /// PDF encoding failed.
    pub const ENCODE_PDF_FAILED: Self = Self("encode.pdf-failed");
    /// An internal renderer invariant failed.
    pub const RENDER_INTERNAL_FAILURE: Self = Self("render.internal-failure");
    /// Admitted scene bytes exceed the configured semantic ceiling.
    pub const RESOURCE_SCENE_BYTES_EXCEEDED: Self = Self("resource.scene-bytes-exceeded");
    /// Source element occurrences exceed the configured ceiling.
    pub const RESOURCE_ELEMENT_COUNT_EXCEEDED: Self = Self("resource.element-count-exceeded");
    /// Asset payload occurrences exceed the configured ceiling.
    pub const RESOURCE_ASSET_COUNT_EXCEEDED: Self = Self("resource.asset-count-exceeded");
    /// Aggregate asset bytes exceed the configured ceiling.
    pub const RESOURCE_TOTAL_ASSET_BYTES_EXCEEDED: Self =
        Self("resource.total-asset-bytes-exceeded");
    /// Relationship traversal exceeds the configured depth.
    pub const RESOURCE_RELATIONSHIP_DEPTH_EXCEEDED: Self =
        Self("resource.relationship-depth-exceeded");
    /// Aggregate geometry exceeds the configured segment ceiling.
    pub const RESOURCE_TOTAL_GEOMETRY_EXCEEDED: Self = Self("resource.total-geometry-exceeded");
    /// Aggregate text exceeds the configured scalar ceiling.
    pub const RESOURCE_TOTAL_TEXT_EXCEEDED: Self = Self("resource.total-text-exceeded");
    /// Aggregate decoded pixels exceed the configured ceiling.
    pub const RESOURCE_TOTAL_DECODED_PIXELS_EXCEEDED: Self =
        Self("resource.total-decoded-pixels-exceeded");
    /// The required main raster surface exceeds a side or area ceiling.
    pub const RESOURCE_RASTER_SURFACE_EXCEEDED: Self = Self("resource.raster-surface-exceeded");
    /// Aggregate fallback rasterization exceeds the configured pixel ceiling.
    pub const RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED: Self =
        Self("resource.total-rasterized-pixels-exceeded");
    /// Logical or scaled SVG dimensions exceed the configured ceiling.
    pub const RESOURCE_SVG_DIMENSIONS_EXCEEDED: Self = Self("resource.svg-dimensions-exceeded");
    /// PDF page dimensions exceed the configured point ceiling.
    pub const RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED: Self =
        Self("resource.pdf-page-dimensions-exceeded");
    /// Diagnostic occurrences exceed the configured ceiling.
    pub const RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED: Self = Self("resource.diagnostic-count-exceeded");
    /// Canonical logical working bytes exceed the configured ceiling.
    pub const RESOURCE_WORKING_BYTES_EXCEEDED: Self = Self("resource.working-bytes-exceeded");
    /// Complete artifact bytes exceed the configured ceiling.
    pub const RESOURCE_OUTPUT_BYTES_EXCEEDED: Self = Self("resource.output-bytes-exceeded");
    /// Aggregate semantic work exceeds the configured ceiling.
    pub const RESOURCE_TOTAL_WORK_EXCEEDED: Self = Self("resource.total-work-exceeded");
    /// A required physical allocation was unavailable.
    pub const RESOURCE_MEMORY_UNAVAILABLE: Self = Self("resource.memory-unavailable");

    /// The scene-document bytes are not valid UTF-8.
    pub const SCENE_INVALID_UTF8: Self = Self("scene.invalid-utf8");

    /// The scene-document bytes do not contain valid JSON syntax.
    pub const SCENE_INVALID_JSON: Self = Self("scene.invalid-json");

    /// The scene envelope does not have the required version-2 shape.
    pub const SCENE_INVALID_ENVELOPE: Self = Self("scene.invalid-envelope");

    /// The scene-document bytes exceed the fixed parser input ceiling.
    pub const SCENE_INPUT_TOO_LARGE: Self = Self("scene.input-too-large");

    /// The JSON nesting exceeds the fixed parser depth ceiling.
    pub const SCENE_JSON_DEPTH_EXCEEDED: Self = Self("scene.json-depth-exceeded");

    /// The JSON structure exceeds the fixed parser entry ceiling.
    pub const SCENE_JSON_STRUCTURE_EXCEEDED: Self = Self("scene.json-structure-exceeded");

    /// A decoded string or the decoded-string aggregate exceeds its ceiling.
    pub const SCENE_STRING_TOO_LARGE: Self = Self("scene.string-too-large");

    /// An object key exceeds the fixed parser key ceiling.
    pub const SCENE_OBJECT_KEY_TOO_LARGE: Self = Self("scene.object-key-too-large");

    /// A number token exceeds the fixed parser token ceiling.
    pub const SCENE_NUMBER_TOKEN_TOO_LARGE: Self = Self("scene.number-token-too-large");

    /// A later object member duplicates an earlier decoded key.
    pub const SCENE_DUPLICATE_OBJECT_KEY: Self = Self("scene.duplicate-object-key");

    /// Canonical parser live bytes exceed the fixed parser memory ceiling.
    pub const SCENE_PARSE_MEMORY_EXCEEDED: Self = Self("scene.parse-memory-exceeded");

    /// The requested compatibility revision is not retained.
    pub const REVISION_UNSUPPORTED: Self = Self("revision.unsupported");

    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Typed details attached to an [`ErrorDescriptor`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorDetails {
    /// Identifies a fixed scene-parser ceiling crossing.
    ParseLimit {
        /// The canonical parser-limit field.
        field: &'static str,
        /// The fixed configured ceiling.
        configured: u64,
        /// The unit measured by this ceiling.
        value_kind: ValueKind,
        /// The measured attempted or lower-bound value.
        measurement: Measurement,
    },
    /// Identifies a requested compatibility revision.
    Revision {
        /// The revision supplied by the caller.
        requested: CompatibilityRevision,
    },
    /// Identifies the rejected request field and any applicable asset or option.
    RequestValue {
        /// The canonical request field.
        field: &'static str,
        /// The applicable supplemental asset ID.
        asset_id: Option<String>,
        /// The applicable format-option name.
        option: Option<&'static str>,
    },
    /// Describes a compatibility recovery and optional renderer recipe.
    Compatibility {
        /// The recovery applied to retain a meaningful artifact.
        recovery: Recovery,
        /// The stable renderer recipe identity, when applicable.
        recipe_id: Option<String>,
    },
    /// Describes an authoritative or supplemental asset recovery.
    Asset {
        /// Scene file ID of the affected asset.
        asset_id: String,
        /// Caller-claimed advisory media type.
        claimed_media_type: Option<String>,
        /// Media type established from the payload bytes.
        validated_media_type: Option<String>,
        /// Recovery applied to the asset occurrence.
        recovery: Recovery,
        /// Stable renderer recipe identity, when applicable.
        recipe_id: Option<String>,
    },
    /// Describes a fatal or recoverable semantic resource crossing.
    ResourceLimit {
        /// Canonical resource-limit field.
        field: &'static str,
        /// Configured semantic ceiling.
        configured: ResourceValue,
        /// Unit measured by the ceiling.
        value_kind: ValueKind,
        /// Measured attempted or lower-bound value.
        measurement: ResourceMeasurement,
        /// Recovery applied for a local crossing; null for a fatal crossing.
        recovery: Option<Recovery>,
    },
    /// Identifies a physical allocation that could not be satisfied.
    MemoryUnavailable {
        /// Stable category of allocation subject.
        subject_kind: String,
        /// Stable identity of the allocation subject.
        subject: String,
    },
    /// Identifies one missing Unicode scalar and its substitution.
    MissingGlyph {
        /// Missing Unicode scalar value.
        codepoint: u32,
        /// Required substitution recovery.
        recovery: Recovery,
    },
    /// Identifies an optional metadata component failure.
    Metadata {
        /// Metadata component being processed.
        component: MetadataComponent,
        /// Operation that failed.
        operation: MetadataOperation,
        /// Required optional-metadata recovery.
        recovery: Recovery,
    },
    /// Identifies one nested source occurrence and recovery.
    NestedSource {
        /// Asset occurrence chain from the scene to the nested source.
        asset_chain: Vec<AssetChainEntry>,
        /// Canonical nested paths that caused the event.
        paths: Vec<NestedPath>,
        /// Recovery applied to the nested source.
        recovery: Recovery,
    },
    /// Describes an emitted or fatal PDF raster fallback closure.
    PdfRasterFallback {
        /// Rasterized for a warning; null for a fatal fallback failure.
        recovery: Option<Recovery>,
        /// Logical closure bounds, when safely established.
        logical_bounds: Option<LogicalBounds>,
        /// Final fallback pixel width, when safely established.
        pixel_width: Option<u64>,
        /// Final fallback pixel height, when safely established.
        pixel_height: Option<u64>,
        /// Canonical nested paths that caused the closure.
        causes: Vec<NestedPath>,
    },
}

/// Stable recovery applied after diagnosed compatibility loss.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum Recovery {
    /// Invalid or absent input was deterministically repaired.
    Repaired,
    /// Unsupported input was ignored.
    Ignored,
    /// Content was replaced by a compatibility placeholder.
    Placeholder,
    /// Content was omitted.
    Omitted,
    /// Unsafe or incompatible content was removed.
    Removed,
    /// Content was substituted deterministically.
    Substituted,
    /// Animation was flattened to a static frame.
    Flattened,
    /// A permitted isolated closure was rasterized.
    Rasterized,
    /// Optional metadata was omitted.
    MetadataOmitted,
}

/// Unit used by a stable limit measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    /// A byte count.
    Bytes,
    /// A structural-entry count.
    Count,
    /// A nesting depth.
    Depth,
    /// A pixel count.
    Pixels,
    /// A deterministic work-unit count.
    WorkUnits,
    /// A dimension.
    Dimension,
    /// A coordinate magnitude.
    Coordinate,
}

/// Stable attempted or lower-bound measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Measurement<T = u64> {
    /// The exact attempted value is known.
    Attempted {
        /// The exact attempted value.
        value: T,
    },
    /// Bounded processing stopped after establishing this lower bound.
    AtLeast {
        /// The established lower bound.
        value: T,
    },
}

/// Finite schema-1 number with canonical JSON serialization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteValue(f64);

impl FiniteValue {
    /// Creates a value only when the supplied number is finite.
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    /// Returns the finite binary64 value.
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Serialize for FiniteValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = self.0;
        if value == 0.0 {
            serializer.serialize_u8(0)
        } else if (0.0..=9_007_199_254_740_992.0).contains(&value) && (value as u64) as f64 == value
        {
            serializer.serialize_u64(value as u64)
        } else if (-9_007_199_254_740_992.0..0.0).contains(&value) && (value as i64) as f64 == value
        {
            serializer.serialize_i64(value as i64)
        } else {
            serializer.serialize_f64(value)
        }
    }
}

/// Nonnegative integer or finite number used by semantic resource details.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResourceValue {
    /// An integer byte, count, depth, pixel, or work value.
    Integer(u64),
    /// A finite dimension or coordinate value.
    Finite(FiniteValue),
}

impl Serialize for ResourceValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            Self::Integer(value) => serializer.serialize_u64(value),
            Self::Finite(value) => value.serialize(serializer),
        }
    }
}

/// Stable measurement form for integer, dimension, or coordinate resources.
pub type ResourceMeasurement = Measurement<ResourceValue>;

/// One entry in a nested asset occurrence chain.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssetChainEntry {
    pub(crate) asset_id: Option<String>,
    pub(crate) occurrence: u64,
}

impl AssetChainEntry {
    /// Returns the scene file ID, or null for an anonymous nested occurrence.
    pub fn asset_id(&self) -> Option<&str> {
        self.asset_id.as_deref()
    }

    /// Returns the zero-based occurrence ordinal.
    pub const fn occurrence(&self) -> u64 {
        self.occurrence
    }
}

/// Kind of one canonical nested-source path segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum NestedSegmentKind {
    /// Nested document node.
    Node,
    /// Nested node attribute.
    Attribute,
    /// CSS rule.
    CssRule,
    /// CSS declaration.
    CssDeclaration,
    /// Nested resource slot.
    ResourceSlot,
    /// Effect input.
    EffectInput,
}

/// One segment of a canonical nested-source path.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NestedPathSegment {
    pub(crate) kind: NestedSegmentKind,
    pub(crate) ordinal: u64,
    pub(crate) name: Option<String>,
}

impl NestedPathSegment {
    /// Returns the nested segment kind.
    pub const fn kind(&self) -> NestedSegmentKind {
        self.kind
    }

    /// Returns the zero-based segment ordinal.
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Returns the optional source name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Canonical identity path within one nested source.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NestedPath {
    pub(crate) segments: Vec<NestedPathSegment>,
}

impl NestedPath {
    /// Returns the canonical nested path segments.
    pub fn segments(&self) -> &[NestedPathSegment] {
        &self.segments
    }
}

/// Finite logical bounds of a PDF raster fallback closure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalBounds {
    pub(crate) x: FiniteValue,
    pub(crate) y: FiniteValue,
    pub(crate) width: FiniteValue,
    pub(crate) height: FiniteValue,
}

impl Serialize for LogicalBounds {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut bounds = serializer.serialize_struct("LogicalBounds", 4)?;
        bounds.serialize_field("x", &self.x)?;
        bounds.serialize_field("y", &self.y)?;
        bounds.serialize_field("width", &self.width)?;
        bounds.serialize_field("height", &self.height)?;
        bounds.end()
    }
}

impl LogicalBounds {
    /// Returns the logical x coordinate.
    pub const fn x(&self) -> f64 {
        self.x.get()
    }

    /// Returns the logical y coordinate.
    pub const fn y(&self) -> f64 {
        self.y.get()
    }

    /// Returns the finite logical width.
    pub const fn width(&self) -> f64 {
        self.width.get()
    }

    /// Returns the finite logical height.
    pub const fn height(&self) -> f64 {
        self.height.get()
    }
}

/// Optional artifact metadata component.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum MetadataComponent {
    /// Renderer provenance.
    Provenance,
    /// Pre-metadata compatibility diagnostic summary.
    DiagnosticSummary,
    /// Canonicalized original scene document.
    EmbeddedScene,
    /// Format-neutral artifact manifest.
    Manifest,
}

/// Operation performed on an optional metadata component.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum MetadataOperation {
    /// Canonical serialization.
    Serialization,
    /// Deterministic compression.
    Compression,
    /// Format-specific embedding.
    Embedding,
}

/// Stable serializable view of a typed renderer failure.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ErrorDescriptor {
    family: ErrorFamily,
    category: Option<RenderErrorCategory>,
    code: ErrorCode,
    message: String,
    format: Option<OutputFormat>,
    element_id: Option<String>,
    json_pointer: Option<String>,
    details: Option<ErrorDetails>,
}

impl ErrorDescriptor {
    pub(crate) fn scene_parse(
        code: ErrorCode,
        message: String,
        json_pointer: Option<String>,
        details: Option<ErrorDetails>,
    ) -> Self {
        Self {
            family: ErrorFamily::SceneParse,
            category: None,
            code,
            message,
            format: None,
            element_id: None,
            json_pointer,
            details,
        }
    }

    pub(crate) fn request(code: ErrorCode, message: String, details: ErrorDetails) -> Self {
        Self {
            family: ErrorFamily::Request,
            category: None,
            code,
            message,
            format: None,
            element_id: None,
            json_pointer: None,
            details: Some(details),
        }
    }

    pub(crate) fn render(
        category: RenderErrorCategory,
        code: ErrorCode,
        message: String,
        format: OutputFormat,
        element_id: Option<String>,
        json_pointer: String,
        details: Option<ErrorDetails>,
    ) -> Self {
        Self {
            family: ErrorFamily::Render,
            category: Some(category),
            code,
            message,
            format: Some(format),
            element_id,
            json_pointer: Some(json_pointer),
            details,
        }
    }

    /// Returns the typed failure family.
    pub const fn family(&self) -> ErrorFamily {
        self.family
    }

    /// Returns the render-failure category, when applicable.
    pub const fn category(&self) -> Option<RenderErrorCategory> {
        self.category
    }

    /// Returns the stable machine-readable error code.
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the human-readable error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the selected output format, when applicable.
    pub const fn format(&self) -> Option<OutputFormat> {
        self.format
    }

    /// Returns the normalized element ID, when applicable.
    pub fn element_id(&self) -> Option<&str> {
        self.element_id.as_deref()
    }

    /// Returns the scene-document JSON pointer, when applicable.
    pub fn json_pointer(&self) -> Option<&str> {
        self.json_pointer.as_deref()
    }

    /// Returns typed details required by the stable error code.
    pub const fn details(&self) -> Option<&ErrorDetails> {
        self.details.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_1_library_detail_kinds_use_the_declared_member_order() {
        let finite = |value| FiniteValue::new(value).expect("test value is finite");
        let path = NestedPath {
            segments: vec![NestedPathSegment {
                kind: NestedSegmentKind::Node,
                ordinal: 1,
                name: None,
            }],
        };
        let cases = [
            (
                ErrorDetails::Asset {
                    asset_id: "image-1".to_owned(),
                    claimed_media_type: Some("image/jpeg".to_owned()),
                    validated_media_type: Some("image/png".to_owned()),
                    recovery: Recovery::Repaired,
                    recipe_id: None,
                },
                concat!(
                    "{\"kind\":\"asset\",\"asset_id\":\"image-1\",",
                    "\"claimed_media_type\":\"image/jpeg\",",
                    "\"validated_media_type\":\"image/png\",",
                    "\"recovery\":\"repaired\",\"recipe_id\":null}"
                ),
            ),
            (
                ErrorDetails::ResourceLimit {
                    field: "max_coordinate_magnitude",
                    configured: ResourceValue::Finite(finite(1e9)),
                    value_kind: ValueKind::Coordinate,
                    measurement: ResourceMeasurement::Attempted {
                        value: ResourceValue::Finite(finite(1_000_000_000.5)),
                    },
                    recovery: Some(Recovery::Omitted),
                },
                concat!(
                    "{\"kind\":\"resource_limit\",",
                    "\"field\":\"max_coordinate_magnitude\",",
                    "\"configured\":1000000000,\"value_kind\":\"coordinate\",",
                    "\"measurement\":{\"kind\":\"attempted\",",
                    "\"value\":1000000000.5},\"recovery\":\"omitted\"}"
                ),
            ),
            (
                ErrorDetails::MemoryUnavailable {
                    subject_kind: "output_buffer".to_owned(),
                    subject: "svg".to_owned(),
                },
                concat!(
                    "{\"kind\":\"memory_unavailable\",",
                    "\"subject_kind\":\"output_buffer\",\"subject\":\"svg\"}"
                ),
            ),
            (
                ErrorDetails::MissingGlyph {
                    codepoint: 0x10ffff,
                    recovery: Recovery::Substituted,
                },
                concat!(
                    "{\"kind\":\"missing_glyph\",\"codepoint\":1114111,",
                    "\"recovery\":\"substituted\"}"
                ),
            ),
            (
                ErrorDetails::Metadata {
                    component: MetadataComponent::EmbeddedScene,
                    operation: MetadataOperation::Compression,
                    recovery: Recovery::MetadataOmitted,
                },
                concat!(
                    "{\"kind\":\"metadata\",\"component\":\"embedded_scene\",",
                    "\"operation\":\"compression\",",
                    "\"recovery\":\"metadata_omitted\"}"
                ),
            ),
            (
                ErrorDetails::NestedSource {
                    asset_chain: vec![AssetChainEntry {
                        asset_id: Some("image-1".to_owned()),
                        occurrence: 0,
                    }],
                    paths: vec![path.clone()],
                    recovery: Recovery::Removed,
                },
                concat!(
                    "{\"kind\":\"nested_source\",",
                    "\"asset_chain\":[{\"asset_id\":\"image-1\",\"occurrence\":0}],",
                    "\"paths\":[{\"segments\":[{\"kind\":\"node\",",
                    "\"ordinal\":1,\"name\":null}]}],\"recovery\":\"removed\"}"
                ),
            ),
            (
                ErrorDetails::PdfRasterFallback {
                    recovery: None,
                    logical_bounds: Some(LogicalBounds {
                        x: finite(-0.0),
                        y: finite(1.5),
                        width: finite(10.0),
                        height: finite(20.0),
                    }),
                    pixel_width: None,
                    pixel_height: None,
                    causes: vec![path],
                },
                concat!(
                    "{\"kind\":\"pdf_raster_fallback\",\"recovery\":null,",
                    "\"logical_bounds\":{\"x\":0,\"y\":1.5,",
                    "\"width\":10,\"height\":20},\"pixel_width\":null,",
                    "\"pixel_height\":null,\"causes\":[{\"segments\":[{",
                    "\"kind\":\"node\",\"ordinal\":1,\"name\":null}]}]}"
                ),
            ),
        ];

        for (details, expected) in cases {
            assert_eq!(
                serde_json::to_string(&details).expect("serialize schema-1 details"),
                expected
            );
        }
    }

    #[test]
    fn stable_numeric_values_reject_non_finite_input() {
        assert_eq!(FiniteValue::new(f64::NAN), None);
        assert_eq!(FiniteValue::new(f64::INFINITY), None);
        assert_eq!(FiniteValue::new(f64::NEG_INFINITY), None);
        assert_eq!(FiniteValue::new(-0.0).map(FiniteValue::get), Some(-0.0));
    }
}
