//! Pinned Excalidraw fonts and complete selection metadata.
//!
//! An implementation detail of `excalidraw-render`, which pins this crate
//! exactly; its API carries no stability guarantee for other consumers.
//!
//! The default `cjk` feature bundles Xiaolai. Disabling it retains every catalog
//! record and declaration; loading an unavailable chunk returns an explicit error.

#![doc(hidden)]

use std::collections::TryReserveError;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

pub mod fonts;

/// Default raw byte ceiling; the common Xiaolai payload has a targeted exception.
pub const RAW_SHARD_LIMIT: usize = 8 * 1024 * 1024;

/// Raw ceiling for the published common-Xiaolai grouping. Package archives are
/// separately required to be smaller than 10,000,000 decimal bytes.
pub const XIAOLAI_RAW_SHARD_LIMIT: usize = 10_000_000;

/// Renderer-owned treatment for one table in the sealed SVG subset profile.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontSubsetTableAction {
    Subset,
    Rewrite,
    PassThrough,
    Drop,
}

/// One frozen table rule in the compatibility-revision-1 SVG subset profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontSubsetTableRule {
    pub tag: [u8; 4],
    pub action: FontSubsetTableAction,
    pub required: bool,
}

const fn table_rule(
    tag: [u8; 4],
    action: FontSubsetTableAction,
    required: bool,
) -> FontSubsetTableRule {
    FontSubsetTableRule {
        tag,
        action,
        required,
    }
}

/// Non-table settings in the compatibility-revision-1 SVG subset profile.
pub const FONT_SUBSET_PROFILE_SETTINGS_V1: &[u8] = b"excalidraw-render:svg-font-subset-profile:v1\nclosure=layout,composite,bidi\nlayout=all\nhinting=retain\nglyph-renumbering=true\nvariations=retain\nnames=0-6,13-14:en-US\nflags=none\noverlap-simple=clear\nempty-glyf=nonempty-notdef\nwoff2=wuff-0.2.9,skera-0.9.0,ttf2woff2-0.14.1,brotli-9.0.0,q11,glyf-loca,threads-1,portable-math\n";

/// Complete table-action manifest for every table admitted by the sealed corpus.
pub const FONT_SUBSET_TABLE_RULES_V1: &[FontSubsetTableRule] = &[
    table_rule(*b"BASE", FontSubsetTableAction::Subset, false),
    table_rule(*b"CBDT", FontSubsetTableAction::Subset, false),
    table_rule(*b"CBLC", FontSubsetTableAction::Subset, false),
    table_rule(*b"COLR", FontSubsetTableAction::Subset, false),
    table_rule(*b"CPAL", FontSubsetTableAction::Subset, false),
    table_rule(*b"GDEF", FontSubsetTableAction::Subset, false),
    table_rule(*b"GSUB", FontSubsetTableAction::Subset, false),
    table_rule(*b"GPOS", FontSubsetTableAction::Subset, false),
    table_rule(*b"OS/2", FontSubsetTableAction::Subset, true),
    table_rule(*b"STAT", FontSubsetTableAction::Subset, false),
    table_rule(*b"VORG", FontSubsetTableAction::Subset, false),
    table_rule(*b"avar", FontSubsetTableAction::Subset, false),
    table_rule(*b"cmap", FontSubsetTableAction::Subset, true),
    table_rule(*b"cvar", FontSubsetTableAction::Subset, false),
    table_rule(*b"cvt ", FontSubsetTableAction::Subset, false),
    table_rule(*b"fvar", FontSubsetTableAction::Subset, false),
    table_rule(*b"glyf", FontSubsetTableAction::Subset, true),
    table_rule(*b"gvar", FontSubsetTableAction::Subset, false),
    table_rule(*b"hdmx", FontSubsetTableAction::Subset, false),
    table_rule(*b"head", FontSubsetTableAction::Subset, true),
    table_rule(*b"hhea", FontSubsetTableAction::Subset, true),
    table_rule(*b"hmtx", FontSubsetTableAction::Subset, true),
    table_rule(*b"loca", FontSubsetTableAction::Subset, true),
    table_rule(*b"maxp", FontSubsetTableAction::Subset, true),
    table_rule(*b"post", FontSubsetTableAction::Subset, false),
    table_rule(*b"sbix", FontSubsetTableAction::Subset, false),
    table_rule(*b"vhea", FontSubsetTableAction::Subset, false),
    table_rule(*b"vmtx", FontSubsetTableAction::Subset, false),
    table_rule(*b"HVAR", FontSubsetTableAction::Subset, false),
    table_rule(*b"VVAR", FontSubsetTableAction::Subset, false),
    table_rule(*b"MVAR", FontSubsetTableAction::Subset, false),
    table_rule(*b"name", FontSubsetTableAction::Rewrite, true),
    table_rule(*b"gasp", FontSubsetTableAction::PassThrough, false),
    table_rule(*b"fpgm", FontSubsetTableAction::PassThrough, false),
    table_rule(*b"prep", FontSubsetTableAction::PassThrough, false),
    table_rule(*b"VDMX", FontSubsetTableAction::PassThrough, false),
    table_rule(*b"morx", FontSubsetTableAction::Drop, false),
    table_rule(*b"mort", FontSubsetTableAction::Drop, false),
    table_rule(*b"kerx", FontSubsetTableAction::Drop, false),
    table_rule(*b"kern", FontSubsetTableAction::Drop, false),
    table_rule(*b"JSTF", FontSubsetTableAction::Drop, false),
    table_rule(*b"DSIG", FontSubsetTableAction::Drop, false),
    table_rule(*b"EBDT", FontSubsetTableAction::Drop, false),
    table_rule(*b"EBLC", FontSubsetTableAction::Drop, false),
    table_rule(*b"EBSC", FontSubsetTableAction::Drop, false),
    table_rule(*b"SVG ", FontSubsetTableAction::Drop, false),
    table_rule(*b"PCLT", FontSubsetTableAction::Drop, false),
    table_rule(*b"LTSH", FontSubsetTableAction::Drop, false),
    table_rule(*b"Feat", FontSubsetTableAction::Drop, false),
    table_rule(*b"Glat", FontSubsetTableAction::Drop, false),
    table_rule(*b"Gloc", FontSubsetTableAction::Drop, false),
    table_rule(*b"Silf", FontSubsetTableAction::Drop, false),
    table_rule(*b"Sill", FontSubsetTableAction::Drop, false),
    table_rule(*b"FFTM", FontSubsetTableAction::Drop, false),
];

