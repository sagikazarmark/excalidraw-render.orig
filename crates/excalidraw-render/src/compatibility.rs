use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use serde::Serialize;

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

const REFERENCE_PROFILE_REVISION_1: ReferenceProfileIdentity = ReferenceProfileIdentity {
    compatibility_revision: CompatibilityRevision::REVISION_1,
    excalidraw_release: "v0.18.1",
    reference_profile_digest:
        "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71",
};

const SUPPORTED_REVISIONS: &[CompatibilityRevision] = &[CompatibilityRevision::REVISION_1];

/// Immutable renderer configured for one compatibility revision.
#[derive(Clone, Debug)]
pub struct Renderer {
    profile: &'static ReferenceProfileIdentity,
}

impl Renderer {
    /// Constructs a renderer for [`DEFAULT_COMPATIBILITY_REVISION`].
    pub const fn new() -> Self {
        Self {
            profile: &REFERENCE_PROFILE_REVISION_1,
        }
    }

    /// Constructs a renderer for a retained compatibility revision.
    pub fn for_revision(revision: CompatibilityRevision) -> Result<Self, UnsupportedRevisionError> {
        Self::profile_identity(revision).map(|profile| Self { profile })
    }

    /// Returns retained compatibility revisions in increasing order.
    pub const fn supported_revisions() -> &'static [CompatibilityRevision] {
        SUPPORTED_REVISIONS
    }

    /// Returns the reference profile identity for a retained revision.
    pub fn profile_identity(
        revision: CompatibilityRevision,
    ) -> Result<&'static ReferenceProfileIdentity, UnsupportedRevisionError> {
        match revision {
            CompatibilityRevision::REVISION_1 => Ok(&REFERENCE_PROFILE_REVISION_1),
            revision => Err(UnsupportedRevisionError { revision }),
        }
    }

    /// Returns this renderer's selected reference profile identity.
    pub const fn selected_profile_identity(&self) -> &'static ReferenceProfileIdentity {
        self.profile
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
    /// A requested compatibility revision is not retained.
    UnsupportedRevision,
}

impl ErrorFamily {
    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorDetails {
    /// Identifies a requested compatibility revision.
    Revision {
        /// The revision supplied by the caller.
        requested: CompatibilityRevision,
    },
}

/// Stable serializable view of a typed renderer failure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ErrorDescriptor {
    family: ErrorFamily,
    category: Option<&'static str>,
    code: ErrorCode,
    message: String,
    format: Option<&'static str>,
    element_id: Option<String>,
    json_pointer: Option<String>,
    details: Option<ErrorDetails>,
}

impl ErrorDescriptor {
    /// Returns the typed failure family.
    pub const fn family(&self) -> ErrorFamily {
        self.family
    }

    /// Returns the render-failure category, when applicable.
    pub const fn category(&self) -> Option<&str> {
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
    pub const fn format(&self) -> Option<&str> {
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
