use super::font_subset;
use crate::resource_accounting::{
    ResourceAccount, ResourceContext, ResourceDelta, ResourcePhase, ResourceRecordKind,
};
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

#[derive(Clone, Copy)]
pub(super) enum Payload {
    Subset,
    #[cfg(test)]
    CompleteChunks,
}

pub(super) struct Session {
    subset: font_subset::Session,
}

impl Session {
    pub(super) fn new() -> Self {
        Self {
            subset: font_subset::Session::new(),
        }
    }

    #[cfg(test)]
    pub(super) const fn stats(&self) -> (usize, usize) {
        self.subset.stats()
    }
}

pub(super) struct Bundle {
    resources: Vec<Resource>,
    mandatory_notices: Option<super::notices::MandatoryNotices>,
}

struct Resource {
    family: &'static str,
    mime: &'static str,
    bytes: Arc<[u8]>,
    metrics: Option<font_subset::DerivationMetrics>,
}

pub(super) struct BuildError {
    pub(super) stage: crate::SvgFontSubsetStage,
    pub(super) family: &'static str,
    pub(super) asset_identity: &'static str,
}

pub(super) struct SubsetNotice {
    pub(super) license_identity: String,
    pub(super) license: &'static str,
    pub(super) resource_identity: String,
    pub(super) resource_text: String,
}

impl Bundle {
    pub(super) fn take_mandatory_notices(&mut self) -> Option<super::notices::MandatoryNotices> {
        self.mandatory_notices.take()
    }

