//! Viewer-resource contracts through the xtask command and real font codec.
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const RETAINED: &str = "tests/conformance/proposals/velox84-evidence-v1/viewers/light";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = root().join("target");
        fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!(
            "viewer-assets-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn svg(&self, svg: &str) {
        for kind in ["official", "native"] {
            fs::write(self.0.join(format!("{kind}.svg")), svg).unwrap();
        }
    }

    fn run(&self, mode: &str, success: bool) -> Output {
        // Exercise caller-relative paths from a directory other than repo root.
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["representative", "viewer-assets", mode])
            .arg(self.0.file_name().unwrap())
            .current_dir(self.0.parent().unwrap())
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if !success {
            assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        }
        output
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rule(font: &[u8], family: &str, range: &str, weight: u32) -> String {
    format!(
        "@font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{}); unicode-range: {range}; font-weight: {weight}; }}",
        base64::engine::general_purpose::STANDARD.encode(font)
    )
}

fn document(rules: &str, family: &str, text: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{rules}</style><text font-family="{family}">{text}</text></svg>"#
    )
}

#[test]
fn percent_encoded_images_preserve_malformed_escapes_and_unicode() {
    let scratch = Scratch::new();
    scratch.svg(
        r#"<svg><image href="data:image/svg+xml,%3Csvg%3E%€%Aéé%FF%00%zz%2%3c/svg%3E%"/></svg>"#,
    );
    scratch.run("collect", true);
    let mut expected = "<svg>%€%Aéé".as_bytes().to_vec();
    expected.extend_from_slice(b"\xff\x00%zz%2</svg>%");
    for kind in ["official", "native"] {
        assert_eq!(
            fs::read(scratch.0.join(format!("assets/{kind}-image-0.svg"))).unwrap(),
            expected
        );
    }
    scratch.run("verify", true);
}

#[test]
fn retained_resources_match_exactly_except_the_v2_plan_encoding() {
    let scratch = Scratch::new();
    let source = root().join(RETAINED);
    for kind in ["official", "native"] {
        fs::copy(
            source.join(format!("{kind}.svg")),
            scratch.0.join(format!("{kind}.svg")),
        )
        .unwrap();
    }
    scratch.run("collect", true);
    scratch.run("verify", true);
    let name = scratch.0.strip_prefix(root()).unwrap().to_str().unwrap();
    for kind in ["official", "native"] {
        for suffix in ["view.svg", "rust.svg", "assets.json"] {
            let file = format!("{kind}-{suffix}");
            let expected = fs::read_to_string(source.join(&file))
                .unwrap()
                .replace(RETAINED, name);
            assert_eq!(
                fs::read(scratch.0.join(&file)).unwrap(),
                expected.as_bytes(),
                "{file}"
            );
        }
        for extension in ["woff2", "sfnt"] {
            let file = format!("assets/{kind}-font-0.{extension}");
            assert_eq!(
                fs::read(scratch.0.join(&file)).unwrap(),
                fs::read(source.join(file)).unwrap()
            );
        }
    }
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(source.join("viewer-assets-plan.json"))
            .unwrap()
            .replace(RETAINED, name),
    )
    .unwrap();
    let actual: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("viewer-assets-plan.json")).unwrap())
            .unwrap();
    assert_eq!(actual, expected);
    let before = fs::read(scratch.0.join("official-assets.json")).unwrap();
    scratch.run("collect", false);
    assert_eq!(
        fs::read(scratch.0.join("official-assets.json")).unwrap(),
        before
    );
}

