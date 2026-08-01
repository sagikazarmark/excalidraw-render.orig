use super::font_subset;
use sha2::{Digest, Sha256};
use std::collections::{hash_map::DefaultHasher, BTreeMap, HashSet, TryReserveError};
use std::hash::BuildHasherDefault;
use std::sync::Arc;

pub(super) struct Demand {
    pub(super) families: Vec<FamilyDemand>,
}

pub(super) struct FamilyDemand {
    pub(super) family: &'static str,
    pub(super) scalars: Vec<u32>,
    scalar_membership: HashSet<u32, BuildHasherDefault<DefaultHasher>>,
}

impl FamilyDemand {
    pub(super) fn new(family: &'static str) -> Self {
        Self {
            family,
            scalars: Vec::new(),
            scalar_membership: HashSet::default(),
        }
    }

    pub(super) fn from_scalars(
        family: &'static str,
        scalars: &[u32],
    ) -> Result<Self, TryReserveError> {
        let mut demand = Self::new(family);
        demand.scalars.try_reserve_exact(scalars.len())?;
        demand.scalar_membership.try_reserve(scalars.len())?;
        for scalar in scalars {
            if demand.scalar_membership.insert(*scalar) {
                demand.scalars.push(*scalar);
            }
        }
        Ok(demand)
    }

    pub(super) fn add_text(&mut self, text: &str) -> Result<(), TryReserveError> {
        let scalar_count = text.chars().count();
        self.scalars.try_reserve(scalar_count)?;
        self.scalar_membership.try_reserve(scalar_count)?;
        for scalar in text.chars().map(u32::from) {
            if self.scalar_membership.insert(scalar) {
                self.scalars.push(scalar);
            }
        }
        Ok(())
    }
}

pub(super) struct UnchangedFont<'a> {
    pub(super) family: &'static str,
    pub(super) mime: &'static str,
    pub(super) bytes: &'a [u8],
}

pub(super) struct Bundle {
    resources: Vec<Resource>,
    mandatory_notices: Option<super::notices::MandatoryNotices>,
}

struct Resource {
    family: &'static str,
    mime: &'static str,
    bytes: Arc<[u8]>,
    metrics: Option<SubsetMetrics>,
}

pub(super) struct SubsetMetrics {
    pub(super) asset_identity: &'static str,
    pub(super) decoded_sfnt_bytes: usize,
    pub(super) subset_sfnt_bytes: usize,
    pub(super) subset_woff2_bytes: usize,
    pub(super) subset_tables: usize,
}

pub(super) struct BuildError {
    pub(super) stage: crate::SvgFontSubsetStage,
    pub(super) family: &'static str,
    pub(super) asset_identity: &'static str,
}

impl Bundle {
    pub(super) fn take_mandatory_notices(&mut self) -> Option<super::notices::MandatoryNotices> {
        self.mandatory_notices.take()
    }

    pub(super) fn subset_metrics(&self) -> impl Iterator<Item = &SubsetMetrics> {
        self.resources
            .iter()
            .filter_map(|resource| resource.metrics.as_ref())
    }

    pub(super) fn write<W: super::svg::SvgSink + ?Sized>(
        &self,
        output: &mut W,
        mut write_base64: impl FnMut(&mut W, &[u8]) -> Result<(), super::FormatEncodeError>,
    ) -> Result<(), super::FormatEncodeError> {
        if self.resources.is_empty() {
            output.push_str("<defs></defs>");
            return Ok(());
        }
        output.push_str("<defs><style class=\"style-fonts\">");
        for resource in &self.resources {
            output.push_str("@font-face{font-family:'");
            output.push_str(resource.family);
            output.push_str("';src:url(data:");
            output.push_str(resource.mime);
            output.push_str(";base64,");
            write_base64(output, resource.bytes.as_ref())?;
            output.push_str(")}");
        }
        output.push_str("</style></defs>");
        Ok(())
    }