    pub(super) fn account_resource_transcript<A: ResourceAccount>(
        &self,
        account: &mut A,
    ) -> Result<(), A::Error> {
        for metrics in self
            .resources
            .iter()
            .filter_map(|resource| resource.metrics.as_ref())
        {
            font_subset::account_derivation(account, metrics)?;
            account.transact_resource(
                ResourceContext::new(
                    ResourcePhase::EncodingAndOutcomeAssembly,
                    "svg_font_css_data_url",
                    "",
                )
                .with_semantic_identity(metrics.asset_identity()),
                ResourceDelta::default(),
            )?;
        }
        Ok(())
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

struct WorkingRecord {
    context: ResourceContext<'static>,
    bytes: u64,
}

pub(super) struct SelectionAccounting {
    retained: Vec<WorkingRecord>,
}

impl SelectionAccounting {
    pub(super) fn release<A: ResourceAccount>(mut self, account: &mut A) -> Result<(), A::Error> {
        while let Some(record) = self.retained.pop() {
            account.release_resource(record.context, record.bytes)?;
        }
        Ok(())
    }
}

pub(super) struct DependencyEnvelope(WorkingRecord);

impl DependencyEnvelope {
    pub(super) fn release<A: ResourceAccount>(self, account: &mut A) -> Result<(), A::Error> {
        account.release_resource(self.0.context, self.0.bytes)
    }
}

pub(super) fn account_selection<A: ResourceAccount>(
    account: &mut A,
    demand: &Demand,
) -> Result<SelectionAccounting, A::Error> {
    let mut retained = Vec::new();
    let result = (|| -> Result<(), A::Error> {
        for family in &demand.families {
            let scalar_count = u64::try_from(family.scalars.len())
                .map_err(|_| account.internal_failure("SVG font scalar count overflowed"))?;
            let scalar_work = checked_product(
                account,
                scalar_count,
                36,
                "SVG font scalar-set work overflowed",
            )?;
            let scalar_working = checked_product(
                account,
                scalar_count,
                64,
                "SVG font scalar-set working bytes overflowed",
            )?;
            retained
                .try_reserve(1)
                .map_err(|_| account.memory_failure("svg_font_selection", "retained_record"))?;
            let context = ResourceContext::new(
                ResourcePhase::FormatPlanning,
                "svg_font_family_scalar_set",
                "",
            )
            .with_semantic_identity(family.family)
            .with_record(ResourceRecordKind::BuiltInGlyphOrRangeState);
            account.transact_resource(
                context,
                ResourceDelta {
                    working_bytes: scalar_working,
                    total_work_units: scalar_work,
                    ..ResourceDelta::default()
                },
            )?;
            retained.push(WorkingRecord {
                context,
                bytes: scalar_working,
            });
        }
        for family in &demand.families {
            let scalar_count = u64::try_from(family.scalars.len())
                .map_err(|_| account.internal_failure("SVG font scalar count overflowed"))?;
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
                .ok_or_else(|| {
                    account.internal_failure("official font range declaration is missing")
                })?;
                let range_count = u64::try_from(ranges.len())
                    .map_err(|_| account.internal_failure("SVG font range count overflowed"))?;
                let pairs = checked_product(
                    account,
                    scalar_count,
                    range_count,
                    "SVG font scalar-range pair count overflowed",
                )?;
                let pair_work =
                    checked_product(account, pairs, 32, "SVG font scalar-range work overflowed")?;
                account.transact_resource(
                    ResourceContext::new(
                        ResourcePhase::FormatPlanning,
                        "svg_font_chunk_considered",
                        "",
                    )
                    .with_semantic_identity(record.identity),
                    ResourceDelta {
                        total_work_units: pair_work,
                        ..ResourceDelta::default()
                    },
                )?;
                if family.scalars.iter().any(|scalar| {
                    ranges
                        .iter()
                        .any(|(start, end)| start <= scalar && scalar <= end)
                }) {
                    retained.try_reserve(1).map_err(|_| {
                        account.memory_failure("svg_font_selection", "retained_record")
                    })?;
                    let context = ResourceContext::new(
                        ResourcePhase::FormatPlanning,
                        "svg_font_chunk_selected",
                        "",
                    )
                    .with_semantic_identity(record.identity)
                    .with_record(ResourceRecordKind::BuiltInSelectionState);
                    account.transact_resource(
                        context,
                        ResourceDelta {
                            working_bytes: 256,
                            total_work_units: 8,
                            ..ResourceDelta::default()
                        },
                    )?;
                    retained.push(WorkingRecord {
                        context,
                        bytes: 256,
                    });
                }
            }
        }
        Ok(())
    })();
    let accounting = SelectionAccounting { retained };
    if let Err(error) = result {
        accounting.release(account)?;
        return Err(error);
    }
    Ok(accounting)
}

pub(super) fn account_dependency_envelope<A: ResourceAccount>(
    account: &mut A,
    demand: &Demand,
) -> Result<Option<DependencyEnvelope>, A::Error> {
    const GLYPH_BOUND: u64 = u16::MAX as u64;
    const ENCODER_WORKSPACE: u64 = 8 * 1_024 * 1_024;

    let mut retained_bound = 0_u64;
    let mut active_bound = 0_u64;
    let mut work_bound = 0_u64;
    for family in &demand.families {
        let catalog_family = catalog_family(family.family);
        let scalar_count = u64::try_from(family.scalars.len())
            .map_err(|_| account.internal_failure("SVG font scalar count overflowed"))?;
        for record in excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .filter(|record| record.family == catalog_family)
        {
            let ranges = excalidraw_render_assets_base::font_face_unicode_ranges(
                catalog_family,
                record.upstream_path,
            )
            .ok_or_else(|| {
                account.internal_failure("official font range declaration is missing")
            })?;
            if !family.scalars.iter().any(|scalar| {
                ranges
                    .iter()
                    .any(|(start, end)| start <= scalar && scalar <= end)
            }) {
                continue;
            }
            let source = u64::try_from(record.complete_length)
                .map_err(|_| account.internal_failure("SVG font source size overflowed"))?;
            let decoded = u64::try_from(
                excalidraw_render_assets_base::font_face_decoded_length(record.upstream_path)
                    .ok_or_else(|| {
                        account.internal_failure("official font decoded bound is missing")
                    })?,
            )
            .map_err(|_| account.internal_failure("SVG font decoded size overflowed"))?;
            let table_bound = decoded / 16;
            let sfnt_bound = checked_product(
                account,
                decoded,
                256,
                "SVG font subset sfnt envelope overflowed",
            )?;
            let woff2_bound = checked_product(
                account,
                decoded,
                2,
                "SVG font subset WOFF2 envelope overflowed",
            )?;
            let base64_groups = checked_sum(
                account,
                &[woff2_bound, 2],
                "SVG font base64 rounding overflowed",
            )? / 3;
            let base64_bound = checked_product(
                account,
                base64_groups,
                4,
                "SVG font base64 envelope overflowed",
            )?;
            let table_work = checked_product(
                account,
                table_bound,
                8,
                "SVG font table work envelope overflowed",
            )?;
            let scalar_work = checked_product(
                account,
                scalar_count,
                32,
                "SVG font scalar work envelope overflowed",
            )?;
            let glyph_work = checked_product(
                account,
                GLYPH_BOUND,
                32,
                "SVG font glyph work envelope overflowed",
            )?;
            let chunk_work = checked_sum(
                account,
                &[
                    source,
                    decoded,
                    table_work,
                    scalar_work,
                    glyph_work,
                    sfnt_bound,
                    table_work,
                    sfnt_bound,
                    woff2_bound,
                    woff2_bound,
                    sfnt_bound,
                    glyph_work,
                    woff2_bound,
                    base64_bound,
                ],
                "SVG font dependency work envelope overflowed",
            )?;
            work_bound = checked_sum(
                account,
                &[work_bound, chunk_work],
                "SVG font aggregate work envelope overflowed",
            )?;
            let plan_records = checked_sum(
                account,
                &[scalar_count, GLYPH_BOUND],
                "SVG font subset plan record envelope overflowed",
            )?;
            let plan_bound = checked_product(
                account,
                plan_records,
                64,
                "SVG font subset plan workspace envelope overflowed",
            )?;
            let decoded_blob = checked_derived_blob_bytes(account, decoded)?;
            let sfnt_blob = checked_derived_blob_bytes(account, sfnt_bound)?;
            let chunk_active = checked_sum(
                account,
                &[decoded_blob, sfnt_blob, ENCODER_WORKSPACE, plan_bound],
                "SVG font active dependency envelope overflowed",
            )?;
            active_bound = active_bound.max(chunk_active);
            let woff2_blob = checked_derived_blob_bytes(account, woff2_bound)?;
            retained_bound = checked_sum(
                account,
                &[retained_bound, woff2_blob],
                "SVG font retained dependency envelope overflowed",
            )?;
        }
    }
    let bytes = checked_sum(
        account,
        &[active_bound, retained_bound],
        "SVG font live dependency envelope overflowed",
    )?;
    let context = ResourceContext::new(
        ResourcePhase::EncodingAndOutcomeAssembly,
        "svg_font_dependency_envelope",
        "",
    )
    .with_record(ResourceRecordKind::SvgFontSubsetWorkspace);
    if bytes == 0 {
        return Ok(None);
    }
    account.transact_resource(
        context,
        ResourceDelta {
            working_bytes: bytes,
            total_work_units: work_bound,
            ..ResourceDelta::default()
        },
    )?;
    Ok(Some(DependencyEnvelope(WorkingRecord { context, bytes })))
}

fn checked_product<A: ResourceAccount>(
    account: &mut A,
    left: u64,
    right: u64,
    subject: &'static str,
) -> Result<u64, A::Error> {
    left.checked_mul(right)
        .ok_or_else(|| account.internal_failure(subject))
}

fn checked_sum<A: ResourceAccount>(
    account: &mut A,
    values: &[u64],
    subject: &'static str,
) -> Result<u64, A::Error> {
    values.iter().try_fold(0_u64, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| account.internal_failure(subject))
    })
}

