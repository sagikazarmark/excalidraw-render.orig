use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const REVISION: u64 = 1;
const EXCALIDRAW_RELEASE: &str = "v0.18.1";
const SOURCE_COMMIT: &str = "a2ec2889babf7d2295469c6d90ebe77fae57df84";
const NPM_INTEGRITY: &str =
    "sha512-6i5Gt7IDTOH//qa0Z315Ly5iVRhjWpu2whrlQFqkuwrkKUWgRsMk0P5qdE7bpyDpai7jeLeWYkyj1eVAfni1lw==";
const PROFILE_DIGEST: &str = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const FRAME_PROFILE_DIGEST: &str =
    "63bf16ee197e7c6e79f917988e04aafec4c89c39de87b08ad10dd19712cde77e";
const ASSET_MANIFEST_DIGEST: &str =
    "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const SCENE_SURFACE_DIGEST: &str =
    "d7dadf8165ef3e42a025e68ec0c1f0f67cc63c7c745199e85442e6938c0c5ade";
const SCENE_SURFACE_SCHEMA_DIGEST: &str =
    "29871fad7bf7665142c8d625c3715b34603a6369a78ffa7baf96ade66be63362";
const SCENE_SURFACE_EVIDENCE_DIGEST: &str =
    "29984c198c9cb222d2aedbcca053e1829743ad686a09a266fdcdfdb192545aeb";
const RAW_SHARD_LIMIT: usize = 8 * 1024 * 1024;
const CATALOG_SCHEMA: &str = "excalidraw-render/revision-catalog/v1";
const CATALOG_SCHEMA_DIGEST: &str =
    "053acdd51de993a5a9a8ba4a8a3bc4bd02255744bfc620e3c58545dda21586e0";
const EXCALIFONT_BASIC_LATIN_PATH: &str =
    "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2";
const EXCALIFONT_BASIC_LATIN_SHA256: &str =
    "e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48";
const EXCALIFONT_BASIC_LATIN_LENGTH: usize = 24_956;
const EXCALIFONT_BASIC_LATIN_SFNT_SHA256: &str =
    "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb";
const EXCALIFONT_BASIC_LATIN_SFNT_LENGTH: usize = 61_948;
const NOTO_EMOJI_SHA256: &str = "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634";
const NOTO_EMOJI_LENGTH: usize = 862_680;
const XIAOLAI_ORDER_SHA256: &str =
    "c2cbbbfa669404795527e06cf596ab4b1b851f9bb993c608c8ab9c3c2aabe9d3";
const SVG_FONT_SUBSET_PROFILE_DIGEST: &str =
    "19a921a3e87bfff3c304db03da012dce812ba7cd84755b410b8e9e7a3e52806d";
const CHROMIUM_149_CJK_SCALARS_DIGEST: &str =
    "fc4e8ce56cb7bf4bca9dfd6f3495c9ae9f5440a5818aa3e6f645d5303265ff85";
const SVG_FONT_SUBSET_QUALIFICATION_CORPUS_DIGEST: &str =
    "5b96d68e542a24f9bbc75e95473ce009cf7a108ae52fa896d75ee5edcd8a07c8";
const SVG_FONT_SUBSET_QUALIFICATION_SCHEMA_DIGEST: &str =
    "a12bcf35a6fba556ce8f7342132a144d6b7494535d016e399f431e126043765c";

struct IncorporatedArtifactSource {
    repository_path: &'static str,
    digest: &'static str,
    packaged_name: &'static str,
    canonical_json: bool,
}

const fn artifact(
    repository_path: &'static str,
    digest: &'static str,
    packaged_name: &'static str,
    canonical_json: bool,
) -> IncorporatedArtifactSource {
    IncorporatedArtifactSource {
        repository_path,
        digest,
        packaged_name,
        canonical_json,
    }
}

const ARTIFACTS: &[IncorporatedArtifactSource] = &[
    artifact(
        "tests/oracle/reference-profiles/revision-1.json",
        PROFILE_DIGEST,
        "reference-profile.json",
        true,
    ),
    artifact(
        "tests/oracle/reference-profiles/revision-1-frame.json",
        FRAME_PROFILE_DIGEST,
        "reference-frame-profile.json",
        true,
    ),
    artifact(
        "tests/oracle/reference-profiles/revision-1-assets.json",
        ASSET_MANIFEST_DIGEST,
        "reference-assets.json",
        true,
    ),
    artifact(
        "tests/oracle/scene-surfaces/revision-1.json",
        SCENE_SURFACE_DIGEST,
        "scene-surface.json",
        true,
    ),
    artifact(
        "tests/oracle/scene-surfaces/scene-surface-v1.schema.json",
        SCENE_SURFACE_SCHEMA_DIGEST,
        "scene-surface.schema.json",
        true,
    ),
    artifact(
        "tests/oracle/scene-surfaces/evidence/revision-1-harmless-fields.json",
        SCENE_SURFACE_EVIDENCE_DIGEST,
        "scene-surface-evidence.json",
        true,
    ),
    artifact(
        "tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json",
        SVG_FONT_SUBSET_PROFILE_DIGEST,
        "svg-font-subset-profile.json",
        true,
    ),
    artifact(
        "tests/oracle/compatibility-artifacts/revision-1-chromium-149-cjk-scalars.bin",
        CHROMIUM_149_CJK_SCALARS_DIGEST,
        "chromium-149-cjk-scalars.bin",
        false,
    ),
];

const LICENSE_PARTS: &[(&str, &str)] = &[
    ("Project MIT license", "../../LICENSE-MIT"),
    ("Project Apache-2.0 license", "../../LICENSE-APACHE"),
    ("Excalidraw MIT license", "excalidraw-MIT.txt"),
    ("Rough.js MIT license", "roughjs-MIT.txt"),
    ("Perfect Freehand MIT license", "perfect-freehand-MIT.txt"),
    ("points-on-curve MIT license", "points-on-curve-MIT.txt"),
    ("Comic Shanns MIT license", "comic-shanns-MIT.txt"),
    ("SIL Open Font License 1.1", "OFL-1.1.txt"),
    ("Creative Commons Attribution 4.0", "CC-BY-4.0.txt"),
    ("Font Awesome Free notice", "font-awesome-NOTICE.txt"),
];

const EXACT_LICENSE_SOURCES: &[(&str, &str, &str)] = &[
    (
        "LICENSE-APACHE",
        "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
        "../../LICENSE-APACHE",
    ),
    (
        "CC-BY-4.0.txt",
        "9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411",
        "license-parts/CC-BY-4.0.txt",
    ),
    (
        "font-awesome-NOTICE.txt",
        "e779748dfe75e84f974df3c7bc07f842011a100159158b0f1f49b2f2a5a515cb",
        "license-parts/font-awesome-NOTICE.txt",
    ),
];

const FONT_NOTICES: &[(&str, &str)] = &[
    (
        "Cascadia Code",
        "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code. Licensed under the SIL Open Font License, Version 1.1.",
    ),
    (
        "Comic Shanns",
        "Copyright (c) 2018 Shannon Miwa. Licensed under the MIT License included in this bundle.",
    ),
    (
        "Excalifont and Virgil provenance",
        "Copyright (c) 2024 by Excalidraw. Excalifont is derived from Virgil by Your Own Font Foundry, with modifications by Jan Filipek / DizajnDesign. Licensed under the SIL Open Font License, Version 1.1. Excalifont and Virgil provenance is retained together.",
    ),
    (
        "Liberation Sans",
        "Digitized data copyright (c) 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine. Copyright (c) 2012 Red Hat, Inc. with Reserved Font Name Liberation. Licensed under the SIL Open Font License, Version 1.1.",
    ),
    (
        "Lilita",
        "Copyright (c) 2011 Juan Montoreano (juan@remolacha.biz), with Reserved Font Name Lilita. Licensed under the SIL Open Font License, Version 1.1.",
    ),
    (
        "Nunito",
        "Copyright 2014 The Nunito Project Authors (https://github.com/googlefonts/nunito). Licensed under the SIL Open Font License, Version 1.1.",
    ),
    (
        "Virgil",
        "Virgil by Your Own Font Foundry, with Reserved Font Name Virgil. Licensed under the SIL Open Font License, Version 1.1.",
    ),
    (
        "Xiaolai SC",
        "Copyright (c) 2020 LXGW. Designed by Nozomi Seto. Licensed under the SIL Open Font License, Version 1.1. The shipped subsets remove non-CJK codepoints, reduce Hangul side-bearings to 40%, and center improperly centered CJK codepoints.",
    ),
];

const FALLBACK_RECIPE_COUNT: usize = 5;
const NOTICE_COUNT: usize = 4;
const SOURCE_COUNT: usize = 34;
const FIELD_DISPOSITION_COUNT: usize = 202;
const ELEMENT_KIND_COUNT: usize = 13;
const REGISTRY_MAPPING_COUNT: usize = 42;
const IMPLEMENTED_DIAGNOSTIC_CODES: &[&str] = &[
    "normalization.missing-element-id",
    "normalization.duplicate-element-id",
    "normalization.missing-timestamp",
    "normalization.zero-rough-seed",
    "normalization.relationship-cycle",
    "compatibility.unsupported-element-type",
    "compatibility.malformed-element",
    "compatibility.unsupported-property",
    "compatibility.unsupported-property-value",
    "compatibility.known-element-replaced",
    "asset.supplement-conflict",
    "asset.media-type-mismatch",
    "asset.malformed-file-record",
    "image.missing-file",
    "image.forbidden-resource",
    "image.animation-flattened",
    "image.unsupported-format",
    "image.decode-failed",
    "image.unsafe-svg-content-removed",
    "link.unsafe-omitted",
    "color.invalid",
    "font.liberation-sans-substituted",
    "font.noto-emoji-substituted",
    "font.missing-glyph",
    "resource.element-complexity-replaced",
    "resource.element-work-exceeded",
    "resource.asset-bytes-exceeded",
    "resource.asset-work-exceeded",
    "resource.image-dimensions-exceeded",
    "resource.image-pixels-exceeded",
    "resource.nested-svg-complexity-exceeded",
    "resource.nested-asset-depth-exceeded",
    "resource.coordinate-range-exceeded",
    "resource.optional-metadata-omitted",
    "metadata.provenance-serialization-failed",
    "metadata.diagnostic-summary-serialization-failed",
    "metadata.scene-serialization-failed",
    "metadata.scene-compression-failed",
    "metadata.scene-embedding-failed",
    "metadata.manifest-serialization-failed",
    "metadata.manifest-embedding-failed",
    "pdf.raster-fallback",
];
const IMPLEMENTED_DOMAIN_IDS: &[&str] = &[
    "any_json",
    "arrow_type",
    "arrowhead",
    "boolean",
    "bound_element_array",
    "bound_element_type",
    "chart_type",
    "css_color",
    "cursor_button",
    "data_url",
    "element_type",
    "fill_style",
    "finite_number",
    "finite_number_array",
    "fixed_segment_array",
    "font_family",
    "frame_rendering",
    "freedraw_points",
    "hyperlink_popup",
    "image_crop",
    "image_mime",
    "image_scale",
    "image_status",
    "integer",
    "literal_excalidraw",
    "literal_v2",
    "nonempty_string",
    "nonnegative_integer",
    "nonnegative_number",
    "nonzero_integer",
    "nullable_boolean",
    "nullable_font_family",
    "nullable_point",
    "nullable_record",
    "nullable_string",
    "object",
    "opacity",
    "open_menu",
    "open_popup",
    "point",
    "point_array",
    "point_binding",
    "pointer_type",
    "positive_number",
    "pressure_array",
    "roundness",
    "roundness_type",
    "scene_elements",
    "string",
    "string_array",
    "stroke_roundness",
    "stroke_style",
    "text_align",
    "theme",
    "vertical_align",
];

struct ImportedAsset {
    path: String,
    bytes: Vec<u8>,
    length: usize,
    digest: String,
    family: String,
    license: &'static str,
    copyright: &'static str,
    reserved_names: &'static [&'static str],
    shard: usize,
    offset: usize,
}

