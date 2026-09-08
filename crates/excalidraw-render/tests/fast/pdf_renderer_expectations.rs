//! The focused rectangle-outline PDF expectations
//! (`pdf-renderer-expectations.json`) and the PDFium tolerance exception they
//! carry stay bound to the case they qualify: the approval block digests the
//! rest of the file, every dependency descriptor still hashes to what it
//! declares, the comparator profile ADR-0005 names is among those
//! dependencies, and the case identity agrees with the case manifest. None of
//! the approval's values are repeated here, so a re-approval round touches
//! only the manifests; what a round cannot change is the binding.

use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::common::{read_json, repository_root};

const CASE: &str = "tests/conformance/cases/focused/rectangle-outline";
const COMPARATOR_PROFILE: &str = "tests/conformance/comparator-profiles/canonical-visual-v1.json";
const VIEWER_PROFILES: &str = "tests/conformance/viewer-profiles";

#[test]
fn focused_pdf_expectations_stay_bound_to_their_case_and_dependencies() {
    let root = repository_root();
    let case = root.join(CASE);
    let case_manifest = read_json(&case.join("manifest.json"));
    let mut expectations = read_json(&case.join("pdf-renderer-expectations.json"));

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
    assert_eq!(
        expectations["case_manifest_approval_subject_sha256"],
        case_manifest["approval"]["approval_subject_sha256"],
        "the PDF expectations are bound to the approved case manifest"
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

    let approval = expectations
        .as_object_mut()
        .expect("PDF expectations are an object")
        .remove("approval")
        .expect("PDF expectations are approved");
    let approving_issue = assert_approval_shape(&approval, &case_manifest["originating_issue"]);
    let mut subject = serde_json::to_vec(&expectations).expect("unsigned expectations serialize");
    subject.push(b'\n');
    assert_eq!(
        approval["approval_subject_sha256"],
        format!("sha256:{}", sha256_hex(&subject)),
        "the approval digests the expectations minus itself"
    );

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
    assert_eq!(
        exception["approved_issue"].as_u64(),
        Some(approving_issue),
        "the exception is carried by the approving issue"
    );
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

/// Asserts the approval block has exactly the recorded fields, each in the form
/// `xtask conformance verify` demands of a case approval, and that the approval
/// URL is a comment on an issue of the case's repository; returns that issue.
fn assert_approval_shape(approval: &Value, originating_issue: &Value) -> u64 {
    assert_eq!(
        approval
            .as_object()
            .expect("approval is an object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "approval_subject_sha256",
            "approval_url",
            "approved_at",
            "approved_by",
        ]
    );
    let approved_by = approval["approved_by"]
        .as_str()
        .expect("approved_by is a string");
    assert!(
        !approved_by.is_empty()
            && approved_by
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-'),
        "approved_by {approved_by:?} is a GitHub login"
    );
    let approved_at = approval["approved_at"]
        .as_str()
        .expect("approved_at is a string");
    let date: Vec<&str> = approved_at.split('-').collect();
    assert!(
        date.len() == 3
            && date[0].len() == 4
            && date[1].len() == 2
            && date[2].len() == 2
            && date
                .iter()
                .all(|part| part.bytes().all(|byte| byte.is_ascii_digit())),
        "approved_at {approved_at:?} is a calendar date"
    );

    let issue_prefix = originating_issue
        .as_str()
        .expect("the case names its originating issue")
        .trim_end_matches(|character: char| character.is_ascii_digit());
    let approval_url = approval["approval_url"]
        .as_str()
        .expect("approval_url is a string");
    let (issue, comment) = approval_url
        .strip_prefix(issue_prefix)
        .and_then(|tail| tail.split_once("#issuecomment-"))
        .unwrap_or_else(|| panic!("{approval_url} is an issue comment under {issue_prefix}"));
    assert!(
        !comment.is_empty() && comment.bytes().all(|byte| byte.is_ascii_digit()),
        "{approval_url} names a comment"
    );
    issue
        .parse()
        .unwrap_or_else(|_| panic!("{approval_url} names an issue"))
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
    format!("{:x}", Sha256::digest(bytes))
}
