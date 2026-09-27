use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::conformance::sha256;
use excalidraw_fonts::FontLicense;
use serde_json::{json, Value};

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
    "93029f2dab63c203dee69da4497ea4c2988a72d73f2a0017e62c529a4caac202";
const SCENE_SURFACE_SCHEMA_DIGEST: &str =
    "c8af2987f69e320bbc2673e252284fda77a18ddaf667bf998055c47cd3aa7341";
const SCENE_SURFACE_EVIDENCE_DIGEST: &str =
    "29984c198c9cb222d2aedbcca053e1829743ad686a09a266fdcdfdb192545aeb";
/// Three physical payloads: small official fonts, common Xiaolai, extended
/// Xiaolai. Whole chunks retain upstream declaration order within each group.
const FONT_PACKAGES: [&str; 3] = [
    "crates/excalidraw-fonts",
    "crates/excalidraw-fonts-xiaolai",
    "crates/excalidraw-fonts-xiaolai-extended",
];
const SHARD_LENGTHS: [usize; 3] = [358_432, 9_596_296, 3_071_196];
const CATALOG_SCHEMA: &str = "excalidraw-render/revision-catalog/v1";
const CATALOG_SCHEMA_DIGEST: &str =
    "eabf5de65012d514ef7b21e340eb80d39943f7c666f496ae6ceeb32d57e7ce5b";
const EXCALIFONT_BASIC_LATIN_PATH: &str =
    "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2";
const EXCALIFONT_BASIC_LATIN_SHA256: &str =
    "e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48";
const EXCALIFONT_BASIC_LATIN_LENGTH: usize = 24_956;
const EXCALIFONT_BASIC_LATIN_SFNT_SHA256: &str =
    "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb";
const EXCALIFONT_BASIC_LATIN_SFNT_LENGTH: usize = 61_948;
/// The one reviewed decode that is additionally checked in byte for byte,
/// relative to the excalidraw-fonts crate root as the record cites it.
const EXCALIFONT_BASIC_LATIN_SFNT_PATH: &str =
    "catalog/predecoded/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.sfnt";
/// The pinned decoder whose output over the exact manifested bytes is every
/// chunk's reviewed decode (spec 8.6). `verify` holds the version to the
/// lockfile, so a decoder bump cannot leave the record describing stale bytes.
const DECODER_NAME: &str = "wuff";
const DECODER_VERSION: &str = "0.2.8";
const DECODER_INVOCATION: &str = "wuff::decompress_woff2(source_bytes)";
const REVIEWED_DECODES_PATH: &str =
    "crates/excalidraw-fonts/catalog/reviewed-decodes/revision-1.json";
const REVIEWED_DECODES_DIGEST_PATH: &str =
    "crates/excalidraw-fonts/catalog/reviewed-decodes/revision-1.sha256";
const REVIEWED_DECODES_SCHEMA: &str = "excalidraw-render/reviewed-decodes/v1";
const REVIEWED_DECODES_APPROVAL_SUBJECT_SCHEMA: &str =
    "excalidraw-render/reviewed-decodes-approval-subject/v1";
/// The issue whose comment thread carries the reviewed-decodes approval.
const NOTO_EMOJI_SHA256: &str = "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634";
const NOTO_EMOJI_LENGTH: usize = 862_680;
const XIAOLAI_ORDER_SHA256: &str =
    "c2cbbbfa669404795527e06cf596ab4b1b851f9bb993c608c8ab9c3c2aabe9d3";
const SVG_FONT_SUBSET_PROFILE_DIGEST: &str =
    "083073315198f4cc4d67cd2f3a3fa7b65c6ce651dfb52fed61f5090c932b92c7";
const CHROMIUM_149_CJK_SCALARS_DIGEST: &str =
    "fc4e8ce56cb7bf4bca9dfd6f3495c9ae9f5440a5818aa3e6f645d5303265ff85";
/// The SVG font subset qualification corpus covers every official chunk and
/// the Noto Emoji fallback sfnt (#150).
const QUALIFICATION_SOURCES: usize = 231;
const NOTO_EMOJI_UPSTREAM_PATH: &str = "scripts/woff2/assets/NotoEmoji-Regular.ttf";
const SVG_FONT_SUBSET_QUALIFICATION_CORPUS_DIGEST: &str =
    "e1526c3a4a4896baadbdfedc623555149146696b845ca64f16cb43733ba7b61b";
const SVG_FONT_SUBSET_QUALIFICATION_SCHEMA_DIGEST: &str =
    "db0e54e516bd82bdc5733d6e950c36bd975bdd7d2a766e89e73804fe21d30264";

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
    ("Skia HDR tone-mapping adaptation", "skia-BSD-3-Clause.txt"),
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

/// The sentence the bundle prints between a notice's attribution and
/// postscript for each license; artifacts carry the notice without it.
const fn license_sentence(license: FontLicense) -> &'static str {
    match license {
        FontLicense::Ofl11 => "Licensed under the SIL Open Font License, Version 1.1.",
        FontLicense::Mit => "Licensed under the MIT License included in this bundle.",
    }
}

/// The license body an artifact carries once, under one identity, for every
/// font family licensed under that text (#151): the named `license-parts`
/// file, as the bundle prints it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FontTerms {
    /// The SIL Open Font License 1.1 text, shared by every OFL family.
    Ofl11,
    /// The Comic Shanns MIT text, which carries its own copyright line.
    ComicShannsMit,
}

impl FontTerms {
    /// The `license-parts` file holding the text.
    const fn bundle_part(self) -> &'static str {
        match self {
            Self::Ofl11 => "OFL-1.1.txt",
            Self::ComicShannsMit => "comic-shanns-MIT.txt",
        }
    }

    /// The notice identity the body is carried under; the attribution notice
    /// of every family sharing the text refers to it.
    const fn identity(self) -> &'static str {
        match self {
            Self::Ofl11 => "license.ofl-1.1",
            Self::ComicShannsMit => "license.comic-shanns.mit",
        }
    }

    /// The generated `const` that holds the terms text, shared by every
    /// notice with the same source.
    fn rust_const(self) -> String {
        format!(
            "{}_TERMS",
            self.bundle_part()
                .trim_end_matches(".txt")
                .replace(['-', '.'], "_")
                .to_uppercase()
        )
    }
}

/// One official rendering font family's notice. The license bundle prints
/// `attribution`, the license sentence and `postscript` as one line under
/// `heading`; the generated `FontNotice` table carries the same line without
/// the sentence, plus the family's license terms, so artifacts and the bundle
/// derive from one source.
#[derive(Clone, Copy, Debug)]
struct FontNotice {
    /// The catalog `family` of the assets the notice covers.
    family: &'static str,
    heading: &'static str,
    attribution: &'static str,
    license: FontLicense,
    /// Provenance the line continues with after the license sentence.
    postscript: &'static str,
    terms: FontTerms,
}

impl FontNotice {
    /// The line the bundle prints under the heading.
    fn bundle_line(&self) -> String {
        let mut line = format!("{} {}", self.attribution, license_sentence(self.license));
        if !self.postscript.is_empty() {
            line.push(' ');
            line.push_str(self.postscript);
        }
        line
    }

    /// The provenance artifacts carry: the bundle line without its license
    /// sentence.
    fn artifact_attribution(&self) -> String {
        if self.postscript.is_empty() {
            self.attribution.to_owned()
        } else {
            format!("{} {}", self.attribution, self.postscript)
        }
    }
}

const FONT_NOTICES: &[FontNotice] = &[
    FontNotice {
        family: "Cascadia",
        heading: "Cascadia Code",
        attribution: "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code.",
        license: FontLicense::Ofl11,
        postscript: "",
        terms: FontTerms::Ofl11,
    },
    FontNotice {
        family: "ComicShanns",
        heading: "Comic Shanns",
        attribution: "Copyright (c) 2018 Shannon Miwa.",
        license: FontLicense::Mit,
        postscript: "",
        terms: FontTerms::ComicShannsMit,
    },
    FontNotice {
        family: "Excalifont",
        heading: "Excalifont and Virgil provenance",
        attribution: "Copyright (c) 2024 by Excalidraw. Excalifont is derived from Virgil by Your Own Font Foundry, with modifications by Jan Filipek / DizajnDesign.",
        license: FontLicense::Ofl11,
        postscript: "Excalifont and Virgil provenance is retained together.",
        terms: FontTerms::Ofl11,
    },
    FontNotice {
        family: "Liberation",
        heading: "Liberation Sans",
        attribution: "Digitized data copyright (c) 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine. Copyright (c) 2012 Red Hat, Inc. with Reserved Font Name Liberation.",
        license: FontLicense::Ofl11,
        postscript: "",
        terms: FontTerms::Ofl11,
    },
    FontNotice {
        family: "Lilita",
        heading: "Lilita",
        attribution: "Copyright (c) 2011 Juan Montoreano (juan@remolacha.biz), with Reserved Font Name Lilita.",
        license: FontLicense::Ofl11,
        postscript: "",
        terms: FontTerms::Ofl11,
    },
    FontNotice {
        family: "Nunito",
        heading: "Nunito",
        attribution: "Copyright 2014 The Nunito Project Authors (https://github.com/googlefonts/nunito).",
        license: FontLicense::Ofl11,
        postscript: "",
        terms: FontTerms::Ofl11,
    },
    FontNotice {
        family: "Virgil",
        heading: "Virgil",
        attribution: "Virgil by Your Own Font Foundry, with Reserved Font Name Virgil.",
        license: FontLicense::Ofl11,
        postscript: "",
        terms: FontTerms::Ofl11,
    },
    FontNotice {
        family: "Xiaolai",
        heading: "Xiaolai SC",
        attribution: "Copyright (c) 2020 LXGW. Designed by Nozomi Seto.",
        license: FontLicense::Ofl11,
        postscript: "The shipped subsets remove non-CJK codepoints, reduce Hangul side-bearings to 40%, and center improperly centered CJK codepoints.",
        terms: FontTerms::Ofl11,
    },
];

/// The marker after which a sealed `*.ofl-full` notice states the OFL terms;
/// what precedes it is the copyright statement.
const OFL_TERMS_START: &str = "This Font Software is licensed";