pub fn import_license_sources(source_root: &str) -> Result<(), String> {
    let root = crate::conformance::repo_root();
    let source_root = Path::new(source_root);
    if !source_root.is_dir() {
        return Err(format!(
            "license import source is not a directory: {}",
            source_root.display()
        ));
    }
    let base = root.join("crates/excalidraw-render-assets-base");
    for (source_name, expected_digest, destination) in EXACT_LICENSE_SOURCES {
        let bytes = read_verified(&source_root.join(source_name), expected_digest, source_name)?;
        write_if_changed(&base.join(destination), &bytes)?;
    }
    Ok(())
}

pub fn import(source_root: &str) -> Result<(), String> {
    let root = crate::conformance::repo_root();
    let source_root = Path::new(source_root);
    if !source_root.is_dir() {
        return Err(format!(
            "catalog import source is not a directory: {}",
            source_root.display()
        ));
    }

    let reference_assets = read_canonical_json(
        &root.join("tests/oracle/reference-profiles/revision-1-assets.json"),
        "reference asset manifest",
    )?;
    let manifest_assets = array_at(
        &reference_assets,
        &["officialPackage", "assets"],
        "official package assets",
    )?;

    let mut assets = Vec::with_capacity(manifest_assets.len());
    for entry in manifest_assets {
        let path = string(entry, "path", "reference asset")?.to_owned();
        if path.starts_with("fonts/Assistant/") {
            return Err("the excluded Assistant family occurs in the asset manifest".to_owned());
        }
        let expected_length = integer(entry, "bytes", "reference asset")? as usize;
        let expected_digest = string(entry, "sha256", "reference asset")?;
        let bytes = fs::read(source_root.join(&path))
            .map_err(|error| format!("failed to read import source {path}: {error}"))?;
        if bytes.len() != expected_length || sha256(&bytes) != expected_digest {
            return Err(format!(
                "import source does not match manifested asset {path}"
            ));
        }
        let family = path
            .split('/')
            .nth(1)
            .ok_or_else(|| format!("asset path has no family: {path}"))?
            .to_owned();
        let metadata = family_metadata(&family)?;
        assets.push(ImportedAsset {
            path,
            length: bytes.len(),
            digest: expected_digest.to_owned(),
            family,
            license: metadata.0,
            copyright: metadata.1,
            reserved_names: metadata.2,
            bytes,
            shard: 0,
            offset: 0,
        });
    }
    assets.sort_by(|left, right| left.path.cmp(&right.path));
    if assets.len() != 230 {
        return Err(format!(
            "expected 230 official rendering assets, found {}",
            assets.len()
        ));
    }
    materialize_predecoded_excalifont(&root, &assets)?;

    let shards = partition(&mut assets)?;
    if shards.len() != 2 {
        return Err(format!(
            "revision 1 official assets must materialize as two shards, found {}",
            shards.len()
        ));
    }

    let base = root.join("crates/excalidraw-render-assets-base");
    materialize_incorporated_artifacts(&root, &base)?;

    for (index, bytes) in shards.iter().enumerate() {
        let data_path = root.join(format!(
            "crates/excalidraw-render-assets-data-{index:03}/data/revision-1-{index:03}.bin"
        ));
        if let Some(parent) = data_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        write_if_changed(&data_path, bytes)?;
    }

    let scene_surface = read_canonical_json(
        &root.join("tests/oracle/scene-surfaces/revision-1.json"),
        "scene-surface manifest",
    )?;
    verify_scene_surface(&root, &scene_surface)?;
    let bundle = build_license_bundle(&base, &scene_surface)?;
    for bundle_path in bundle_paths(&root) {
        write_if_changed(&bundle_path, bundle.as_bytes())?;
    }
    write_digest_sidecar(&base.join("LICENSE-BUNDLE.sha256"), bundle.as_bytes())?;

    let catalog = build_catalog(&assets, &scene_surface)?;
    let catalog_bytes = serde_json::to_vec(&catalog)
        .map_err(|error| format!("failed to serialize revision catalog: {error}"))?;
    let catalog_path = base.join("catalog/revision-1.json");
    write_if_changed(&catalog_path, &catalog_bytes)?;
    write_digest_sidecar(&base.join("catalog/revision-1.sha256"), &catalog_bytes)?;

    let generated = generate_rust(&catalog, &scene_surface, &catalog_bytes)?;
    let generated_path = base.join("src/generated/revision_1.rs");
    if let Some(parent) = generated_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    write_if_changed(&generated_path, generated.as_bytes())?;
    let scene_policy = generated_scene_surface(&scene_surface)?;
    let scene_policy_path =
        root.join("crates/excalidraw-render/src/generated/scene_surface_revision_1.rs");
    if let Some(parent) = scene_policy_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    write_if_changed(&scene_policy_path, scene_policy.as_bytes())?;

    verify()
}

pub fn materialize() -> Result<(), String> {
    let root = crate::conformance::repo_root();
    let base = root.join("crates/excalidraw-render-assets-base");
    materialize_svg_font_subset_profile(&root)?;
    materialize_incorporated_artifacts(&root, &base)?;

    let scene_surface = read_canonical_json(
        &root.join("tests/oracle/scene-surfaces/revision-1.json"),
        "scene-surface manifest",
    )?;
    verify_scene_surface(&root, &scene_surface)?;
    let reference_assets = read_canonical_json(
        &root.join("tests/oracle/reference-profiles/revision-1-assets.json"),
        "reference asset manifest",
    )?;
    let catalog = expected_catalog(&reference_assets, &scene_surface)?;
    let catalog_bytes = serde_json::to_vec(&catalog)
        .map_err(|error| format!("failed to serialize revision catalog: {error}"))?;
    write_if_changed(&base.join("catalog/revision-1.json"), &catalog_bytes)?;
    write_digest_sidecar(&base.join("catalog/revision-1.sha256"), &catalog_bytes)?;

    let generated = generate_rust(&catalog, &scene_surface, &catalog_bytes)?;
    write_if_changed(
        &base.join("src/generated/revision_1.rs"),
        generated.as_bytes(),
    )?;
    verify()
}