fn checked_derived_blob_bytes<A: ResourceAccount>(
    account: &mut A,
    bytes: u64,
) -> Result<u64, A::Error> {
    let rounded = checked_sum(account, &[bytes, 63], "SVG font blob alignment overflowed")? / 64;
    let aligned = checked_product(account, rounded, 64, "SVG font blob size overflowed")?;
    checked_sum(account, &[256, aligned], "SVG font blob record overflowed")
}

pub(super) fn build(
    demand: Demand,
    unchanged_fonts: &[UnchangedFont<'_>],
    payload: Payload,
    session: &mut Session,
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
                Payload::Subset => {
                    let semantic_identity =
                        semantic_identity(record, &family.scalars).ok_or(BuildError {
                            stage: crate::SvgFontSubsetStage::Plan,
                            family: family.family,
                            asset_identity: record.identity,
                        })?;
                    let derived = session
                        .subset
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
                    let notice = subset_notice(
                        record,
                        family.family,
                        effective_alias,
                        &semantic_identity,
                        &derived,
                    )?;
                    licenses
                        .entry(notice.license_identity)
                        .or_insert(notice.license);
                    resource_notices.push((notice.resource_identity, notice.resource_text));
                    resources.push(Resource {
                        family: effective_alias,
                        mime: "font/woff2",
                        metrics: Some(font_subset::DerivationMetrics::new(
                            record.identity,
                            &derived,
                        )),
                        bytes: derived.woff2,
                    });
                }
                #[cfg(test)]
                Payload::CompleteChunks => {
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
            if let Some(notice) = super::notices::noto_emoji_notice() {
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

pub(super) fn subset_notice(
    record: &excalidraw_render_assets_base::AssetRecord,
    family: &'static str,
    effective_alias: &str,
    semantic_identity: &str,
    derived: &font_subset::Derived,
) -> Result<SubsetNotice, BuildError> {
    let (provenance, license) = notice_inputs(record, family)?;
    let output_digest = format!("{:x}", Sha256::digest(derived.woff2.as_ref()));
    let profile_digest =
        encode_hex_32(&excalidraw_render_assets_base::font_subset_profile_digest_v1());
    Ok(SubsetNotice {
        license_identity: provenance.license_identity.clone(),
        license,
        resource_identity: format!("svg-font-subset@sha256:{semantic_identity}"),
        resource_text: format!(
            "Derived font subset of {}. Source SHA-256: {}. {} Modified Version under {}. Transformation: Unicode and required layout/glyph closure subset; names rewritten for compatibility revision 1. Effective alias: {}. Subset profile SHA-256: {}. Semantic subset identity: {}. Output WOFF2: {} bytes, SHA-256 {}.",
            record.identity,
            record.complete_sha256,
            provenance.text,
            provenance.license_identity,
            effective_alias,
            profile_digest,
            semantic_identity,
            derived.woff2.len(),
            output_digest,
        ),
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
        super::notices::comic_shanns_notice()
    } else if provenance.license_identity.ends_with(".ofl-1.1") {
        super::notices::liberation_sans_notice().and_then(|notice| {
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
    super::notices::push_json_string_content(json, identity);
    json.push_str("\",\"text\":\"");
    super::notices::push_json_string_content(json, text);
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