#[test]
fn complementary_same_family_faces_are_rejected_in_either_order() {
    let fonts = [
        "fonts/ComicShanns/ComicShanns-Regular-279a7b317d12eb88de06167bd672b4b4.woff2",
        "fonts/ComicShanns/ComicShanns-Regular-6e066e8de2ac57ea9283adb9c24d7f0c.woff2",
    ]
    .map(|path| {
        excalidraw_fonts::load_asset(path)
            .unwrap()
            .unwrap()
            .into_bytes()
    });
    let decoded = fonts
        .each_ref()
        .map(|bytes| wuff::decompress_woff2(bytes).unwrap());
    let faces = decoded
        .each_ref()
        .map(|bytes| rustybuzz::Face::from_slice(bytes, 0).unwrap());
    let exclusive = [0, 1].map(|index| {
        (0x80..=0xffff)
            .filter_map(char::from_u32)
            .find(|&ch| {
                faces[index].glyph_index(ch).is_some() && faces[1 - index].glyph_index(ch).is_none()
            })
            .expect("official chunks have complementary glyph maps")
    });
    let rules = [0, 1].map(|i| {
        rule(
            &fonts[i],
            "Chunks",
            &format!("U+{:X}", exclusive[i] as u32),
            400,
        )
    });
    for order in [[0, 1], [1, 0]] {
        let scratch = Scratch::new();
        scratch.svg(&document(
            &format!("{}{}", rules[order[0]], rules[order[1]]),
            "Chunks, fallback",
            &exclusive.iter().collect::<String>(),
        ));
        for mode in ["collect", "verify"] {
            assert!(String::from_utf8_lossy(&scratch.run(mode, false).stderr)
                .contains("unsupported same-family multiple faces"));
        }
        assert!(!scratch.0.join("official-assets.json").exists());
    }
    // The same two resources remain supported under distinct family names.
    let scratch = Scratch::new();
    let distinct = [0, 1]
        .map(|i| {
            rule(
                &fonts[i],
                &format!("Chunk{i}"),
                &format!("U+{:X}", exclusive[i] as u32),
                400,
            )
        })
        .join("");
    scratch.svg(&format!(r#"<svg><style>{distinct}</style><text font-family="Chunk0">{}</text><text font-family="Chunk1">{}</text></svg>"#, exclusive[0], exclusive[1]));
    scratch.run("collect", true);
    scratch.run("verify", true);
    let assets: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("official-assets.json")).unwrap()).unwrap();
    assert_eq!(assets["fonts"].as_array().unwrap().len(), 2);
}

#[test]
fn ranges_restrict_coverage_and_same_family_styles_fail_explicitly() {
    let font = excalidraw_fonts::load_asset(
        "fonts/ComicShanns/ComicShanns-Regular-279a7b317d12eb88de06167bd672b4b4.woff2",
    )
    .unwrap()
    .unwrap()
    .into_bytes();
    for (range, text, success) in [
        ("u+4?", "A", true),
        ("U+41-42, U+43", "ABC", true),
        ("U+42", "A", false),
        ("U+42-41", "A", false),
        ("U+110000", "A", false),
        ("U+4?1", "A", false),
        ("U+41, U+41", "A", false),
        ("U+€", "A", false),
    ] {
        let scratch = Scratch::new();
        scratch.svg(&document(&rule(&font, "Range", range, 400), "Range", text));
        scratch.run("collect", success);
    }
    let scratch = Scratch::new();
    scratch.svg(&document(
        &format!(
            "{}{}",
            rule(&font, "Range", "U+41", 400),
            rule(&font, "Range", "U+42", 700)
        ),
        "Range",
        "AB",
    ));
    assert!(
        String::from_utf8_lossy(&scratch.run("collect", false).stderr)
            .contains("unsupported same-family multiple faces")
    );
}

#[test]
fn text_entities_are_decoded_once_for_coverage_without_rewriting_svg() {
    let font = excalidraw_fonts::load_asset(
        "fonts/ComicShanns/ComicShanns-Regular-279a7b317d12eb88de06167bd672b4b4.woff2",
    )
    .unwrap()
    .unwrap()
    .into_bytes();
    for (range, text, success) in [
        // Literal &lt; needs &, l, t and ;, not the less-than glyph.
        ("U+26, U+3B, U+6C, U+74", "&amp;lt;", true),
        ("U+3C", "&amp;lt;", false),
        ("U+3C", "&lt;", true),
        ("U+26, U+3B, U+6C, U+74", "&lt;", false),
        (
            "U+22, U+26-27, U+3C, U+3E",
            "&quot;&amp;&apos;&lt;&gt;",
            true,
        ),
    ] {
        let scratch = Scratch::new();
        let svg = document(&rule(&font, "Entities", range, 400), "Entities", text);
        scratch.svg(&svg);
        let output = scratch.run("collect", success);
        if success {
            scratch.run("verify", true);
            let rust = fs::read_to_string(scratch.0.join("official-rust.svg")).unwrap();
            assert!(rust.contains(&format!(">{text}</text>")));
            assert_eq!(
                fs::read_to_string(scratch.0.join("official.svg")).unwrap(),
                svg
            );
        } else {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("does not cover every character")
            );
        }
    }
}

