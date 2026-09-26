//! Font-oriented access to the sealed assets. Families enumerate faces in
//! declaration order even when their payloads are not bundled. Inspecting this
//! catalog never loads or decodes a font. Excalidraw source IDs, layout metrics,
//! fallback policy and derived subset names belong to the renderer.

use super::{AssetIntegrityError, AssetRecord, FontNotice, VerifiedAsset};

/// A bundled family, independent of Excalidraw's source-family identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyId {
    Cascadia,
    ComicShanns,
    Excalifont,
    Liberation,
    Lilita,
    Nunito,
    Virgil,
    Xiaolai,
    NotoEmoji,
}

impl FamilyId {
    pub const ALL: [Self; 9] = [
        Self::Cascadia,
        Self::ComicShanns,
        Self::Excalifont,
        Self::Liberation,
        Self::Lilita,
        Self::Nunito,
        Self::Virgil,
        Self::Xiaolai,
        Self::NotoEmoji,
    ];

    /// Accepts catalog and canonical display names; never an asset path.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|id| name == id.catalog_name() || name == id.display_name())
    }

    pub const fn catalog_name(self) -> &'static str {
        match self {
            Self::Cascadia => "Cascadia",
            Self::ComicShanns => "ComicShanns",
            Self::Excalifont => "Excalifont",
            Self::Liberation => "Liberation",
            Self::Lilita => "Lilita",
            Self::Nunito => "Nunito",
            Self::Virgil => "Virgil",
            Self::Xiaolai => "Xiaolai",
            Self::NotoEmoji => "NotoEmoji",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Cascadia => "Cascadia Code",
            Self::ComicShanns => "Comic Shanns",
            Self::Liberation => "Liberation Sans",
            Self::Lilita => "Lilita One",
            Self::NotoEmoji => "Noto Emoji",
            _ => self.catalog_name(),
        }
    }

    pub const fn pdf_name(self) -> &'static str {
        match self {
            Self::Cascadia => "CascadiaCode",
            Self::Liberation => "LiberationSans",
            Self::Lilita => "LilitaOne",
            _ => self.catalog_name(),
        }
    }

    /// All faces in pinned declaration order, including unavailable faces.
    pub fn faces(self) -> &'static [Face] {
        match self {
            Self::Cascadia => super::CASCADIA_FONT_FACES,
            Self::ComicShanns => super::COMICSHANNS_FONT_FACES,
            Self::Excalifont => super::EXCALIFONT_FONT_FACES,
            Self::Liberation => super::LIBERATION_FONT_FACES,
            Self::Lilita => super::LILITA_FONT_FACES,
            Self::Nunito => super::NUNITO_FONT_FACES,
            Self::Virgil => super::VIRGIL_FONT_FACES,
            Self::Xiaolai => super::XIAOLAI_FONT_FACES,
            Self::NotoEmoji => &NOTO_EMOJI_FACES,
        }
    }

    pub fn notice(self) -> Option<&'static FontNotice> {
        super::revision_1().font_notice(self.catalog_name())
    }
}

/// An opaque key within this catalog. Output provenance uses `Face::identity`,
/// not this key; its representation is not a serialized compatibility promise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaceId {
    family: FamilyId,
    order: usize,
}

impl FaceId {
    pub fn face(self) -> &'static Face {
        &self.family.faces()[self.order]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Container {
    Woff2,
    Sfnt,
}

/// A selectable program, either an official chunk or a complete fallback face.
/// Declared ranges govern eligibility; actual cmap coverage requires decoding.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    id: FaceId,
    source: Source,
    declared_ranges: &'static [(u32, u32)],
}

#[derive(Clone, Copy, Debug)]
enum Source {
    Chunk(&'static AssetRecord),
    CompleteNotoEmoji,
}

static NOTO_EMOJI_FACES: [Face; 1] = [Face {
    id: FaceId {
        family: FamilyId::NotoEmoji,
        order: 0,
    },
    source: Source::CompleteNotoEmoji,
    declared_ranges: super::ALL_UNICODE,
}];

impl Face {
    pub(super) const fn chunk(
        family: FamilyId,
        order: usize,
        record: &'static AssetRecord,
        declared_ranges: &'static [(u32, u32)],
    ) -> Self {
        Self {
            id: FaceId { family, order },
            source: Source::Chunk(record),
            declared_ranges,
        }
    }