    pub(super) fn effective_family(family: &'static str) -> &'static str {
        match family {
            "Virgil" => "ExcalidrawRender-1",
            "Cascadia Code" => "ExcalidrawRender-3",
            "Lilita One" => "ExcalidrawRender-7",
            "Liberation Sans" => "ExcalidrawRender-9",
            _ => family,
        }
    }
}

pub(super) fn build(
    demand: Demand,
    unchanged_fonts: &[UnchangedFont<'_>],
    payload: super::SvgFontPayload,
    subset_session: &mut font_subset::Session,
) -> Result<Bundle, BuildError> {
    let mut resources = Vec::new();
    let mut licenses = BTreeMap::<String, &'static str>::new();
    let mut resource_notices = Vec::<(String, String)>::new();
    for family in demand.families {
        if family.scalars.is_empty() || family.family == "Noto Emoji" {
            continue;
        }
        let catalog_family = catalog_family(family.family);
        let mut records = excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .filter(|record| record.family == catalog_family)
            .collect::<Vec<_>>();
        records.sort_unstable_by_key(|record| {
            excalidraw_render_assets_base::font_face_order(catalog_family, record.upstream_path)
                .unwrap_or(usize::MAX)
        });
        for record in records {
            let ranges = excalidraw_render_assets_base::font_face_unicode_ranges(
                catalog_family,
                record.upstream_path,
            )
            .ok_or(BuildError {
                stage: crate::SvgFontSubsetStage::Plan,
                family: family.family,
                asset_identity: record.identity,
            })?;
            let selected = family.scalars.iter().any(|scalar| {
                ranges
                    .iter()
                    .any(|(start, end)| start <= scalar && scalar <= end)
            });
            if !selected {
                continue;
            }
            let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
                .ok()
                .flatten()
                .ok_or(BuildError {
                    stage: crate::SvgFontSubsetStage::Decode,
                    family: family.family,
                    asset_identity: record.identity,
                })?;
            let effective_alias = Bundle::effective_family(family.family);
            match payload {
                super::SvgFontPayload::Subset => {
                    let semantic_identity =
                        semantic_identity(record, &family.scalars).ok_or(BuildError {
                            stage: crate::SvgFontSubsetStage::Plan,
                            family: family.family,
                            asset_identity: record.identity,
                        })?;
                    let derived = subset_session
                        .derive(
                            record.identity,
                            asset.bytes(),
                            &family.scalars,
                            effective_alias,
                            &semantic_identity,
                        )
                        .map_err(|error| BuildError {
                            stage: error.stage,
                            family: family.family,
                            asset_identity: record.identity,
                        })?;
                    let (provenance, license) = notice_inputs(record, family.family)?;
                    licenses
                        .entry(provenance.license_identity.clone())
                        .or_insert(license);
                    let output_digest = format!("{:x}", Sha256::digest(derived.woff2.as_ref()));
                    let profile_digest = encode_hex_32(
                        &excalidraw_render_assets_base::font_subset_profile_digest_v1(),
                    );
                    resource_notices.push((
                        format!("svg-font-subset@sha256:{semantic_identity}"),
                        format!(
                            "Derived font subset of {}. {} Modified Version under {}. Transformation: Unicode and required layout/glyph closure subset; names rewritten for compatibility revision 1. Effective alias: {}. Subset profile SHA-256: {}. Semantic subset identity: {}. Output WOFF2: {} bytes, SHA-256 {}.",
                            record.identity,
                            provenance.text,
                            provenance.license_identity,
                            effective_alias,
                            profile_digest,
                            semantic_identity,
                            derived.woff2.len(),
                            output_digest,
                        ),
                    ));
                    resources.push(Resource {
                        family: effective_alias,
                        mime: "font/woff2",
                        metrics: Some(SubsetMetrics {
                            asset_identity: record.identity,
                            decoded_sfnt_bytes: derived.decoded_sfnt_bytes,
                            subset_sfnt_bytes: derived.subset_sfnt_bytes,
                            subset_woff2_bytes: derived.woff2.len(),
                            subset_tables: derived.subset_tables,
                        }),
                        bytes: derived.woff2,
                    });
                }
                #[cfg(test)]
                super::SvgFontPayload::CompleteChunks => {
                    let (provenance, license) = notice_inputs(record, family.family)?;
                    let notice = if provenance.license_identity.ends_with(".ofl-1.1") {
                        format!("{}\n\n{}", provenance.text, license)
                    } else {
                        license.to_owned()
                    };
                    resource_notices.push((record.identity.to_owned(), notice));
                    let mut bytes = Vec::new();
                    bytes
                        .try_reserve_exact(asset.bytes().len())
                        .map_err(|_| BuildError {
                            stage: crate::SvgFontSubsetStage::Encode,
                            family: family.family,
                            asset_identity: record.identity,
                        })?;
                    bytes.extend_from_slice(asset.bytes());
                    resources.push(Resource {
                        family: effective_alias,
                        mime: "font/woff2",
                        bytes: bytes.into(),
                        metrics: None,
                    });
                }
            }
        }
    }
    for unchanged in unchanged_fonts {
        if resources.iter().any(|resource| {
            resource.family == Bundle::effective_family(unchanged.family)
                && resource.bytes.as_ref() == unchanged.bytes
        }) {
            continue;
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(unchanged.bytes.len())
            .map_err(|_| BuildError {
                stage: crate::SvgFontSubsetStage::Encode,
                family: unchanged.family,
                asset_identity: "unchanged-fallback",
            })?;
        bytes.extend_from_slice(unchanged.bytes);
        resources.push(Resource {
            family: Bundle::effective_family(unchanged.family),
            mime: unchanged.mime,
            bytes: bytes.into(),
            metrics: None,
        });
        if unchanged.family == "Noto Emoji" {
            if let Some(notice) = super::text::noto_emoji_notice() {
                licenses
                    .entry("font.noto-emoji.ofl-1.1".to_owned())
                    .or_insert(notice);
            }
        }
    }
    resource_notices.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mandatory_notices = (!licenses.is_empty() || !resource_notices.is_empty()).then(|| {
        let mut json = String::from("[");
        let mut separator = "";
        for (identity, text) in licenses {
            push_notice(&mut json, &mut separator, &identity, text);
        }
        for (identity, text) in resource_notices {
            push_notice(&mut json, &mut separator, &identity, &text);
        }
        json.push(']');
        super::notices::MandatoryNotices::from_json(json)
    });
    Ok(Bundle {
        resources,
        mandatory_notices,
    })
}

fn notice_inputs(
    record: &excalidraw_render_assets_base::AssetRecord,
    family: &'static str,
) -> Result<(crate::catalog::ManifestedFontAttribution, &'static str), BuildError> {
    let provenance =
        crate::catalog::manifested_font_attribution(record.identity).map_err(|_| BuildError {
            stage: crate::SvgFontSubsetStage::Validate,
            family,
            asset_identity: record.identity,
        })?;
    let license = if provenance.license_identity == "font.comic-shanns.mit" {
        super::text::comic_shanns_notice()
    } else if provenance.license_identity.ends_with(".ofl-1.1") {
        super::text::liberation_sans_notice().and_then(|notice| {
            notice
                .find("This Font Software is licensed")
                .and_then(|start| notice.get(start..))
        })
    } else {
        None
    }
    .ok_or(BuildError {
        stage: crate::SvgFontSubsetStage::Validate,
        family,
        asset_identity: record.identity,
    })?;
    Ok((provenance, license))
}

fn push_notice(json: &mut String, separator: &mut &'static str, identity: &str, text: &str) {
    json.push_str(separator);
    json.push_str("{\"asset_identity\":\"");
    super::text::push_json_string_content(json, identity);
    json.push_str("\",\"text\":\"");
    super::text::push_json_string_content(json, text);
    json.push_str("\"}");
    *separator = ",";
}

pub(super) fn semantic_identity(
    record: &excalidraw_render_assets_base::AssetRecord,
    scalars: &[u32],
) -> Option<String> {
    let profile_digest = excalidraw_render_assets_base::font_subset_profile_digest_v1();
    let source_digest = decode_hex_32(record.complete_sha256)?;
    let mut hasher = Sha256::new();
    hasher.update(b"excalidraw-render:svg-font-subset:v1");
    hasher.update([0]);
    hasher.update(1_u64.to_le_bytes());
    hasher.update(profile_digest);
    hasher.update(u64::try_from(record.identity.len()).ok()?.to_le_bytes());
    hasher.update(record.identity.as_bytes());
    hasher.update(source_digest);
    hasher.update(u64::try_from(scalars.len()).ok()?.to_le_bytes());
    for scalar in scalars {
        hasher.update(scalar.to_le_bytes());
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index.checked_mul(2)?;
        *byte = u8::from_str_radix(value.get(offset..offset.checked_add(2)?)?, 16).ok()?;
    }
    Some(output)
}

fn encode_hex_32(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(super) fn catalog_family(family: &str) -> &str {
    match family {
        "Cascadia Code" => "Cascadia",
        "Comic Shanns" => "ComicShanns",
        "Liberation Sans" => "Liberation",
        "Lilita One" => "Lilita",
        family => family,
    }
}