fn materialize_svg_font_subset_profile(root: &Path) -> Result<(), String> {
    use excalidraw_render_assets_base::{
        FontSubsetTableAction, FONT_SUBSET_PROFILE_SETTINGS_V1, FONT_SUBSET_TABLE_RULES_V1,
    };

    let settings = std::str::from_utf8(FONT_SUBSET_PROFILE_SETTINGS_V1)
        .map_err(|error| format!("invalid SVG font subset profile settings: {error}"))?;
    let rules = FONT_SUBSET_TABLE_RULES_V1
        .iter()
        .map(|rule| {
            Ok(json!({
                "action": match rule.action {
                    FontSubsetTableAction::Subset => "subset",
                    FontSubsetTableAction::Rewrite => "rewrite",
                    FontSubsetTableAction::PassThrough => "pass_through",
                    FontSubsetTableAction::Drop => "drop",
                },
                "required": rule.required,
                "tag": std::str::from_utf8(&rule.tag)
                    .map_err(|error| format!("invalid subset table tag: {error}"))?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let semantic_digest = excalidraw_render_assets_base::font_subset_profile_digest_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let profile = json!({
        "semanticDigest": format!("sha256:{semantic_digest}"),
        "settingsPreimage": settings,
        "tableRules": rules,
    });
    let bytes = serde_json::to_vec(&profile)
        .map_err(|error| format!("failed to serialize SVG font subset profile: {error}"))?;
    let path =
        root.join("tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json");
    write_if_changed(&path, &bytes)?;
    write_digest_sidecar(&path.with_extension("sha256"), &bytes)
}

fn materialize_incorporated_artifacts(root: &Path, base: &Path) -> Result<(), String> {
    let incorporated = base.join("catalog/incorporated");
    fs::create_dir_all(&incorporated)
        .map_err(|error| format!("failed to create catalog directory: {error}"))?;
    verify_incorporated_inventory(&incorporated)?;
    for artifact in ARTIFACTS {
        verify_authoritative_sidecar(root, artifact.repository_path, artifact.digest)?;
        let bytes = read_verified(
            &root.join(artifact.repository_path),
            artifact.digest,
            artifact.repository_path,
        )?;
        write_if_changed(&incorporated.join(artifact.packaged_name), &bytes)?;
    }

    let schema_path = base.join("catalog/revision-catalog-v1.schema.json");
    let mut schema: Value = serde_json::from_slice(
        &fs::read(&schema_path)
            .map_err(|error| format!("failed to read {}: {error}", schema_path.display()))?,
    )
    .map_err(|error| format!("invalid revision catalog schema: {error}"))?;
    schema["properties"]["incorporatedArtifacts"]["minItems"] = json!(ARTIFACTS.len());
    schema["properties"]["incorporatedArtifacts"]["maxItems"] = json!(ARTIFACTS.len());
    let schema_bytes = serde_json::to_vec(&schema)
        .map_err(|error| format!("failed to canonicalize revision catalog schema: {error}"))?;
    let schema_digest = sha256(&schema_bytes);
    if schema_digest != CATALOG_SCHEMA_DIGEST {
        return Err(format!(
            "revision catalog schema differs from its pinned materialization: {schema_digest}"
        ));
    }
    write_if_changed(&schema_path, &schema_bytes)?;
    write_digest_sidecar(
        &base.join("catalog/revision-catalog-v1.schema.sha256"),
        &schema_bytes,
    )
}

fn verify_incorporated_inventory(incorporated: &Path) -> Result<(), String> {
    let expected = ARTIFACTS
        .iter()
        .map(|artifact| artifact.packaged_name)
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(incorporated)
        .map_err(|error| format!("failed to read {}: {error}", incorporated.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read incorporated artifact in {}: {error}",
                incorporated.display()
            )
        })?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            format!(
                "incorporated artifact name is not UTF-8: {}",
                entry.path().display()
            )
        })?;
        if !entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?
            .is_file()
            || !expected.contains(name)
        {
            return Err(format!(
                "unexpected incorporated artifact: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub fn verify() -> Result<(), String> {
    let root = crate::conformance::repo_root();
    let base = root.join("crates/excalidraw-render-assets-base");
    verify_incorporated_inventory(&base.join("catalog/incorporated"))?;

    for artifact in ARTIFACTS {
        verify_authoritative_sidecar(&root, artifact.repository_path, artifact.digest)?;
        let authoritative = read_verified(
            &root.join(artifact.repository_path),
            artifact.digest,
            artifact.repository_path,
        )?;
        if artifact.canonical_json {
            verify_canonical_json_bytes(&authoritative, artifact.repository_path)?;
        }
        let packaged_path = base
            .join("catalog/incorporated")
            .join(artifact.packaged_name);
        let packaged = fs::read(&packaged_path)
            .map_err(|error| format!("failed to read {}: {error}", packaged_path.display()))?;
        if packaged != authoritative {
            return Err(format!(
                "packaged incorporated artifact differs from {}",
                artifact.repository_path
            ));
        }
    }
    let frame_profile = read_canonical_json(
        &root.join("tests/oracle/reference-profiles/revision-1-frame.json"),
        "frame reference profile",
    )?;
    verify_frame_reference_profile(&root, &frame_profile)?;

    let scene_surface = read_canonical_json(
        &root.join("tests/oracle/scene-surfaces/revision-1.json"),
        "scene-surface manifest",
    )?;
    verify_scene_surface(&root, &scene_surface)?;

    let catalog_path = base.join("catalog/revision-1.json");
    let catalog_bytes = fs::read(&catalog_path)
        .map_err(|error| format!("failed to read {}: {error}", catalog_path.display()))?;
    verify_canonical_json_bytes(&catalog_bytes, "revision catalog")?;
    verify_digest_sidecar(&base.join("catalog/revision-1.sha256"), &catalog_bytes)?;
    let catalog: Value = serde_json::from_slice(&catalog_bytes)
        .map_err(|error| format!("invalid revision catalog JSON: {error}"))?;
    let reference_assets = read_canonical_json(
        &root.join("tests/oracle/reference-profiles/revision-1-assets.json"),
        "reference asset manifest",
    )?;
    let expected_catalog = expected_catalog(&reference_assets, &scene_surface)?;
    if catalog != expected_catalog {
        return Err("revision catalog differs from its authoritative materialization".to_owned());
    }
    verify_catalog_schema(&base, &catalog)?;
    verify_catalog_identity(&catalog, &scene_surface)?;
    verify_font_subset_qualification_corpus(&root, &catalog)?;
    let excalifont_source = verify_assets(&root, &catalog)?;
    verify_predecoded_excalifont(&root, &excalifont_source)?;
    verify_noto_emoji(&root)?;
    let xiaolai_order =
        root.join("crates/excalidraw-render-assets-base/catalog/order/Xiaolai-index.ts");
    let order_bytes = fs::read(&xiaolai_order)
        .map_err(|error| format!("failed to read {}: {error}", xiaolai_order.display()))?;
    if sha256(&order_bytes) != XIAOLAI_ORDER_SHA256 {
        return Err("pinned Xiaolai declaration order digest is invalid".to_owned());
    }

    let generated_path = base.join("src/generated/revision_1.rs");
    let generated = fs::read_to_string(&generated_path)
        .map_err(|error| format!("failed to read {}: {error}", generated_path.display()))?;
    let expected_generated = generate_rust(&catalog, &scene_surface, &catalog_bytes)?;
    if generated != expected_generated {
        return Err("generated Rust catalog is stale".to_owned());
    }
    let scene_policy_path =
        root.join("crates/excalidraw-render/src/generated/scene_surface_revision_1.rs");
    let scene_policy = fs::read_to_string(&scene_policy_path)
        .map_err(|error| format!("failed to read {}: {error}", scene_policy_path.display()))?;
    if scene_policy != generated_scene_surface(&scene_surface)? {
        return Err("generated scene-surface policy is stale".to_owned());
    }

    let bundle_path = base.join("LICENSE-BUNDLE.txt");
    let bundle = fs::read(&bundle_path)
        .map_err(|error| format!("failed to read {}: {error}", bundle_path.display()))?;
    std::str::from_utf8(&bundle).map_err(|_| "license bundle is not UTF-8".to_owned())?;
    verify_digest_sidecar(&base.join("LICENSE-BUNDLE.sha256"), &bundle)?;
    let expected_bundle = build_license_bundle(&base, &scene_surface)?;
    if bundle != expected_bundle.as_bytes() {
        return Err("license bundle is stale".to_owned());
    }
    for copy in bundle_paths(&root) {
        let bytes = fs::read(&copy)
            .map_err(|error| format!("failed to read {}: {error}", copy.display()))?;
        if bytes != bundle {
            return Err(format!("license bundle copy is stale: {}", copy.display()));
        }
    }
    Ok(())
}

fn materialize_predecoded_excalifont(root: &Path, assets: &[ImportedAsset]) -> Result<(), String> {
    let source = assets
        .iter()
        .find(|asset| asset.path == EXCALIFONT_BASIC_LATIN_PATH)
        .ok_or_else(|| "manifested Excalifont basic-Latin source is missing".to_owned())?;
    let sfnt = wuff::decompress_woff2(&source.bytes)
        .map_err(|error| format!("failed to predecode Excalifont basic Latin: {error}"))?;
    if sfnt.len() != EXCALIFONT_BASIC_LATIN_SFNT_LENGTH
        || sha256(&sfnt) != EXCALIFONT_BASIC_LATIN_SFNT_SHA256
    {
        return Err(
            "predecoded Excalifont basic-Latin bytes do not match the sealed identity".to_owned(),
        );
    }
    rustybuzz::Face::from_slice(&sfnt, 0)
        .ok_or_else(|| "predecoded Excalifont basic Latin is not a valid sfnt".to_owned())?;
    let destination = root.join(
        "crates/excalidraw-render-assets-base/catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt",
    );
    write_if_changed(&destination, &sfnt)
}

fn verify_predecoded_excalifont(root: &Path, source: &[u8]) -> Result<(), String> {
    let path = root.join(
        "crates/excalidraw-render-assets-base/catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt",
    );
    let sfnt =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if sfnt.len() != EXCALIFONT_BASIC_LATIN_SFNT_LENGTH
        || sha256(&sfnt) != EXCALIFONT_BASIC_LATIN_SFNT_SHA256
    {
        return Err("checked-in Excalifont basic-Latin sfnt digest is invalid".to_owned());
    }
    rustybuzz::Face::from_slice(&sfnt, 0)
        .ok_or_else(|| "checked-in Excalifont basic Latin is not a valid sfnt".to_owned())?;
    let metadata_path = path.with_extension("sfnt.json");
    let metadata: Value = serde_json::from_slice(
        &fs::read(&metadata_path)
            .map_err(|error| format!("failed to read {}: {error}", metadata_path.display()))?,
    )
    .map_err(|error| format!("invalid Excalifont transformation record: {error}"))?;
    let expected = json!({
        "approval": {
            "date": "2026-07-22",
            "exact_output_bytes_reviewed": true,
            "issue": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/58#issuecomment-5051018482",
            "limitation": "Approval applies only to output SHA-256 a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb; any byte change requires fresh review.",
            "policy_approved_by": "sagikazarmark",
            "policy_approval_date": "2026-07-18",
            "reviewer": "OpenAI gpt-5.6-sol",
            "scope": "independent automated byte-level and SFNT structural review"
        },
        "comparison": {
            "container_decode": "wuff 0.2.8 output and checked SFNT are byte-identical",
            "glyphs": "217 glyphs; complete glyf, loca, bounds, outlines, and horizontal metrics are identical",
            "license_metadata": "embedded copyright and legal name records are identical; catalog OFL identity and combined Excalifont/Virgil package notice verified",
            "metrics": "units-per-em, global bounds, ascender, descender, line gap, weight, width, advances, and side bearings are identical",
            "names": "all seven name records are identical; no renaming occurred",
            "tables": "all 14 SFNT tables and declared checksums are identical"
        },
        "output": {
            "length": EXCALIFONT_BASIC_LATIN_SFNT_LENGTH,
            "path": "catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt",
            "sha256": EXCALIFONT_BASIC_LATIN_SFNT_SHA256
        },
        "schema_version": 1,
        "source": {
            "length": EXCALIFONT_BASIC_LATIN_LENGTH,
            "path": EXCALIFONT_BASIC_LATIN_PATH,
            "sha256": EXCALIFONT_BASIC_LATIN_SHA256
        },
        "tool": {
            "invocation": "wuff::decompress_woff2(source_bytes)",
            "name": "wuff",
            "version": "0.2.8"
        }
    });
    if metadata != expected {
        return Err("Excalifont predecoded transformation record is stale".to_owned());
    }
    if source.len() != EXCALIFONT_BASIC_LATIN_LENGTH
        || sha256(source) != EXCALIFONT_BASIC_LATIN_SHA256
    {
        return Err("Excalifont predecoded transformation source is invalid".to_owned());
    }
    let decoded = wuff::decompress_woff2(source)
        .map_err(|error| format!("failed to verify Excalifont transformation: {error}"))?;
    if decoded != sfnt {
        return Err("checked-in Excalifont sfnt differs from the recorded decode".to_owned());
    }
    Ok(())
}

fn verify_noto_emoji(root: &Path) -> Result<(), String> {
    let path =
        root.join("crates/excalidraw-render-assets-base/catalog/fallback/NotoEmoji-Regular.ttf");
    let sfnt =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if sfnt.len() != NOTO_EMOJI_LENGTH || sha256(&sfnt) != NOTO_EMOJI_SHA256 {
        return Err("checked-in Noto Emoji 3.003 fallback digest is invalid".to_owned());
    }
    let face = rustybuzz::Face::from_slice(&sfnt, 0)
        .ok_or_else(|| "checked-in Noto Emoji fallback is not a valid sfnt".to_owned())?;
    if face.units_per_em() != 1_000
        || face.glyph_index('\u{1f600}').is_none()
        || face.glyph_index('A').is_some()
    {
        return Err("checked-in Noto Emoji fallback metrics or mappings are invalid".to_owned());
    }
    let metadata_path = path.with_extension("ttf.json");
    let metadata_bytes = fs::read(&metadata_path)
        .map_err(|error| format!("failed to read {}: {error}", metadata_path.display()))?;
    verify_canonical_json_bytes(&metadata_bytes, "Noto Emoji transformation record")?;
    let metadata: Value = serde_json::from_slice(&metadata_bytes)
        .map_err(|error| format!("invalid Noto Emoji transformation record: {error}"))?;
    if metadata["approval"]["exact_output_bytes_reviewed"] != true
        || metadata["comparison"]["version"] != "Version 3.003"
        || metadata["source"]["sha256"] != NOTO_EMOJI_SHA256
        || metadata["source"]["length"] != NOTO_EMOJI_LENGTH
        || metadata["output"]["sha256"] != NOTO_EMOJI_SHA256
        || metadata["output"]["length"] != NOTO_EMOJI_LENGTH
        || metadata["transformation"]["identity"] != "identity.byte-exact.v1"
    {
        return Err("Noto Emoji transformation record is stale".to_owned());
    }
    Ok(())
}

fn bundle_paths(root: &Path) -> [PathBuf; 5] {
    [
        root.join("crates/excalidraw-render-assets-base/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-render-assets-data-000/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-render-assets-data-001/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-render/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-render-cli/LICENSE-BUNDLE.txt"),
    ]
}

fn build_catalog(assets: &[ImportedAsset], scene_surface: &Value) -> Result<Value, String> {
    let fallback_ids = ids(scene_surface, "fallbackRecipes", "recipeId")?;
    let notice_ids = ids(scene_surface, "noticeCatalog", "id")?;
    let source_ids = ids(scene_surface, "sourceCatalog", "id")?;
    let asset_values = assets
        .iter()
        .map(|asset| {
            let identity = format!("{}@sha256:{}", asset.path, asset.digest);
            let segment_identity = format!("{identity}/segment/0");
            json!({
                "completeLength": asset.length,
                "completeSha256": asset.digest,
                "copyright": asset.copyright,
                "family": asset.family,
                "identity": identity,
                "licenseIdentity": asset.license,
                "outputFormats": ["svg", "png", "pdf"],
                "profile": format!("sha256:{PROFILE_DIGEST}"),
                "provenanceIdentity": "excalidraw.npm-v0.18.1",
                "reservedFontNames": asset.reserved_names,
                "role": "official-rendering-font",
                "segments": [{
                    "identity": segment_identity,
                    "length": asset.length,
                    "offset": asset.offset,
                    "order": 0,
                    "sha256": asset.digest,
                    "shard": format!("revision-1-{:03}", asset.shard),
                }],
                "transformationIdentity": "identity.byte-exact.v1",
                "upstreamPath": asset.path,
                "upstreamRevision": SOURCE_COMMIT,
                "upstreamUrl": format!(
                    "https://github.com/excalidraw/excalidraw/blob/{SOURCE_COMMIT}/packages/excalidraw/{}",
                    asset.path
                ),
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "assets": asset_values,
        "compatibilityRevision": REVISION,
        "excalidrawRelease": EXCALIDRAW_RELEASE,
        "fallbackRecipeIdentities": fallback_ids,
        "incorporatedArtifacts": ARTIFACTS.iter().map(|artifact| json!({
            "path": artifact.repository_path,
            "sha256": artifact.digest,
        })).collect::<Vec<_>>(),
        "licenseIdentities": license_identities(),
        "noticeIdentities": notice_ids,
        "npmIntegrity": NPM_INTEGRITY,
        "provenanceIdentities": [{
            "id": "excalidraw.npm-v0.18.1",
            "npmIntegrity": NPM_INTEGRITY,
            "sourceCommit": SOURCE_COMMIT,
            "sourceIds": source_ids,
        }],
        "referenceProfileDigest": format!("sha256:{PROFILE_DIGEST}"),
        "schema": CATALOG_SCHEMA,
        "sourceCommit": SOURCE_COMMIT,
        "transformationIdentities": [{
            "description": "Imported bytes are unchanged from the authoritative npm package asset.",
            "id": "identity.byte-exact.v1",
        }],
    }))
}

fn expected_catalog(reference_assets: &Value, scene_surface: &Value) -> Result<Value, String> {
    let manifest_assets = array_at(
        reference_assets,
        &["officialPackage", "assets"],
        "official package assets",
    )?;
    let mut assets = manifest_assets
        .iter()
        .map(|entry| {
            let path = string(entry, "path", "reference asset")?.to_owned();
            let family = path
                .split('/')
                .nth(1)
                .ok_or_else(|| format!("asset path has no family: {path}"))?
                .to_owned();
            let metadata = family_metadata(&family)?;
            Ok(ImportedAsset {
                path,
                bytes: Vec::new(),
                length: integer(entry, "bytes", "reference asset")? as usize,
                digest: string(entry, "sha256", "reference asset")?.to_owned(),
                family,
                license: metadata.0,
                copyright: metadata.1,
                reserved_names: metadata.2,
                shard: 0,
                offset: 0,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    assets.sort_by(|left, right| left.path.cmp(&right.path));

    let mut shard = 0;
    let mut offset = 0;
    for asset in &mut assets {
        if offset != 0 && offset + asset.length > RAW_SHARD_LIMIT {
            shard += 1;
            offset = 0;
        }
        asset.shard = shard;
        asset.offset = offset;
        offset += asset.length;
    }
    if shard != 1 {
        return Err(format!(
            "authoritative assets must materialize as two shards, found {}",
            shard + 1
        ));
    }
    build_catalog(&assets, scene_surface)
}

fn partition(assets: &mut [ImportedAsset]) -> Result<Vec<Vec<u8>>, String> {
    let mut shards = vec![Vec::new()];
    for asset in assets {
        if asset.length > RAW_SHARD_LIMIT {
            return Err(format!("asset {} exceeds the raw shard limit", asset.path));
        }
        if !shards.last().unwrap().is_empty()
            && shards.last().unwrap().len() + asset.length > RAW_SHARD_LIMIT
        {
            shards.push(Vec::new());
        }
        let shard = shards.len() - 1;
        let bytes = shards.last_mut().unwrap();
        asset.shard = shard;
        asset.offset = bytes.len();
        bytes.extend_from_slice(&asset.bytes);
    }
    Ok(shards)
}

fn generate_rust(
    catalog: &Value,
    scene_surface: &Value,
    catalog_bytes: &[u8],
) -> Result<String, String> {
    let mut output = String::from(
        "// Generated by `cargo run --locked -p xtask -- catalog import` or `catalog materialize`; do not edit.\n\n",
    );
    output.push_str("#[rustfmt::skip]\npub static ASSETS: [AssetRecord; 230] = [\n");
    let assets = catalog["assets"]
        .as_array()
        .ok_or_else(|| "revision catalog assets must be an array".to_owned())?;
    for asset in assets {
        output.push_str("    AssetRecord {\n");
        for (rust_name, json_name) in [
            ("identity", "identity"),
            ("upstream_path", "upstreamPath"),
            ("upstream_revision", "upstreamRevision"),
            ("upstream_url", "upstreamUrl"),
            ("complete_sha256", "completeSha256"),
        ] {
            rust_field(
                &mut output,
                rust_name,
                string(asset, json_name, "catalog asset")?,
            );
        }
        output.push_str(&format!(
            "        complete_length: {},\n",
            integer(asset, "completeLength", "catalog asset")?
        ));
        for (rust_name, json_name) in [
            ("family", "family"),
            ("license_identity", "licenseIdentity"),
            ("provenance_identity", "provenanceIdentity"),
            ("transformation_identity", "transformationIdentity"),
        ] {
            rust_field(
                &mut output,
                rust_name,
                string(asset, json_name, "catalog asset")?,
            );
        }
        output.push_str("        reserved_font_names: &[");
        for name in asset["reservedFontNames"]
            .as_array()
            .ok_or_else(|| "catalog asset reservedFontNames must be an array".to_owned())?
        {
            let name = name
                .as_str()
                .ok_or_else(|| "reserved font names must be strings".to_owned())?;
            output.push_str(&format!("{name:?},"));
        }
        output.push_str("],\n");
        rust_field(&mut output, "role", string(asset, "role", "catalog asset")?);
        output.push_str("        output_formats: &[");
        for format in asset["outputFormats"]
            .as_array()
            .ok_or_else(|| "catalog asset outputFormats must be an array".to_owned())?
        {
            output.push_str(&format!(
                "{:?},",
                format
                    .as_str()
                    .ok_or_else(|| "output formats must be strings".to_owned())?
            ));
        }
        output.push_str("],\n        segments: &[\n");
        for segment in asset["segments"]
            .as_array()
            .ok_or_else(|| "catalog asset segments must be an array".to_owned())?
        {
            let shard = string(segment, "shard", "catalog segment")?;
            let shard_expr = match shard {
                "revision-1-000" => "excalidraw_render_assets_data_000::BYTES",
                "revision-1-001" => "excalidraw_render_assets_data_001::BYTES",
                other => return Err(format!("no data crate exists for shard {other}")),
            };
            output.push_str("            AssetSegment {\n");
            rust_field_indented(
                &mut output,
                "identity",
                string(segment, "identity", "catalog segment")?,
                16,
            );
            rust_field_indented(&mut output, "shard", shard, 16);
            output.push_str(&format!("                bytes: {shard_expr},\n"));
            for field in ["offset", "length", "order"] {
                output.push_str(&format!(
                    "                {field}: {},\n",
                    integer(segment, field, "catalog segment")?
                ));
            }
            rust_field_indented(
                &mut output,
                "sha256",
                string(segment, "sha256", "catalog segment")?,
                16,
            );
            output.push_str("            },\n");
        }
        output.push_str("        ],\n    },\n");
    }
    output.push_str("];\n\n");

    let fallback_ids = ids(scene_surface, "fallbackRecipes", "recipeId")?;
    let notice_ids = ids(scene_surface, "noticeCatalog", "id")?;
    let provenance_ids = catalog["provenanceIdentities"]
        .as_array()
        .ok_or_else(|| "catalog provenanceIdentities must be an array".to_owned())?
        .iter()
        .map(|entry| string(entry, "id", "catalog provenance").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    rust_str_array(&mut output, "FALLBACK_RECIPE_IDENTITIES", &fallback_ids);
    rust_str_array(&mut output, "NOTICE_IDENTITIES", &notice_ids);
    rust_str_array(&mut output, "PROVENANCE_IDENTITIES", &provenance_ids);
    rust_str_array(
        &mut output,
        "TRANSFORMATION_IDENTITIES",
        &["identity.byte-exact.v1".to_owned()],
    );
    rust_str_array(
        &mut output,
        "LICENSE_IDENTITIES",
        &license_identities()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    );
    output.push_str("\npub static REVISION_1: RevisionCatalog = RevisionCatalog {\n");
    output.push_str("    compatibility_revision: 1,\n");
    rust_field(&mut output, "excalidraw_release", EXCALIDRAW_RELEASE);
    rust_field(&mut output, "source_commit", SOURCE_COMMIT);
    rust_field(&mut output, "npm_integrity", NPM_INTEGRITY);
    rust_field(
        &mut output,
        "reference_profile_digest",
        &format!("sha256:{PROFILE_DIGEST}"),
    );
    rust_field(&mut output, "catalog_sha256", &sha256(catalog_bytes));
    output.push_str("    catalog_json: include_bytes!(\"../../catalog/revision-1.json\"),\n");
    output.push_str("    incorporated_artifacts: &[\n");
    for artifact in ARTIFACTS {
        output.push_str("        IncorporatedArtifact {\n");
        rust_field_indented(&mut output, "repository_path", artifact.repository_path, 12);
        rust_field_indented(&mut output, "sha256", artifact.digest, 12);
        output.push_str(&format!(
            "            bytes: include_bytes!(\"../../catalog/incorporated/{}\"),\n",
            artifact.packaged_name
        ));
        output.push_str("        },\n");
    }
    output.push_str("    ],\n");
    output.push_str("    fallback_recipe_identities: FALLBACK_RECIPE_IDENTITIES,\n");
    output.push_str("    notice_identities: NOTICE_IDENTITIES,\n");
    output.push_str("    provenance_identities: PROVENANCE_IDENTITIES,\n");
    output.push_str("    transformation_identities: TRANSFORMATION_IDENTITIES,\n");
    output.push_str("    license_identities: LICENSE_IDENTITIES,\n");
    output.push_str("    assets: &ASSETS,\n");
    output.push_str("};\n");
    Ok(output)
}

fn generated_scene_surface(scene_surface: &Value) -> Result<String, String> {
    let mut output = String::from(
        "// Generated by `cargo run --locked -p xtask -- catalog import`; do not edit.\n\n",
    );
    generate_scene_surface_rust(&mut output, scene_surface)?;
    Ok(output)
}

fn generate_scene_surface_rust(output: &mut String, scene_surface: &Value) -> Result<(), String> {
    output.push_str("#[rustfmt::skip]\nstatic SCENE_RECORD_POLICIES: &[SceneRecordPolicy] = &[\n");
    for record in scene_surface["recordKinds"]
        .as_array()
        .ok_or_else(|| "scene-surface recordKinds must be an array".to_owned())?
    {
        output.push_str("    SceneRecordPolicy {\n");
        rust_field(
            output,
            "identity",
            string(record, "id", "scene record kind")?,
        );
        rust_string_values(
            output,
            "field_paths",
            &record["fieldPaths"],
            "scene record fieldPaths",
        )?;
        rust_string_values(
            output,
            "inherits",
            &record["inherits"],
            "scene record inherits",
        )?;
        output.push_str("    },\n");
    }
    output.push_str("];\n\n");

    output.push_str("#[rustfmt::skip]\nstatic SCENE_FIELD_POLICIES: &[SceneFieldPolicy] = &[\n");
    for (ordinal, field) in scene_surface["fieldDispositions"]
        .as_array()
        .ok_or_else(|| "scene-surface fieldDispositions must be an array".to_owned())?
        .iter()
        .enumerate()
    {
        let disposition = match string(field, "disposition", "scene field disposition")? {
            "supported" => "SceneFieldDisposition::Supported",
            "static-export-irrelevant" => "SceneFieldDisposition::StaticExportIrrelevant",
            "opaque-metadata" => "SceneFieldDisposition::OpaqueMetadata",
            other => return Err(format!("unknown scene field disposition {other}")),
        };
        output.push_str("    SceneFieldPolicy {\n");
        for (rust_name, json_name) in [
            ("record_kind", "recordKind"),
            ("path", "path"),
            ("accepted_domain", "acceptedDomain"),
        ] {
            rust_field(
                output,
                rust_name,
                string(field, json_name, "scene field disposition")?,
            );
        }
        output.push_str(&format!("        disposition: {disposition},\n"));
        output.push_str(&format!(
            "        required: {},\n",
            field["required"]
                .as_bool()
                .ok_or_else(|| "scene field required must be a boolean".to_owned())?
        ));
        rust_field(
            output,
            "normalization",
            string(field, "normalization", "scene field disposition")?,
        );
        rust_field(
            output,
            "participation",
            string(&field["scope"], "participation", "scene field scope")?,
        );
        for name in ["formats", "options", "states"] {
            rust_string_values(output, name, &field["scope"][name], "scene field scope")?;
        }
        generate_scene_field_fallback(output, "missing", &field["missing"])?;
        generate_scene_field_fallback(output, "invalid", &field["invalid"])?;
        output.push_str("        value_rules: &[\n");
        for rule in field["valueRules"]
            .as_array()
            .ok_or_else(|| "scene field valueRules must be an array".to_owned())?
        {
            output.push_str("            SceneFieldValueRule {\n");
            generate_optional_rust_string(output, "code", &rule["code"], 16)?;
            generate_optional_rust_string(output, "recovery", &rule["recovery"], 16)?;
            rust_json_field(output, "value_json", &rule["value"], 16)?;
            rust_json_field(output, "format_rules_json", &rule["formatRules"], 16)?;
            rust_field_indented(
                output,
                "when",
                string(rule, "when", "scene field value rule")?,
                16,
            );
            output.push_str("            },\n");
        }
        output.push_str("        ],\n");
        output.push_str(&format!("        ordinal: {ordinal},\n"));
        output.push_str("    },\n");
    }
    output.push_str("];\n\n");

    output
        .push_str("#[rustfmt::skip]\nstatic SCENE_ELEMENT_POLICIES: &[SceneElementPolicy] = &[\n");
    for element in scene_surface["elementKinds"]
        .as_array()
        .ok_or_else(|| "scene-surface elementKinds must be an array".to_owned())?
    {
        output.push_str("    SceneElementPolicy {\n");
        for (rust_name, json_name) in [
            ("kind", "kind"),
            ("record_kind", "recordKind"),
            ("status", "status"),
        ] {
            rust_field(
                output,
                rust_name,
                string(element, json_name, "scene element kind")?,
            );
        }
        rust_string_values(
            output,
            "ignored_inherited_paths",
            &element["ignoredInheritedPaths"],
            "scene element ignoredInheritedPaths",
        )?;
        output.push_str("    },\n");
    }
    output.push_str("];\n\n");

    rust_str_array(
        output,
        "SCENE_DIAGNOSTIC_CODES",
        &ids(scene_surface, "registryMappings", "code")?,
    );
    output.push_str(
        "pub(crate) static SCENE_SURFACE_POLICY: SceneSurfacePolicy = SceneSurfacePolicy {\n",
    );
    output.push_str("    record_kinds: SCENE_RECORD_POLICIES,\n");
    output.push_str("    fields: SCENE_FIELD_POLICIES,\n");
    output.push_str("    element_kinds: SCENE_ELEMENT_POLICIES,\n");
    output.push_str("    diagnostic_codes: SCENE_DIAGNOSTIC_CODES,\n");
    output.push_str("};\n\n");
    Ok(())
}

fn generate_scene_field_fallback(
    output: &mut String,
    name: &str,
    fallback: &Value,
) -> Result<(), String> {
    output.push_str(&format!("        {name}: SceneFieldFallback {{\n"));
    rust_field_indented(
        output,
        "action",
        string(fallback, "action", "scene field fallback")?,
        12,
    );
    for field in ["code", "recovery", "recipe"] {
        generate_optional_rust_string(output, field, &fallback[field], 12)?;
    }
    rust_string_values(
        output,
        "effects",
        &fallback["effects"],
        "scene field fallback effects",
    )?;
    rust_json_field(output, "value_json", &fallback["value"], 12)?;
    rust_json_field(output, "format_rules_json", &fallback["formatRules"], 12)?;
    output.push_str("            format_recoveries: [");
    for format in ["svg", "png", "pdf"] {
        match fallback["formatRules"][format]["recovery"].as_str() {
            Some(recovery) => output.push_str(&format!("Some({recovery:?}),")),
            None => output.push_str("None,"),
        }
    }
    output.push_str("],\n");
    rust_field_indented(
        output,
        "when",
        string(fallback, "when", "scene field fallback")?,
        12,
    );
    output.push_str("        },\n");
    Ok(())
}

fn generate_optional_rust_string(
    output: &mut String,
    name: &str,
    value: &Value,
    indent: usize,
) -> Result<(), String> {
    output.push_str(&format!("{:indent$}{name}: ", ""));
    match value.as_str() {
        Some(value) => output.push_str(&format!("Some({value:?}),\n")),
        None if value.is_null() => output.push_str("None,\n"),
        None => return Err(format!("scene policy {name} must be string or null")),
    }
    Ok(())
}

fn rust_json_field(
    output: &mut String,
    name: &str,
    value: &Value,
    indent: usize,
) -> Result<(), String> {
    let value = serde_json::to_string(value)
        .map_err(|error| format!("failed to serialize scene policy {name}: {error}"))?;
    rust_field_indented(output, name, &value, indent);
    Ok(())
}

fn verify_catalog_schema(base: &Path, catalog: &Value) -> Result<(), String> {
    let schema_path = base.join("catalog/revision-catalog-v1.schema.json");
    let schema_bytes = fs::read(&schema_path)
        .map_err(|error| format!("failed to read {}: {error}", schema_path.display()))?;
    verify_canonical_json_bytes(&schema_bytes, "revision catalog schema")?;
    verify_digest_sidecar(
        &base.join("catalog/revision-catalog-v1.schema.sha256"),
        &schema_bytes,
    )?;
    if sha256(&schema_bytes) != CATALOG_SCHEMA_DIGEST {
        return Err("revision catalog schema digest is not pinned".to_owned());
    }
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("invalid revision catalog schema: {error}"))?;
    jsonschema::draft202012::meta::validate(&schema)
        .map_err(|error| format!("revision catalog schema is invalid: {error}"))?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| format!("revision catalog schema could not be compiled: {error}"))?;
    validator
        .validate(catalog)
        .map_err(|error| format!("revision catalog does not satisfy its schema: {error}"))
}

fn verify_font_subset_qualification_corpus(root: &Path, catalog: &Value) -> Result<(), String> {
    const CLASSES: [&str; 8] = [
        "mapped-scalar",
        "range-boundary",
        "empty-intersection",
        "full-intersection",
        "non-bmp",
        "combining-mark",
        "composite-closure",
        "layout-closure-heavy",
    ];
    let corpus_path = root.join(
        "tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-qualification-corpus.json",
    );
    let packaged_corpus_path =
        root.join("crates/excalidraw-render/tests/qualification/font-subset-corpus.json");
    let schema_path = root.join(
        "tests/oracle/compatibility-artifacts/svg-font-subset-qualification-corpus-v1.schema.json",
    );
    let corpus_bytes = fs::read(&corpus_path)
        .map_err(|error| format!("failed to read {}: {error}", corpus_path.display()))?;
    let packaged_corpus_bytes = fs::read(&packaged_corpus_path).map_err(|error| {
        format!(
            "failed to read packaged corpus copy {}: {error}",
            packaged_corpus_path.display()
        )
    })?;
    let schema_bytes = fs::read(&schema_path)
        .map_err(|error| format!("failed to read {}: {error}", schema_path.display()))?;
    verify_canonical_json_bytes(&corpus_bytes, "font subset qualification corpus")?;
    if packaged_corpus_bytes != corpus_bytes {
        return Err("packaged font subset qualification corpus copy is stale".to_owned());
    }
    verify_digest_sidecar(&corpus_path.with_extension("sha256"), &corpus_bytes)?;
    verify_digest_sidecar(&schema_path.with_extension("sha256"), &schema_bytes)?;
    if sha256(&corpus_bytes) != SVG_FONT_SUBSET_QUALIFICATION_CORPUS_DIGEST
        || sha256(&schema_bytes) != SVG_FONT_SUBSET_QUALIFICATION_SCHEMA_DIGEST
    {
        return Err("font subset qualification artifact digest is not pinned".to_owned());
    }
    let corpus: Value = serde_json::from_slice(&corpus_bytes)
        .map_err(|error| format!("invalid font subset qualification corpus: {error}"))?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("invalid font subset qualification schema: {error}"))?;
    jsonschema::draft202012::meta::validate(&schema)
        .map_err(|error| format!("font subset qualification schema is invalid: {error}"))?;
    jsonschema::draft202012::options()
        .build(&schema)
        .map_err(|error| {
            format!("font subset qualification schema could not be compiled: {error}")
        })?
        .validate(&corpus)
        .map_err(|error| {
            format!("font subset qualification corpus does not satisfy its schema: {error}")
        })?;

    if corpus["schema"] != json!("excalidraw-render/svg-font-subset-qualification-corpus/v1")
        || corpus["compatibilityRevision"] != json!(REVISION)
        || corpus["originatingIssue"] != json!(98)
        || corpus["sourceManifestSha256"] != json!(ASSET_MANIFEST_DIGEST)
        || corpus["subsetProfileSha256"]
            != json!("c79d0f7905709f5177a2ec8cd502be2eed9dd0c35806dfae06642e6cf4dd4ff5")
    {
        return Err("font subset qualification corpus identity is invalid".to_owned());
    }
    let classes = array_at(
        &corpus,
        &["classes"],
        "font subset qualification corpus classes",
    )?;
    if classes.len() != CLASSES.len()
        || classes
            .iter()
            .zip(CLASSES)
            .any(|(class, expected)| class["id"] != json!(expected))
    {
        return Err("font subset qualification classes are incomplete or unordered".to_owned());
    }
    let assets = array_at(catalog, &["assets"], "revision catalog assets")?;
    let sources = array_at(
        &corpus,
        &["sources"],
        "font subset qualification corpus sources",
    )?;
    if sources.len() != 230 || sources.len() != assets.len() {
        return Err("font subset qualification source inventory is incomplete".to_owned());
    }
    let mut source_ids = BTreeSet::new();
    let mut cell_ids = BTreeSet::new();
    for (source, asset) in sources.iter().zip(assets) {
        let source_id = string(source, "sourceIdentity", "font subset source")?;
        if source_id != string(asset, "identity", "catalog asset")?
            || source["upstreamPath"] != asset["upstreamPath"]
            || source["sourceSha256"] != asset["completeSha256"]
            || !source_ids.insert(source_id)
        {
            return Err(
                "font subset qualification source identity is stale or duplicated".to_owned(),
            );
        }
        let cells = array_at(source, &["cells"], "font subset source cells")?;
        if cells.len() != CLASSES.len() {
            return Err(format!(
                "font subset qualification source {source_id} has missing cells"
            ));
        }
        for (cell, expected_class) in cells.iter().zip(CLASSES) {
            let class = string(cell, "class", "font subset qualification cell")?;
            if class != expected_class || !cell_ids.insert((source_id, class)) {
                return Err("font subset qualification cell is stale or duplicated".to_owned());
            }
            let status = string(cell, "status", "font subset qualification cell")?;
            let has = |name: &str| cell.get(name).is_some();
            let valid = match status {
                "success" => {
                    cell["expectedValidationOutcome"] == json!("valid")
                        && has("orderedScalars")
                        && has("semanticSubsetIdentity")
                        && has("generatedWoff2Sha256")
                        && has("canonicalResourceTraceSha256")
                        && !has("reason")
                }
                "not_selected" => {
                    cell["expectedValidationOutcome"] == json!("not_selected")
                        && has("orderedScalars")
                        && !has("semanticSubsetIdentity")
                        && !has("generatedWoff2Sha256")
                        && has("canonicalResourceTraceSha256")
                        && !has("reason")
                }
                "not_applicable" => {
                    cell["expectedValidationOutcome"] == json!("not_applicable")
                        && !has("orderedScalars")
                        && !has("semanticSubsetIdentity")
                        && !has("generatedWoff2Sha256")
                        && !has("canonicalResourceTraceSha256")
                        && has("reason")
                }
                _ => false,
            };
            if !valid {
                return Err(format!(
                    "font subset qualification cell {source_id}/{class} has inconsistent evidence"
                ));
            }
        }
    }
    if source_ids.len() != 230 || cell_ids.len() != 230 * CLASSES.len() {
        return Err("font subset qualification coverage is incomplete".to_owned());
    }
    Ok(())
}

fn verify_catalog_identity(catalog: &Value, scene_surface: &Value) -> Result<(), String> {
    if catalog["schema"] != json!(CATALOG_SCHEMA)
        || catalog["compatibilityRevision"] != json!(REVISION)
        || catalog["excalidrawRelease"] != json!(EXCALIDRAW_RELEASE)
        || catalog["sourceCommit"] != json!(SOURCE_COMMIT)
        || catalog["npmIntegrity"] != json!(NPM_INTEGRITY)
        || catalog["referenceProfileDigest"] != json!(format!("sha256:{PROFILE_DIGEST}"))
    {
        return Err("revision catalog compatibility identity is stale".to_owned());
    }
    if catalog["fallbackRecipeIdentities"]
        != json!(ids(scene_surface, "fallbackRecipes", "recipeId")?)
        || catalog["noticeIdentities"] != json!(ids(scene_surface, "noticeCatalog", "id")?)
    {
        return Err("revision catalog fallback or notice index is stale".to_owned());
    }
    Ok(())
}

fn verify_assets(root: &Path, catalog: &Value) -> Result<Vec<u8>, String> {
    let reference = read_canonical_json(
        &root.join("tests/oracle/reference-profiles/revision-1-assets.json"),
        "reference asset manifest",
    )?;
    let expected = array_at(
        &reference,
        &["officialPackage", "assets"],
        "official package assets",
    )?
    .iter()
    .map(|entry| {
        Ok((
            string(entry, "path", "reference asset")?.to_owned(),
            (
                integer(entry, "bytes", "reference asset")? as usize,
                string(entry, "sha256", "reference asset")?.to_owned(),
            ),
        ))
    })
    .collect::<Result<BTreeMap<_, _>, String>>()?;
    let assets = catalog["assets"]
        .as_array()
        .ok_or_else(|| "revision catalog assets must be an array".to_owned())?;
    if assets.len() != 230 || expected.len() != 230 {
        return Err("revision catalog must contain all 230 official assets".to_owned());
    }
    let licenses = catalog["licenseIdentities"]
        .as_array()
        .ok_or_else(|| "license identities must be an array".to_owned())?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let transformations = catalog["transformationIdentities"]
        .as_array()
        .ok_or_else(|| "transformation identities must be an array".to_owned())?
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeSet::new();
    let mut shard_cache = BTreeMap::<String, Vec<u8>>::new();
    let mut covered_ends = BTreeMap::<String, usize>::new();
    let mut excalifont_source = None;
    for asset in assets {
        let path = string(asset, "upstreamPath", "catalog asset")?;
        if !paths.insert(path) || path.starts_with("fonts/Assistant/") {
            return Err(format!("duplicate or excluded catalog asset {path}"));
        }
        let Some((expected_length, expected_digest)) = expected.get(path) else {
            return Err(format!(
                "catalog asset is absent from the reference manifest: {path}"
            ));
        };
        if integer(asset, "completeLength", "catalog asset")? as usize != *expected_length
            || string(asset, "completeSha256", "catalog asset")? != expected_digest
            || !licenses.contains(string(asset, "licenseIdentity", "catalog asset")?)
            || !transformations.contains(string(asset, "transformationIdentity", "catalog asset")?)
        {
            return Err(format!("catalog metadata is stale for {path}"));
        }
        let segments = asset["segments"]
            .as_array()
            .ok_or_else(|| format!("catalog asset {path} has no segments"))?;
        let mut complete = Vec::new();
        for (order, segment) in segments.iter().enumerate() {
            if integer(segment, "order", "asset segment")? as usize != order {
                return Err(format!("catalog asset {path} has unordered segments"));
            }
            let shard = string(segment, "shard", "asset segment")?;
            let bytes = if let Some(bytes) = shard_cache.get(shard) {
                bytes
            } else {
                let suffix = shard
                    .strip_prefix("revision-1-")
                    .ok_or_else(|| format!("unknown shard identity {shard}"))?;
                let shard_path = root.join(format!(
                    "crates/excalidraw-render-assets-data-{suffix}/data/{shard}.bin"
                ));
                let bytes = fs::read(&shard_path)
                    .map_err(|error| format!("failed to read {}: {error}", shard_path.display()))?;
                if bytes.len() > RAW_SHARD_LIMIT {
                    return Err(format!("asset shard {shard} exceeds 8 MiB"));
                }
                shard_cache.insert(shard.to_owned(), bytes);
                shard_cache.get(shard).unwrap()
            };
            let offset = integer(segment, "offset", "asset segment")? as usize;
            let length = integer(segment, "length", "asset segment")? as usize;
            let end = offset
                .checked_add(length)
                .ok_or_else(|| format!("asset segment overflows for {path}"))?;
            let covered_end = covered_ends.entry(shard.to_owned()).or_default();
            if offset != *covered_end {
                return Err(format!(
                    "asset shard {shard} has a gap, overlap, or reordered segment at {path}"
                ));
            }
            *covered_end = end;
            let segment_bytes = bytes
                .get(offset..end)
                .ok_or_else(|| format!("asset segment is outside shard for {path}"))?;
            if sha256(segment_bytes) != string(segment, "sha256", "asset segment")? {
                return Err(format!("asset segment digest mismatch for {path}"));
            }
            complete.extend_from_slice(segment_bytes);
        }
        if complete.len() != *expected_length || sha256(&complete) != *expected_digest {
            return Err(format!("reassembled asset digest mismatch for {path}"));
        }
        if path == EXCALIFONT_BASIC_LATIN_PATH {
            excalifont_source = Some(complete);
        }
    }
    if paths != expected.keys().map(String::as_str).collect() {
        return Err("revision catalog asset inventory is incomplete".to_owned());
    }
    for (shard, bytes) in &shard_cache {
        if covered_ends.get(shard).copied() != Some(bytes.len()) {
            return Err(format!(
                "asset shard {shard} contains unmanifested or uncovered bytes"
            ));
        }
    }
    excalifont_source.ok_or_else(|| "manifested Excalifont source is missing".to_owned())
}

fn verify_scene_surface(root: &Path, manifest: &Value) -> Result<(), String> {
    let schema_bytes = read_verified(
        &root.join("tests/oracle/scene-surfaces/scene-surface-v1.schema.json"),
        SCENE_SURFACE_SCHEMA_DIGEST,
        "scene-surface schema",
    )?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("invalid scene-surface schema: {error}"))?;
    jsonschema::draft202012::meta::validate(&schema)
        .map_err(|error| format!("scene-surface schema is invalid: {error}"))?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| format!("scene-surface schema could not be compiled: {error}"))?;
    validator
        .validate(manifest)
        .map_err(|error| format!("scene-surface manifest violates its schema: {error}"))?;

    require_len(manifest, "sourceCatalog", SOURCE_COUNT)?;
    require_len(manifest, "fieldDispositions", FIELD_DISPOSITION_COUNT)?;
    require_len(manifest, "elementKinds", ELEMENT_KIND_COUNT)?;
    require_len(manifest, "fallbackRecipes", FALLBACK_RECIPE_COUNT)?;
    require_len(manifest, "noticeCatalog", NOTICE_COUNT)?;
    require_len(manifest, "registryMappings", REGISTRY_MAPPING_COUNT)?;
    if manifest["coverage"]["status"] != json!("complete")
        || manifest["coverage"]["unresolved"] != json!([])
    {
        return Err("scene-surface coverage is not complete".to_owned());
    }

    let sources = unique_ids(manifest, "sourceCatalog", "id")?;
    let evidence = unique_ids(manifest, "evidenceCatalog", "id")?;
    let notices = unique_ids(manifest, "noticeCatalog", "id")?;
    let recipes = unique_ids(manifest, "fallbackRecipes", "recipeId")?;
    let registry_codes = unique_ids(manifest, "registryMappings", "code")?;
    let field_identities =
        unique_composite_ids(manifest, "fieldDispositions", "recordKind", "path")?;
    let record_kinds = unique_ids(manifest, "recordKinds", "id")?;
    let element_kinds = unique_ids(manifest, "elementKinds", "kind")?;
    verify_scene_surface_coverage(
        manifest,
        &field_identities,
        &record_kinds,
        &element_kinds,
        &registry_codes,
    )?;
    let renderer_recipes = manifest["rendererOwnedRecipeReferences"]
        .as_array()
        .ok_or_else(|| "renderer-owned recipe references must be an array".to_owned())?
        .iter()
        .filter_map(|entry| entry["recipeId"].as_str())
        .collect::<BTreeSet<_>>();
    verify_references(
        manifest,
        &sources,
        &evidence,
        &notices,
        &recipes,
        &renderer_recipes,
    )?;

    let domains = manifest["domainCatalog"]
        .as_object()
        .ok_or_else(|| "domain catalog must be an object".to_owned())?
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let implemented_domains = IMPLEMENTED_DOMAIN_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if implemented_domains.len() != IMPLEMENTED_DOMAIN_IDS.len() || implemented_domains != domains {
        return Err(
            "scene-surface domains have missing, unknown, or duplicate implementation ownership"
                .to_owned(),
        );
    }
    for source in manifest["sourceCatalog"].as_array().unwrap() {
        let id = string(source, "id", "source")?;
        let url = string(source, "url", "source")?;
        let revision = string(source, "immutableRevision", "source")?;
        if !url.starts_with("https://") || revision.is_empty() {
            return Err(format!("source {id} lacks an immutable HTTPS pointer"));
        }
    }
    for evidence_record in manifest["evidenceCatalog"].as_array().unwrap() {
        if evidence_record["kind"] == json!("approved-compatibility-exception") {
            let decision_sources = evidence_record["sourceIds"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|id| {
                    manifest["sourceCatalog"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|source| source["id"] == json!(id))
                })
                .filter(|source| source["kind"] == json!("project-decision"))
                .collect::<Vec<_>>();
            if decision_sources.len() != 1
                || evidence_record["digest"]
                    != json!(format!(
                        "sha256:{}",
                        string(decision_sources[0], "contentSha256", "decision source")?
                    ))
            {
                return Err(format!(
                    "compatibility exception {} is not bound to one immutable decision",
                    string(evidence_record, "id", "evidence")?
                ));
            }
        }
    }
    for field in manifest["fieldDispositions"].as_array().unwrap() {
        let identity = format!(
            "{}:{}",
            string(field, "recordKind", "field disposition")?,
            string(field, "path", "field disposition")?
        );
        let domain = string(field, "acceptedDomain", "field disposition")?;
        if !domains.contains(domain) {
            return Err(format!("{identity} references unknown domain {domain}"));
        }
        if field["harmlessNewerBehavior"] == json!(true)
            && field["evidenceIds"].as_array().is_none_or(Vec::is_empty)
        {
            return Err(format!(
                "{identity} classifies harmless behavior without evidence"
            ));
        }
        for selected in [&field["missing"], &field["invalid"]] {
            verify_diagnostic_and_recipe(
                selected,
                &identity,
                &registry_codes,
                &recipes,
                &renderer_recipes,
            )?;
        }
        for rule in field["valueRules"].as_array().unwrap() {
            verify_diagnostic_and_recipe(
                rule,
                &identity,
                &registry_codes,
                &recipes,
                &renderer_recipes,
            )?;
        }
    }
    for rule in manifest["normalizationRules"].as_array().unwrap() {
        if let Some(code) = rule["code"].as_str() {
            if !registry_codes.contains(code) {
                return Err(format!(
                    "normalization rule references unmapped diagnostic {code}"
                ));
            }
        }
    }

    for notice in manifest["noticeCatalog"].as_array().unwrap() {
        let text = string(notice, "text", "notice")?;
        if sha256(text.as_bytes()) != string(notice, "textSha256", "notice")? {
            return Err(format!(
                "notice text digest is stale for {}",
                string(notice, "id", "notice")?
            ));
        }
    }
    for recipe in manifest["fallbackRecipes"].as_array().unwrap() {
        let content = serde_json::to_vec(&recipe["content"])
            .map_err(|error| format!("failed to canonicalize fallback recipe: {error}"))?;
        let digest = sha256(&content);
        let recipe_id = string(recipe, "recipeId", "fallback recipe")?;
        if string(recipe, "contentDigest", "fallback recipe")? != format!("sha256:{digest}")
            || !recipe_id.ends_with(&format!("sha256:{digest}"))
        {
            return Err(format!("fallback recipe digest is stale for {recipe_id}"));
        }
    }
    for recipe in &recipes {
        let reachable = manifest["registryMappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mapping| {
                let selection = &mapping["recipeSelection"];
                selection
                    .as_str()
                    .is_some_and(|value| value.contains(recipe))
                    || selection.as_array().is_some_and(|values| {
                        values.iter().any(|value| value.as_str() == Some(recipe))
                    })
            });
        if !reachable {
            return Err(format!("fallback recipe {recipe} has no registry mapping"));
        }
    }
    verify_recoveries_and_format_rules(&manifest["registryMappings"], "registryMappings")?;
    verify_recoveries_and_format_rules(&manifest["fieldDispositions"], "fieldDispositions")?;
    if manifest["rendererOwnedRecipeReferences"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reference| reference["definitionAuthority"] != json!("outcomes"))
    {
        return Err("renderer-owned recipes must reference the outcomes authority".to_owned());
    }
    let evidence_index = read_canonical_json(
        &root.join("tests/oracle/scene-surfaces/evidence/revision-1-harmless-fields.json"),
        "scene-surface evidence index",
    )?;
    if evidence_index["cases"]
        .as_array()
        .is_none_or(|cases| cases.len() != 7)
        || evidence_index["snapshots"]
            .as_array()
            .is_none_or(|snapshots| snapshots.len() != 5)
    {
        return Err("scene-surface focused evidence inventory is incomplete".to_owned());
    }
    Ok(())
}

fn verify_scene_surface_coverage(
    manifest: &Value,
    field_identities: &BTreeSet<String>,
    record_kinds: &BTreeSet<&str>,
    element_kinds: &BTreeSet<&str>,
    registry_codes: &BTreeSet<&str>,
) -> Result<(), String> {
    let coverage_records = strings(&manifest["coverage"]["recordKinds"], "coverage.recordKinds")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if coverage_records != *record_kinds {
        return Err(
            "scene-surface record-kind coverage has missing or unknown identities".to_owned(),
        );
    }
    let coverage_elements = strings(
        &manifest["coverage"]["elementKinds"],
        "coverage.elementKinds",
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    if coverage_elements != *element_kinds {
        return Err(
            "scene-surface element-kind coverage has missing or unknown identities".to_owned(),
        );
    }
    if manifest["coverage"]["fieldDispositionCount"].as_u64()
        != Some(u64::try_from(field_identities.len()).unwrap_or(u64::MAX))
    {
        return Err("scene-surface field-disposition coverage count is stale".to_owned());
    }

    let records = manifest["recordKinds"]
        .as_array()
        .ok_or_else(|| "recordKinds must be an array".to_owned())?;
    let record_index = records
        .iter()
        .map(|record| Ok((string(record, "id", "record kind")?, record)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    fn visit_record<'a>(
        identity: &'a str,
        records: &BTreeMap<&'a str, &'a Value>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> Result<(), String> {
        if visited.contains(identity) {
            return Ok(());
        }
        if !visiting.insert(identity) {
            return Err(format!(
                "scene-surface record inheritance contains a cycle at {identity}"
            ));
        }
        let record = records
            .get(identity)
            .ok_or_else(|| format!("unknown inherited record kind {identity}"))?;
        for inherited in strings(&record["inherits"], "record kind inherits")? {
            visit_record(inherited, records, visiting, visited)?;
        }
        visiting.remove(identity);
        visited.insert(identity);
        Ok(())
    }
    let mut visited_records = BTreeSet::new();
    for identity in record_index.keys().copied() {
        visit_record(
            identity,
            &record_index,
            &mut BTreeSet::new(),
            &mut visited_records,
        )?;
    }
    let mut declared_fields = BTreeSet::new();
    for record in records {
        let identity = string(record, "id", "record kind")?;
        for inherited in strings(&record["inherits"], "record kind inherits")? {
            if !record_index.contains_key(inherited) {
                return Err(format!(
                    "record kind {identity} inherits unknown record {inherited}"
                ));
            }
        }
        let mut direct_paths = BTreeSet::new();
        for path in strings(&record["fieldPaths"], "record kind fieldPaths")? {
            if !direct_paths.insert(path) {
                return Err(format!(
                    "record kind {identity} owns field path {path} twice"
                ));
            }
            declared_fields.insert(format!("{identity}:{path}"));
        }
    }
    if declared_fields != *field_identities {
        return Err(
            "scene-surface fields have a missing disposition, unknown identity, or duplicate owner"
                .to_owned(),
        );
    }

    for element in manifest["elementKinds"].as_array().unwrap() {
        let kind = string(element, "kind", "element kind")?;
        let record_kind = string(element, "recordKind", "element kind")?;
        if !record_index.contains_key(record_kind) {
            return Err(format!(
                "element kind {kind} references unknown record kind {record_kind}"
            ));
        }
        let mut inherited_paths = BTreeSet::new();
        let mut pending = strings(
            &record_index[record_kind]["inherits"],
            "record kind inherits",
        )?;
        let mut visited = BTreeSet::new();
        while let Some(inherited) = pending.pop() {
            if !visited.insert(inherited) {
                continue;
            }
            let record = record_index[inherited];
            inherited_paths.extend(strings(
                &record["fieldPaths"],
                "inherited record fieldPaths",
            )?);
            pending.extend(strings(&record["inherits"], "record kind inherits")?);
        }
        let mut ignored = BTreeSet::new();
        for path in strings(
            &element["ignoredInheritedPaths"],
            "element kind ignoredInheritedPaths",
        )? {
            if !ignored.insert(path) || !inherited_paths.contains(path) {
                return Err(format!(
                    "element kind {kind} ignores duplicate or unknown inherited path {path}"
                ));
            }
        }
    }

    let implemented = IMPLEMENTED_DIAGNOSTIC_CODES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if implemented.len() != IMPLEMENTED_DIAGNOSTIC_CODES.len() || implemented != *registry_codes {
        return Err(
            "scene-surface diagnostic registry has missing, unknown, or duplicate implementation ownership"
                .to_owned(),
        );
    }
    Ok(())
}

fn verify_diagnostic_and_recipe<'a>(
    value: &'a Value,
    subject: &str,
    registry_codes: &BTreeSet<&'a str>,
    recipes: &BTreeSet<&'a str>,
    renderer_recipes: &BTreeSet<&'a str>,
) -> Result<(), String> {
    if let Some(code) = value["code"].as_str() {
        if !code.starts_with("scene.") && !registry_codes.contains(code) {
            return Err(format!("{subject} references unmapped diagnostic {code}"));
        }
    }
    if let Some(recipe) = value["recipe"].as_str() {
        if !recipes.contains(recipe) && !renderer_recipes.contains(recipe) {
            return Err(format!("{subject} references unknown recipe {recipe}"));
        }
    }
    Ok(())
}

fn verify_recoveries_and_format_rules(value: &Value, pointer: &str) -> Result<(), String> {
    const RECOVERIES: &[&str] = &[
        "repaired",
        "ignored",
        "placeholder",
        "omitted",
        "removed",
        "substituted",
        "flattened",
        "rasterized",
        "metadata_omitted",
    ];
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                verify_recoveries_and_format_rules(value, &format!("{pointer}/{index}"))?;
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                let member_pointer = format!("{pointer}/{key}");
                if key == "recovery" {
                    if let Some(recovery) = value.as_str() {
                        if !RECOVERIES.contains(&recovery) {
                            return Err(format!(
                                "{member_pointer} uses unknown recovery {recovery}"
                            ));
                        }
                    }
                }
                if key == "formatRules" {
                    if value.is_null() {
                        continue;
                    }
                    let rules = value
                        .as_object()
                        .ok_or_else(|| format!("{member_pointer} must be an object"))?;
                    if rules.is_empty()
                        || rules
                            .keys()
                            .any(|format| !matches!(format.as_str(), "svg" | "png" | "pdf"))
                    {
                        return Err(format!(
                            "{member_pointer} must contain at least one known format"
                        ));
                    }
                    for (format, branches) in rules {
                        let branches = branches
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or_else(|| std::slice::from_ref(branches));
                        for (index, branch) in branches.iter().enumerate() {
                            if branch["action"].as_str().is_none_or(str::is_empty) {
                                return Err(format!(
                                    "{member_pointer}/{format}/{index} has an empty action"
                                ));
                            }
                        }
                    }
                }
                verify_recoveries_and_format_rules(value, &member_pointer)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn verify_references<'a>(
    value: &'a Value,
    sources: &BTreeSet<&'a str>,
    evidence: &BTreeSet<&'a str>,
    notices: &BTreeSet<&'a str>,
    recipes: &BTreeSet<&'a str>,
    renderer_recipes: &BTreeSet<&'a str>,
) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                verify_references(value, sources, evidence, notices, recipes, renderer_recipes)?;
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                let expected = match key.as_str() {
                    "sourceId" | "sourceIds" => Some((sources, "source")),
                    "evidenceId" | "evidenceIds" => Some((evidence, "evidence")),
                    "noticeId" | "noticeIds" => Some((notices, "notice")),
                    _ => None,
                };
                if let Some((identities, kind)) = expected {
                    for identity in strings(value, key)? {
                        if !identities.contains(identity) {
                            return Err(format!("unknown {kind} reference {identity}"));
                        }
                    }
                }
                if key == "recipeId" && object.len() < 3 {
                    if let Some(identity) = value.as_str() {
                        if !recipes.contains(identity) && !renderer_recipes.contains(identity) {
                            return Err(format!("unknown fallback recipe reference {identity}"));
                        }
                    }
                }
                verify_references(value, sources, evidence, notices, recipes, renderer_recipes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn verify_frame_reference_profile(root: &Path, profile: &Value) -> Result<(), String> {
    if profile["schema"] != json!("excalidraw-render/frame-reference-profile/v1")
        || profile["compatibilityRevision"] != json!(REVISION)
        || profile["excalidrawRelease"] != json!(EXCALIDRAW_RELEASE)
        || profile["exportTarget"] != json!("frame")
        || profile["baseSceneReferenceProfile"]["digest"]
            != json!(format!("sha256:{PROFILE_DIGEST}"))
        || profile["baseSceneReferenceProfile"]["path"]
            != json!("tests/oracle/reference-profiles/revision-1.json")
    {
        return Err("frame reference profile identity is stale".to_owned());
    }
    let invocation = &profile["referenceInvocation"];
    if invocation["api"] != json!({"png": "exportToBlob", "svg": "exportToSvg"})
        || invocation["exportingFrame"] != json!("live source-order frame occurrence")
        || invocation["frameRendering"]
            != json!({"clip": true, "enabled": true, "name": false, "outline": false})
        || invocation["padding"] != json!(0)
        || invocation["sizing"]
            != json!("exactly one of exportScale or maxWidthOrHeight is non-null")
    {
        return Err("frame reference invocation is stale".to_owned());
    }

    let expected_reference_paths = BTreeSet::from([
        "tools/reference-oracle/frame-index.html",
        "tools/reference-oracle/package-lock.json",
        "tools/reference-oracle/run-frame-conformance.sh",
        "tools/reference-oracle/run-frame-profile.sh",
        "tools/reference-oracle/scripts/canonical-json.mjs",
        "tools/reference-oracle/scripts/frame-conformance-case.mjs",
        "tools/reference-oracle/scripts/frame-profile.mjs",
        "tools/reference-oracle/src/frame-browser-entry.js",
        "tools/reference-oracle/vite.frame.config.mjs",
    ]);
    let reference_files = profile["referenceFiles"]["files"]
        .as_array()
        .ok_or_else(|| "frame referenceFiles.files must be an array".to_owned())?;
    verify_frame_file_records(
        root,
        reference_files,
        &profile["referenceFiles"]["recordsSha256"],
        "frame reference file",
        true,
    )?;
    let actual_reference_paths = reference_files
        .iter()
        .map(|record| string(record, "path", "frame reference file"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual_reference_paths != expected_reference_paths {
        return Err("frame reference file set is stale".to_owned());
    }

    if profile["browserBundle"]["root"] != json!("tools/reference-oracle/frame-dist") {
        return Err("frame browser bundle root is stale".to_owned());
    }
    let bundle_files = profile["browserBundle"]["files"]
        .as_array()
        .ok_or_else(|| "frame browserBundle.files must be an array".to_owned())?;
    if bundle_files.is_empty() {
        return Err("frame browser bundle has no files".to_owned());
    }
    verify_frame_file_records(
        &root.join("tools/reference-oracle/frame-dist"),
        bundle_files,
        &profile["browserBundle"]["recordsSha256"],
        "frame browser bundle file",
        false,
    )
}

fn verify_frame_file_records(
    root: &Path,
    records: &[Value],
    expected_records_digest: &Value,
    description: &str,
    verify_files: bool,
) -> Result<(), String> {
    let records_bytes = serde_json::to_vec(records)
        .map_err(|error| format!("failed to canonicalize {description} records: {error}"))?;
    if expected_records_digest != &json!(sha256(&records_bytes)) {
        return Err(format!("{description} record digest is stale"));
    }
    let mut previous = None;
    for record in records {
        let path = string(record, "path", description)?;
        if previous.is_some_and(|previous| previous >= path) {
            return Err(format!(
                "{description} records are not uniquely path ordered"
            ));
        }
        previous = Some(path);
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!("{description} has an unsafe path {path:?}"));
        }
        let byte_length = integer(record, "bytes", description)?;
        let digest = string(record, "sha256", description)?;
        if byte_length == 0
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!("{description} {path} has an invalid byte record"));
        }
        if verify_files {
            let bytes = fs::read(root.join(relative))
                .map_err(|error| format!("failed to read {description} {path}: {error}"))?;
            if byte_length != bytes.len() as u64 || digest != sha256(&bytes) {
                return Err(format!("{description} {path} does not match its record"));
            }
        }
    }
    Ok(())
}

fn build_license_bundle(base: &Path, scene_surface: &Value) -> Result<String, String> {
    let mut bundle = String::from("Excalidraw Render License Bundle\nCompatibility revision 1\n\n");
    for (title, file) in LICENSE_PARTS {
        let path = if file.starts_with("../") {
            base.join(file)
        } else {
            base.join("license-parts").join(file)
        };
        let part = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        bundle.push_str(
            "==============================================================================\n",
        );
        bundle.push_str(title);
        bundle.push_str(
            "\n==============================================================================\n\n",
        );
        bundle.push_str(part.trim_end_matches(['\r', '\n']));
        bundle.push_str("\n\n");
    }
    bundle.push_str("==============================================================================\nOfficial rendering font notices\n==============================================================================\n\n");
    for (family, notice) in FONT_NOTICES {
        bundle.push_str(family);
        bundle.push('\n');
        bundle.push_str(notice);
        bundle.push_str("\n\n");
    }
    bundle.push_str("==============================================================================\nRevision 1 output-traveling notices\n==============================================================================\n\n");
    for notice in scene_surface["noticeCatalog"]
        .as_array()
        .ok_or_else(|| "notice catalog must be an array".to_owned())?
    {
        bundle.push_str(string(notice, "id", "notice")?);
        bundle.push('\n');
        bundle.push_str(string(notice, "text", "notice")?);
        bundle.push_str("\n\n");
    }
    bundle.truncate(bundle.trim_end_matches(['\r', '\n']).len());
    bundle.push('\n');
    Ok(bundle)
}

fn family_metadata(
    family: &str,
) -> Result<(&'static str, &'static str, &'static [&'static str]), String> {
    match family {
        "Cascadia" => Ok((
            "font.cascadia-code.ofl-1.1",
            "Copyright (c) 2019 - Present, Microsoft Corporation.",
            &["Cascadia Code"],
        )),
        "ComicShanns" => Ok((
            "font.comic-shanns.mit",
            "Copyright (c) 2018 Shannon Miwa.",
            &[],
        )),
        "Excalifont" => Ok((
            "font.excalifont.ofl-1.1",
            "Copyright (c) 2024 Excalidraw; Virgil by Your Own Font Foundry; modifications by Jan Filipek / DizajnDesign.",
            &[],
        )),
        "Liberation" => Ok((
            "font.liberation-sans.ofl-1.1",
            "Digitized data copyright (c) 2010 Google Corporation; Copyright (c) 2012 Red Hat, Inc.",
            &["Liberation"],
        )),
        "Lilita" => Ok((
            "font.lilita.ofl-1.1",
            "Copyright (c) 2011 Juan Montoreano (juan@remolacha.biz).",
            &["Lilita"],
        )),
        "Nunito" => Ok((
            "font.nunito.ofl-1.1",
            "Copyright 2014 The Nunito Project Authors (https://github.com/googlefonts/nunito).",
            &[],
        )),
        "Virgil" => Ok((
            "font.virgil.ofl-1.1",
            "Virgil by Your Own Font Foundry.",
            &["Virgil"],
        )),
        "Xiaolai" => Ok((
            "font.xiaolai.ofl-1.1",
            "Copyright (c) 2020 LXGW; designed by Nozomi Seto.",
            &[],
        )),
        _ => Err(format!("unknown official font family {family}")),
    }
}

fn license_identities() -> Vec<&'static str> {
    vec![
        "application.apache-2.0",
        "application.mit",
        "excalidraw.mit",
        "font.cascadia-code.ofl-1.1",
        "font.comic-shanns.mit",
        "font.excalifont.ofl-1.1",
        "font-awesome.icons.cc-by-4.0",
        "font.liberation-sans.ofl-1.1",
        "font.lilita.ofl-1.1",
        "font.noto-emoji.ofl-1.1",
        "font.nunito.ofl-1.1",
        "font.virgil.ofl-1.1",
        "font.xiaolai.ofl-1.1",
        "perfect-freehand.mit",
        "points-on-curve.mit",
        "roughjs.mit",
    ]
}

fn read_canonical_json(path: &Path, description: &str) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    verify_canonical_json_bytes(&bytes, description)?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid {description}: {error}"))
}

fn verify_canonical_json_bytes(bytes: &[u8], description: &str) -> Result<(), String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid {description} JSON: {error}"))?;
    let canonical = serde_json::to_vec(&value)
        .map_err(|error| format!("failed to canonicalize {description}: {error}"))?;
    if canonical != bytes {
        return Err(format!("{description} is not canonical JSON"));
    }
    Ok(())
}

fn read_verified(path: &Path, expected_digest: &str, description: &str) -> Result<Vec<u8>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if sha256(&bytes) != expected_digest {
        return Err(format!("{description} digest does not match its authority"));
    }
    Ok(bytes)
}

fn verify_authoritative_sidecar(
    root: &Path,
    repository_path: &str,
    expected_digest: &str,
) -> Result<(), String> {
    if repository_path.ends_with("scene-surface-v1.schema.json") {
        return Ok(());
    }
    let sidecar_path = root.join(repository_path).with_extension("sha256");
    let sidecar = fs::read_to_string(&sidecar_path)
        .map_err(|error| format!("failed to read {}: {error}", sidecar_path.display()))?;
    if sidecar != format!("{expected_digest}\n") {
        return Err(format!(
            "authoritative digest sidecar is stale: {}",
            sidecar_path.display()
        ));
    }
    Ok(())
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn write_digest_sidecar(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_if_changed(path, format!("{}\n", sha256(bytes)).as_bytes())
}

fn verify_digest_sidecar(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let sidecar = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if sidecar != format!("{}\n", sha256(bytes)) {
        return Err(format!("digest sidecar is stale: {}", path.display()));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn array_at<'a>(value: &'a Value, path: &[&str], name: &str) -> Result<&'a [Value], String> {
    let mut current = value;
    for key in path {
        current = &current[*key];
    }
    current
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{name} must be an array"))
}

fn string<'a>(value: &'a Value, key: &str, name: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("{name}.{key} must be a string"))
}

