//! Sealed implementation-detail catalogs for `excalidraw-render`.

#![doc(hidden)]

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

/// Maximum raw byte length targeted by one generated data shard.
pub const RAW_SHARD_LIMIT: usize = 8 * 1024 * 1024;

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
pub const FONT_SUBSET_PROFILE_SETTINGS_V1: &[u8] = b"excalidraw-render:svg-font-subset-profile:v1\nclosure=layout,composite,bidi\nlayout=all\nhinting=retain\nglyph-renumbering=true\nvariations=retain\nnames=0-6,13-14:en-US\nflags=none\noverlap-simple=clear\nwoff2=wuff-0.2.8,skera-0.5.1,excalidraw-render-ttf2woff2-0.1.0(upstream-0.13.2),excalidraw-render-brotli-0.1.0(upstream-8.0.4),q11,glyf-loca,threads-1,portable-math\n";

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

/// One ordered byte segment of a built-in asset.
#[derive(Clone, Copy, Debug)]
pub struct AssetSegment {
    pub identity: &'static str,
    pub shard: &'static str,
    pub bytes: &'static [u8],
    pub offset: usize,
    pub length: usize,
    pub sha256: &'static str,
    pub order: usize,
}

/// One byte-bearing entry in the sealed built-in asset catalog.
#[derive(Clone, Copy, Debug)]
pub struct AssetRecord {
    pub identity: &'static str,
    pub upstream_path: &'static str,
    pub upstream_revision: &'static str,
    pub upstream_url: &'static str,
    pub complete_sha256: &'static str,
    pub complete_length: usize,
    pub family: &'static str,
    pub license_identity: &'static str,
    pub provenance_identity: &'static str,
    pub transformation_identity: &'static str,
    pub reserved_font_names: &'static [&'static str],
    pub role: &'static str,
    pub output_formats: &'static [&'static str],
    pub segments: &'static [AssetSegment],
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
    pub assets: &'static [AssetRecord],
}

/// Bytes returned only after segment and complete-file digest verification.
#[derive(Debug)]
pub struct VerifiedAsset {
    bytes: Vec<u8>,
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
}

/// Indicates that checked-in asset bytes no longer match their sealed identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetIntegrityError {
    asset_identity: &'static str,
    segment_identity: Option<&'static str>,
}

impl AssetIntegrityError {
    /// Returns the affected complete asset identity.
    pub const fn asset_identity(&self) -> &'static str {
        self.asset_identity
    }

    /// Returns the affected segment identity, or `None` for a complete digest mismatch.
    pub const fn segment_identity(&self) -> Option<&'static str> {
        self.segment_identity
    }
}

impl fmt::Display for AssetIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.segment_identity {
            Some(segment) => write!(
                formatter,
                "asset {} has invalid segment {segment}",
                self.asset_identity
            ),
            None => write!(
                formatter,
                "asset {} has an invalid complete digest",
                self.asset_identity
            ),
        }
    }
}

impl Error for AssetIntegrityError {}

include!("generated/revision_1.rs");

/// Returns the sealed compatibility revision 1 catalog.
pub const fn revision_1() -> &'static RevisionCatalog {
    &REVISION_1
}

/// Looks up and verifies one complete built-in asset by its upstream package path.
pub fn load_asset(upstream_path: &str) -> Result<Option<VerifiedAsset>, AssetIntegrityError> {
    let Ok(index) = ASSETS.binary_search_by_key(&upstream_path, |asset| asset.upstream_path) else {
        return Ok(None);
    };
    verify_asset(&ASSETS[index]).map(Some)
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
            segment_identity: None,
        });
    }
    Ok(VerifiedAsset {
        bytes: EXCALIFONT_BASIC_LATIN_SFNT.to_vec(),
    })
}

const NOTO_EMOJI_SFNT_IDENTITY: &str = concat!(
    "scripts/woff2/assets/NotoEmoji-Regular.ttf@sha256:",
    "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"
);
const NOTO_EMOJI_SFNT: &[u8] = include_bytes!("../catalog/fallback/NotoEmoji-Regular.ttf");

/// Loads the digest-verified Noto Emoji 3.003 fallback input.
pub fn load_noto_emoji_sfnt() -> Result<VerifiedAsset, AssetIntegrityError> {
    if NOTO_EMOJI_SFNT.len() != 862_680
        || sha256(NOTO_EMOJI_SFNT)
            != "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"
    {
        return Err(AssetIntegrityError {
            asset_identity: NOTO_EMOJI_SFNT_IDENTITY,
            segment_identity: None,
        });
    }
    Ok(VerifiedAsset {
        bytes: NOTO_EMOJI_SFNT.to_vec(),
    })
}

