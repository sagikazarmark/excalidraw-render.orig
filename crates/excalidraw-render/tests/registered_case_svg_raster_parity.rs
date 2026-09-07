//! SVG document parity for every registered scene case.
//!
//! The direct-PNG harness compares rasters two different rasterizers produced,
//! so a difference there can be the document or the rasterizer. This harness
//! removes the rasterizer from the question: the renderer's SVG and the official
//! export's SVG are both rasterized in-process by the same `resvg`, each with a
//! font database built from its own embedded `@font-face` payloads, and compared
//! under the pure-Rust SVG budgets. What is left is the document itself: path
//! geometry, stroke and fill attributes, text placement and sizing, dark-mode
//! filters, and embedded images.
//!
//! This is document parity, not a canonical viewer gate. The spec's canonical
//! SVG viewers (`canonical-svg.chromium.v1`, `canonical-svg.pure-rust.v1`) are
//! pinned, isolated and declaration-bound; neither currently admits the
//! renderer's or the official export's text-bearing SVG, and running them is a
//! separate piece of work. The workspace `resvg` used here is not the pinned
//! viewer, so its output is never evidence and never approved.
//!
//! Fonts. Both documents embed WOFF2 subsets under CSS families; each payload is
//! decoded to its sfnt and registered under the family the document names, so
//! `Excalifont` in the official export and `Excalifont` in ours resolve to their
//! respective embedded subsets. The official export names some system families
//! without embedding them (`Helvetica`); those are resolved through the
//! renderer's manifested metric-compatible substitute so that the comparison is
//! about placement, not about which face the browser happened to have. A family
//! that is neither embedded nor substituted (`Segoe UI Emoji` in a fallback
//! list) stays unresolved and contributes nothing on either side.

// Included by path rather than through `common/mod.rs` so this binary does not
// also compile the PDF harness helpers, and the PDF binaries do not compile this.
#[path = "common/registered_cases.rs"]
mod registered_cases;

use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;

use base64::Engine;
use excalidraw_render::{OutputFormat, Renderer, SceneDocument};
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, ComparisonReport, RegionReport, SsimOutcome,
};
use resvg::tiny_skia::{Pixmap, Transform};
use serde_json::Value;
use usvg::fontdb;

use registered_cases::{
    observed, repository_root, variant_request, visual_class, PinnedDivergence,
};

/// Diagnosed defects whose fix is pending a maintainer decision. See
/// `PinnedDivergence` for why these are pinned rather than excepted.
const KNOWN_DEFECTS: &[PinnedDivergence] = &[];

/// Compatibility exceptions whose approved tolerance exception the case
/// manifest carries for the direct PNG, observed here on the SVG document as
/// well. The manifest admits no `format: svg` exception, so each is pinned at
/// its exact measurement and must correspond to that recorded exception; see
/// `PinnedDivergence`.
const COMPATIBILITY_EXCEPTION_PINS: &[PinnedDivergence] = &[
    // Spec 8.3's replacement glyph (#124): the official document sets the three
    // unmapped scalars in the embedded Excalifont and Liberation Sans subsets,
    // which have no glyph for them, while the renderer's sets `?` from
    // Liberation Sans, and the differing advances shift the trailing word.
    PinnedDivergence {
        case: "focused.missing-glyph.v1",
        variant: "default",
        issue: "#124",
        changed_pixels: 473,
        ssim_bits: 0x3fea_c711_a5b4_5706,
    },
    // Spec 8.3's Noto Emoji substitution (#121): the official document sets
    // the emoji in the embedded Excalifont and Liberation Sans subsets, which
    // have no glyph for them (its `Segoe UI Emoji` fallback is unresolved
    // here, see the module docs), while the renderer's sets them from the
    // complete monochrome Noto Emoji face it embeds.
    PinnedDivergence {
        case: "focused.emoji-fallback.v1",
        variant: "default",
        issue: "#121",
        changed_pixels: 577,
        ssim_bits: 0x3fea_ff42_54bc_d188,
    },
];

/// System families the official export names without embedding, and the
/// renderer's manifested substitute for each. The official raster evidence was
/// produced with whatever the reference container resolved these to; document
/// parity needs one face on both sides, and the substitute is the face the
/// renderer's own SVG embeds for the same family.
const SYSTEM_FAMILY_SUBSTITUTES: &[(&str, &str)] =
    &[("Helvetica", "fonts/Liberation/LiberationSans-Regular.woff2")];