fn integer(value: &Value, key: &str, name: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("{name}.{key} must be a nonnegative integer"))
}

fn ids(value: &Value, array: &str, field: &str) -> Result<Vec<String>, String> {
    value[array]
        .as_array()
        .ok_or_else(|| format!("{array} must be an array"))?
        .iter()
        .map(|entry| string(entry, field, array).map(str::to_owned))
        .collect()
}

fn unique_ids<'a>(value: &'a Value, array: &str, field: &str) -> Result<BTreeSet<&'a str>, String> {
    let values = value[array]
        .as_array()
        .ok_or_else(|| format!("{array} must be an array"))?;
    let identities = values
        .iter()
        .map(|entry| string(entry, field, array))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if identities.len() != values.len() {
        return Err(format!("{array} contains duplicate {field} values"));
    }
    Ok(identities)
}

fn unique_composite_ids(
    value: &Value,
    array: &str,
    first: &str,
    second: &str,
) -> Result<BTreeSet<String>, String> {
    let values = value[array]
        .as_array()
        .ok_or_else(|| format!("{array} must be an array"))?;
    let identities = values
        .iter()
        .map(|entry| {
            Ok(format!(
                "{}:{}",
                string(entry, first, array)?,
                string(entry, second, array)?
            ))
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if identities.len() != values.len() {
        return Err(format!(
            "{array} contains duplicate {first}/{second} identities"
        ));
    }
    Ok(identities)
}

fn strings<'a>(value: &'a Value, name: &str) -> Result<Vec<&'a str>, String> {
    if let Some(value) = value.as_str() {
        return Ok(vec![value]);
    }
    value
        .as_array()
        .ok_or_else(|| format!("{name} must be a string or string array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{name} entries must be strings"))
        })
        .collect()
}