/// Digest of the settings and executable table rules in canonical rule order.
pub fn font_subset_profile_digest_v1() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FONT_SUBSET_PROFILE_SETTINGS_V1);
    hasher.update((FONT_SUBSET_TABLE_RULES_V1.len() as u64).to_le_bytes());
    for rule in FONT_SUBSET_TABLE_RULES_V1 {
        hasher.update(rule.tag);
        hasher.update([rule.action as u8, u8::from(rule.required)]);
    }
    hasher.finalize().into()
}

/// Chromium 149's union of the four CJK script properties and Excalidraw's
/// explicit punctuation, symbol, and currency classes.
pub const CJK_SCALAR_RANGES_V1: &[(u32, u32)] = &[
    (0x1100, 0x11ff),
    (0x2025, 0x2025),
    (0x2e80, 0x2e99),
    (0x2e9b, 0x2ef3),
    (0x2f00, 0x2fd5),
    (0x3001, 0x3003),
    (0x3005, 0x3012),
    (0x3014, 0x301f),
    (0x3021, 0x3029),
    (0x302e, 0x3030),
    (0x3038, 0x303b),
    (0x3041, 0x3096),
    (0x309d, 0x309f),
    (0x30a1, 0x30ff),
    (0x3131, 0x318e),
    (0x31f0, 0x321e),
    (0x3260, 0x327e),
    (0x32d0, 0x32fe),
    (0x3300, 0x3357),
    (0x3400, 0x4dbf),
    (0x4e00, 0x9fff),
    (0xa960, 0xa97c),
    (0xac00, 0xd7a3),
    (0xd7b0, 0xd7c6),
    (0xd7cb, 0xd7fb),
    (0xf900, 0xfa6d),
    (0xfa70, 0xfad9),
    (0xff01, 0xff01),
    (0xff03, 0xff04),
    (0xff06, 0xff0f),
    (0xff1a, 0xff1f),
    (0xff3b, 0xff3e),
    (0xff40, 0xff40),
    (0xff5b, 0xff5d),
    (0xff5f, 0xff60),
    (0xff62, 0xff63),
    (0xff66, 0xff6f),
    (0xff71, 0xff9d),
    (0xffa0, 0xffbe),
    (0xffc2, 0xffc7),
    (0xffca, 0xffcf),
    (0xffd2, 0xffd7),
    (0xffda, 0xffdc),
    (0xffe0, 0xffe6),
    (0x16fe2, 0x16fe3),
    (0x16ff0, 0x16ff6),
    (0x1aff0, 0x1aff3),
    (0x1aff5, 0x1affb),
    (0x1affd, 0x1affe),
    (0x1b000, 0x1b122),
    (0x1b132, 0x1b132),
    (0x1b150, 0x1b152),
    (0x1b155, 0x1b155),
    (0x1b164, 0x1b167),
    (0x1f200, 0x1f200),
    (0x20000, 0x2a6df),
    (0x2a700, 0x2b81d),
    (0x2b820, 0x2cead),
    (0x2ceb0, 0x2ebe0),
    (0x2ebf0, 0x2ee5d),
    (0x2f800, 0x2fa1d),
    (0x30000, 0x3134a),
    (0x31350, 0x33479),
];

