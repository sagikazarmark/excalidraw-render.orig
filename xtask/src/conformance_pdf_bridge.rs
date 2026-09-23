//! A bounded provenance bridge for the authorized image ColorSpace correction.
//! Original diagnosis inputs stay immutable; every raw PDF stream and every
//! non-image dictionary must remain identical, except derived trailer IDs.
use crate::{
    conformance::{canonical_bytes, repo_root, sha256},
    conformance_candidate_gate::{descriptor, read, value},
    conformance_candidate_native as native,
};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
pub const DIRECTORY: &str = "tests/conformance/candidates/stage4-pdf-colorspace-bridge-v9";
const OLD: &str = "tests/conformance/candidates/stage4-frames-text-assets-v4";
const OLD_HASH: &str = "95c399f0eaaee5bd0b2ae13a50a1a68171c0d5abaeab0b71bdf8816abda5817b";
fn model(path: &Path) -> Result<Value> {
    let out = Command::new(repo_root().join("target/pdf-harness/qpdf/bin/qpdf"))
        .args([
            "--json=2",
            "--json-stream-data=inline",
            "--decode-level=none",
        ])
        .arg(path)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}
pub(crate) fn colour_only(old: &Value, new: &Value) -> Result<Value> {
    let original = old["qpdf"][1].as_object().ok_or("old PDF objects")?;
    let mut normalized = new.clone();
    let current = normalized["qpdf"][1]
        .as_object_mut()
        .ok_or("new PDF objects")?;
    if original.keys().collect::<Vec<_>>() != current.keys().collect::<Vec<_>>() {
        return Err("bridge changed PDF object identities".into());
    }
    let space = original
        .values()
        .find(|o| o["value"]["/Type"] == "/Page")
        .ok_or("bridge page")?["value"]["/Resources"]["/ColorSpace"]["/CS1"]
        .clone();
    if space
        .as_array()
        .is_none_or(|a| a.len() != 2 || a[0] != "/ICCBased")
    {
        return Err("bridge original ICC space".into());
    }
    let mut changed = Vec::new();
    for (key, o) in current.iter_mut() {
        let before = &original[key];
        if before["stream"]["dict"]["/Subtype"] == "/Image"
            && before["stream"]["dict"]["/ColorSpace"] == "/CS1"
        {
            if o["stream"]["dict"]["/ColorSpace"] != space {
                return Err("bridge image must directly bind the original ICC profile".into());
            }
            o["stream"]["dict"]["/ColorSpace"] = json!("/CS1");
            changed.push(key.clone());
        }
    }
    if changed.is_empty() {
        return Err("bridge contains no authorized image colour-space correction".into());
    }
    if original["trailer"]["value"]["/ID"].as_array().is_none() {
        return Err("bridge trailer IDs".into());
    }
    current.get_mut("trailer").ok_or("new trailer")?["value"]["/ID"] =
        original["trailer"]["value"]["/ID"].clone();
    if normalized["qpdf"] != old["qpdf"] {
        return Err("bridge changes raw streams, fonts, text coordinates, image samples/flags, or other PDF objects".into());
    }
    Ok(
        json!({"changed_image_objects":changed,"permitted_changes":["image ColorSpace resource alias to original ICCBased array","derived trailer IDs/xref offsets"],"all_raw_streams_fonts_text_coordinates_and_other_objects_identical":true}),
    )
}
fn sources() -> Result<Vec<Value>> {
    [
        "xtask/src/conformance_pdf_bridge.rs",
        "xtask/src/conformance_candidate_native.rs",
        "tools/pdf-harness/run-pdfium.sh",
        "tools/pdf-harness/pdfium/render.py",
        "tools/pdf-harness/run-pdfjs.sh",
        "tools/pdf-harness/pdfjs/render.mjs",
        "tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json",
        "tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json",
        "tests/conformance/structure-profiles/canonical-pdf-structure-v1.json",
        "tests/conformance/comparator-profiles/canonical-visual-v1.json",
        "target/pdf-harness/qpdf/bin/qpdf",
    ]
    .iter()
    .map(|p| descriptor(&repo_root(), p))
    .collect()
}
fn put(root: &Path, path: &str, v: &Value) -> Result<()> {
    native::write_new(
        &root.join(path),
        &[canonical_bytes(v)?, b"\n".to_vec()].concat(),
    )
}
fn metrics(root: &Path, viewer: &str) -> Result<Value> {
    use excalidraw_render_compare::{
        CanonicalRgba8, ComparisonMode, Crop, SsimOutcome, VisualClass,
    };
    let a = CanonicalRgba8::new(
        200,
        120,
        read(
            &repo_root(),
            &format!("{OLD}/viewer-evidence/official-chromium.rgba"),
        )?,
    )
    .map_err(|e| e.to_string())?;
    let b = CanonicalRgba8::new(200, 120, read(root, &format!("{viewer}.rgba"))?)
        .map_err(|e| e.to_string())?;
    let c = excalidraw_render_compare::compare(
        &a,
        &b,
        VisualClass::GeometryImage,
        &[
            Crop::new("typography", 10, 12, 124, 41, VisualClass::Typography)
                .map_err(|e| e.to_string())?,
        ],
        if viewer == "pdfjs" {
            ComparisonMode::PdfJs
        } else {
            ComparisonMode::Canonical
        },
    )
    .map_err(|e| e.to_string())?;
    let bits = |s: &SsimOutcome| -> Result<String> {
        match s {
            SsimOutcome::Score(s) => Ok(format!("{:016x}", s.to_bits())),
            _ => Err("bridge nonfinite SSIM".into()),
        }
    };
    let (count, whole, typo) = if viewer == "pdfium" {
        (118, "3fefacd49b980278", "3fee1f978eeaac28")
    } else {
        (16, "3feff3e226be27cf", "3fefbe7a57566bff")
    };
    if c.whole.changed_pixels != count
        || c.crops[0].region.changed_pixels != count
        || bits(&c.whole.ssim)? != whole
        || bits(&c.crops[0].region.ssim)? != typo
    {
        return Err("bridge typography measurements moved".into());
    }
    Ok(
        json!({"whole":{"changed_pixels":count,"ssim_bits":whole},"typography":{"changed_pixels":count,"ssim_bits":typo,"canonical_passed":c.crops[0].region.passed()}}),
    )
}
pub fn collect() -> Result<Value> {
    crate::pdf_viewers::verify(true)?;
    let root = repo_root().join(DIRECTORY);
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    let before = native::producers()?;
    let tool_sources = sources()?;
    let (executable, build) = native::build()?;
    let scene = read(&repo_root(), &format!("{OLD}/scene.json"))?;
    let old = read(&repo_root(), &format!("{OLD}/native.pdf"))?;
    if sha256(&old) != OLD_HASH {
        return Err("original typography PDF changed".into());
    }
    let first = native::render(&executable, &scene, "pdf", &native::OPTIONS)?;
    let second = native::render(&executable, &scene, "pdf", &native::OPTIONS)?;
    if first != second {
        return Err("bridge release PDF/report do not repeat".into());
    }
    native::write_new(&root.join("native.pdf"), &first.0)?;
    native::write_new(&root.join("native-repeat.pdf"), &second.0)?;
    native::write_new(&root.join("native-report.json"), &first.1)?;
    native::write_new(&root.join("native-repeat-report.json"), &second.1)?;
    let old_model = model(&repo_root().join(OLD).join("native.pdf"))?;
    let new_model = model(&root.join("native.pdf"))?;
    let changes = colour_only(&old_model, &new_model)?;
    put(&root, "old-objects.json", &old_model)?;
    put(&root, "new-objects.json", &new_model)?;
    let mut views = Vec::new();
    for viewer in ["pdfium", "pdfjs"] {
        for suffix in ["", "-repeat"] {
            let output = root.join(format!("{viewer}{suffix}.rgba"));
            let out = Command::new("bash")
                .current_dir(repo_root())
                .arg(format!("tools/pdf-harness/run-{viewer}.sh"))
                .args(["render"])
                .arg(root.join("native.pdf"))
                .arg(&output)
                .args(["200", "120"])
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).into_owned());
            }
            let bytes = fs::read(&output).map_err(|e| e.to_string())?;
            let historical = read(
                &repo_root(),
                &format!("{OLD}/viewer-evidence/native-{viewer}.rgba"),
            )?;
            if bytes != historical {
                return Err(format!("{viewer} bridge raster differs from the diagnosed PDF; renewed diagnosis required"));
            }
            native::write_new(
                &root.join(format!("{viewer}{suffix}-runtime.json")),
                &out.stdout,
            )?;
        }
        views.push(json!({"viewer":viewer,"fixed":descriptor(&root,&format!("{viewer}.rgba"))?,"repeat":descriptor(&root,&format!("{viewer}-repeat.rgba"))?,"diagnosed":descriptor(&repo_root(),&format!("{OLD}/viewer-evidence/native-{viewer}.rgba"))?,"byte_identical":true,"metrics":metrics(&root,viewer)?}));
    }
    native::check_producers(&before)?;
    if tool_sources != sources()? {
        return Err("bridge tool sources changed".into());
    }
    let record = json!({"schema":"excalidraw-render/pdf-colorspace-provenance-bridge/v1","scope":"authorized external image ColorSpace binding correction only","diagnosed_pdf":descriptor(&repo_root(),&format!("{OLD}/native.pdf"))?,"fixed_pdf":descriptor(&root,"native.pdf")?,"scene":descriptor(&repo_root(),&format!("{OLD}/scene.json"))?,"build":build,"native_producers":before,"tool_sources":tool_sources,"structural_changes":changes,"viewers":views,"approval":null});
    put(&root, "bridge.json", &record)?;
    Ok(
        json!({"bridge_sha256":sha256(&canonical_bytes(&record)?),"new_pdf_sha256":sha256(&first.0),"structural_changes":changes,"viewers":views,"approval":null}),
    )
}
pub fn validate(current_pdf: &[u8]) -> Result<Value> {
    let root = repo_root().join(DIRECTORY);
    let record = value(&root, "bridge.json")?;
    if record["schema"] != "excalidraw-render/pdf-colorspace-provenance-bridge/v1"
        || record["diagnosed_pdf"]["sha256"] != OLD_HASH
        || record["fixed_pdf"] != descriptor(&root, "native.pdf")?
        || sha256(current_pdf) != record["fixed_pdf"]["sha256"]
        || record["tool_sources"] != json!(sources()?)
    {
        return Err("stale or unrelated PDF colourspec bridge".into());
    }
    native::check_producers(&record["native_producers"])?;
    if record["diagnosed_pdf"] != descriptor(&repo_root(), &format!("{OLD}/native.pdf"))?
        || record["scene"] != descriptor(&repo_root(), &format!("{OLD}/scene.json"))?
    {
        return Err("bridge diagnosis inputs changed".into());
    }
    let changes = colour_only(
        &model(&repo_root().join(OLD).join("native.pdf"))?,
        &model(&root.join("native.pdf"))?,
    )?;
    if record["structural_changes"] != changes {
        return Err("bridge structural claim changed".into());
    }
    let viewers = record["viewers"].as_array().ok_or("bridge viewers")?;
    if viewers.len() != 2 {
        return Err("incomplete bridge viewers".into());
    }
    for (row, viewer) in viewers.iter().zip(["pdfium", "pdfjs"]) {
        if row["viewer"] != viewer
            || row["fixed"] != descriptor(&root, &format!("{viewer}.rgba"))?
            || row["repeat"] != descriptor(&root, &format!("{viewer}-repeat.rgba"))?
            || row["diagnosed"]
                != descriptor(
                    &repo_root(),
                    &format!("{OLD}/viewer-evidence/native-{viewer}.rgba"),
                )?
        {
            return Err("stale bridge viewer binding".into());
        }
        let expected = read(
            &repo_root(),
            &format!("{OLD}/viewer-evidence/native-{viewer}.rgba"),
        )?;
        if read(&root, &format!("{viewer}.rgba"))? != expected
            || read(&root, &format!("{viewer}-repeat.rgba"))? != expected
        {
            return Err("bridge viewer equality failed".into());
        }
        if row["metrics"] != metrics(&root, viewer)? {
            return Err("bridge metric claim changed".into());
        }
    }
    Ok(
        json!({"record":descriptor(&repo_root(),&format!("{DIRECTORY}/bridge.json"))?,"diagnosed_pdf":record["diagnosed_pdf"],"fixed_pdf":record["fixed_pdf"],"structural_changes":changes,"viewers":record["viewers"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colourspec_bridge_rejects_any_other_stream_or_dictionary_change() {
        let old = json!({"qpdf":[{"jsonversion":2},{"obj:1 0 R":{"value":{"/Type":"/Page","/Resources":{"/ColorSpace":{"/CS1":["/ICCBased","5 0 R"]}}}},"obj:7 0 R":{"stream":{"dict":{"/Subtype":"/Image","/ColorSpace":"/CS1","/Interpolate":true},"data":"same raw bytes"}},"trailer":{"value":{"/ID":["old","old"]}}}]});
        let mut fixed = old.clone();
        fixed["qpdf"][1]["obj:7 0 R"]["stream"]["dict"]["/ColorSpace"] =
            json!(["/ICCBased", "5 0 R"]);
        fixed["qpdf"][1]["trailer"]["value"]["/ID"] = json!(["new", "new"]);
        colour_only(&old, &fixed).unwrap();
        for key in ["data", "dict"] {
            let mut bad = fixed.clone();
            bad["qpdf"][1]["obj:7 0 R"]["stream"][key] = json!("unreviewed change");
            assert!(colour_only(&old, &bad).is_err());
        }
        let mut wrong = fixed;
        wrong["qpdf"][1]["obj:7 0 R"]["stream"]["dict"]["/ColorSpace"] =
            json!(["/ICCBased", "6 0 R"]);
        assert!(colour_only(&old, &wrong).is_err());
    }
}