fn require_len(value: &Value, field: &str, expected: usize) -> Result<(), String> {
    let actual = value[field]
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?
        .len();
    if actual != expected {
        return Err(format!(
            "{field} must contain {expected} entries, found {actual}"
        ));
    }
    Ok(())
}

fn rust_field(output: &mut String, name: &str, value: &str) {
    rust_field_indented(output, name, value, 8);
}

fn rust_field_indented(output: &mut String, name: &str, value: &str, indent: usize) {
    output.push_str(&format!("{:indent$}{name}: {value:?},\n", ""));
}

fn rust_string_values(
    output: &mut String,
    field: &str,
    values: &Value,
    subject: &str,
) -> Result<(), String> {
    let values = values
        .as_array()
        .ok_or_else(|| format!("{subject} must be an array"))?;
    output.push_str(&format!("        {field}: &["));
    for value in values {
        let value = value
            .as_str()
            .ok_or_else(|| format!("{subject} values must be strings"))?;
        output.push_str(&format!("{value:?},"));
    }
    output.push_str("],\n");
    Ok(())
}

fn rust_str_array(output: &mut String, name: &str, values: &[String]) {
    output.push_str(&format!("#[rustfmt::skip]\nstatic {name}: &[&str] = &[\n"));
    for value in values {
        output.push_str(&format!("    {value:?},\n"));
    }
    output.push_str("];\n\n");
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::verify_scene_surface;

    fn manifest() -> Value {
        serde_json::from_slice(include_bytes!(
            "../../tests/oracle/scene-surfaces/revision-1.json"
        ))
        .expect("approved scene-surface manifest is valid JSON")
    }

    fn assert_rejected(mut manifest: Value, mutate: impl FnOnce(&mut Value)) {
        mutate(&mut manifest);
        let error = verify_scene_surface(&crate::conformance::repo_root(), &manifest)
            .expect_err("mutated coverage must fail closed");
        assert!(!error.is_empty());
    }

    #[test]
    fn scene_surface_rejects_unknown_field_identity() {
        assert_rejected(manifest(), |manifest| {
            manifest["fieldDispositions"][0]["path"] = json!("/unknown");
        });
    }

    #[test]
    fn scene_surface_rejects_missing_disposition() {
        assert_rejected(manifest(), |manifest| {
            manifest["fieldDispositions"]
                .as_array_mut()
                .expect("field dispositions are an array")
                .pop();
        });
    }

    #[test]
    fn scene_surface_rejects_duplicate_ownership() {
        assert_rejected(manifest(), |manifest| {
            let first_record = manifest["fieldDispositions"][0]["recordKind"].clone();
            let first_path = manifest["fieldDispositions"][0]["path"].clone();
            manifest["fieldDispositions"][1]["recordKind"] = first_record;
            manifest["fieldDispositions"][1]["path"] = first_path;
        });
    }

    #[test]
    fn scene_surface_rejects_unresolved_coverage() {
        assert_rejected(manifest(), |manifest| {
            manifest["coverage"]["unresolved"] = json!(["future field"]);
        });
    }

    #[test]
    fn scene_surface_rejects_record_inheritance_cycles() {
        assert_rejected(manifest(), |manifest| {
            manifest["recordKinds"][0]["inherits"] = json!(["scene"]);
        });
    }
}