/// The catalog family of the Noto Emoji fallback face, which is no asset
/// record but is subset for direct SVG like the chunks are (#150), so it
/// needs a `FontNotice` too. Its attribution is the copyright statement the
/// sealed `noto-emoji.ofl-full` notice opens with; the bundle already prints
/// that notice complete under the output-traveling notices, so the fallback
/// adds no line to the bundle's font section.
const NOTO_EMOJI_FAMILY: &str = "NotoEmoji";
const NOTO_EMOJI_NOTICE_ID: &str = "noto-emoji.ofl-full";
const NOTO_EMOJI_LICENSE_IDENTITY: &str = "font.noto-emoji.ofl-1.1";

const FALLBACK_RECIPE_COUNT: usize = 5;
const NOTICE_COUNT: usize = 4;
const SOURCE_COUNT: usize = 36;
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
    let base = root.join("crates/excalidraw-fonts");
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
    let reviewed_decodes = materialize_reviewed_decodes(&root, &assets)?;
    materialize_predecoded_excalifont(&root, &assets)?;

    assign_layout(&mut assets)?;

    let base = root.join("crates/excalidraw-fonts");
    materialize_incorporated_artifacts(&root, &base)?;

    for asset in &assets {
        let data_path = root.join(FONT_PACKAGES[asset.shard]).join(&asset.path);
        if let Some(parent) = data_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        write_if_changed(&data_path, &asset.bytes)?;
    }
    materialize_payload_indexes(&root, &assets)?;

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

    let catalog = build_catalog(&assets, &scene_surface, &reviewed_decodes)?;
    let catalog_bytes = serde_json::to_vec(&catalog)
        .map_err(|error| format!("failed to serialize revision catalog: {error}"))?;
    let catalog_path = base.join("catalog/revision-1.json");
    write_if_changed(&catalog_path, &catalog_bytes)?;
    write_digest_sidecar(&base.join("catalog/revision-1.sha256"), &catalog_bytes)?;

    let generated = generate_rust(&base, &catalog, &scene_surface, &catalog_bytes)?;
    let generated_path = base.join("src/generated/revision_1.rs");
    if let Some(parent) = generated_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    write_if_changed(&generated_path, generated.as_bytes())?;
    write_if_changed(
        &root.join(XIAOLAI_FACES_GENERATED_PATH),
        generate_xiaolai_faces_rust(&root)?.as_bytes(),
    )?;
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
    let base = root.join("crates/excalidraw-fonts");
    materialize_svg_font_subset_qualification_schema(&root)?;
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
    let assets = expected_assets(&reference_assets)?;
    materialize_payload_indexes(&root, &assets)?;
    let reviewed_decodes = materialize_reviewed_decodes(&root, &assets)?;
    let catalog = build_catalog(&assets, &scene_surface, &reviewed_decodes)?;
    let catalog_bytes = serde_json::to_vec(&catalog)
        .map_err(|error| format!("failed to serialize revision catalog: {error}"))?;
    write_if_changed(&base.join("catalog/revision-1.json"), &catalog_bytes)?;
    write_digest_sidecar(&base.join("catalog/revision-1.sha256"), &catalog_bytes)?;

    let generated = generate_rust(&base, &catalog, &scene_surface, &catalog_bytes)?;
    write_if_changed(
        &base.join("src/generated/revision_1.rs"),
        generated.as_bytes(),
    )?;
    write_if_changed(
        &root.join(XIAOLAI_FACES_GENERATED_PATH),
        generate_xiaolai_faces_rust(&root)?.as_bytes(),
    )?;
    let scene_policy = generated_scene_surface(&scene_surface)?;
    write_if_changed(
        &root.join("crates/excalidraw-render/src/generated/scene_surface_revision_1.rs"),
        scene_policy.as_bytes(),
    )?;
    verify()
}

fn materialize_svg_font_subset_qualification_schema(root: &Path) -> Result<(), String> {
    let path = root.join(
        "tests/oracle/compatibility-artifacts/svg-font-subset-qualification-corpus-v1.schema.json",
    );
    let mut schema: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("invalid SVG font subset qualification schema: {error}"))?;
    schema["$defs"]["cell"]["properties"]["transformationNoticeSha256"] =
        json!({"$ref": "#/$defs/sha256"});
    // Per-cell canonical resource trace digests are no longer evidence; the
    // determinism property is cross-target equality of the aggregate trace
    // measurement (ADR-0002).
    schema["$defs"]["cell"]["properties"]
        .as_object_mut()
        .ok_or_else(|| "SVG font subset cell schema has no properties".to_owned())?
        .remove("canonicalResourceTraceSha256");
    schema["$defs"]["source"]["properties"]["licenseIdentity"] =
        json!({"minLength": 1, "type": "string"});
    schema["$defs"]["source"]["properties"]["reservedFontNames"] = json!({
        "items": {"minLength": 1, "type": "string"},
        "type": "array",
        "uniqueItems": true,
    });
    // A source is either an official WOFF2 chunk or the manifested sfnt of a
    // non-WOFF2 fallback (the Noto Emoji face, #150); an sfnt source names the
    // WOFF2 the subsetter encodes from it so both canonical viewers, which admit
    // only WOFF2, can render the source face.
    schema["$defs"]["source"]["properties"]["sourceContainer"] =
        json!({"enum": ["woff2", "sfnt"], "type": "string"});
    schema["$defs"]["source"]["properties"]["viewerSourceWoff2Sha256"] =
        json!({"$ref": "#/$defs/sha256"});
    schema["$defs"]["source"]["properties"]["upstreamPath"] =
        json!({"pattern": "^(fonts/.+\\.woff2|scripts/woff2/assets/.+\\.ttf)$", "type": "string"});
    schema["properties"]["sources"]["minItems"] = json!(QUALIFICATION_SOURCES);
    schema["properties"]["sources"]["maxItems"] = json!(QUALIFICATION_SOURCES);
    let required = schema["$defs"]["source"]["required"]
        .as_array_mut()
        .ok_or_else(|| "SVG font subset source schema has no required array".to_owned())?;
    for field in ["licenseIdentity", "reservedFontNames", "sourceContainer"] {
        if !required.iter().any(|value| value == field) {
            required.push(json!(field));
        }
    }
    let bytes = serde_json::to_vec(&schema).map_err(|error| {
        format!("failed to serialize SVG font subset qualification schema: {error}")
    })?;
    write_if_changed(&path, &bytes)?;
    write_digest_sidecar(&path.with_extension("sha256"), &bytes)
}