pub const CJK_SCALAR_COUNT_V1: u32 = 115_861;
pub const CJK_SCALAR_TABLE_SHA256_V1: &str =
    "fc4e8ce56cb7bf4bca9dfd6f3495c9ae9f5440a5818aa3e6f645d5303265ff85";
pub const CJK_SCALAR_TABLE_BYTES_V1: [u8; 520] = cjk_scalar_table_bytes_v1();

const fn cjk_scalar_table_bytes_v1() -> [u8; 520] {
    let mut bytes = [0_u8; 520];
    bytes[0] = b'E';
    bytes[1] = b'R';
    bytes[2] = b'-';
    bytes[3] = b'C';
    bytes[4] = b'J';
    bytes[5] = b'K';
    bytes[6] = b'1';
    put_u32_le(&mut bytes, 8, CJK_SCALAR_COUNT_V1);
    put_u32_le(&mut bytes, 12, CJK_SCALAR_RANGES_V1.len() as u32);
    let mut index = 0;
    while index < CJK_SCALAR_RANGES_V1.len() {
        let offset = 16 + index * 8;
        put_u32_le(&mut bytes, offset, CJK_SCALAR_RANGES_V1[index].0);
        put_u32_le(&mut bytes, offset + 4, CJK_SCALAR_RANGES_V1[index].1);
        index += 1;
    }
    bytes
}

const fn put_u32_le(bytes: &mut [u8; 520], offset: usize, value: u32) {
    let value = value.to_le_bytes();
    bytes[offset] = value[0];
    bytes[offset + 1] = value[1];
    bytes[offset + 2] = value[2];
    bytes[offset + 3] = value[3];
}

/// Frozen Chromium-149 CJK predicate used by compatibility revision 1.
pub fn is_cjk_scalar_v1(scalar: u32) -> bool {
    CJK_SCALAR_RANGES_V1
        .binary_search_by(|(start, end)| {
            if scalar < *start {
                std::cmp::Ordering::Greater
            } else if scalar > *end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// One immutable artifact incorporated by a compatibility revision.
#[derive(Clone, Copy, Debug)]
pub struct IncorporatedArtifact {
    pub repository_path: &'static str,
    pub sha256: &'static str,
    pub bytes: &'static [u8],
}

/// Historical packing coordinates retained for frozen catalog provenance.
/// Runtime loading uses the asset's individual embedded file instead.
#[derive(Clone, Copy, Debug)]
pub struct AssetSegment {
    pub identity: &'static str,
    pub shard: &'static str,
    pub offset: usize,
    pub length: usize,
    pub sha256: &'static str,
    pub order: usize,
}

/// One entry in the complete built-in asset catalog, including unbundled assets.
///
/// `decoded_length` and `decoded_sha256` identify the chunk's reviewed decode:
/// the sfnt the pinned `wuff 0.2.9` decode of the exact stored bytes produces,
/// as `catalog/reviewed-decodes/revision-1.json` records and its approval binds
/// (spec 8.6). [`verify_reviewed_decode`] is the only way a decode becomes
/// embeddable bytes.
#[derive(Clone, Copy, Debug)]
pub struct AssetRecord {
    pub identity: &'static str,
    pub upstream_path: &'static str,
    pub upstream_revision: &'static str,
    pub upstream_url: &'static str,
    pub complete_sha256: &'static str,
    pub complete_length: usize,
    pub decoded_sha256: &'static str,
    pub decoded_length: usize,
    pub family: &'static str,
    pub license_identity: &'static str,
    pub provenance_identity: &'static str,
    pub transformation_identity: &'static str,
    pub reserved_font_names: &'static [&'static str],
    pub role: &'static str,
    pub output_formats: &'static [&'static str],
    /// The complete original file, absent only when its bundling feature is disabled.
    pub bytes: Option<&'static [u8]>,
    pub segments: &'static [AssetSegment],
}

impl AssetRecord {
    /// Whether the font file is bundled. This does not verify integrity.
    pub fn is_available(&self) -> bool {
        self.bytes.is_some()
    }
}

/// The license an official rendering font family is offered under.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontLicense {
    /// SIL Open Font License, Version 1.1.
    Ofl11,
    /// MIT License.
    Mit,
}

impl FontLicense {
    /// The sentence an artifact's attribution notice states the license with,
    /// without its terminal period.
    pub const fn sentence(self) -> &'static str {
        match self {
            Self::Ofl11 => "Licensed under the SIL Open Font License, Version 1.1",
            Self::Mit => "Licensed under the MIT License",
        }
    }
}

