//! Exact case-specific proposals, not approvals. Canonical measurements remain
//! visible; scoped limits are evaluated anew from the captured image bytes.
use crate::conformance::{canonical_bytes, repo_root, sha256};
use crate::conformance_candidate_gate::{
    candidate_root, case_id, descriptor, historical_binding, historical_record, read, value,
    COMPARATOR_PROFILE,
};
use serde_json::{json, Value};
use std::path::Path;
type Result<T> = std::result::Result<T, String>;
const REPORT: &str = "docs/research/stage4-pdf-typography-diagnosis.md";
const EVIDENCE: &str = "docs/research/stage4-pdf-typography-evidence.json";
const REPORT_HASH: &str = "8bba3edd8b7b3928aed1794e53bda1b2f21b42b449e01c87d1087be68571b766";
const EVIDENCE_HASH: &str = "5efa8854d7cecd731427b3a18ce69d728e791e2d420ba8d5d06640a7357ace72";
const DIAGNOSED: &str = "tests/conformance/candidates/stage4-frames-text-assets-v4";
const PROPOSAL: &str = "exception-proposal.json";
/// The Krilla render of the diagnosed scene (ADR-0021). Every PDF byte moved
/// with the backend, but both pinned viewers rasterize it byte for byte as
/// they rasterized the diagnosed PDF, so the diagnosis covers the same pixels.
/// A changed Krilla render requires renewed review here.
const KRILLA_SUCCESSOR_PDF: (usize, &str) = (
    11023,
    "abbc64e2f341569d67d306eaaeea6f810ded8e8356c4f83841608c2b17fc5642",
);
const SCOPES: [(&str, &str, u64, &str); 3] = [
    ("pdfium", "whole", 118, "3fefacd49b980278"),
    ("pdfium", "typography", 118, "3fee1f978eeaac28"),
    ("pdfjs", "whole", 16, "3feff3e226be27cf"),
];
fn pinned(bytes: &[u8], hash: &str) -> Result<()> {
    if sha256(bytes) != hash {
        return Err("stale diagnosis proof; a changed proof requires explicit review".into());
    }
    Ok(())
}
fn proof() -> Result<Value> {
    let root = repo_root();
    pinned(&read(&root, REPORT)?, REPORT_HASH)?;
    let bytes = read(&root, EVIDENCE)?;
    pinned(&bytes, EVIDENCE_HASH)?;
    let evidence: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    // This is the diagnosis's *historical* producer chain. Check its explicit
    // repository bindings, not obsolete transitive claims in old collections.
    for d in evidence["repository_bindings"]
        .as_array()
        .ok_or("diagnosis producer chain")?
    {
        let path = d["path"].as_str().ok_or("proof path")?;
        if historical_record(path) {
            historical_binding(d)?;
        } else if d != &descriptor(&root, path)? {
            return Err(format!("stale diagnosis input {path}"));
        }
    }
    Ok(evidence)
}
/// The backend successor record for `pdf`, the reviewed Krilla render of the
/// diagnosed scene. It covers only the PDF bytes: `expected` still requires
/// every diagnosed viewer raster to be byte-identical.
pub(crate) fn backend_successor(pdf: &[u8]) -> Result<Value> {
    if pdf.len() != KRILLA_SUCCESSOR_PDF.0 || sha256(pdf) != KRILLA_SUCCESSOR_PDF.1 {
        return Err("stale or unrelated Krilla backend successor PDF".into());
    }
    Ok(json!({
        "decision": "docs/adr/0021-move-the-pdf-backend-from-svg2pdf-to-krilla.md",
        "diagnosed_pdf": format!("{DIAGNOSED}/native.pdf"),
        "successor_pdf": {"byte_length": pdf.len(), "sha256": sha256(pdf)},
        "rule": "diagnosed viewer rasters byte-identical",
    }))
}
fn expected(directory: &str) -> Result<Value> {
    let root = candidate_root(directory)?;
    let evidence = proof()?;
    // The diagnosed font, geometry, PDF and viewer rasters are the scope of the
    // decision. Equal summary metrics on different inputs are not sufficient.
    let mut inputs = Vec::new();
    let mut successor = Value::Null;
    for name in [
        "scene.json",
        "native.pdf",
        "official.svg",
        "judgment-plan.json",
        "viewer-evidence/official-chromium.rgba",
        "viewer-evidence/native-pdfium.rgba",
        "viewer-evidence/native-pdfjs.rgba",
    ] {
        let historical = format!("{DIAGNOSED}/{name}");
        let bound = evidence["repository_bindings"]
            .as_array()
            .ok_or("bindings")?
            .iter()
            .find(|d| d["path"] == historical)
            .ok_or("diagnosis lacks required input")?;
        let current = descriptor(&root, name)?;
        if current["sha256"] != bound["sha256"] || current["byte_length"] != bound["byte_length"] {
            if name == "native.pdf" {
                successor = backend_successor(&read(&root, name)?)?;
            } else {
                return Err(format!(
                    "exception diagnosis does not cover changed input {name}"
                ));
            }
        }
        inputs.push(current);
    }
    let mut scopes = Vec::new();
    for (viewer, region, pixels, bits) in SCOPES {
        let profile = format!("tests/conformance/viewer-profiles/canonical-pdf-{viewer}-v1.json");
        let reason = if viewer == "pdfium" {
            "Pinned PDFium rasterizes the correctly embedded and exactly positioned Excalifont text using whole-device-Y glyph origins, LCD-mask X phases and its text-coverage pipeline; Chromium uses quarter-pixel glyph phases. Geometry and the font program are correct. The diagnosis does not claim an independently quantified gamma contribution."
        } else {
            "The pinned PDF.js disableFontFace path-font renderer paints exact fractional glyph origins, unlike Chromium quarter-pixel mask placement. Independently applying the quarter-pixel rule in a diagnostic-only PDF removes the changed pixels; production PDF geometry remains full-f64."
        };
        scopes.push(json!({"case_id":case_id(directory)?,"variant":"default","format":"pdf","comparison_id":format!("pdf-{viewer}"),"viewer_profile":{"id":value(&repo_root(),&profile)?["id"],"descriptor":descriptor(&repo_root(),&profile)?},"region":region,"bounds":if region=="whole"{json!([0,0,200,120])}else{json!([10,12,124,41])},"max_changed_pixels":pixels,"minimum_ssim_bits":bits,"reason":reason,"approving_issue":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85"}));
    }
    let mut proposal = json!({"schema":"excalidraw-render/representative-exception-proposal/v1","status":"proposed-not-approved","case_id":case_id(directory)?,"diagnosis":[descriptor(&repo_root(),REPORT)?,descriptor(&repo_root(),EVIDENCE)?],"diagnosis_producer_chain":evidence["repository_bindings"],"diagnosis_upstream_sources":evidence["upstream_sources"],"colour_binding_bridge":Value::Null,"comparator":descriptor(&repo_root(),COMPARATOR_PROFILE)?,"inputs":inputs,"exceptions":scopes});
    // Only a Krilla-era proposal carries the key, so earlier proposals keep
    // their exact shape.
    if !successor.is_null() {
        proposal["backend_successor"] = successor;
    }
    Ok(proposal)
}
fn validate(proposal: &Value, wanted: &Value) -> Result<()> {
    // Exact agreement rejects unknown/unused scopes, duplicate records, omitted
    // causes/proof, changed viewer/crop/issue identity and any loosened ceiling.
    if proposal != wanted {
        return Err("exception proposal differs from the reviewed scopes, exact limits, cause or proof bindings".into());
    }
    Ok(())
}
/// A proposal keeps the record of the comparator it was measured under; the
/// gate recomputes its measurements with the live comparator (ADR-0019).
fn with_measured_comparator(mut wanted: Value, proposal: &Value) -> Value {
    let comparator = &proposal["comparator"];
    if comparator["path"] == COMPARATOR_PROFILE && historical_binding(comparator).is_ok() {
        wanted["comparator"] = comparator.clone();
    }
    wanted
}
fn observations(canonical: &Value, proposal: &Value) -> Result<()> {
    for scope in proposal["exceptions"]
        .as_array()
        .ok_or("exception scopes")?
    {
        let row = canonical["rows"]
            .as_array()
            .ok_or("canonical rows")?
            .iter()
            .find(|r| r["id"] == scope["comparison_id"])
            .ok_or("unknown exception comparison")?;
        let region = if scope["region"] == "whole" {
            &row["whole"]
        } else {
            &row["crops"]
                .as_array()
                .ok_or("canonical crops")?
                .iter()
                .find(|c| c["id"] == scope["region"])
                .ok_or("unknown exception region")?["region"]
        };
        if region["changed_pixels_pass"] == true && region["ssim_pass"] == true {
            return Err("unused exception: region now passes its canonical gate".into());
        }
        if region["changed_pixels"] != scope["max_changed_pixels"]
            || region["ssim_bits"] != scope["minimum_ssim_bits"]
        {
            return Err(
                "exception measurement moved; exact no-margin proposal requires renewed review"
                    .into(),
            );
        }
    }
    Ok(())
}
pub fn proposed(root: &Path) -> Result<Value> {
    if root.join(PROPOSAL).exists() {
        value(root, PROPOSAL)
    } else {
        Ok(Value::Null)
    }
}
pub fn prepare(directory: &str) -> Result<Value> {
    let root = candidate_root(directory)?;
    if root.join("manifest.json").exists() {
        return Err(
            "exception preparation requires a fresh unmaterialized candidate revision".into(),
        );
    }
    let proposal = expected(directory)?;
    let canonical = crate::conformance_candidate::comparisons(&format!(
        "{directory}/viewer-evidence/collection.json"
    ))?;
    observations(&canonical, &proposal)?;
    crate::conformance_candidate_native::write_new(
        &root.join(PROPOSAL),
        &[canonical_bytes(&proposal)?, b"\n".to_vec()].concat(),
    )?;
    Ok(
        json!({"status":"proposed-not-approved","proposal_sha256":sha256(&canonical_bytes(&proposal)?),"exceptions":proposal["exceptions"],"approval_subject_sha256":null}),
    )
}
pub fn evaluate(directory: &str) -> Result<Value> {
    let root = candidate_root(directory)?;
    let collection = format!("{directory}/viewer-evidence/collection.json");
    // Never read judgments.json or a caller-supplied pass flag.
    let canonical = crate::conformance_candidate::comparisons(&collection)?;
    let proposal = proposed(&root)?;
    let effective = if proposal.is_null() {
        canonical.clone()
    } else {
        validate(
            &proposal,
            &with_measured_comparator(expected(directory)?, &proposal),
        )?;
        observations(&canonical, &proposal)?;
        crate::conformance_candidate::comparisons_with_scopes(
            &collection,
            proposal["exceptions"].as_array().ok_or("scopes")?,
        )?
    };
    Ok(
        json!({"schema":"excalidraw-render/representative-judgments/v2","approval_status":"not_evaluated","exception_proposal":proposal,"canonical":canonical,"rows":effective["rows"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proposals_keep_the_comparator_record_they_were_measured_under() {
        let live = descriptor(&repo_root(), COMPARATOR_PROFILE).unwrap();
        let measured = json!({"path":COMPARATOR_PROFILE,"byte_length":1,"sha256":"0".repeat(64)});
        let wanted = json!({"comparator": live, "exceptions": []});
        let proposal = json!({"comparator": measured, "exceptions": []});
        validate(
            &proposal,
            &with_measured_comparator(wanted.clone(), &proposal),
        )
        .unwrap();
        validate(&wanted, &with_measured_comparator(wanted.clone(), &wanted)).unwrap();
        for (key, bad) in [("sha256", json!("00")), ("path", json!("Cargo.toml"))] {
            let mut forged = proposal.clone();
            forged["comparator"][key] = bad;
            assert!(validate(&forged, &with_measured_comparator(wanted.clone(), &forged)).is_err());
        }
    }
    #[test]
    fn krilla_successor_covers_only_the_reviewed_pdf_with_identical_rasters() {
        use std::fs;
        const V28: &str = "tests/conformance/candidates/stage4-frames-text-assets-v28";
        let pdf = read(&repo_root(), &format!("{V28}/native.pdf")).unwrap();
        let record = backend_successor(&pdf).unwrap();
        assert_eq!(record["successor_pdf"]["sha256"], KRILLA_SUCCESSOR_PDF.1);
        let mut changed = pdf.clone();
        changed[100] ^= 1;
        assert!(backend_successor(&changed).is_err());
        let live = expected(V28).unwrap();
        assert_eq!(live["backend_successor"], record);
        assert!(live["colour_binding_bridge"].is_null());
        // The successor covers the PDF bytes only: a moved viewer raster is
        // still an uncovered input.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = format!(
            "target/krilla-successor-{}-{nonce}/stage4-frames-text-assets-v28",
            std::process::id()
        );
        let root = repo_root().join(&dir);
        fs::create_dir_all(root.join("viewer-evidence")).unwrap();
        for name in [
            "scene.json",
            "native.pdf",
            "official.svg",
            "judgment-plan.json",
            "viewer-evidence/official-chromium.rgba",
            "viewer-evidence/native-pdfium.rgba",
            "viewer-evidence/native-pdfjs.rgba",
        ] {
            fs::write(
                root.join(name),
                read(&repo_root(), &format!("{V28}/{name}")).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(expected(&dir).unwrap()["backend_successor"], record);
        let raster = root.join("viewer-evidence/native-pdfium.rgba");
        let mut moved = fs::read(&raster).unwrap();
        moved[0] ^= 1;
        fs::write(&raster, moved).unwrap();
        assert!(expected(&dir)
            .unwrap_err()
            .contains("does not cover changed input"));
    }
    #[test]
    fn scoped_evaluation_ignores_forged_stored_judgment_booleans() {
        use std::fs;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = format!(
            "target/stage4-exceptions-{}-{nonce}/stage4-frames-text-assets-v5",
            std::process::id()
        );
        let root = repo_root().join(&dir);
        fs::create_dir_all(root.join("viewer-evidence")).unwrap();
        for name in [
            "scene.json",
            "native.pdf",
            "native.png",
            "official.svg",
            "official-png.rgba",
            "judgment-plan.json",
        ] {
            fs::write(
                root.join(name),
                read(&repo_root(), &format!("{DIAGNOSED}/{name}")).unwrap(),
            )
            .unwrap();
        }
        let mut collection = value(
            &repo_root(),
            &format!("{DIAGNOSED}/viewer-evidence/collection.json"),
        )
        .unwrap();
        collection["candidate_directory"] = json!(dir);
        for row in collection["captures"].as_array_mut().unwrap() {
            let bytes = read(&repo_root(), row["raster"]["path"].as_str().unwrap()).unwrap();
            let name = format!("viewer-evidence/{}.rgba", row["id"].as_str().unwrap());
            fs::write(root.join(&name), bytes).unwrap();
            row["raster"] = descriptor(&repo_root(), &format!("{dir}/{name}")).unwrap();
        }
        fs::write(
            root.join("viewer-evidence/collection.json"),
            canonical_bytes(&collection).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("judgments.json"),
            br#"{"rows":[{"passed":true}],"approved":true}"#,
        )
        .unwrap();
        let raw = evaluate(&dir).unwrap();
        assert_eq!(raw["rows"][2]["passed"], false);
        assert_eq!(raw["rows"][4]["passed"], false);
        let prepared = prepare(&dir).unwrap();
        assert_eq!(prepared["status"], "proposed-not-approved");
        assert!(prepared["approval_subject_sha256"].is_null());
        let effective = evaluate(&dir).unwrap();
        assert_eq!(effective["rows"][2]["passed"], true);
        assert_eq!(effective["rows"][3]["passed"], true);
        assert_eq!(effective["rows"][4]["passed"], false);
        assert_eq!(effective["canonical"], raw["canonical"]);
        let mut bad = proposed(&root).unwrap();
        bad["exceptions"][0]["max_changed_pixels"] = json!(119);
        fs::write(root.join(PROPOSAL), canonical_bytes(&bad).unwrap()).unwrap();
        assert!(evaluate(&dir).unwrap_err().contains("reviewed scopes"));
    }
    #[test]
    fn scoped_proposal_rejects_scope_limit_cause_and_proof_mutations() {
        let wanted = expected(DIAGNOSED).unwrap();
        validate(&wanted, &wanted).unwrap();
        for change in 0..10 {
            let mut bad = wanted.clone();
            match change {
                0 => {
                    bad["exceptions"].as_array_mut().unwrap().pop();
                }
                1 => {
                    let extra = bad["exceptions"][2].clone();
                    bad["exceptions"].as_array_mut().unwrap().push(extra);
                }
                2 => bad["exceptions"][2]["region"] = json!("typography"),
                3 => bad["exceptions"][0]["max_changed_pixels"] = json!(119),
                4 => bad["exceptions"][0]["minimum_ssim_bits"] = json!("3fef000000000000"),
                5 => bad["exceptions"][0]["reason"] = json!(""),
                6 => bad["diagnosis"][0]["sha256"] = json!("stale"),
                7 => bad["exceptions"][0]["viewer_profile"]["id"] = json!("unknown"),
                8 => bad["exceptions"][0]["bounds"] = json!([0, 0, 400, 240]),
                _ => {
                    bad["exceptions"][0]["approving_issue"] = json!("https://example.com/approval")
                }
            }
            assert!(validate(&bad, &wanted).is_err(), "mutation {change}");
        }
        let mut changed = read(&repo_root(), EVIDENCE).unwrap();
        changed.push(b' ');
        assert!(pinned(&changed, EVIDENCE_HASH).is_err());
        let canonical = crate::conformance_candidate::comparisons(&format!(
            "{DIAGNOSED}/viewer-evidence/collection.json"
        ))
        .unwrap();
        observations(&canonical, &wanted).unwrap();
        let mut unused = canonical.clone();
        unused["rows"][2]["whole"]["changed_pixels_pass"] = json!(true);
        unused["rows"][2]["whole"]["ssim_pass"] = json!(true);
        assert!(observations(&unused, &wanted)
            .unwrap_err()
            .contains("unused exception"));
        let mut moved = canonical;
        moved["rows"][2]["whole"]["changed_pixels"] = json!(117);
        assert!(observations(&moved, &wanted).is_err());
    }
    #[test]
    fn scoped_proposals_recompute_pixels_and_cannot_hide_png_or_passing_crop() {
        let proposal = expected(DIAGNOSED).unwrap();
        let canonical = crate::conformance_candidate::comparisons(&format!(
            "{DIAGNOSED}/viewer-evidence/collection.json"
        ))
        .unwrap();
        let result = crate::conformance_candidate::comparisons_with_scopes(
            &format!("{DIAGNOSED}/viewer-evidence/collection.json"),
            proposal["exceptions"].as_array().unwrap(),
        )
        .unwrap();
        assert_eq!(result["rows"][2]["passed"], true);
        assert_eq!(result["rows"][3]["passed"], true);
        assert_eq!(
            result["rows"][4], canonical["rows"][4],
            "PNG retains its canonical gate"
        );
        assert_eq!(
            result["rows"][3]["crops"], canonical["rows"][3]["crops"],
            "PDF.js crops retain canonical gates"
        );
        assert_eq!(
            result["rows"][2]["crops"][1],
            canonical["rows"][2]["crops"][1]
        );
        assert_eq!(
            result["rows"][2]["crops"][2],
            canonical["rows"][2]["crops"][2]
        );
    }
}
