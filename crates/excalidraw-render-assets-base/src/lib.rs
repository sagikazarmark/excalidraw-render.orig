//! Sealed implementation-detail catalogs for `excalidraw-render`.

#![doc(hidden)]

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

/// Maximum raw byte length targeted by one generated data shard.
pub const RAW_SHARD_LIMIT: usize = 8 * 1024 * 1024;

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