/// One official rendering font family's notice, as the license bundle prints
/// it under "Official rendering font notices" and as artifacts carry it. The
/// generator emits the bundle and this table from one source, so an artifact
/// never has to read the bundle.
///
/// An artifact carries the family's notice as two objects (spec 12, #151):
/// one attribution notice under `license_identity`, stating the copyright
/// and Reserved Font Name lines and naming the license, and the license body
/// itself under `terms_identity`, which every family licensed under the same
/// text shares, so a document embedding four OFL fonts carries the OFL once.
#[derive(Clone, Copy, Debug)]
pub struct FontNotice {
    /// The catalog `family` of the assets the notice covers.
    pub family: &'static str,
    pub license_identity: &'static str,
    pub license: FontLicense,
    /// The provenance line without its license sentence.
    pub attribution: &'static str,
    /// The identity of the license body notice the attribution refers to:
    /// `license.ofl-1.1` for every OFL family, the family's own MIT text for
    /// Comic Shanns, whose MIT file carries its copyright line.
    pub terms_identity: &'static str,
    /// The license body the artifact carries once under `terms_identity`: the
    /// bundle's copy of the license text.
    pub terms: &'static str,
}

impl FontNotice {
    /// The attribution notice an artifact carries for the family: the
    /// copyright and Reserved Font Name lines, the license sentence, and the
    /// identity of the notice carrying the license text.
    pub fn attribution_notice(&self) -> Result<String, TryReserveError> {
        const REFERENCE: &str = "; the license text is this manifest's ";
        const SUFFIX: &str = " notice.";
        let sentence = self.license.sentence();
        let mut text = String::new();
        text.try_reserve_exact(
            self.attribution
                .len()
                .saturating_add(1)
                .saturating_add(sentence.len())
                .saturating_add(REFERENCE.len())
                .saturating_add(self.terms_identity.len())
                .saturating_add(SUFFIX.len()),
        )?;
        text.push_str(self.attribution);
        text.push(' ');
        text.push_str(sentence);
        text.push_str(REFERENCE);
        text.push_str(self.terms_identity);
        text.push_str(SUFFIX);
        Ok(text)
    }
}

/// Immutable compatibility-revision catalog selected by the renderer.
#[derive(Debug)]
pub struct RevisionCatalog {
    pub compatibility_revision: u64,
    pub excalidraw_release: &'static str,
    pub source_commit: &'static str,
    pub npm_integrity: &'static str,
    pub reference_profile_digest: &'static str,
    pub catalog_sha256: &'static str,
    pub catalog_json: &'static [u8],
    pub incorporated_artifacts: &'static [IncorporatedArtifact],
    pub fallback_recipe_identities: &'static [&'static str],
    pub notice_identities: &'static [&'static str],
    pub provenance_identities: &'static [&'static str],
    pub transformation_identities: &'static [&'static str],
    pub license_identities: &'static [&'static str],
    pub font_notices: &'static [FontNotice],
    pub assets: &'static [AssetRecord],
}

impl RevisionCatalog {
    /// Looks up one built-in asset's record by its catalog identity.
    pub fn asset(&self, identity: &str) -> Option<&'static AssetRecord> {
        self.assets.iter().find(|asset| asset.identity == identity)
    }

    /// Looks up the notice covering one official rendering font family.
    pub fn font_notice(&self, family: &str) -> Option<&'static FontNotice> {
        self.font_notices
            .iter()
            .find(|notice| notice.family == family)
    }
}

/// Bytes returned only after complete-file digest verification. Embedded files
/// are borrowed; reviewed runtime decodes own their bytes.
#[derive(Debug)]
pub struct VerifiedAsset {
    bytes: std::borrow::Cow<'static, [u8]>,
}

/// Auditable transformation metadata for one checked-in predecoded font.
#[derive(Clone, Copy, Debug)]
pub struct PredecodedFontTransformation {
    pub source_path: &'static str,
    pub source_sha256: &'static str,
    pub output_sha256: &'static str,
    pub tool: &'static str,
    pub invocation: &'static str,
    pub reviewer: &'static str,
    pub review_date: &'static str,
    pub exact_output_bytes_reviewed: bool,
    pub comparison: &'static str,
    pub metadata_json: &'static [u8],
}

impl VerifiedAsset {
    /// Returns the verified complete asset bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the verification, yielding the verified bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes.into_owned()
    }
}

/// Indicates unavailable payload bytes or bytes that fail their sealed identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetIntegrityError {
    asset_identity: &'static str,
    stage: IntegrityStage,
}

/// Whether the complete file or its reviewed decode failed verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IntegrityStage {
    Unavailable,
    Complete,
    ReviewedDecode,
}

impl AssetIntegrityError {
    /// Whether the asset is known but its payload is not bundled in this build.
    pub const fn is_unavailable(&self) -> bool {
        matches!(self.stage, IntegrityStage::Unavailable)
    }

    /// Returns the affected complete asset identity.
    pub const fn asset_identity(&self) -> &'static str {
        self.asset_identity
    }

    /// Historical compatibility accessor. Individual files have no failing
    /// segment identity; use `asset_identity` to identify the font.
    pub const fn segment_identity(&self) -> Option<&'static str> {
        match self.stage {
            IntegrityStage::Unavailable
            | IntegrityStage::Complete
            | IntegrityStage::ReviewedDecode => None,
        }
    }
}