#[test]
fn every_registered_scene_case_matches_its_official_svg_document() {
    let root = repository_root();
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("tests/conformance/cases/registry.json"))
            .expect("case registry is present"),
    )
    .expect("case registry is valid JSON");

    let mut total = 0usize;
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();

    for case in registry["cases"]
        .as_array()
        .expect("registry cases are an array")
        .iter()
        .filter(|case| case["kind"] == "scene")
    {
        let id = case["id"].as_str().expect("case id is a string");
        let dir = root.join(case["path"].as_str().expect("case path is a string"));
        let manifest: Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("manifest present"))
                .expect("manifest is valid JSON");
        total += 1;

        let scene = SceneDocument::parse(
            &fs::read(dir.join("scene.excalidraw")).expect("scene document present"),
        )
        .unwrap_or_else(|error| panic!("{id} scene is admitted: {error:?}"));
        for variant in manifest["variants"]
            .as_array()
            .expect("manifest variants are an array")
        {
            let variant_id = variant["id"].as_str().expect("variant id is a string");
            seen.insert((id.to_owned(), variant_id.to_owned()));
            let context = format!("{id} [{variant_id}]");
            let request = variant_request(OutputFormat::Svg, &variant["options"], &context);
            let outcome = Renderer::new()
                .render(&scene, &request)
                .unwrap_or_else(|error| panic!("{context} renders as SVG: {error:?}"));
            let candidate_svg =
                std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

            let svg = &variant["formats"]["svg"];
            // A report mismatch is never pinned or excepted: the renderer's
            // diagnostics disagreeing with what the case declares is a contract
            // break, not a document difference. The checked report is canonical
            // JSON and the renderer's is spec-ordered, so they compare as values.
            let expected_report: Value = serde_json::from_slice(
                &fs::read(
                    dir.join(
                        svg["expected_report"]["path"]
                            .as_str()
                            .expect("expected report path"),
                    ),
                )
                .expect("expected report present"),
            )
            .expect("expected report is valid JSON");
            assert_eq!(
                serde_json::to_value(outcome.report()).expect("report serializes"),
                expected_report,
                "{context}: SVG report differs from its declared expectation"
            );
            let official_svg = fs::read_to_string(
                dir.join(svg["artifact"]["path"].as_str().expect("official SVG path")),
            )
            .expect("official SVG present");

            let reference = rasterize(&official_svg, &format!("{context} official"));
            let candidate = rasterize(candidate_svg, &format!("{context} candidate"));

            // The evidence records the size Chromium rasterized the official SVG
            // at; resvg must land on the same size or the comparison is not
            // of the artifact the case declares.
            let raw = &svg["raw_raster"];
            let declared = (
                raw["width"].as_u64().expect("raster width") as u32,
                raw["height"].as_u64().expect("raster height") as u32,
            );
            if (reference.width(), reference.height()) != declared {
                failures.push(format!(
                    "{context}: official SVG rasterizes to {}x{}, evidence declares {}x{}",
                    reference.width(),
                    reference.height(),
                    declared.0,
                    declared.1
                ));
                continue;
            }
            if (candidate.width(), candidate.height()) != declared {
                failures.push(format!(
                    "{context}: candidate SVG rasterizes to {}x{}, official is {}x{}",
                    candidate.width(),
                    candidate.height(),
                    declared.0,
                    declared.1
                ));
                continue;
            }

            let class = visual_class(svg["visual_class"].as_str().expect("visual class"));
            let report = compare(
                &reference,
                &candidate,
                class,
                &[],
                ComparisonMode::PureRustSvg,
            )
            .expect("dimensions agree");
            let defect = PinnedDivergence::lookup(KNOWN_DEFECTS, id, variant_id);
            let exception = PinnedDivergence::lookup(COMPATIBILITY_EXCEPTION_PINS, id, variant_id);
            assert!(
                defect.is_none() || exception.is_none(),
                "{context} is both a known defect and a compatibility exception; a defect has \
                 no non-defect cause to except"
            );
            if exception.is_some() {
                // The pin stands for the manifest's approved judgment on this
                // variant, so that judgment must exist: a pin with no recorded
                // exception would be a suppression.
                assert!(
                    has_recorded_exception(&manifest, variant_id),
                    "{context} carries a compatibility exception pin but its manifest records \
                     no direct-PNG tolerance exception for the variant"
                );
            }
            let divergence = match (defect, exception) {
                (Some(defect), _) => defect.mismatch(&report),
                (None, Some(exception)) => exception.mismatch(&report),
                (None, None) => (!report.passed())
                    .then(|| format!("outside the pure-Rust SVG budget: {}", observed(&report))),
            };
            if let Some(detail) = divergence {
                failures.push(format!("{context}: {detail}"));
            }
        }
    }

    assert!(total > 0, "the registry declared no scene cases");
    PinnedDivergence::assert_all_observed(KNOWN_DEFECTS, &seen);
    PinnedDivergence::assert_all_observed(COMPATIBILITY_EXCEPTION_PINS, &seen);
    assert!(
        failures.is_empty(),
        "{} variant(s) across {total} cases diverge from the official SVG document:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Whether the case manifest records a whole direct-PNG tolerance exception
/// for the variant: the same record `registered_case_png_parity` consumes, and
/// the approved judgment a compatibility exception pin here stands for.
fn has_recorded_exception(manifest: &Value, variant_id: &str) -> bool {
    manifest["exceptions"]
        .as_array()
        .expect("manifest exceptions are an array")
        .iter()
        .any(|exception| {
            exception["variant"] == variant_id
                && exception["format"] == "png"
                && exception["region"] == "whole"
        })
}

/// Rasterizes one SVG document with the workspace `resvg`, resolving text
/// through the faces the document itself embeds.
fn rasterize(svg: &str, context: &str) -> CanonicalRgba8 {
    let mut database = fontdb::Database::new();
    let mut registered = BTreeSet::new();
    for face in embedded_faces(svg) {
        let sfnt = match face.media_type.as_str() {
            "font/woff2" => wuff::decompress_woff2(&face.bytes)
                .unwrap_or_else(|error| panic!("{context}: embedded WOFF2 decodes: {error:?}")),
            _ => face.bytes.clone(),
        };
        register_face(&mut database, sfnt, &face.family);
        registered.insert(face.family);
    }
    for family in referenced_families(svg) {
        if registered.contains(&family) {
            continue;
        }
        if let Some((_, asset)) = SYSTEM_FAMILY_SUBSTITUTES
            .iter()
            .find(|(system, _)| *system == family)
        {
            let woff2 = excalidraw_render_assets_base::load_asset(asset)
                .expect("substitute asset verifies")
                .expect("substitute asset is manifested");
            let sfnt = wuff::decompress_woff2(woff2.bytes()).expect("substitute WOFF2 decodes");
            register_face(&mut database, sfnt, &family);
            registered.insert(family);
        }
    }

    let options = usvg::Options {
        fontdb: Arc::new(database),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(svg, &options)
        .unwrap_or_else(|error| panic!("{context}: SVG parses: {error}"));
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height()).expect("raster size is valid");
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());

    // tiny-skia stores premultiplied pixels; the comparator wants straight
    // alpha, un-premultiplied by the canonical integer rule (spec 14.3,
    // `(c * 255 + alpha / 2) / alpha`) rather than tiny-skia's float division,
    // so that a pinned measurement here is the one the pinned viewer would take.
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for pixel in pixmap.pixels() {
        let alpha = u32::from(pixel.alpha());
        let straight = |channel: u8| -> u8 {
            (u32::from(channel) * 255 + alpha / 2)
                .checked_div(alpha)
                .map_or(0, |value| value.min(255) as u8)
        };
        rgba.extend_from_slice(&[
            straight(pixel.red()),
            straight(pixel.green()),
            straight(pixel.blue()),
            pixel.alpha(),
        ]);
    }
    CanonicalRgba8::new(size.width(), size.height(), rgba).expect("raster is canonical RGBA8")
}