#[test]
fn substitute_names_are_validated_before_browser_observation_or_encoding() {
    for family in [
        "Bad; color:red",
        "Bad} body {",
        "Bad\\41",
        "Bad&amp;Name",
        "Bad&#59;Name",
        "-option",
        "initial",
        "Bad\nName",
    ] {
        let scratch = Scratch::new();
        scratch.svg(&document("", family, "A"));
        for mode in ["collect", "verify"] {
            assert!(
                String::from_utf8_lossy(&scratch.run(mode, false).stderr)
                    .contains("unsupported substitute font family"),
                "{family:?}"
            );
        }
        assert!(!scratch.0.join("official-font-resolution.json").exists());
        assert!(!scratch.0.join("official-substituted.svg").exists());
    }
    // Valid simple multi-word names still reach resolution validation.
    let scratch = Scratch::new();
    scratch.svg(&document("", "Liberation Sans", "A"));
    assert!(
        String::from_utf8_lossy(&scratch.run("verify", false).stderr)
            .contains("official-font-resolution.json")
    );
}

#[test]
fn substitution_records_require_agreement_pinned_bytes_and_one_match() {
    let original: Value = serde_json::from_slice(&fs::read(root().join("tests/conformance/proposals/slides84-evidence-v2/viewers/light/official-font-resolution.json")).unwrap()).unwrap();
    for (mutation, expected) in [
        (0, "Chromium and fontconfig disagree"),
        (1, "substitute is not a pinned fontconfig match"),
        (2, "every undeclared family needs one observed substitute"),
        (3, "retained substitute changed"),
    ] {
        let scratch = Scratch::new();
        let svg = document("", "Helvetica", "A");
        scratch.svg(&svg);
        fs::create_dir(scratch.0.join("assets")).unwrap();
        fs::write(
            scratch.0.join("assets/official-substitute-0.ttf"),
            b"changed",
        )
        .unwrap();
        let mut resolution = original.clone();
        resolution["input"]["sha256"] = json!(format!("{:x}", Sha256::digest(svg.as_bytes())));
        match mutation {
            0 => resolution["undeclared"][0]["chromium"]["postScriptName"] = json!("different"),
            1 => resolution["undeclared"][0]["system_font"]["sha256"] = json!("0".repeat(64)),
            2 => {
                let duplicate = resolution["undeclared"][0].clone();
                resolution["undeclared"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            _ => {}
        }
        fs::write(
            scratch.0.join("official-font-resolution.json"),
            serde_json::to_vec(&resolution).unwrap(),
        )
        .unwrap();
        let output = scratch.run("verify", false);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires REPRESENTATIVE_SUBSTITUTE_FONT pointing to the pinned LiberationSans-Regular.ttf"]
fn caller_relative_verification_encodes_the_retained_substitute() {
    let scratch = Scratch::new();
    let svg = document("", "Helvetica", "A");
    scratch.svg(&svg);
    fs::create_dir(scratch.0.join("assets")).unwrap();
    let bytes = fs::read(std::env::var("REPRESENTATIVE_SUBSTITUTE_FONT").unwrap()).unwrap();
    let mut resolution: Value = serde_json::from_slice(&fs::read(root().join("tests/conformance/proposals/slides84-evidence-v2/viewers/light/official-font-resolution.json")).unwrap()).unwrap();
    assert_eq!(
        resolution["undeclared"][0]["system_font"]["sha256"],
        format!("{:x}", Sha256::digest(&bytes))
    );
    resolution["input"]["sha256"] = json!(format!("{:x}", Sha256::digest(svg.as_bytes())));
    fs::write(
        scratch.0.join("official-font-resolution.json"),
        serde_json::to_vec(&resolution).unwrap(),
    )
    .unwrap();
    fs::write(scratch.0.join("assets/official-substitute-0.ttf"), bytes).unwrap();
    // Supply an empty native SVG so verification reaches comparison only after
    // successfully encoding and decoding the official substitute.
    fs::write(scratch.0.join("native.svg"), "<svg/>").unwrap();
    let output = scratch.run("verify", false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("assets/official-font-0.sfnt"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