/// Returns the pinned upstream declaration order for one official font face.
pub fn font_face_order(family: &str, upstream_path: &str) -> Option<usize> {
    let file_name = upstream_path.rsplit('/').next()?;
    let ordered: &[&str] = match family {
        "ComicShanns" => &[
            "ComicShanns-Regular-279a7b317d12eb88de06167bd672b4b4.woff2",
            "ComicShanns-Regular-fcb0fc02dcbee4c9846b3e2508668039.woff2",
            "ComicShanns-Regular-dc6a8806fa96795d7b3be5026f989a17.woff2",
            "ComicShanns-Regular-6e066e8de2ac57ea9283adb9c24d7f0c.woff2",
        ],
        "Excalifont" => &[
            "Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
            "Excalifont-Regular-be310b9bcd4f1a43f571c46df7809174.woff2",
            "Excalifont-Regular-b9dcf9d2e50a1eaf42fc664b50a3fd0d.woff2",
            "Excalifont-Regular-41b173a47b57366892116a575a43e2b6.woff2",
            "Excalifont-Regular-3f2c5db56cc93c5a6873b1361d730c16.woff2",
            "Excalifont-Regular-349fac6ca4700ffec595a7150a0d1e1d.woff2",
            "Excalifont-Regular-623ccf21b21ef6b3a0d87738f77eb071.woff2",
        ],
        "Lilita" => &[
            "Lilita-Regular-i7dPIFZ9Zz-WBtRtedDbYE98RXi4EwSsbg.woff2",
            "Lilita-Regular-i7dPIFZ9Zz-WBtRtedDbYEF8RXi4EwQ.woff2",
        ],
        "Nunito" => &[
            "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTk3j6zbXWjgevT5.woff2",
            "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTA3j6zbXWjgevT5.woff2",
            "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTs3j6zbXWjgevT5.woff2",
            "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTo3j6zbXWjgevT5.woff2",
            "Nunito-Regular-XRXI3I6Li01BKofiOc5wtlZ2di8HDIkhdTQ3j6zbXWjgeg.woff2",
        ],
        "Xiaolai" => return xiaolai_face_order(file_name),
        "Cascadia" | "Liberation" | "Virgil" => return Some(0),
        _ => return None,
    };
    ordered.iter().position(|candidate| *candidate == file_name)
}

/// Returns the pinned upstream CSS Unicode ranges for one official font face.
pub fn font_face_unicode_ranges(family: &str, upstream_path: &str) -> Option<Vec<(u32, u32)>> {
    const LATIN: &str = "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD";
    const LATIN_EXT: &str = "U+0100-02AF,U+0304,U+0308,U+0329,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF";
    const CYRILIC_EXT: &str = "U+0460-052F,U+1C80-1C88,U+20B4,U+2DE0-2DFF,U+A640-A69F,U+FE2E-FE2F";
    const CYRILIC: &str = "U+0301,U+0400-045F,U+0490-0491,U+04B0-04B1,U+2116";
    const VIETNAMESE: &str = "U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB";
    const EXCALIFONT: [&str; 7] = [
        "U+20-7e,U+a0-a3,U+a5-a6,U+a8-ab,U+ad-b1,U+b4,U+b6-b8,U+ba-ff,U+131,U+152-153,U+2bc,U+2c6,U+2da,U+2dc,U+304,U+308,U+2013-2014,U+2018-201a,U+201c-201e,U+2020,U+2022,U+2024-2026,U+2030,U+2039-203a,U+20ac,U+2122,U+2212",
        "U+100-130,U+132-137,U+139-149,U+14c-151,U+154-17e,U+192,U+1fc-1ff,U+218-21b,U+237,U+1e80-1e85,U+1ef2-1ef3,U+2113",
        "U+400-45f,U+490-491,U+2116",
        "U+37e,U+384-38a,U+38c,U+38e-393,U+395-3a1,U+3a3-3a8,U+3aa-3cf,U+3d7",
        "U+2c7,U+2d8-2d9,U+2db,U+2dd,U+302,U+306-307,U+30a-30c,U+326-328,U+212e,U+2211,U+fb01-fb02",
        "U+462-463,U+472-475,U+4d8-4d9,U+4e2-4e3,U+4e6-4e9,U+4ee-4ef",
        "U+300-301,U+303",
    ];
    const COMIC_SHANNS: [&str; 4] = [
        "U+20-7e,U+a1-a6,U+a8,U+ab-ac,U+af-b1,U+b4,U+b8,U+bb-bc,U+bf-cf,U+d1-d7,U+d9-de,U+e0-ef,U+f1-f7,U+f9-ff,U+131,U+152-153,U+2c6,U+2da,U+2dc,U+2013-2014,U+2018-201a,U+201c-201d,U+2020-2022,U+2026,U+2039-203a,U+2044,U+20ac,U+2191,U+2193,U+2212",
        "U+100-10f,U+112-125,U+128-130,U+134-137,U+139-13c,U+141-148,U+14c-151,U+154-161,U+164-165,U+168-17f,U+1bf,U+1f7,U+218-21b,U+237,U+1e80-1e85,U+1ef2-1ef3,U+a75b",
        "U+2c7,U+2d8-2d9,U+2db,U+2dd,U+315,U+2190,U+2192,U+2200,U+2203-2204,U+2264-2265,U+f6c3",
        "U+3bb",
    ];
    let order = font_face_order(family, upstream_path)?;
    let declared = match family {
        "Cascadia" | "Liberation" | "Virgil" => return Some(vec![(0, 0x10ffff)]),
        "ComicShanns" => *COMIC_SHANNS.get(order)?,
        "Excalifont" => *EXCALIFONT.get(order)?,
        "Lilita" => *[LATIN_EXT, LATIN].get(order)?,
        "Nunito" => *[CYRILIC_EXT, CYRILIC, VIETNAMESE, LATIN_EXT, LATIN].get(order)?,
        "Xiaolai" => return xiaolai_unicode_ranges(order),
        _ => return None,
    };
    parse_unicode_ranges(declared)
}