/// Loads one sfnt and registers it under the CSS family the document uses for
/// it, whatever the font's own name table says: a subset chunk of Xiaolai is
/// `Xiaolai` to the document even when its name table says otherwise, and the
/// renderer's aliases (`ExcalidrawRender-9`) name no real family at all.
fn register_face(database: &mut fontdb::Database, sfnt: Vec<u8>, family: &str) {
    for id in database.load_font_source(fontdb::Source::Binary(Arc::new(sfnt))) {
        let mut info = database.face(id).expect("loaded face is present").clone();
        database.remove_face(id);
        info.families = vec![(family.to_owned(), fontdb::Language::English_UnitedStates)];
        database.push_face_info(info);
    }
}

/// One `@font-face` declaration's family and decoded payload.
#[derive(Debug, PartialEq)]
struct EmbeddedFace {
    family: String,
    media_type: String,
    bytes: Vec<u8>,
}

/// Every `@font-face` in the document. Accepts both the official export's
/// spacing (`font-family: Excalifont; src: url(...);`) and the renderer's
/// (`font-family:'Excalifont';src:url(...)`).
fn embedded_faces(svg: &str) -> Vec<EmbeddedFace> {
    let mut faces = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find("@font-face") {
        rest = &rest[start + "@font-face".len()..];
        let Some(open) = rest.find('{') else { break };
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let block = &rest[open + 1..open + close];
        rest = &rest[open + close..];

        let family = declaration(block, "font-family")
            .expect("@font-face declares font-family")
            .trim_matches(|c| c == '\'' || c == '"')
            .to_owned();
        let src = declaration(block, "src").expect("@font-face declares src");
        let url = src
            .strip_prefix("url(")
            .and_then(|url| url.split_once(')'))
            .map(|(url, _)| url)
            .expect("src is a url()");
        let (media_type, encoded) = url
            .strip_prefix("data:")
            .and_then(|data| data.split_once(";base64,"))
            .expect("src is a base64 data URL");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("font data URL is valid base64");
        faces.push(EmbeddedFace {
            family,
            media_type: media_type.to_owned(),
            bytes,
        });
    }
    faces
}