    pub const fn id(&self) -> FaceId {
        self.id
    }
    pub const fn family(&self) -> FamilyId {
        self.id.family
    }
    pub const fn declared_ranges(&self) -> &'static [(u32, u32)] {
        self.declared_ranges
    }

    pub fn declares(&self, character: char) -> bool {
        let scalar = u32::from(character);
        self.declared_ranges
            .iter()
            .any(|(start, end)| (*start..=*end).contains(&scalar))
    }

    fn official_chunk(&self) -> Option<&'static AssetRecord> {
        match self.source {
            Source::Chunk(record) => Some(record),
            Source::CompleteNotoEmoji => None,
        }
    }

    /// Complete fallback programs must not receive an official-chunk PDF tag.
    pub const fn is_official_chunk(&self) -> bool {
        matches!(self.source, Source::Chunk(_))
    }

    /// Verifies the exact reviewed program without exposing storage segments.
    pub fn verify_decode(&self, bytes: Vec<u8>) -> Result<Vec<u8>, AssetIntegrityError> {
        match self.source {
            Source::Chunk(record) => {
                super::verify_reviewed_decode(record, bytes).map(VerifiedAsset::into_bytes)
            }
            Source::CompleteNotoEmoji => {
                if bytes.len() != super::NOTO_EMOJI_LENGTH
                    || super::sha256(&bytes) != super::NOTO_EMOJI_SHA256
                {
                    return Err(AssetIntegrityError {
                        asset_identity: self.identity(),
                        stage: super::IntegrityStage::ReviewedDecode,
                    });
                }
                Ok(bytes)
            }
        }
    }

    pub const fn identity(&self) -> &'static str {
        match self.source {
            Source::Chunk(record) => record.identity,
            Source::CompleteNotoEmoji => super::NOTO_EMOJI_IDENTITY,
        }
    }

    pub const fn source_sha256(&self) -> &'static str {
        match self.source {
            Source::Chunk(record) => record.complete_sha256,
            Source::CompleteNotoEmoji => super::NOTO_EMOJI_SHA256,
        }
    }

    pub const fn source_length(&self) -> usize {
        match self.source {
            Source::Chunk(record) => record.complete_length,
            Source::CompleteNotoEmoji => super::NOTO_EMOJI_LENGTH,
        }
    }

    pub const fn decoded_length(&self) -> usize {
        match self.source {
            Source::Chunk(record) => record.decoded_length,
            Source::CompleteNotoEmoji => super::NOTO_EMOJI_LENGTH,
        }
    }

    pub const fn container(&self) -> Container {
        match self.source {
            Source::Chunk(_) => Container::Woff2,
            Source::CompleteNotoEmoji => Container::Sfnt,
        }
    }

    pub fn is_available(&self) -> bool {
        self.official_chunk().is_none_or(AssetRecord::is_available)
    }

    /// Original, verified source bytes. SVG subsetting consumes these rather
    /// than a derived subset or an arbitrary equivalent font program.
    pub fn load_source(&self) -> Result<VerifiedAsset, AssetIntegrityError> {
        match self.source {
            Source::Chunk(record) => super::verify_asset(record),
            Source::CompleteNotoEmoji => super::load_noto_emoji_sfnt(),
        }
    }

    /// The optional checked-in reviewed decode of this same face.
    pub fn load_predecoded(&self) -> Result<Option<VerifiedAsset>, AssetIntegrityError> {
        if self.official_chunk().is_some_and(|record| {
            record.upstream_path == super::excalifont_basic_latin_sfnt_transformation().source_path
        }) {
            super::load_excalifont_basic_latin_sfnt().map(Some)
        } else {
            Ok(None)
        }
    }
}

/// Looks up an official chunk without making callers join its family and
/// declaration tables. Complete fallback faces are reached through their family.
pub fn find_face(upstream_path: &str) -> Option<&'static Face> {
    let record = super::find_asset(upstream_path)?;
    FamilyId::from_name(record.family)?
        .faces()
        .iter()
        .find(|face| face.identity() == record.identity)
}