/// Returns the source-declared uncompressed sfnt length from a sealed WOFF2 header.
pub fn font_face_decoded_length(upstream_path: &str) -> Option<usize> {
    let asset = ASSETS
        .binary_search_by_key(&upstream_path, |asset| asset.upstream_path)
        .ok()
        .and_then(|index| ASSETS.get(index))?;
    let first = asset.segments.first()?;
    let start = first.offset;
    let header = first.bytes.get(start..start.checked_add(20)?)?;
    if header.get(0..4)? != b"wOF2" {
        return None;
    }
    usize::try_from(u32::from_be_bytes(header.get(16..20)?.try_into().ok()?)).ok()
}

fn xiaolai_unicode_ranges(order: usize) -> Option<Vec<(u32, u32)>> {
    const SOURCE: &str = include_str!("../catalog/order/Xiaolai-index.ts");
    let declarations = SOURCE.split_once("export const XiaolaiFontFaces")?.1;
    let tail = declarations.split("\"U+").nth(order.checked_add(1)?)?;
    let range = tail.split_once('"')?.0;
    let mut declared = String::from("U+");
    declared.push_str(range);
    parse_unicode_ranges(&declared)
}

fn parse_unicode_ranges(declared: &str) -> Option<Vec<(u32, u32)>> {
    let mut ranges = Vec::new();
    for part in declared.split(',') {
        let value = part
            .trim()
            .strip_prefix("U+")
            .or_else(|| part.trim().strip_prefix("u+"))?;
        let (start, end) = value.split_once('-').unwrap_or((value, value));
        let start = u32::from_str_radix(start, 16).ok()?;
        let end = u32::from_str_radix(end, 16).ok()?;
        if start > end || end > 0x10ffff {
            return None;
        }
        ranges.push((start, end));
    }
    Some(ranges)
}

fn xiaolai_face_order(file_name: &str) -> Option<usize> {
    const SOURCE: &str = include_str!("../catalog/order/Xiaolai-index.ts");
    SOURCE
        .lines()
        .filter_map(|line| {
            line.strip_prefix("import _")?
                .split_once(" from \"./")
                .map(|(_, tail)| tail.trim_end_matches("\";"))
        })
        .position(|candidate| candidate == file_name)
}

/// Returns the checked-in transformation record for the predecoded sfnt bytes.
pub const fn excalifont_basic_latin_sfnt_transformation() -> PredecodedFontTransformation {
    PredecodedFontTransformation {
        source_path:
            "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
        source_sha256: "e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48",
        output_sha256: "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb",
        tool: "wuff 0.2.8",
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
    let mut complete = Vec::with_capacity(asset.complete_length);
    for (expected_order, segment) in asset.segments.iter().enumerate() {
        let end = segment
            .offset
            .checked_add(segment.length)
            .ok_or(AssetIntegrityError {
                asset_identity: asset.identity,
                segment_identity: Some(segment.identity),
            })?;
        let bytes = segment
            .bytes
            .get(segment.offset..end)
            .ok_or(AssetIntegrityError {
                asset_identity: asset.identity,
                segment_identity: Some(segment.identity),
            })?;
        if segment.order != expected_order || sha256(bytes) != segment.sha256 {
            return Err(AssetIntegrityError {
                asset_identity: asset.identity,
                segment_identity: Some(segment.identity),
            });
        }
        complete.extend_from_slice(bytes);
    }
    if complete.len() != asset.complete_length || sha256(&complete) != asset.complete_sha256 {
        return Err(AssetIntegrityError {
            asset_identity: asset.identity,
            segment_identity: None,
        });
    }
    Ok(VerifiedAsset { bytes: complete })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