impl fmt::Display for AssetIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.stage {
            IntegrityStage::Unavailable => write!(
                formatter,
                "asset {} is unavailable in this build",
                self.asset_identity
            ),
            IntegrityStage::Complete => write!(
                formatter,
                "asset {} has an invalid complete digest",
                self.asset_identity
            ),
            IntegrityStage::ReviewedDecode => write!(
                formatter,
                "asset {} decoded to bytes that are not its reviewed decode",
                self.asset_identity
            ),
        }
    }
}

impl Error for AssetIntegrityError {}

/// One official font chunk as its pinned upstream `@font-face` declaration
/// names it: the WOFF2 file name and the Unicode range it declares.
///
/// Face tables are immutable pinned constants: the hand-pinned families parse
/// their upstream CSS at compile time and Xiaolai's table is generated by
/// `xtask catalog` from the digest-pinned index. Neither is a lazily initialised
/// or mutable global, so no render parses face metadata or allocates for it.
#[derive(Clone, Copy, Debug)]
struct FontFaceDeclaration {
    file_name: &'static str,
    unicode_ranges: &'static [(u32, u32)],
}

include!("generated/revision_1.rs");
include!("generated/xiaolai_faces_1.rs");

/// Returns the sealed compatibility revision 1 catalog.
pub const fn revision_1() -> &'static RevisionCatalog {
    &REVISION_1
}

/// Looks up one built-in asset's catalog record by its upstream package path.
pub fn find_asset(upstream_path: &str) -> Option<&'static AssetRecord> {
    let index = ASSETS
        .binary_search_by_key(&upstream_path, |asset| asset.upstream_path)
        .ok()?;
    ASSETS.get(index)
}

/// Looks up and verifies one complete built-in asset by its upstream package path.
pub fn load_asset(upstream_path: &str) -> Result<Option<VerifiedAsset>, AssetIntegrityError> {
    find_asset(upstream_path).map(verify_asset).transpose()
}

/// Accepts decoded sfnt bytes as one official font chunk's reviewed decode
/// exactly when they have the length and digest the catalog records for it;
/// any other bytes are an integrity failure, whatever produced them.
pub fn verify_reviewed_decode(
    asset: &'static AssetRecord,
    decoded: Vec<u8>,
) -> Result<VerifiedAsset, AssetIntegrityError> {
    if decoded.len() != asset.decoded_length || sha256(&decoded) != asset.decoded_sha256 {
        return Err(AssetIntegrityError {
            asset_identity: asset.identity,
            stage: IntegrityStage::ReviewedDecode,
        });
    }
    Ok(VerifiedAsset {
        bytes: decoded.into(),
    })
}

const EXCALIFONT_BASIC_LATIN_SFNT_IDENTITY: &str = concat!(
    "catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt@sha256:",
    "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb"
);
const EXCALIFONT_BASIC_LATIN_SFNT: &[u8] = include_bytes!(
    "../catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt"
);

/// Loads the digest-verified predecoded sfnt for the manifested Excalifont basic-Latin chunk.
pub fn load_excalifont_basic_latin_sfnt() -> Result<VerifiedAsset, AssetIntegrityError> {
    if EXCALIFONT_BASIC_LATIN_SFNT.len() != 61_948
        || sha256(EXCALIFONT_BASIC_LATIN_SFNT)
            != "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb"
    {
        return Err(AssetIntegrityError {
            asset_identity: EXCALIFONT_BASIC_LATIN_SFNT_IDENTITY,
            stage: IntegrityStage::Complete,
        });
    }
    Ok(VerifiedAsset {
        bytes: std::borrow::Cow::Borrowed(EXCALIFONT_BASIC_LATIN_SFNT),
    })
}

/// The SHA-256 of the exact manifested Noto Emoji 3.003 fallback input.
pub const NOTO_EMOJI_SHA256: &str =
    "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634";
/// The byte length of the exact manifested Noto Emoji 3.003 fallback input.
pub const NOTO_EMOJI_LENGTH: usize = 862_680;
/// The catalog identity of the Noto Emoji 3.003 fallback input: its upstream
/// path and digest, as `catalog/fallback/NotoEmoji-Regular.ttf.json` records
/// it. The face is a TrueType sfnt, not a WOFF2 chunk, and belongs to no
/// resolved family; it is the one non-catalog source direct SVG subsets.
pub const NOTO_EMOJI_IDENTITY: &str = concat!(
    "scripts/woff2/assets/NotoEmoji-Regular.ttf@sha256:",
    "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"
);
/// The catalog family the Noto Emoji notice is filed under.
pub const NOTO_EMOJI_FAMILY: &str = "NotoEmoji";
const NOTO_EMOJI_SFNT: &[u8] = include_bytes!("../catalog/fallback/NotoEmoji-Regular.ttf");

