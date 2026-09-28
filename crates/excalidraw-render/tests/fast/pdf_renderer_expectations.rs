//! The focused rectangle-outline PDF expectations
//! (`pdf-renderer-expectations.json`) and the PDFium tolerance exception they
//! carry stay bound to the case, dependencies, comparator and viewer profiles.
//! Expectations and tolerances are reviewed in Git and exercised by rendering.

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::common::lower_hex;
use crate::common::{read_json, repository_root};

const CASE: &str = "tests/conformance/cases/focused/rectangle-outline";
const COMPARATOR_PROFILE: &str = "tests/conformance/comparator-profiles/canonical-visual-v1.json";
const VIEWER_PROFILES: &str = "tests/conformance/viewer-profiles";

#[test]
fn focused_pdf_expectations_stay_bound_to_their_case_and_dependencies() {
    let root = repository_root();
    let case = root.join(CASE);
    let case_manifest = read_json(&case.join("manifest.json"));
    let expectations = read_json(&case.join("pdf-renderer-expectations.json"));

    assert_eq!(
        expectations["schema"],
        "excalidraw-render/pdf-renderer-expectations/v1"
    );
    assert_eq!(expectations["format"], "pdf");
    assert_eq!(expectations["case_id"], case_manifest["case_id"]);
    assert_eq!(
        expectations["compatibility_revision"],
        case_manifest["compatibility_revision"]
    );
    let variant = case_manifest["variants"]
        .as_array()
        .expect("case manifest lists its variants")
        .iter()
        .find(|variant| variant["id"] == expectations["variant"])
        .expect("PDF expectations name a variant of the case");
    assert_eq!(
        expectations["visual_class"], variant["formats"]["svg"]["visual_class"],
        "the PDF is judged in the visual class of its SVG authority"
    );
    assert!(!expectations["coverage"]
        .as_array()
        .expect("coverage is an array")
        .is_empty());
    assert!(!expectations["regions"]
        .as_array()
        .expect("regions are an array")
        .is_empty());
    assert!(expectations["crops"].is_array());

    let files = expectations["files"]
        .as_array()
        .expect("PDF expectation files are an array");
    for required in [
        format!("{CASE}/manifest.json"),
        format!("{CASE}/scene.excalidraw"),
        format!("{CASE}/pdf-visual-exception.json"),
        COMPARATOR_PROFILE.to_owned(),
    ] {
        assert!(
            files
                .iter()
                .any(|descriptor| descriptor["path"] == required),
            "{required} is among the bound dependencies"
        );
    }
    for descriptor in files {
        let path = descriptor["path"]
            .as_str()
            .expect("PDF expectation path is a string");
        let bytes = fs::read(root.join(path)).expect("PDF expectation dependency exists");
        assert_eq!(
            descriptor["sha256"],
            sha256_hex(&bytes),
            "stale PDF expectation dependency {path}"
        );
    }

    let exception = read_json(&case.join("pdf-visual-exception.json"));
    assert_eq!(
        exception
            .as_object()
            .expect("PDF visual exception is an object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "approved_issue",
            "authority",
            "case_id",
            "format",
            "max_changed_pixels",
            "minimum_ssim_bits",
            "reason",
            "region",
            "schema",
            "variant",
            "viewer_profile",
            "visual_class",
        ]
    );
    assert_eq!(
        exception["schema"],
        "excalidraw-render/pdf-compatibility-exception/v1"
    );
    for shared in ["case_id", "variant", "format", "visual_class"] {
        assert_eq!(exception[shared], expectations[shared], "{shared} agrees");
    }
    assert!(expectations["regions"]
        .as_array()
        .expect("regions are an array")
        .contains(&exception["region"]));
    let viewer_profile = exception["viewer_profile"]
        .as_str()
        .expect("the exception names a viewer profile");
    assert!(
        pdf_viewer_profile_ids(&root).contains(&viewer_profile.to_owned()),
        "{viewer_profile} is a canonical PDF viewer profile"
    );
    assert!(exception["max_changed_pixels"].is_u64());
    let minimum_ssim_bits = exception["minimum_ssim_bits"]
        .as_str()
        .expect("minimum SSIM is recorded as f64 bits");
    assert_eq!(minimum_ssim_bits.len(), 16);
    let minimum_ssim =
        f64::from_bits(u64::from_str_radix(minimum_ssim_bits, 16).expect("SSIM bits are hex"));
    assert!(
        (0.0..=1.0).contains(&minimum_ssim),
        "minimum SSIM {minimum_ssim} is a score"
    );
}

/// The ids of the canonical PDF viewer profiles the repository declares.
fn pdf_viewer_profile_ids(root: &Path) -> Vec<String> {
    fs::read_dir(root.join(VIEWER_PROFILES))
        .expect("viewer profiles are present")
        .map(|entry| entry.expect("viewer profile entry is readable").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("canonical-pdf-") && name.ends_with(".json"))
        })
        .map(|path| {
            read_json(&path)["id"]
                .as_str()
                .expect("viewer profile has an id")
                .to_owned()
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}
