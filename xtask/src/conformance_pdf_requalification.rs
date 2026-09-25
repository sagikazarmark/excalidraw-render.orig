//! Historical PDF requalification policy tests only; no commands or production workflow.
#![cfg(test)]

use crate::{
    conformance::{canonical_bytes, repo_root, sha256},
    conformance_candidate_gate::{read, value},
};
use serde_json::{json, Value};
type Result<T> = std::result::Result<T, String>;
const APPROVED_PDF: &str = "b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43";
const CURRENT_PDF: &str = "50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc";
const APPROVED_SUBJECT: &str = "c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389";
const OLD_EXPECTATION: &str = "b6cbc6049556ae87780a83e9c252f64193594bf37c9d6a2f18a80434b31cb7b9";
const NEW_EXPECTATION: &str = "442f0cb2580a4066d64bcb924eaa6e070252faca2fa2b37fa4f34c81ff600b1b";
const TEST_SOURCE: &str = "crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs";
const APPROVAL_URL: &str =
    "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5748855374";

fn scoped_approval(approval: &Value, comment: &Value) -> Result<()> {
    crate::conformance::verify_approval_for_issue(approval, APPROVED_SUBJECT, "85")?;
    if approval["approved_by"] != "sagikazarmark"
        || approval["approved_at"] != "2026-09-20"
        || approval["approval_url"] != APPROVAL_URL
        || comment["html_url"] != APPROVAL_URL
        || comment["user"]["login"] != approval["approved_by"]
        || comment["created_at"].as_str().and_then(|s| s.get(..10))
            != approval["approved_at"].as_str()
        || comment["body"].as_str().is_none_or(|s| {
            !s.contains(APPROVED_SUBJECT)
                || !s.contains(CURRENT_PDF)
                || !s.contains("not an autonomous approval decision by the coding agent")
        })
    {
        return Err("scoped approval does not match the recorded user decision".into());
    }
    Ok(())
}
fn exact_pin_transition(
    before: &[u8],
    after: &[u8],
    old_test: &[u8],
    new_test: &[u8],
) -> Result<()> {
    if sha256(before) != OLD_EXPECTATION
        || sha256(after) != NEW_EXPECTATION
        || sha256(old_test) != "fedf835ed08f63fd930fe01b403b85d959752dce66c6547057753efa33e8ceb5"
    {
        return Err("unexpected protected pin transition identity".into());
    }
    let mut expected: Value = serde_json::from_slice(before).map_err(|e| e.to_string())?;
    expected["operation_sha256"] =
        json!("ebf933b7a43e6d53730578e44562484e2aa04eb4cc0d47af7c876625e054de16");
    expected["inventory_sha256"] =
        json!("a993befecf6da05bbf204d240bc3242b045a9eefd7460348811e1dfe3e83159b");
    if expected != serde_json::from_slice::<Value>(after).map_err(|e| e.to_string())? {
        return Err("transition changed unrelated structural expectations".into());
    }
    let expected_test = std::str::from_utf8(old_test)
        .map_err(|e| e.to_string())?
        .replace(APPROVED_PDF, CURRENT_PDF)
        .replace(OLD_EXPECTATION, NEW_EXPECTATION);
    if new_test != expected_test.as_bytes() {
        return Err("transition changed unrelated test logic or pins".into());
    }
    Ok(())
}
fn structural_delta(old: &Value, new: &Value) -> Result<Value> {
    let expected_old = json!(["40", "0", "0", "40", "0", "0"]);
    let expected_new = json!(["40", "0", "0", "-40", "0", "40"]);
    if old["pages"][0]["operations"][377]["operator"] != "cm"
        || new["pages"][0]["operations"][377]["operator"] != "cm"
        || old["pages"][0]["operations"][377]["operands"] != expected_old
        || new["pages"][0]["operations"][377]["operands"] != expected_new
    {
        return Err("unexpected image matrix delta".into());
    }
    let mut normalized = new.clone();
    if old["pages"][0]["images"][0]["color_space"] != "/CS1"
        || new["pages"][0]["images"][0]["color_space"] != json!(["/ICCBased", "5 0 R"])
    {
        return Err("unexpected direct image ICC binding delta".into());
    }
    normalized["pages"][0]["images"][0]["color_space"] =
        old["pages"][0]["images"][0]["color_space"].clone();
    normalized["document"]["file_ids"] = old["document"]["file_ids"].clone();
    normalized["pages"][0]["operations"][377] = old["pages"][0]["operations"][377].clone();
    for i in 0..2 {
        if old["pages"][0]["images"][i]["interpolate"] != false
            || new["pages"][0]["images"][i]["interpolate"] != true
        {
            return Err("unexpected interpolation delta".into());
        }
        normalized["pages"][0]["images"][i]["interpolate"] = json!(false);
    }
    if normalized["document"] != old["document"] || normalized["pages"] != old["pages"] {
        return Err("unexplained geometry, text, font, image, metadata or inventory change".into());
    }
    Ok(
        json!({"allowed_changes":["document file IDs","source RGB and attached mask Interpolate flags","image placement matrix at operation 377","image ColorSpace direct ICCBased reference"],"all_other_document_and_page_fields_equal":true,"text_operations_fonts_mappings_vectors_clips_forms_and_image_samples_unchanged":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approved_pin_history_rejects_unrelated_changes_and_wrong_approval() {
        let root =
            repo_root().join("tests/conformance/candidates/stage4-pdf-backend-requalification-v2");
        let old = read(&root, "before-project-operations-structure.json").unwrap();
        let new = read(&root, "proposed-structure.json").unwrap();
        let old_test = read(&root, "before-pdf_backend_qualification.rs").unwrap();
        let new_test = read(&repo_root(), TEST_SOURCE).unwrap();
        exact_pin_transition(&old, &new, &old_test, &new_test).unwrap();
        let mut bad: Value = serde_json::from_slice(&new).unwrap();
        bad["image_count"] = json!(99);
        assert!(
            exact_pin_transition(&old, &canonical_bytes(&bad).unwrap(), &old_test, &new_test)
                .is_err()
        );
        let mut changed_test = new_test;
        changed_test.extend_from_slice(b"\n// unrelated change\n");
        assert!(exact_pin_transition(&old, &new, &old_test, &changed_test).is_err());
        let approval = value(&root, "approval.json").unwrap();
        let comment = value(&root, "user-approval-comment.json").unwrap();
        scoped_approval(&approval, &comment).unwrap();
        let mut wrong = approval.clone();
        wrong["approval_subject_sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
        assert!(scoped_approval(&wrong, &comment).is_err());
        assert!(scoped_approval(&Value::Null, &comment).is_err());
        let mut other = comment;
        other["user"]["login"] = json!("someone-else");
        assert!(scoped_approval(&approval, &other).is_err());
    }
    #[test]
    fn image_only_structural_delta_rejects_unrelated_changes() {
        let mut ops = vec![json!({"operator":"q","operands":[]}); 378];
        ops[377] = json!({"operator":"cm","operands":["40","0","0","40","0","0"]});
        let old = json!({"document":{"file_ids":["old","old"],"xmp":"same"},"pages":[{"operations":ops,"images":[{"interpolate":false,"color_space":"/CS1","data_sha256":"same"},{"interpolate":false,"data_sha256":"mask"}],"text_operations":["exact text"],"fonts":["same font"]}]});
        let mut new = old.clone();
        new["pages"][0]["images"][0]["color_space"] = json!(["/ICCBased", "5 0 R"]);
        new["document"]["file_ids"] = json!(["new", "new"]);
        new["pages"][0]["operations"][377]["operands"] = json!(["40", "0", "0", "-40", "0", "40"]);
        for i in 0..2 {
            new["pages"][0]["images"][i]["interpolate"] = json!(true);
        }
        structural_delta(&old, &new).unwrap();
        for change in 0..4 {
            let mut bad = new.clone();
            match change {
                0 => bad["pages"][0]["fonts"] = json!(["other"]),
                1 => bad["pages"][0]["text_operations"] = json!(["moved"]),
                2 => bad["pages"][0]["images"][1]["data_sha256"] = json!("different mask"),
                _ => bad["document"]["xmp"] = json!("changed metadata"),
            };
            assert!(structural_delta(&old, &bad).is_err());
        }
    }
}