/// Loads the digest-verified Noto Emoji 3.003 fallback input.
pub fn load_noto_emoji_sfnt() -> Result<VerifiedAsset, AssetIntegrityError> {
    if NOTO_EMOJI_SFNT.len() != NOTO_EMOJI_LENGTH || sha256(NOTO_EMOJI_SFNT) != NOTO_EMOJI_SHA256 {
        return Err(AssetIntegrityError {
            asset_identity: NOTO_EMOJI_IDENTITY,
            stage: IntegrityStage::Complete,
        });
    }
    Ok(VerifiedAsset {
        bytes: std::borrow::Cow::Borrowed(NOTO_EMOJI_SFNT),
    })
}

/// Number of comma-separated tokens in a pinned CSS `unicode-range` value.
const fn unicode_range_count(declared: &str) -> usize {
    let bytes = declared.as_bytes();
    let mut count = 1;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b',' {
            count += 1;
        }
        index += 1;
    }
    count
}

/// Parses the hexadecimal scalar starting at `cursor`, returning it with the
/// position after its last digit.
const fn parse_unicode_range_scalar(bytes: &[u8], mut cursor: usize) -> (u32, usize) {
    let start = cursor;
    let mut value = 0_u32;
    while cursor < bytes.len() {
        let digit = match bytes[cursor] {
            byte @ b'0'..=b'9' => byte - b'0',
            byte @ b'a'..=b'f' => byte - b'a' + 10,
            byte @ b'A'..=b'F' => byte - b'A' + 10,
            _ => break,
        };
        assert!(
            cursor - start < 6,
            "unicode-range scalar has more than six hex digits"
        );
        value = value * 16 + digit as u32;
        cursor += 1;
    }
    assert!(cursor > start, "unicode-range scalar has no hex digits");
    (value, cursor)
}

/// Parses a pinned CSS `unicode-range` value into its `N == unicode_range_count`
/// ascending inclusive scalar ranges at compile time, so a malformed pin fails
/// the build rather than a render. `xtask::catalog::parse_unicode_ranges` applies
/// the same grammar to the Xiaolai index at generation time; `xtask` tests hold
/// the two in agreement.
const fn parse_unicode_ranges<const N: usize>(declared: &str) -> [(u32, u32); N] {
    let bytes = declared.as_bytes();
    let mut ranges = [(0_u32, 0_u32); N];
    let mut cursor = 0;
    let mut index = 0;
    while index < N {
        assert!(
            cursor + 2 <= bytes.len()
                && (bytes[cursor] == b'U' || bytes[cursor] == b'u')
                && bytes[cursor + 1] == b'+',
            "unicode-range token must start with U+"
        );
        let (start, next) = parse_unicode_range_scalar(bytes, cursor + 2);
        cursor = next;
        let end = if cursor < bytes.len() && bytes[cursor] == b'-' {
            let (end, next) = parse_unicode_range_scalar(bytes, cursor + 1);
            cursor = next;
            end
        } else {
            start
        };
        assert!(
            start <= end && end <= 0x10ffff,
            "unicode-range token must be an ascending Unicode scalar range"
        );
        ranges[index] = (start, end);
        index += 1;
        if index < N {
            assert!(
                cursor < bytes.len() && bytes[cursor] == b',',
                "unicode-range tokens must be comma separated"
            );
            cursor += 1;
        }
    }
    assert!(cursor == bytes.len(), "unicode-range has trailing content");
    ranges
}

/// Parses one upstream CSS `unicode-range` literal into a `&'static` range table.
macro_rules! unicode_ranges {
    ($declared:literal) => {
        &parse_unicode_ranges::<{ unicode_range_count($declared) }>($declared)
    };
}

/// The one declared range of the single-face families, covering all of Unicode.
const ALL_UNICODE: &[(u32, u32)] = &[(0, 0x10ffff)];

/// Google Fonts subset declarations shared by the Lilita and Nunito faces.
const LATIN: &[(u32, u32)] = unicode_ranges!("U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD");
const LATIN_EXT: &[(u32, u32)] = unicode_ranges!("U+0100-02AF,U+0304,U+0308,U+0329,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF");
const CYRILLIC_EXT: &[(u32, u32)] =
    unicode_ranges!("U+0460-052F,U+1C80-1C88,U+20B4,U+2DE0-2DFF,U+A640-A69F,U+FE2E-FE2F");
const CYRILLIC: &[(u32, u32)] =
    unicode_ranges!("U+0301,U+0400-045F,U+0490-0491,U+04B0-04B1,U+2116");
const VIETNAMESE: &[(u32, u32)] = unicode_ranges!("U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB");