/// The value of one `name: value;` declaration inside a CSS block. A `;` inside
/// parentheses does not end a declaration, since a data URL carries one.
fn declaration<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut entries = Vec::new();
    for (index, character) in block.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                entries.push(&block[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    entries.push(&block[start..]);
    entries.into_iter().find_map(|entry| {
        let (key, value) = entry.split_once(':')?;
        (key.trim() == name).then(|| value.trim())
    })
}

/// Every family named by a `font-family="..."` attribute, split on commas, in
/// document order and without duplicates.
fn referenced_families(svg: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut families = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find(" font-family=\"") {
        rest = &rest[start + " font-family=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        for family in rest[..end].split(',') {
            let family = family.trim().trim_matches(|c| c == '\'' || c == '"');
            if !family.is_empty() && seen.insert(family.to_owned()) {
                families.push(family.to_owned());
            }
        }
        rest = &rest[end..];
    }
    families
}

#[test]
fn a_pinned_divergence_holds_its_variant_to_the_exact_measurement() {
    const PINNED_SSIM_BITS: u64 = 0x3fea_c711_a5b4_5706;
    let pin = PinnedDivergence {
        case: "focused.example.v1",
        variant: "default",
        issue: "#124",
        changed_pixels: 473,
        ssim_bits: PINNED_SSIM_BITS,
    };
    let report = |changed_pixels: u64, ssim: SsimOutcome| ComparisonReport {
        whole: RegionReport {
            width: 1,
            height: 1,
            area: 1,
            changed_pixels,
            changed_mask: Vec::new(),
            ssim,
            changed_pixels_pass: false,
            ssim_pass: false,
        },
        crops: Vec::new(),
    };
    let pinned_score = SsimOutcome::Score(f64::from_bits(PINNED_SSIM_BITS));

    assert_eq!(pin.mismatch(&report(473, pinned_score.clone())), None);
    // The pin is exact in both directions: an improvement is as much a moved
    // measurement as a regression, and says so with the values to transcribe.
    let improved = pin
        .mismatch(&report(472, pinned_score.clone()))
        .expect("fewer changed pixels is a moved measurement");
    assert!(improved.contains("#124"), "{improved}");
    assert!(improved.contains("changed_pixels=472"), "{improved}");
    assert!(pin.mismatch(&report(474, pinned_score)).is_some());
    assert!(pin
        .mismatch(&report(473, SsimOutcome::Score(0.9)))
        .is_some());
    assert!(pin
        .mismatch(&report(473, SsimOutcome::SmallArtifactMismatch))
        .is_some());
}

#[test]
fn embedded_faces_read_the_official_export_and_the_renderer_declarations() {
    let payload = base64::engine::general_purpose::STANDARD.encode(b"wOF2-bytes");
    let official = format!(
        "<style class=\"style-fonts\">\n      @font-face {{ font-family: Excalifont; \
         src: url(data:font/woff2;base64,{payload}); }}\n      @font-face {{ font-family: \
         Xiaolai; src: url(data:font/woff2;base64,{payload}); }}\n    </style>"
    );
    let renderer = format!(
        "<style class=\"style-fonts\">@font-face{{font-family:'ExcalidrawRender-9';\
         src:url(data:font/ttf;base64,{payload})}}</style>"
    );
    assert_eq!(
        embedded_faces(&official),
        [
            EmbeddedFace {
                family: "Excalifont".into(),
                media_type: "font/woff2".into(),
                bytes: b"wOF2-bytes".to_vec(),
            },
            EmbeddedFace {
                family: "Xiaolai".into(),
                media_type: "font/woff2".into(),
                bytes: b"wOF2-bytes".to_vec(),
            },
        ]
    );
    assert_eq!(
        embedded_faces(&renderer),
        [EmbeddedFace {
            family: "ExcalidrawRender-9".into(),
            media_type: "font/ttf".into(),
            bytes: b"wOF2-bytes".to_vec(),
        }]
    );
    assert!(embedded_faces("<svg><path d=\"M0 0\"/></svg>").is_empty());
}

#[test]
fn referenced_families_split_lists_and_keep_document_order() {
    let svg = "<text font-family=\"Helvetica, Segoe UI Emoji\">a</text>\
               <text font-family=\"Excalifont\"><tspan font-family=\"Xiaolai\">b</tspan></text>\
               <text font-family=\"Helvetica\">c</text>";
    assert_eq!(
        referenced_families(svg),
        ["Helvetica", "Segoe UI Emoji", "Excalifont", "Xiaolai"]
    );
}