fn materialize_svg_font_subset_profile(root: &Path) -> Result<(), String> {
    use excalidraw_fonts::{
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
    let semantic_digest = excalidraw_fonts::font_subset_profile_digest_v1()
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

/// Verifies the sealed catalog and reproduces its reviewed decode bytes offline.
pub fn verify() -> Result<(), String> {
    let root = crate::conformance::repo_root();
    let base = root.join("crates/excalidraw-fonts");
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
    verify_catalog_schema(&base, &catalog)?;
    verify_catalog_identity(&catalog, &scene_surface)?;
    verify_font_subset_qualification_corpus(&root, &catalog)?;
    let sources = verify_assets(&root, &catalog)?;
    let reviewed_decodes = read_reviewed_decodes(&root)?;
    verify_reviewed_decodes(&root, &reviewed_decodes, &catalog, &sources)?;
    let expected_catalog = expected_catalog(&reference_assets, &scene_surface, &reviewed_decodes)?;
    if catalog != expected_catalog {
        return Err("revision catalog differs from its authoritative materialization".to_owned());
    }
    let imported = expected_assets(&reference_assets)?;
    for (ordinal, package) in FONT_PACKAGES.iter().enumerate().skip(1) {
        let path = root.join(package).join("src/generated.rs");
        if fs::read_to_string(&path).map_err(|error| error.to_string())?
            != payload_index(&imported, ordinal)
        {
            return Err(format!(
                "generated font file index is stale: {}",
                path.display()
            ));
        }
    }
    let excalifont_source = sources
        .get(EXCALIFONT_BASIC_LATIN_PATH)
        .ok_or_else(|| "manifested Excalifont source is missing".to_owned())?;
    verify_predecoded_excalifont(&root, excalifont_source)?;
    verify_noto_emoji(&root)?;

    let generated_path = base.join("src/generated/revision_1.rs");
    let generated = fs::read_to_string(&generated_path)
        .map_err(|error| format!("failed to read {}: {error}", generated_path.display()))?;
    let expected_generated = generate_rust(&base, &catalog, &scene_surface, &catalog_bytes)?;
    if generated != expected_generated {
        return Err("generated Rust catalog is stale".to_owned());
    }
    let xiaolai_faces_path = root.join(XIAOLAI_FACES_GENERATED_PATH);
    let xiaolai_faces = fs::read_to_string(&xiaolai_faces_path)
        .map_err(|error| format!("failed to read {}: {error}", xiaolai_faces_path.display()))?;
    if xiaolai_faces != generate_xiaolai_faces_rust(&root)? {
        return Err("generated Xiaolai face table is stale".to_owned());
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
    let destination = root
        .join("crates/excalidraw-fonts")
        .join(EXCALIFONT_BASIC_LATIN_SFNT_PATH);
    write_if_changed(&destination, &sfnt)
}

fn verify_predecoded_excalifont(root: &Path, source: &[u8]) -> Result<(), String> {
    let path = root
        .join("crates/excalidraw-fonts")
        .join(EXCALIFONT_BASIC_LATIN_SFNT_PATH);
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
            "path": EXCALIFONT_BASIC_LATIN_SFNT_PATH,
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
    let mut metadata = metadata;
    let mut expected = expected;
    metadata
        .as_object_mut()
        .ok_or("invalid font transformation record")?
        .remove("approval");
    expected.as_object_mut().unwrap().remove("approval");
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

/// One official font chunk's reviewed decode: what the pinned decoder produces
/// for the chunk's exact manifested bytes, identified by length and digest.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewedDecode {
    identity: String,
    upstream_path: String,
    source_length: usize,
    source_sha256: String,
    decoded_length: usize,
    decoded_sha256: String,
}

/// Derives one chunk's reviewed decode. The decode must be a parseable sfnt of
/// exactly the size the WOFF2 header declares; anything else fails closed,
/// because the record may only ever describe what the renderer will embed.
fn review_decode(upstream_path: &str, source: &[u8]) -> Result<ReviewedDecode, String> {
    let declared_length = source
        .get(0..20)
        .filter(|header| &header[0..4] == b"wOF2")
        .and_then(|header| header[16..20].try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| format!("{upstream_path} is not a WOFF2 container"))?;
    let decoded = wuff::decompress_woff2(source)
        .map_err(|error| format!("{upstream_path} does not decode: {error}"))?;
    if decoded.len() != declared_length as usize {
        return Err(format!(
            "{upstream_path} decodes to {} bytes but declares {declared_length}",
            decoded.len()
        ));
    }
    rustybuzz::Face::from_slice(&decoded, 0)
        .ok_or_else(|| format!("{upstream_path} does not decode to a valid sfnt"))?;
    let source_sha256 = sha256(source);
    Ok(ReviewedDecode {
        identity: format!("{upstream_path}@sha256:{source_sha256}"),
        upstream_path: upstream_path.to_owned(),
        source_length: source.len(),
        source_sha256,
        decoded_length: decoded.len(),
        decoded_sha256: sha256(&decoded),
    })
}

/// The reviewed-decodes record: the decoder identity, the manifests that
/// qualified it, every chunk's reviewed decode in catalog order, the one decode
/// that is additionally checked in byte for byte, and the maintainer approval
/// that binds all of it.
fn reviewed_decodes_record(decodes: &[ReviewedDecode], approval: Value) -> Value {
    json!({
        "approval": approval,
        "checkedInDecodes": [{
            "identity": format!("{EXCALIFONT_BASIC_LATIN_PATH}@sha256:{EXCALIFONT_BASIC_LATIN_SHA256}"),
            "length": EXCALIFONT_BASIC_LATIN_SFNT_LENGTH,
            "path": EXCALIFONT_BASIC_LATIN_SFNT_PATH,
            "sha256": EXCALIFONT_BASIC_LATIN_SFNT_SHA256,
        }],
        "chunks": decodes.iter().map(|decode| json!({
            "decodedLength": decode.decoded_length,
            "decodedSha256": decode.decoded_sha256,
            "identity": decode.identity,
            "sourceLength": decode.source_length,
            "sourceSha256": decode.source_sha256,
            "upstreamPath": decode.upstream_path,
        })).collect::<Vec<_>>(),
        "compatibilityRevision": REVISION,
        "decoder": {
            "invocation": DECODER_INVOCATION,
            "name": DECODER_NAME,
            "version": DECODER_VERSION,
        },
        "qualificationCorpusSha256": SVG_FONT_SUBSET_QUALIFICATION_CORPUS_DIGEST,
        "schema": REVIEWED_DECODES_SCHEMA,
        "sourceManifestSha256": ASSET_MANIFEST_DIGEST,
    })
}

/// The approval subject of a reviewed-decodes record: the record without its
/// approval object, so recording the approval does not move it.
fn reviewed_decodes_subject(record: &Value) -> Result<String, String> {
    let mut record = record.clone();
    record
        .as_object_mut()
        .ok_or_else(|| "reviewed-decodes record must be an object".to_owned())?
        .remove("approval");
    Ok(sha256(&crate::conformance::canonical_bytes(&json!({
        "record": record,
        "schema": REVIEWED_DECODES_APPROVAL_SUBJECT_SCHEMA,
    }))?))
}

/// Carries an existing record's approval onto a freshly derived one exactly
/// when both have the same subject; otherwise the fresh record stays pending.
fn carry_approval(existing: Option<&Value>, fresh: &mut Value) -> Result<(), String> {
    let Some(existing) = existing else {
        return Ok(());
    };
    if existing["approval"].is_null()
        || reviewed_decodes_subject(existing)? != reviewed_decodes_subject(fresh)?
    {
        return Ok(());
    }
    fresh["approval"] = existing["approval"].clone();
    Ok(())
}

/// Reviews every asset's decode and writes the record, carrying the checked-in
/// approval forward only while its subject holds. Returns the written record.
fn materialize_reviewed_decodes(root: &Path, assets: &[ImportedAsset]) -> Result<Value, String> {
    let decodes = assets
        .iter()
        .map(|asset| review_decode(&asset.path, &asset.bytes))
        .collect::<Result<Vec<_>, String>>()?;
    let mut record = reviewed_decodes_record(&decodes, Value::Null);
    let path = root.join(REVIEWED_DECODES_PATH);
    let existing = match fs::read(&path) {
        Ok(bytes) => Some(
            serde_json::from_slice::<Value>(&bytes)
                .map_err(|error| format!("invalid reviewed-decodes record: {error}"))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    carry_approval(existing.as_ref(), &mut record)?;
    let bytes = serde_json::to_vec(&record)
        .map_err(|error| format!("failed to serialize reviewed-decodes record: {error}"))?;
    write_if_changed(&path, &bytes)?;
    write_digest_sidecar(&root.join(REVIEWED_DECODES_DIGEST_PATH), &bytes)?;
    Ok(record)
}

/// Reads the checked-in record as canonical JSON with a current sidecar.
fn read_reviewed_decodes(root: &Path) -> Result<Value, String> {
    let path = root.join(REVIEWED_DECODES_PATH);
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    verify_canonical_json_bytes(&bytes, "reviewed-decodes record")?;
    verify_digest_sidecar(&root.join(REVIEWED_DECODES_DIGEST_PATH), &bytes)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid reviewed-decodes record: {error}"))
}

/// The lockfile must resolve exactly one decoder package, at the version the
/// record names: the record describes that decoder's output and no other.
fn verify_decoder_pin(root: &Path) -> Result<(), String> {
    let lock_path = root.join("Cargo.lock");
    let lock = fs::read_to_string(&lock_path)
        .map_err(|error| format!("failed to read {}: {error}", lock_path.display()))?;
    let packages = lock
        .matches(&format!("\nname = \"{DECODER_NAME}\"\n"))
        .count();
    let pinned = lock.contains(&format!(
        "\nname = \"{DECODER_NAME}\"\nversion = \"{DECODER_VERSION}\"\n"
    ));
    if packages != 1 || !pinned {
        return Err(format!(
            "Cargo.lock does not resolve exactly one {DECODER_NAME} at {DECODER_VERSION}, \
             the decoder the reviewed-decodes record names"
        ));
    }
    Ok(())
}

/// Verifies the record's content against fresh decodes of every reassembled
/// chunk: the decoder is the pinned one, every chunk's decode re-derives to the
/// recorded length and digest in catalog order, and each checked-in decode is
/// one of them byte for byte. The approval itself is checked separately, after
/// everything it binds.
fn verify_reviewed_decodes(
    root: &Path,
    record: &Value,
    catalog: &Value,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    verify_decoder_pin(root)?;
    let decodes = array_at(catalog, &["assets"], "revision catalog assets")?
        .iter()
        .map(|asset| {
            let path = string(asset, "upstreamPath", "catalog asset")?;
            let source = sources
                .get(path)
                .ok_or_else(|| format!("no reassembled source for {path}"))?;
            review_decode(path, source)
        })
        .collect::<Result<Vec<_>, String>>()?;
    if decodes.len() != 230 {
        return Err("reviewed decodes must cover all 230 official chunks".to_owned());
    }
    let expected = reviewed_decodes_record(&decodes, record["approval"].clone());
    if *record != expected {
        return Err(
            "reviewed-decodes record differs from the pinned decoder's output over the checked-in chunks"
                .to_owned(),
        );
    }
    let base = root.join("crates/excalidraw-fonts");
    for checked_in in array_at(record, &["checkedInDecodes"], "checked-in decodes")? {
        let identity = string(checked_in, "identity", "checked-in decode")?;
        let relative = string(checked_in, "path", "checked-in decode")?;
        let sfnt_path = base.join(relative);
        let sfnt = fs::read(&sfnt_path)
            .map_err(|error| format!("failed to read {}: {error}", sfnt_path.display()))?;
        let decode = decodes
            .iter()
            .find(|decode| decode.identity == identity)
            .ok_or_else(|| format!("checked-in decode {identity} is not an official chunk"))?;
        if sfnt.len() != decode.decoded_length
            || sha256(&sfnt) != decode.decoded_sha256
            || integer(checked_in, "length", "checked-in decode")? as usize != sfnt.len()
            || string(checked_in, "sha256", "checked-in decode")? != decode.decoded_sha256
        {
            return Err(format!(
                "checked-in decode {relative} is not the reviewed decode of {identity}"
            ));
        }
    }
    Ok(())
}

fn verify_noto_emoji(root: &Path) -> Result<(), String> {
    let path = root.join("crates/excalidraw-fonts/catalog/fallback/NotoEmoji-Regular.ttf");
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
    if metadata["comparison"]["version"] != "Version 3.003"
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
        root.join("crates/excalidraw-fonts/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-fonts-xiaolai/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-fonts-xiaolai-extended/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-render/LICENSE-BUNDLE.txt"),
        root.join("crates/excalidraw-render-cli/LICENSE-BUNDLE.txt"),
    ]
}

fn build_catalog(
    assets: &[ImportedAsset],
    scene_surface: &Value,
    reviewed_decodes: &Value,
) -> Result<Value, String> {
    let fallback_ids = ids(scene_surface, "fallbackRecipes", "recipeId")?;
    let notice_ids = ids(scene_surface, "noticeCatalog", "id")?;
    let source_ids = ids(scene_surface, "sourceCatalog", "id")?;
    let decodes = reviewed_decode_index(reviewed_decodes)?;
    let asset_values = assets
        .iter()
        .map(|asset| {
            let identity = format!("{}@sha256:{}", asset.path, asset.digest);
            let segment_identity = format!("{identity}/segment/0");
            let (decoded_length, decoded_sha256) = decodes.get(identity.as_str()).ok_or_else(|| {
                format!("reviewed-decodes record has no decode for {identity}")
            })?;
            Ok(json!({
                "completeLength": asset.length,
                "completeSha256": asset.digest,
                "copyright": asset.copyright,
                "decodedLength": decoded_length,
                "decodedSha256": decoded_sha256,
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
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;

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

/// Each chunk's reviewed decode as the record states it, keyed by asset
/// identity, so the catalog can carry the decode beside the source digests.
fn reviewed_decode_index(record: &Value) -> Result<BTreeMap<&str, (usize, &str)>, String> {
    let mut index = BTreeMap::new();
    for chunk in array_at(record, &["chunks"], "reviewed-decodes chunks")? {
        let identity = string(chunk, "identity", "reviewed decode")?;
        let decoded_length = integer(chunk, "decodedLength", "reviewed decode")? as usize;
        let decoded_sha256 = string(chunk, "decodedSha256", "reviewed decode")?;
        if index
            .insert(identity, (decoded_length, decoded_sha256))
            .is_some()
        {
            return Err(format!("reviewed-decodes record repeats {identity}"));
        }
    }
    Ok(index)
}

/// The 230 official assets as the authoritative reference manifest lists them,
/// in catalog order. Files are read directly; historical segment coordinates
/// remain reproducible provenance and never participate in loading.
fn expected_assets(reference_assets: &Value) -> Result<Vec<ImportedAsset>, String> {
    let root = crate::conformance::repo_root();
    let mut sources = BTreeMap::new();
    for package in FONT_PACKAGES {
        for path in font_files(&root.join(package).join("fonts"))? {
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            if sources.insert(sha256(&bytes), bytes).is_some() {
                return Err("duplicate source in font packages".to_owned());
            }
        }
    }
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
                path: path.clone(),
                bytes: sources
                    .remove(string(entry, "sha256", "reference asset")?)
                    .ok_or_else(|| format!("missing manifested font source {path}"))?,
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
    if !sources.is_empty() {
        return Err("unmanifested sources in font packages".to_owned());
    }
    assign_layout(&mut assets)?;
    Ok(assets)
}

fn font_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            files.extend(font_files(&entry.path())?);
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "woff2") {
            files.push(entry.path());
        } else {
            return Err(format!(
                "unexpected font package entry: {}",
                entry.path().display()
            ));
        }
    }
    files.sort();
    Ok(files)
}

fn payload_index(assets: &[ImportedAsset], ordinal: usize) -> String {
    let mut output = String::from("// Generated by `xtask catalog materialize`; do not edit.\n#[rustfmt::skip]\npub static FILES: &[&[u8]] = &[\n");
    for asset in assets.iter().filter(|asset| asset.shard == ordinal) {
        output.push_str(&format!("    include_bytes!(\"../{}\"),\n", asset.path));
    }
    output.push_str("];\n");
    output
}

/// Map frozen provenance to today's package, without interpreting byte offsets.
fn font_package_group(asset: &Value) -> Result<usize, String> {
    asset["segments"][0]["shard"]
        .as_str()
        .and_then(|shard| shard.strip_prefix("revision-1-"))
        .and_then(|ordinal| ordinal.parse::<usize>().ok())
        .filter(|ordinal| *ordinal < FONT_PACKAGES.len())
        .ok_or_else(|| "invalid historical font group".to_owned())
}

fn materialize_payload_indexes(root: &Path, assets: &[ImportedAsset]) -> Result<(), String> {
    for (ordinal, package) in FONT_PACKAGES.iter().enumerate().skip(1) {
        write_if_changed(
            &root.join(package).join("src/generated.rs"),
            payload_index(assets, ordinal).as_bytes(),
        )?;
    }
    Ok(())
}

fn expected_catalog(
    reference_assets: &Value,
    scene_surface: &Value,
    reviewed_decodes: &Value,
) -> Result<Value, String> {
    build_catalog(
        &expected_assets(reference_assets)?,
        scene_surface,
        reviewed_decodes,
    )
}

// Preserve the frozen catalog's historical packing coordinates. They are
// provenance only; file paths and per-package indexes locate actual bytes.
fn assign_layout(assets: &mut [ImportedAsset]) -> Result<(), String> {
    let mut ordered = assets
        .iter_mut()
        .map(|asset| {
            let order = if asset.family == "Xiaolai" {
                Some(
                    excalidraw_fonts::font_face_order("Xiaolai", &asset.path)
                        .ok_or_else(|| format!("unknown Xiaolai chunk {}", asset.path))?,
                )
            } else {
                None
            };
            Ok((order, asset))
        })
        .collect::<Result<Vec<_>, String>>()?;
    ordered.sort_by(|(left_order, left), (right_order, right)| {
        left_order
            .cmp(right_order)
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut lengths = [0_usize; 3];
    for (order, asset) in ordered {
        let shard = match order {
            None => 0,
            Some(0..=208) => xiaolai_shard(&asset.bytes)?,
            Some(_) => return Err("Xiaolai declaration order exceeds 208".to_owned()),
        };
        asset.shard = shard;
        asset.offset = lengths[shard];
        lengths[shard] = lengths[shard]
            .checked_add(asset.length)
            .ok_or_else(|| "payload length overflow".to_owned())?;
    }
    if lengths != SHARD_LENGTHS {
        return Err(format!(
            "font payload layout has unexpected lengths: {lengths:?}"
        ));
    }
    verify_published_xiaolai_groups(assets)?;
    Ok(())
}

/// Port of dominantBucket/bucketOf/EXTRA_BUCKETS in
/// https://github.com/rickardp/excalidraw-image/blob/c33f236d9167845c327893b6e1a19728e763d173/src/scripts/sync-fonts.mjs
/// These are upstream's named buckets (some combine Unicode blocks), not CSS
/// unicodeRange declarations. Count distinct cmap codepoints in ascending order;
/// ties retain the first encountered bucket, matching fontkit's characterSet and
/// JavaScript Map insertion order. Unmapped glyph zero is not a character.
fn xiaolai_shard(source: &[u8]) -> Result<usize, String> {
    let decoded = wuff::decompress_woff2(source)
        .map_err(|error| format!("Xiaolai classification decode failed: {error}"))?;
    let face = rustybuzz::ttf_parser::Face::parse(&decoded, 0)
        .map_err(|error| format!("Xiaolai classification cmap parse failed: {error}"))?;
    let cmap = face.tables().cmap.ok_or("Xiaolai has no cmap")?;
    let mut codepoints = BTreeSet::new();
    for table in cmap
        .subtables
        .into_iter()
        .filter(|table| table.is_unicode())
    {
        table.codepoints(|cp| {
            if table.glyph_index(cp).is_some() {
                codepoints.insert(cp);
            }
        });
    }
    dominant_xiaolai_shard(codepoints)
}

fn dominant_xiaolai_shard(codepoints: BTreeSet<u32>) -> Result<usize, String> {
    // (inclusive bounds, extended group). Last bucket is Other.
    const BUCKETS: &[(u32, u32, bool)] = &[
        (0x0000, 0x04ff, false),  // Latin/Greek/Cyrillic
        (0x2000, 0x206f, false),  // General Punctuation
        (0x3000, 0x303f, false),  // CJK Symbols & Punct
        (0x3040, 0x309f, false),  // Hiragana
        (0x30a0, 0x30ff, false),  // Katakana
        (0x3100, 0x312f, false),  // Bopomofo
        (0x3130, 0x318f, false),  // Hangul Compat Jamo
        (0x3200, 0x32ff, false),  // Enclosed CJK
        (0x3300, 0x33ff, false),  // CJK Compat
        (0x3400, 0x4dbf, true),   // CJK Ext A
        (0x4e00, 0x9fff, false),  // CJK Unified
        (0xac00, 0xd7af, false),  // Hangul Syllables
        (0xf900, 0xfaff, true),   // CJK Compat Ideographs
        (0xff00, 0xffef, false),  // Halfwidth/Fullwidth
        (0x20000, 0x3ffff, true), // CJK Ext B+
        (0, u32::MAX, true),      // Other
    ];
    let mut counts = Vec::<(usize, usize)>::new();
    for cp in codepoints {
        let bucket = BUCKETS
            .iter()
            .position(|&(lo, hi, _)| (lo..=hi).contains(&cp))
            .unwrap();
        if let Some((_, count)) = counts.iter_mut().find(|(index, _)| *index == bucket) {
            *count += 1;
        } else {
            counts.push((bucket, 1));
        }
    }
    let mut best = None;
    let mut best_count = 0;
    for (bucket, count) in counts {
        if count > best_count {
            best = Some(bucket);
            best_count = count;
        }
    }
    let bucket = best.ok_or("Xiaolai has no mapped Unicode codepoints")?;
    Ok(if BUCKETS[bucket].2 { 2 } else { 1 })
}

/// Published 1.0.0 Cargo.toml font-fingerprint values for excalidraw-image-fonts-cjk
/// and excalidraw-image-fonts-cjk-extra. Reproduce their exact sorted
/// `Xiaolai/file.woff2:sha256` lines joined by LF with no trailing LF. This pins
/// membership AND original bytes, independently of our payload offsets/order.
fn verify_published_xiaolai_groups(assets: &[ImportedAsset]) -> Result<(), String> {
    for (shard, count, fingerprint) in [
        (
            1,
            165,
            "630cb497d7b7de4fea58a890b78a2de5ea0e74221d8b876156de4993698e24ad",
        ),
        (
            2,
            44,
            "023c933b9a01e3b09cc6d8f58dbb7c9aaf6376086290338814ffa1e51c1a8a61",
        ),
    ] {
        let mut lines = assets
            .iter()
            .filter(|asset| asset.shard == shard)
            .map(|asset| {
                format!(
                    "{}:{}",
                    asset.path.strip_prefix("fonts/").unwrap(),
                    sha256(&asset.bytes)
                )
            })
            .collect::<Vec<_>>();
        lines.sort();
        if lines.len() != count || sha256(lines.join("\n").as_bytes()) != fingerprint {
            return Err(format!(
                "Xiaolai shard {shard} differs from published 1.0.0 classification/bytes"
            ));
        }
    }
    Ok(())
}

fn generate_rust(
    base: &Path,
    catalog: &Value,
    scene_surface: &Value,
    catalog_bytes: &[u8],
) -> Result<String, String> {
    let mut output = String::from(
        "// Generated by `cargo run --locked -p xtask -- catalog import` or `catalog materialize`; do not edit.\n\n",
    );
    for (ordinal, package) in [
        (1, "excalidraw_fonts_xiaolai"),
        (2, "excalidraw_fonts_xiaolai_extended"),
    ] {
        output.push_str(&format!("#[cfg(feature = \"cjk\")]\nconst fn payload_{ordinal}(index: usize) -> Option<&'static [u8]> {{ Some({package}::FILES[index]) }}\n#[cfg(not(feature = \"cjk\"))]\nconst fn payload_{ordinal}(_index: usize) -> Option<&'static [u8]> {{ None }}\n"));
    }
    output.push('\n');
    output.push_str("#[rustfmt::skip]\npub static ASSETS: [AssetRecord; 230] = [\n");
    let assets = catalog["assets"]
        .as_array()
        .ok_or_else(|| "revision catalog assets must be an array".to_owned())?;
    let mut families = BTreeSet::new();
    let mut file_indexes = [0; 3];
    for asset in assets {
        families.insert(string(asset, "family", "catalog asset")?);
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
        rust_field(
            &mut output,
            "decoded_sha256",
            string(asset, "decodedSha256", "catalog asset")?,
        );
        output.push_str(&format!(
            "        decoded_length: {},\n",
            integer(asset, "decodedLength", "catalog asset")?
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
        let group = font_package_group(asset)?;
        let path = string(asset, "upstreamPath", "catalog asset")?;
        let bytes = if group == 0 {
            format!("Some(include_bytes!(\"../../{path}\"))")
        } else {
            format!("payload_{group}({})", file_indexes[group])
        };
        file_indexes[group] += 1;
        output.push_str(&format!(
            "],\n        bytes: {bytes},\n        segments: &[\n"
        ));
        for segment in asset["segments"]
            .as_array()
            .ok_or_else(|| "catalog asset segments must be an array".to_owned())?
        {
            let shard = string(segment, "shard", "catalog segment")?;
            output.push_str("            AssetSegment {\n");
            rust_field_indented(
                &mut output,
                "identity",
                string(segment, "identity", "catalog segment")?,
                16,
            );
            rust_field_indented(&mut output, "shard", shard, 16);
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

    // Join storage records to declarations once, at generation time. Runtime
    // consumers enumerate these immutable slices without scanning or sorting.
    for family in &families {
        let mut ordered = assets
            .iter()
            .enumerate()
            .filter(|(_, asset)| asset["family"].as_str() == Some(*family))
            .map(|(index, asset)| {
                let path = string(asset, "upstreamPath", "catalog asset")?;
                let order = excalidraw_fonts::font_face_order(family, path)
                    .ok_or_else(|| format!("missing font declaration for {path}"))?;
                Ok((order, index))
            })
            .collect::<Result<Vec<_>, String>>()?;
        ordered.sort_unstable();
        output.push_str(&format!(
            "#[rustfmt::skip]\npub(crate) static {}_FONT_FACES: &[fonts::Face] = &[\n",
            family.to_uppercase()
        ));
        for (order, index) in ordered {
            let ranges = match *family {
                "Cascadia" | "Liberation" | "Virgil" => "ALL_UNICODE".to_owned(),
                "ComicShanns" => format!("COMIC_SHANNS_FACES[{order}].unicode_ranges"),
                family => format!("{}_FACES[{order}].unicode_ranges", family.to_uppercase()),
            };
            output.push_str(&format!("    fonts::Face::chunk(fonts::FamilyId::{family}, {order}, &ASSETS[{index}], {ranges}),\n"));
        }
        output.push_str("];\n\n");
    }

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
    generate_font_notices(&mut output, base, scene_surface, &families)?;
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
    output.push_str("    font_notices: &FONT_NOTICES,\n");
    output.push_str("    assets: &ASSETS,\n");
    output.push_str("};\n");
    Ok(output)
}

/// Emits the official rendering font notices as the `FontNotice` table the
/// renderer reads attribution and license terms from, with one `const` per
/// distinct terms text. Every asset family must have exactly one notice; the
/// Noto Emoji fallback face follows them with the attribution its sealed
/// notice states.
fn generate_font_notices(
    output: &mut String,
    base: &Path,
    scene_surface: &Value,
    families: &BTreeSet<&str>,
) -> Result<(), String> {
    let mut noticed = BTreeSet::new();
    for notice in FONT_NOTICES {
        if !noticed.insert(notice.family) {
            return Err(format!(
                "font family {} has more than one notice",
                notice.family
            ));
        }
    }
    if noticed != *families {
        return Err(format!(
            "font notices cover {noticed:?} but the catalog assets belong to {families:?}"
        ));
    }
    if families.contains(NOTO_EMOJI_FAMILY) {
        return Err(format!(
            "the Noto Emoji fallback family {NOTO_EMOJI_FAMILY} must not be a catalog asset family"
        ));
    }

    let mut terms = BTreeMap::new();
    for notice in FONT_NOTICES {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            terms.entry(notice.terms.rust_const())
        {
            entry.insert(font_terms(base, scene_surface, notice.terms)?);
        }
    }
    for (name, text) in &terms {
        output.push_str(&format!(
            "#[rustfmt::skip]\nconst {name}: &str = {text:?};\n\n"
        ));
    }

    output.push_str(&format!(
        "#[rustfmt::skip]\nstatic FONT_NOTICES: [FontNotice; {}] = [\n",
        FONT_NOTICES.len() + 1
    ));
    for notice in FONT_NOTICES {
        let (license_identity, _, _) = family_metadata(notice.family)?;
        output.push_str("    FontNotice {\n");
        rust_field(output, "family", notice.family);
        rust_field(output, "license_identity", license_identity);
        output.push_str(&format!(
            "        license: FontLicense::{:?},\n",
            notice.license
        ));
        rust_field(output, "attribution", &notice.artifact_attribution());
        rust_field(output, "terms_identity", notice.terms.identity());
        output.push_str(&format!("        terms: {},\n", notice.terms.rust_const()));
        output.push_str("    },\n");
    }
    output.push_str("    FontNotice {\n");
    rust_field(output, "family", NOTO_EMOJI_FAMILY);
    rust_field(output, "license_identity", NOTO_EMOJI_LICENSE_IDENTITY);
    output.push_str("        license: FontLicense::Ofl11,\n");
    rust_field(
        output,
        "attribution",
        &noto_emoji_attribution(scene_surface)?,
    );
    rust_field(output, "terms_identity", FontTerms::Ofl11.identity());
    output.push_str(&format!(
        "        terms: {},\n",
        FontTerms::Ofl11.rust_const()
    ));
    output.push_str("    },\n");
    output.push_str("];\n");
    Ok(())
}

/// The copyright statement the sealed `noto-emoji.ofl-full` notice opens
/// with, before the OFL terms it goes on to state: the attribution the Noto
/// Emoji subset notice carries. Its line breaks become one line and it ends
/// in a period, as the bundle's font notices do.
fn noto_emoji_attribution(scene_surface: &Value) -> Result<String, String> {
    let notice = scene_surface["noticeCatalog"]
        .as_array()
        .ok_or_else(|| "notice catalog must be an array".to_owned())?
        .iter()
        .find(|notice| notice["id"] == NOTO_EMOJI_NOTICE_ID)
        .ok_or_else(|| format!("notice catalog lacks {NOTO_EMOJI_NOTICE_ID}"))?;
    let text = string(notice, "text", "notice")?;
    let terms = text
        .find(OFL_TERMS_START)
        .ok_or_else(|| format!("the {NOTO_EMOJI_NOTICE_ID} notice does not state the OFL terms"))?;
    let mut attribution = text[..terms]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if attribution.is_empty() {
        return Err(format!(
            "the {NOTO_EMOJI_NOTICE_ID} notice states no copyright before the OFL terms"
        ));
    }
    if !attribution.ends_with('.') {
        attribution.push('.');
    }
    Ok(attribution)
}

/// One Xiaolai face as the pinned upstream index declares it: the WOFF2 file
/// name its `import` names and the CSS `unicode-range` its descriptor declares.
#[derive(Debug, Eq, PartialEq)]
struct XiaolaiIndexFace {
    file_name: String,
    unicode_ranges: Vec<(u32, u32)>,
}

/// Parses the pinned `Xiaolai-index.ts` into faces in `XiaolaiFontFaces`
/// declaration order, resolving each descriptor's `uri: _N` binding to the file
/// name its import names. Every import must be declared as exactly one face.
fn parse_xiaolai_index(source: &str) -> Result<Vec<XiaolaiIndexFace>, String> {
    let mut imports = BTreeMap::new();
    for line in source.lines() {
        let Some(import) = line.strip_prefix("import _") else {
            continue;
        };
        let (binding, file_name) = import
            .split_once(" from \"./")
            .and_then(|(binding, tail)| Some((binding, tail.strip_suffix("\";")?)))
            .ok_or_else(|| format!("malformed Xiaolai import: {line}"))?;
        if imports.insert(binding, file_name).is_some() {
            return Err(format!("Xiaolai import _{binding} is declared twice"));
        }
    }
    let declarations = source
        .split_once("export const XiaolaiFontFaces")
        .ok_or_else(|| "Xiaolai index declares no XiaolaiFontFaces".to_owned())?
        .1;
    let mut faces = Vec::new();
    let mut declared = BTreeSet::new();
    for descriptor in declarations.split("uri: _").skip(1) {
        let (binding, descriptor) = descriptor
            .split_once(',')
            .ok_or_else(|| "Xiaolai face descriptor has an unterminated uri".to_owned())?;
        let file_name = *imports
            .get(binding)
            .ok_or_else(|| format!("Xiaolai face _{binding} is not imported"))?;
        if !declared.insert(binding) {
            return Err(format!("Xiaolai face _{binding} is declared twice"));
        }
        let range = descriptor
            .split_once("unicodeRange:")
            .and_then(|(_, after_key)| after_key.split_once('"'))
            .and_then(|(_, after_quote)| after_quote.split_once('"'))
            .map(|(range, _)| range)
            .ok_or_else(|| format!("Xiaolai face _{binding} declares no unicodeRange"))?;
        let unicode_ranges = parse_unicode_ranges(range)
            .map_err(|error| format!("Xiaolai face _{binding} ({file_name}) {error}"))?;
        faces.push(XiaolaiIndexFace {
            file_name: file_name.to_owned(),
            unicode_ranges,
        });
    }
    if let Some(binding) = imports.keys().find(|binding| !declared.contains(*binding)) {
        return Err(format!("Xiaolai import _{binding} declares no face"));
    }
    Ok(faces)
}

const XIAOLAI_INDEX_PATH: &str = "crates/excalidraw-fonts/catalog/order/Xiaolai-index.ts";
const XIAOLAI_FACES_GENERATED_PATH: &str =
    "crates/excalidraw-fonts/src/generated/xiaolai_faces_1.rs";

/// Reads the pinned Xiaolai index, checks its digest, and renders the Rust face
/// table `excalidraw-fonts` includes, so `verify` can compare it against the checked-in
/// generated file and `import`/`materialize` can write it.
fn generate_xiaolai_faces_rust(root: &Path) -> Result<String, String> {
    let path = root.join(XIAOLAI_INDEX_PATH);
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if sha256(&bytes) != XIAOLAI_ORDER_SHA256 {
        return Err("pinned Xiaolai declaration order digest is invalid".to_owned());
    }
    let source =
        std::str::from_utf8(&bytes).map_err(|_| "Xiaolai index is not UTF-8".to_owned())?;
    let faces = parse_xiaolai_index(source)?;
    let mut output = String::from(
        "// Generated by `cargo run --locked -p xtask -- catalog import` or `catalog materialize`\n\
         // from `catalog/order/Xiaolai-index.ts`; do not edit.\n\n",
    );
    output.push_str(&format!(
        "/// Every Xiaolai official font chunk in pinned face order with the Unicode\n\
         /// range its upstream declaration pins. `xtask catalog verify` pins the index\n\
         /// digest and regenerates this table, so it can only change with the index.\n\
         #[rustfmt::skip]\nstatic XIAOLAI_FACES: [FontFaceDeclaration; {}] = [\n",
        faces.len()
    ));
    for face in &faces {
        output.push_str("    FontFaceDeclaration {\n");
        rust_field(&mut output, "file_name", &face.file_name);
        output.push_str("        unicode_ranges: &[\n");
        for line in face.unicode_ranges.chunks(8) {
            output.push_str("           ");
            for (start, end) in line {
                output.push_str(&format!(" (0x{start:04x}, 0x{end:04x}),"));
            }
            output.push('\n');
        }
        output.push_str("        ],\n    },\n");
    }
    output.push_str("];\n");
    Ok(output)
}

/// Parses one CSS `unicode-range` value into ascending inclusive scalar ranges,
/// with the grammar `excalidraw_fonts` applies at compile time to
/// its hand-pinned faces; `unicode_range_parsers_agree` keeps them aligned.
fn parse_unicode_ranges(declared: &str) -> Result<Vec<(u32, u32)>, String> {
    declared
        .split(',')
        .map(|token| {
            let value = token
                .strip_prefix("U+")
                .or_else(|| token.strip_prefix("u+"))
                .ok_or_else(|| format!("unicode-range token {token:?} does not start with U+"))?;
            let (start, end) = value.split_once('-').unwrap_or((value, value));
            let parse = |digits: &str| {
                if digits.is_empty()
                    || digits.len() > 6
                    || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err(format!("unicode-range token {token:?} is not hexadecimal"));
                }
                u32::from_str_radix(digits, 16)
                    .map_err(|_| format!("unicode-range token {token:?} is not hexadecimal"))
            };
            let (start, end) = (parse(start)?, parse(end)?);
            if end > 0x10ffff {
                return Err(format!("unicode-range token {token:?} exceeds U+10FFFF"));
            }
            if start > end {
                return Err(format!("unicode-range token {token:?} is descending"));
            }
            Ok((start, end))
        })
        .collect()
}

fn generated_scene_surface(scene_surface: &Value) -> Result<String, String> {
    let mut output = String::from(
        "// Generated by `cargo run --locked -p xtask -- catalog import` or `catalog materialize`; do not edit.\n\n",
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
        rust_field(
            output,
            "participation",
            string(&field["scope"], "participation", "scene field scope")?,
        );
        rust_string_values(
            output,
            "formats",
            &field["scope"]["formats"],
            "scene field scope",
        )?;
        generate_scene_field_fallback(output, "invalid", &field["invalid"])?;
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

    output.push_str(
        "#[rustfmt::skip]\nstatic SCENE_FALLBACK_RECIPE_POLICIES: &[SceneFallbackRecipePolicy] = &[\n",
    );
    for recipe in scene_surface["fallbackRecipes"]
        .as_array()
        .ok_or_else(|| "scene-surface fallbackRecipes must be an array".to_owned())?
    {
        output.push_str("    SceneFallbackRecipePolicy {\n");
        rust_field(
            output,
            "content_kind",
            string(&recipe["content"], "kind", "scene fallback recipe content")?,
        );
        rust_string_values(
            output,
            "notices",
            &recipe["noticeIds"],
            "scene fallback recipe noticeIds",
        )?;
        output.push_str("        paints: [");
        for format in ["svg", "png", "pdf"] {
            let disposition = string(
                &recipe["content"]["format"],
                format,
                "scene fallback recipe content format",
            )?;
            match disposition {
                "paint" => output.push_str("true,"),
                "omit" => output.push_str("false,"),
                other => {
                    return Err(format!(
                        "unknown scene fallback recipe format disposition {other}"
                    ))
                }
            }
        }
        output.push_str("],\n");
        output.push_str("    },\n");
    }
    output.push_str("];\n\n");

    output.push_str("#[rustfmt::skip]\nstatic SCENE_NOTICE_POLICIES: &[SceneNoticePolicy] = &[\n");
    for notice in scene_surface["noticeCatalog"]
        .as_array()
        .ok_or_else(|| "scene-surface noticeCatalog must be an array".to_owned())?
    {
        output.push_str("    SceneNoticePolicy {\n");
        rust_field(output, "identity", string(notice, "id", "scene notice")?);
        rust_field(
            output,
            "asset_identity",
            string(notice, "assetIdentity", "scene notice")?,
        );
        rust_string_values(
            output,
            "required_formats",
            &notice["requiredFormats"],
            "scene notice requiredFormats",
        )?;
        rust_field(output, "text", string(notice, "text", "scene notice")?);
        output.push_str("    },\n");
    }
    output.push_str("];\n\n");

    output.push_str(
        "pub(crate) static SCENE_SURFACE_POLICY: SceneSurfacePolicy = SceneSurfacePolicy {\n",
    );
    output.push_str("    record_kinds: SCENE_RECORD_POLICIES,\n");
    output.push_str("    fields: SCENE_FIELD_POLICIES,\n");
    output.push_str("    element_kinds: SCENE_ELEMENT_POLICIES,\n");
    output.push_str("    fallback_recipes: SCENE_FALLBACK_RECIPE_POLICIES,\n");
    output.push_str("    notices: SCENE_NOTICE_POLICIES,\n");
    output.push_str("};\n\n");
    Ok(())
}

fn generate_scene_field_fallback(
    output: &mut String,
    name: &str,
    fallback: &Value,
) -> Result<(), String> {
    output.push_str(&format!("        {name}: SceneFieldFallback {{\n"));
    for field in ["code", "recovery", "recipe"] {
        generate_optional_rust_string(output, field, &fallback[field], 12)?;
    }
    rust_string_values(
        output,
        "effects",
        &fallback["effects"],
        "scene field fallback effects",
    )?;
    output.push_str("            format_recoveries: [");
    for format in ["svg", "png", "pdf"] {
        match fallback["formatRules"][format]["recovery"].as_str() {
            Some(recovery) => output.push_str(&format!("Some({recovery:?}),")),
            None => output.push_str("None,"),
        }
    }
    output.push_str("],\n");
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
    const CLASSES: [&str; 9] = [
        "mapped-scalar",
        "range-boundary",
        "empty-intersection",
        "full-intersection",
        "non-bmp",
        "combining-mark",
        "composite-closure",
        "layout-closure-heavy",
        "default-ignorable-no-space",
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
            != json!("522b5b9a6f1ae71af1ce72ddda9db2c6aca6607fc7567b1362ec449c2339abd0")
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
    if sources.len() != QUALIFICATION_SOURCES || assets.len() != 230 {
        return Err("font subset qualification source inventory is incomplete".to_owned());
    }
    let (chunk_sources, sfnt_sources) = sources.split_at(assets.len());
    let mut source_ids = BTreeSet::new();
    let mut cell_ids = BTreeSet::new();
    let expected_sfnt = json!({
        "sourceIdentity": format!("{NOTO_EMOJI_UPSTREAM_PATH}@sha256:{NOTO_EMOJI_SHA256}"),
        "upstreamPath": NOTO_EMOJI_UPSTREAM_PATH,
        "completeSha256": NOTO_EMOJI_SHA256,
    });
    for (source, asset) in chunk_sources
        .iter()
        .zip(assets.iter().map(|asset| (asset, "woff2", false)))
        .chain(sfnt_sources.iter().zip([(&expected_sfnt, "sfnt", true)]))
    {
        let (asset, container, viewer_woff2) = asset;
        let source_id = string(source, "sourceIdentity", "font subset source")?;
        if source_id
            != string(asset, "identity", "catalog asset")
                .or_else(|_| string(asset, "sourceIdentity", "fallback source"))?
            || source["upstreamPath"] != asset["upstreamPath"]
            || source["sourceSha256"] != asset["completeSha256"]
            || source["sourceContainer"] != json!(container)
            || source.get("viewerSourceWoff2Sha256").is_some() != viewer_woff2
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
                        && !has("reason")
                }
                "not_selected" => {
                    cell["expectedValidationOutcome"] == json!("not_selected")
                        && has("orderedScalars")
                        && !has("semanticSubsetIdentity")
                        && !has("generatedWoff2Sha256")
                        && !has("reason")
                }
                "not_applicable" => {
                    cell["expectedValidationOutcome"] == json!("not_applicable")
                        && !has("orderedScalars")
                        && !has("semanticSubsetIdentity")
                        && !has("generatedWoff2Sha256")
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
    if source_ids.len() != QUALIFICATION_SOURCES
        || cell_ids.len() != QUALIFICATION_SOURCES * CLASSES.len()
    {
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

/// Verifies every catalog asset against the reference manifest and font files,
/// returning each complete source keyed by upstream path for the checks that
/// need the bytes themselves.
fn verify_assets(root: &Path, catalog: &Value) -> Result<BTreeMap<String, Vec<u8>>, String> {
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
    let mut expected_files = BTreeSet::new();
    let mut sources = BTreeMap::new();
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
        let group = font_package_group(asset)?;
        let file = root.join(FONT_PACKAGES[group]).join(path);
        expected_files.insert(file.clone());
        let complete = fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        if complete.len() != *expected_length || sha256(&complete) != *expected_digest {
            return Err(format!("font file digest mismatch for {path}"));
        }
        sources.insert(path.to_owned(), complete);
    }
    if paths != expected.keys().map(String::as_str).collect() {
        return Err("revision catalog asset inventory is incomplete".to_owned());
    }
    let mut actual_files = BTreeSet::new();
    for package in FONT_PACKAGES {
        actual_files.extend(font_files(&root.join(package).join("fonts"))?);
    }
    if actual_files != expected_files {
        return Err("font packages contain missing or unmanifested files".into());
    }
    Ok(sources)
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
        // Orchestration source is capture-time provenance. The locked package,
        // browser entry, invocation and reference output still bind behavior.
        if verify_files && path != "tools/reference-oracle/scripts/frame-conformance-case.mjs" {
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
        bundle.push_str(
            "==============================================================================\n",
        );
        bundle.push_str(title);
        bundle.push_str(
            "\n==============================================================================\n\n",
        );
        bundle.push_str(&read_license_part(base, file)?);
        bundle.push_str("\n\n");
    }
    bundle.push_str("==============================================================================\nOfficial rendering font notices\n==============================================================================\n\n");
    for notice in FONT_NOTICES {
        bundle.push_str(notice.heading);
        bundle.push('\n');
        bundle.push_str(&notice.bundle_line());
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

/// One license text as the bundle prints it: the file under `license-parts`
/// (or, for `../`-prefixed files, under the excalidraw-fonts crate) without its
/// trailing line terminators.
fn read_license_part(base: &Path, file: &str) -> Result<String, String> {
    let path = if file.starts_with("../") {
        base.join(file)
    } else {
        base.join("license-parts").join(file)
    };
    let part = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(part.trim_end_matches(['\r', '\n']).to_owned())
}

/// The license body a font notice's artifacts carry, as text: the bundle's
/// copy of the license. The OFL body must be the license every sealed
/// `*.ofl-full` notice goes on to state after its copyright lines, so the two
/// distribution copies cannot drift apart.
fn font_terms(base: &Path, scene_surface: &Value, terms: FontTerms) -> Result<String, String> {
    let text = read_license_part(base, terms.bundle_part())?;
    if terms == FontTerms::Ofl11 {
        for notice in scene_surface["noticeCatalog"]
            .as_array()
            .ok_or_else(|| "notice catalog must be an array".to_owned())?
            .iter()
            .filter(|notice| {
                notice["id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with(".ofl-full"))
            })
        {
            let id = string(notice, "id", "notice")?;
            let stated = string(notice, "text", "notice")?;
            let start = stated
                .find("SIL OPEN FONT LICENSE Version 1.1")
                .ok_or_else(|| format!("the {id} notice does not state the OFL text"))?;
            if stated[start..] != text {
                return Err(format!(
                    "the {id} notice states an OFL text other than license-parts/OFL-1.1.txt"
                ));
            }
        }
    }
    Ok(text)
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

    use super::FontLicense;
    use super::{
        carry_approval, generate_scene_surface_rust, parse_unicode_ranges, parse_xiaolai_index,
        review_decode, reviewed_decodes_record, reviewed_decodes_subject, verify_scene_surface,
        XiaolaiIndexFace, EXCALIFONT_BASIC_LATIN_PATH, EXCALIFONT_BASIC_LATIN_SHA256, FONT_NOTICES,
    };

    #[test]
    fn dominant_cmap_bucket_uses_upstream_ties_and_long_tail_rules() {
        let classify =
            |points: &[u32]| super::dominant_xiaolai_shard(points.iter().copied().collect());
        assert_eq!(classify(&[0x4e00, 0x4e01, 0x3400]).unwrap(), 1);
        assert_eq!(classify(&[0x4e00, 0x3400, 0x3401]).unwrap(), 2);
        // First encountered block wins a tie, not bucket-table order or the
        // last block. Other precedes CJK Unified here despite being listed last.
        assert_eq!(classify(&[0x1100, 0x4e00]).unwrap(), 2);
        assert_eq!(classify(&[0x3042, 0xf900]).unwrap(), 1);
        for cp in [0x3400, 0x4dbf, 0xf900, 0xfaff, 0x20000, 0x3ffff, 0x1100] {
            assert_eq!(classify(&[cp]).unwrap(), 2);
        }
        for cp in [
            0x3042, 0x30a2, 0x3105, 0x3131, 0x4e00, 0x9fff, 0xac00, 0xd7af,
        ] {
            assert_eq!(classify(&[cp]).unwrap(), 1);
        }
        assert!(classify(&[]).is_err());
        assert!(super::xiaolai_shard(b"not a font").is_err());
    }

    #[test]
    fn importer_reproduces_all_three_checked_in_payloads_byte_for_byte() {
        let root = crate::conformance::repo_root();
        let reference = super::read_canonical_json(
            &root.join("tests/oracle/reference-profiles/revision-1-assets.json"),
            "reference assets",
        )
        .unwrap();
        let assets = super::expected_assets(&reference).unwrap();
        let mut lengths = [0; 3];
        for asset in assets {
            let bytes = std::fs::read(
                root.join(super::FONT_PACKAGES[asset.shard])
                    .join(&asset.path),
            )
            .unwrap();
            assert_eq!(bytes, asset.bytes);
            lengths[asset.shard] += bytes.len();
        }
        assert_eq!(lengths, super::SHARD_LENGTHS);
    }

    /// The bundle prints each family's attribution, license sentence and
    /// postscript as one line; artifacts carry the same line without the
    /// license sentence, wherever in the line it sits.
    #[test]
    fn font_notices_project_one_line_for_the_bundle_and_one_attribution_for_artifacts() {
        let bundle = include_str!("../../crates/excalidraw-fonts/LICENSE-BUNDLE.txt");
        for notice in FONT_NOTICES {
            let expected_line = bundle
                .split_once(&format!("\n{}\n", notice.heading))
                .and_then(|(_, rest)| rest.lines().next())
                .unwrap_or_else(|| panic!("{} has a bundle line", notice.heading));
            assert_eq!(notice.bundle_line(), expected_line, "{}", notice.heading);
        }

        let by_family = |family: &str| {
            FONT_NOTICES
                .iter()
                .find(|notice| notice.family == family)
                .expect("family has a notice")
        };
        assert_eq!(
            by_family("Cascadia").artifact_attribution(),
            "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code."
        );
        assert_eq!(
            by_family("ComicShanns").artifact_attribution(),
            "Copyright (c) 2018 Shannon Miwa."
        );
        assert_eq!(
            by_family("Excalifont").artifact_attribution(),
            "Copyright (c) 2024 by Excalidraw. Excalifont is derived from Virgil by Your Own Font Foundry, with modifications by Jan Filipek / DizajnDesign. Excalifont and Virgil provenance is retained together."
        );
        assert_eq!(
            by_family("Xiaolai").artifact_attribution(),
            "Copyright (c) 2020 LXGW. Designed by Nozomi Seto. The shipped subsets remove non-CJK codepoints, reduce Hangul side-bearings to 40%, and center improperly centered CJK codepoints."
        );
        // Every OFL family refers to the one shared OFL body; Comic Shanns to
        // its own MIT text, which carries its copyright line (#151).
        for notice in FONT_NOTICES {
            let expected = match notice.license {
                FontLicense::Ofl11 => ("license.ofl-1.1", "OFL-1.1.txt"),
                FontLicense::Mit => ("license.comic-shanns.mit", "comic-shanns-MIT.txt"),
            };
            assert_eq!(
                (notice.terms.identity(), notice.terms.bundle_part()),
                expected,
                "{}",
                notice.family
            );
        }
    }

    /// The scene-surface emitter carries the notice catalog and each fallback
    /// recipe's notice obligations, so render-time notice selection reads
    /// tables instead of the JSON manifest.
    #[test]
    fn scene_surface_emitter_carries_notices_and_recipe_notice_obligations() {
        let mut output = String::new();
        generate_scene_surface_rust(&mut output, &manifest()).expect("manifest generates");

        assert!(output.contains("static SCENE_NOTICE_POLICIES: &[SceneNoticePolicy] = &[\n"));
        assert!(output.contains("        identity: \"font-awesome.ban.cc-by-4.0\",\n"));
        assert!(output.contains("        required_formats: &[\"svg\",\"pdf\",],\n"));
        assert!(output.contains(
            "static SCENE_FALLBACK_RECIPE_POLICIES: &[SceneFallbackRecipePolicy] = &[\n"
        ));
        assert!(output.contains(
            "        content_kind: \"image-error\",\n        notices: &[\"font-awesome.ban.cc-by-4.0\",\"font-awesome.image.cc-by-4.0\",],\n        paints: [false,true,false,],\n"
        ));
        assert!(output.contains(
            "        content_kind: \"iframe-embeddable\",\n        notices: &[\"liberation-sans.ofl-full\",\"noto-emoji.ofl-full\",],\n        paints: [true,true,true,],\n"
        ));
        assert!(output.contains("    fallback_recipes: SCENE_FALLBACK_RECIPE_POLICIES,\n"));
        assert!(output.contains("    notices: SCENE_NOTICE_POLICIES,\n"));
    }

    /// A pinned-index excerpt covering both descriptor layouts the upstream
    /// formatter emits: the wrapped `unicodeRange:` on its own line and the
    /// single-line `{ uri: _N, descriptors: { unicodeRange: "..." } }` form.
    const XIAOLAI_INDEX_EXCERPT: &str = r#"// generated header
import _0 from "./Xiaolai-Regular-aaaa.woff2";
import _1 from "./Xiaolai-Regular-bbbb.woff2";
import _2 from "./Xiaolai-Regular-cccc.woff2";

import { type ExcalidrawFontFaceDescriptor } from "../Fonts";

export const XiaolaiFontFaces: ExcalidrawFontFaceDescriptor[] = [
  {
    uri: _0,
    descriptors: {
      unicodeRange:
        "U+f9b8-fa6d,U+fe32,U+fe45-fe4f",
    },
  },
  { uri: _1, descriptors: { unicodeRange: "U+d7eb-d7fb,U+f900-f9b7" } },
  {
    uri: _2,
    descriptors: {
      unicodeRange:
        "U+7f3b",
    },
  },
];
"#;

    #[test]
    fn xiaolai_index_yields_faces_in_declaration_order_with_parsed_ranges() {
        let faces = parse_xiaolai_index(XIAOLAI_INDEX_EXCERPT).expect("excerpt parses");
        assert_eq!(
            faces,
            vec![
                XiaolaiIndexFace {
                    file_name: "Xiaolai-Regular-aaaa.woff2".to_owned(),
                    unicode_ranges: vec![(0xf9b8, 0xfa6d), (0xfe32, 0xfe32), (0xfe45, 0xfe4f)],
                },
                XiaolaiIndexFace {
                    file_name: "Xiaolai-Regular-bbbb.woff2".to_owned(),
                    unicode_ranges: vec![(0xd7eb, 0xd7fb), (0xf900, 0xf9b7)],
                },
                XiaolaiIndexFace {
                    file_name: "Xiaolai-Regular-cccc.woff2".to_owned(),
                    unicode_ranges: vec![(0x7f3b, 0x7f3b)],
                },
            ]
        );
    }

    fn assert_xiaolai_index_rejected(expected: &str, mutate: impl FnOnce(&str) -> String) {
        let mutated = mutate(XIAOLAI_INDEX_EXCERPT);
        assert_ne!(
            mutated, XIAOLAI_INDEX_EXCERPT,
            "mutation must change the excerpt"
        );
        let error = parse_xiaolai_index(&mutated).expect_err("mutated index must fail closed");
        assert!(
            error.contains(expected),
            "expected {expected:?} in error {error:?}"
        );
    }

    #[test]
    fn xiaolai_index_rejects_an_import_that_declares_no_face() {
        assert_xiaolai_index_rejected("import _1 declares no face", |source| {
            source.replace(
                "  { uri: _1, descriptors: { unicodeRange: \"U+d7eb-d7fb,U+f900-f9b7\" } },\n",
                "",
            )
        });
    }

    #[test]
    fn xiaolai_index_rejects_a_face_declared_twice() {
        assert_xiaolai_index_rejected("face _1 is declared twice", |source| {
            source.replace("uri: _2,", "uri: _1,")
        });
    }

    #[test]
    fn xiaolai_index_rejects_a_face_whose_binding_is_not_imported() {
        assert_xiaolai_index_rejected("face _3 is not imported", |source| {
            source.replace("uri: _2,", "uri: _3,")
        });
    }

    #[test]
    fn xiaolai_index_rejects_a_descriptor_without_a_unicode_range() {
        assert_xiaolai_index_rejected("face _1 declares no unicodeRange", |source| {
            source.replace(
                "descriptors: { unicodeRange: \"U+d7eb-d7fb,U+f900-f9b7\" }",
                "descriptors: {}",
            )
        });
    }

    #[test]
    fn xiaolai_index_rejects_a_malformed_unicode_range() {
        assert_xiaolai_index_rejected("is descending", |source| {
            source.replace("U+d7eb-d7fb", "U+d7fb-d7eb")
        });
        assert_xiaolai_index_rejected("is not hexadecimal", |source| {
            source.replace("U+d7eb-d7fb", "U+d7eb-d7zz")
        });
        assert_xiaolai_index_rejected("exceeds U+10FFFF", |source| {
            source.replace("U+d7eb-d7fb", "U+110000")
        });
        assert_xiaolai_index_rejected("does not start with U+", |source| {
            source.replace("U+d7eb-d7fb", "d7eb-d7fb")
        });
    }

    /// The generator's runtime grammar and the compile-time grammar
    /// `excalidraw_fonts` applies to its hand-pinned faces must
    /// agree, or a re-pinned Xiaolai index could parse differently from the CSS
    /// the other families carry. Nunito's five faces exercise the upper-case
    /// Google Fonts subsets; Excalifont's Latin face the lower-case upstream form.
    #[test]
    fn unicode_range_parsers_agree() {
        const NUNITO_CSS: [&str; 5] = [
            "U+0460-052F,U+1C80-1C88,U+20B4,U+2DE0-2DFF,U+A640-A69F,U+FE2E-FE2F",
            "U+0301,U+0400-045F,U+0490-0491,U+04B0-04B1,U+2116",
            "U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB",
            "U+0100-02AF,U+0304,U+0308,U+0329,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF",
            "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD",
        ];
        const EXCALIFONT_LATIN_CSS: &str = "U+20-7e,U+a0-a3,U+a5-a6,U+a8-ab,U+ad-b1,U+b4,U+b6-b8,U+ba-ff,U+131,U+152-153,U+2bc,U+2c6,U+2da,U+2dc,U+304,U+308,U+2013-2014,U+2018-201a,U+201c-201e,U+2020,U+2022,U+2024-2026,U+2030,U+2039-203a,U+20ac,U+2122,U+2212";

        let catalog = excalidraw_fonts::revision_1();
        let mut nunito = catalog
            .assets
            .iter()
            .filter(|asset| asset.family == "Nunito")
            .collect::<Vec<_>>();
        nunito
            .sort_by_key(|asset| excalidraw_fonts::font_face_order("Nunito", asset.upstream_path));
        assert_eq!(nunito.len(), NUNITO_CSS.len());
        for (asset, css) in nunito.iter().zip(NUNITO_CSS) {
            let compile_time =
                excalidraw_fonts::font_face_unicode_ranges("Nunito", asset.upstream_path)
                    .expect("Nunito face declares ranges");
            assert_eq!(parse_unicode_ranges(css).expect("CSS parses"), compile_time);
        }

        let excalifont_latin = excalidraw_fonts::font_face_unicode_ranges(
            "Excalifont",
            "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
        )
        .expect("Excalifont Latin face declares ranges");
        assert_eq!(
            parse_unicode_ranges(EXCALIFONT_LATIN_CSS).expect("CSS parses"),
            excalifont_latin
        );
    }

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

    fn excalifont_basic_latin_source() -> Vec<u8> {
        excalidraw_fonts::load_asset(EXCALIFONT_BASIC_LATIN_PATH)
            .expect("checked-in shards retain their digests")
            .expect("the Excalifont basic-Latin chunk is manifested")
            .bytes()
            .to_vec()
    }

    fn excalifont_reviewed_decodes(approval: Value) -> Value {
        let decode = review_decode(
            EXCALIFONT_BASIC_LATIN_PATH,
            &excalifont_basic_latin_source(),
        )
        .expect("the manifested chunk decodes");
        reviewed_decodes_record(&[decode], approval)
    }

    fn approval(subject: &str, url: &str) -> Value {
        json!({
            "approval_subject_sha256": format!("sha256:{subject}"),
            "approval_url": url,
            "approved_at": "2026-09-07",
            "approved_by": "sagikazarmark",
        })
    }

    /// #58 approved the Excalifont basic-Latin decode after an independent
    /// byte-level review of exactly these output bytes. Reviewing the chunk
    /// again must land on that approved output, so the one chunk with a
    /// checked-in decode anchors the derivation for the other 229.
    #[test]
    fn review_decode_reproduces_the_independently_reviewed_excalifont_decode() {
        let decode = review_decode(
            EXCALIFONT_BASIC_LATIN_PATH,
            &excalifont_basic_latin_source(),
        )
        .expect("the manifested chunk decodes");

        assert_eq!(
            decode.identity,
            format!("{EXCALIFONT_BASIC_LATIN_PATH}@sha256:{EXCALIFONT_BASIC_LATIN_SHA256}")
        );
        assert_eq!(decode.upstream_path, EXCALIFONT_BASIC_LATIN_PATH);
        assert_eq!(decode.source_length, 24_956);
        assert_eq!(decode.source_sha256, EXCALIFONT_BASIC_LATIN_SHA256);
        assert_eq!(decode.decoded_length, 61_948);
        assert_eq!(
            decode.decoded_sha256,
            "a648fcf03710b94a05748575a4bc5ddb056b19806869326f140a7d4ccf3f8bbb"
        );
    }

    #[test]
    fn review_decode_fails_closed_on_a_source_that_does_not_decode() {
        let source = excalifont_basic_latin_source();
        let error = review_decode(EXCALIFONT_BASIC_LATIN_PATH, &source[..source.len() / 2])
            .expect_err("a truncated source has no reviewed decode");
        assert!(error.contains(EXCALIFONT_BASIC_LATIN_PATH));
    }

    /// The approval is excluded from its own subject, so recording it does not
    /// move the subject; any recorded digest moving does, which is what makes
    /// the record's approval bind exactly the bytes it approved.
    #[test]
    fn reviewed_decodes_approval_subject_excludes_the_approval_and_moves_with_a_digest() {
        let pending = excalifont_reviewed_decodes(Value::Null);
        let subject = reviewed_decodes_subject(&pending).expect("subject derives");
        let approved = excalifont_reviewed_decodes(approval(
            &subject,
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/120#issuecomment-1",
        ));
        assert_eq!(
            reviewed_decodes_subject(&approved).expect("subject derives"),
            subject
        );

        let mut moved = pending.clone();
        moved["chunks"][0]["decodedSha256"] =
            json!("0000000000000000000000000000000000000000000000000000000000000000");
        assert_ne!(
            reviewed_decodes_subject(&moved).expect("subject derives"),
            subject
        );
    }

    /// `materialize` and `import` re-derive the record from the shards; an
    /// approval survives that only while the bytes it approved do.
    #[test]
    fn rematerializing_keeps_an_approval_only_while_its_subject_holds() {
        let pending = excalifont_reviewed_decodes(Value::Null);
        let subject = reviewed_decodes_subject(&pending).expect("subject derives");
        let existing = excalifont_reviewed_decodes(approval(
            &subject,
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/120#issuecomment-1",
        ));

        let mut unchanged = pending.clone();
        carry_approval(Some(&existing), &mut unchanged).expect("approval carries");
        assert_eq!(unchanged["approval"], existing["approval"]);

        let mut moved = pending.clone();
        moved["chunks"][0]["decodedSha256"] =
            json!("0000000000000000000000000000000000000000000000000000000000000000");
        carry_approval(Some(&existing), &mut moved).expect("a moved record is not an error");
        assert!(moved["approval"].is_null());

        let mut first = pending;
        carry_approval(None, &mut first).expect("no prior record is not an error");
        assert!(first["approval"].is_null());
    }
}