/// Comic Shanns faces in pinned face order.
const COMIC_SHANNS_FACES: &[FontFaceDeclaration] = &[
    FontFaceDeclaration {
        file_name: "ComicShanns-Regular-279a7b317d12eb88de06167bd672b4b4.woff2",
        unicode_ranges: unicode_ranges!("U+20-7e,U+a1-a6,U+a8,U+ab-ac,U+af-b1,U+b4,U+b8,U+bb-bc,U+bf-cf,U+d1-d7,U+d9-de,U+e0-ef,U+f1-f7,U+f9-ff,U+131,U+152-153,U+2c6,U+2da,U+2dc,U+2013-2014,U+2018-201a,U+201c-201d,U+2020-2022,U+2026,U+2039-203a,U+2044,U+20ac,U+2191,U+2193,U+2212"),
    },
    FontFaceDeclaration {
        file_name: "ComicShanns-Regular-fcb0fc02dcbee4c9846b3e2508668039.woff2",
        unicode_ranges: unicode_ranges!("U+100-10f,U+112-125,U+128-130,U+134-137,U+139-13c,U+141-148,U+14c-151,U+154-161,U+164-165,U+168-17f,U+1bf,U+1f7,U+218-21b,U+237,U+1e80-1e85,U+1ef2-1ef3,U+a75b"),
    },
    FontFaceDeclaration {
        file_name: "ComicShanns-Regular-dc6a8806fa96795d7b3be5026f989a17.woff2",
        unicode_ranges: unicode_ranges!("U+2c7,U+2d8-2d9,U+2db,U+2dd,U+315,U+2190,U+2192,U+2200,U+2203-2204,U+2264-2265,U+f6c3"),
    },
    FontFaceDeclaration {
        file_name: "ComicShanns-Regular-6e066e8de2ac57ea9283adb9c24d7f0c.woff2",
        unicode_ranges: unicode_ranges!("U+3bb"),
    },
];

/// Excalifont faces in pinned face order.
const EXCALIFONT_FACES: &[FontFaceDeclaration] = &[
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
        unicode_ranges: unicode_ranges!("U+20-7e,U+a0-a3,U+a5-a6,U+a8-ab,U+ad-b1,U+b4,U+b6-b8,U+ba-ff,U+131,U+152-153,U+2bc,U+2c6,U+2da,U+2dc,U+304,U+308,U+2013-2014,U+2018-201a,U+201c-201e,U+2020,U+2022,U+2024-2026,U+2030,U+2039-203a,U+20ac,U+2122,U+2212"),
    },
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-be310b9bcd4f1a43f571c46df7809174.woff2",
        unicode_ranges: unicode_ranges!("U+100-130,U+132-137,U+139-149,U+14c-151,U+154-17e,U+192,U+1fc-1ff,U+218-21b,U+237,U+1e80-1e85,U+1ef2-1ef3,U+2113"),
    },
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-b9dcf9d2e50a1eaf42fc664b50a3fd0d.woff2",
        unicode_ranges: unicode_ranges!("U+400-45f,U+490-491,U+2116"),
    },
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-41b173a47b57366892116a575a43e2b6.woff2",
        unicode_ranges: unicode_ranges!("U+37e,U+384-38a,U+38c,U+38e-393,U+395-3a1,U+3a3-3a8,U+3aa-3cf,U+3d7"),
    },
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-3f2c5db56cc93c5a6873b1361d730c16.woff2",
        unicode_ranges: unicode_ranges!("U+2c7,U+2d8-2d9,U+2db,U+2dd,U+302,U+306-307,U+30a-30c,U+326-328,U+212e,U+2211,U+fb01-fb02"),
    },
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-349fac6ca4700ffec595a7150a0d1e1d.woff2",
        unicode_ranges: unicode_ranges!("U+462-463,U+472-475,U+4d8-4d9,U+4e2-4e3,U+4e6-4e9,U+4ee-4ef"),
    },
    FontFaceDeclaration {
        file_name: "Excalifont-Regular-623ccf21b21ef6b3a0d87738f77eb071.woff2",
        unicode_ranges: unicode_ranges!("U+300-301,U+303"),
    },
];

/// Lilita One faces in pinned face order.
const LILITA_FACES: &[FontFaceDeclaration] = &[
    FontFaceDeclaration {
        file_name: "Lilita-Regular-i7dPIFZ9Zz-WBtRtedDbYE98RXi4EwSsbg.woff2",
        unicode_ranges: LATIN_EXT,
    },
    FontFaceDeclaration {
        file_name: "Lilita-Regular-i7dPIFZ9Zz-WBtRtedDbYEF8RXi4EwQ.woff2",
        unicode_ranges: LATIN,
    },
];

/// Nunito faces in pinned face order.
const NUNITO_FACES: &[FontFaceDeclaration] = &[
    FontFaceDeclaration {
        file_name: "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTk3j6zbXWjgevT5.woff2",
        unicode_ranges: CYRILLIC_EXT,
    },
    FontFaceDeclaration {
        file_name: "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTA3j6zbXWjgevT5.woff2",
        unicode_ranges: CYRILLIC,
    },
    FontFaceDeclaration {
        file_name: "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTs3j6zbXWjgevT5.woff2",
        unicode_ranges: VIETNAMESE,
    },
    FontFaceDeclaration {
        file_name: "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTo3j6zbXWjgevT5.woff2",
        unicode_ranges: LATIN_EXT,
    },
    FontFaceDeclaration {
        file_name: "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTQ3j6zbXWjgeg.woff2",
        unicode_ranges: LATIN,
    },
];

/// Whether the family ships one face that covers all of Unicode.
fn is_single_face_family(family: &str) -> bool {
    matches!(family, "Cascadia" | "Liberation" | "Virgil")
}

/// Returns the official font chunks of one multi-face family in pinned face
/// order.
fn family_faces(family: &str) -> Option<&'static [FontFaceDeclaration]> {
    Some(match family {
        "ComicShanns" => COMIC_SHANNS_FACES,
        "Excalifont" => EXCALIFONT_FACES,
        "Lilita" => LILITA_FACES,
        "Nunito" => NUNITO_FACES,
        "Xiaolai" => XIAOLAI_FACES.as_slice(),
        _ => return None,
    })
}

/// Returns the pinned face order and declaration of one official font chunk of
/// a multi-face family.
fn font_face(family: &str, upstream_path: &str) -> Option<(usize, &'static FontFaceDeclaration)> {
    let file_name = upstream_path.rsplit('/').next()?;
    family_faces(family)?
        .iter()
        .enumerate()
        .find(|(_, face)| face.file_name == file_name)
}

/// Returns the pinned face order of one official font chunk.
pub fn font_face_order(family: &str, upstream_path: &str) -> Option<usize> {
    if is_single_face_family(family) {
        return Some(0);
    }
    Some(font_face(family, upstream_path)?.0)
}

/// Returns the declared Unicode range of one official font chunk, as its pinned
/// upstream CSS declares it.
pub fn font_face_unicode_ranges(
    family: &str,
    upstream_path: &str,
) -> Option<&'static [(u32, u32)]> {
    if is_single_face_family(family) {
        return Some(ALL_UNICODE);
    }
    Some(font_face(family, upstream_path)?.1.unicode_ranges)
}

/// Returns the reviewed sfnt length without accessing payload bytes.
/// Catalog verification holds this equal to the source WOFF2 header's length.
pub fn font_face_decoded_length(upstream_path: &str) -> Option<usize> {
    Some(find_asset(upstream_path)?.decoded_length)
}

/// Returns the checked-in transformation record for the predecoded sfnt bytes.
pub const fn excalifont_basic_latin_sfnt_transformation() -> PredecodedFontTransformation {
    PredecodedFontTransformation {
        source_path:
            "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
        source_sha256: "e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48",
        output_sha256: "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb",
        tool: "wuff 0.2.9",
        invocation: "wuff::decompress_woff2(source_bytes)",
        reviewer: "OpenAI gpt-5.6-sol",
        review_date: "2026-07-22",
        exact_output_bytes_reviewed: true,
        comparison: "byte-identical container decode; all 14 tables, names, metrics, glyphs, outlines, mappings, and legal metadata reviewed",
        metadata_json: include_bytes!(
            "../catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt.json"
        ),
    }
}

/// Returns the package-wide canonical license and notice bundle.
pub const fn license_bundle() -> &'static str {
    include_str!("../LICENSE-BUNDLE.txt")
}

fn verify_asset(asset: &'static AssetRecord) -> Result<VerifiedAsset, AssetIntegrityError> {
    let bytes = asset.bytes.ok_or(AssetIntegrityError {
        asset_identity: asset.identity,
        stage: IntegrityStage::Unavailable,
    })?;
    if bytes.len() != asset.complete_length || sha256(bytes) != asset.complete_sha256 {
        return Err(AssetIntegrityError {
            asset_identity: asset.identity,
            stage: IntegrityStage::Complete,
        });
    }
    Ok(VerifiedAsset {
        bytes: std::borrow::Cow::Borrowed(bytes),
    })
}

fn sha256(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod source_file_tests {
    use super::*;

    #[test]
    fn complete_file_verification_rejects_corruption_and_ignores_historical_offsets() {
        let mut record = ASSETS[0];
        record.segments = &[];
        let verified = verify_asset(Box::leak(Box::new(record))).unwrap();
        assert!(std::ptr::eq(verified.bytes(), record.bytes.unwrap()));

        record.bytes = Some(b"not a WOFF2 file");
        let error = verify_asset(Box::leak(Box::new(record))).unwrap_err();
        assert_eq!(error.asset_identity(), record.identity);
        assert_eq!(error.stage, IntegrityStage::Complete);
    }
}
