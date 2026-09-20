//! Evidence for a protected PDF backend requalification, never a baseline update.
use crate::{
    conformance::{canonical_bytes, repo_root, sha256},
    conformance_candidate_gate::{candidate_root, descriptor, read, value},
    conformance_candidate_native as native,
};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const EXPECTATION: &str =
    "tests/conformance/pdf-backend-qualification/project-operations-structure.json";
const SCENE: &str = "tests/conformance/pdf-backend-qualification/project-operations.excalidraw";
const APPROVED_PDF: &str = "b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43";
const CURRENT_PDF: &str = "50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc";
const APPROVED_SUBJECT: &str = "c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389";
const OLD_EXPECTATION: &str = "b6cbc6049556ae87780a83e9c252f64193594bf37c9d6a2f18a80434b31cb7b9";
const NEW_EXPECTATION: &str = "442f0cb2580a4066d64bcb924eaa6e070252faca2fa2b37fa4f34c81ff600b1b";
const TEST_SOURCE: &str = "crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs";
const VERIFIER_SOURCE: &str = "xtask/src/conformance_pdf_requalification.rs";
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
fn applied_transition(root: &Path, subject: &Value) -> Result<Option<Value>> {
    if !root.join("applied-transition.json").exists() {
        return Ok(None);
    }
    if sha256(&canonical_bytes(subject)?) != APPROVED_SUBJECT {
        return Err("transition belongs to a different approved proposal".into());
    }
    let transition = value(root, "applied-transition.json")?;
    let approval = value(root, "approval.json")?;
    scoped_approval(&approval, &value(root, "user-approval-comment.json")?)?;
    if transition["schema"] != "excalidraw-render/approved-pdf-pin-transition/v1"
        || transition["approval"] != approval
        || transition["approval_subject_sha256"] != format!("sha256:{APPROVED_SUBJECT}")
    {
        return Err("invalid approved transition record".into());
    }
    for (key, path) in [
        (
            "before_expectation",
            "before-project-operations-structure.json",
        ),
        ("before_test", "before-pdf_backend_qualification.rs"),
        (
            "before_verifier",
            "before-conformance_pdf_requalification.rs",
        ),
        ("comment_snapshot", "user-approval-comment.json"),
    ] {
        let relative = root
            .join(path)
            .strip_prefix(repo_root())
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("transition path")?
            .to_owned();
        if transition[key] != descriptor(&repo_root(), &relative)? {
            return Err(format!("stale transition archive {key}"));
        }
    }
    for (key, path) in [
        ("after_expectation", EXPECTATION),
        ("after_test", TEST_SOURCE),
        ("current_verifier", VERIFIER_SOURCE),
    ] {
        if transition[key] != descriptor(&repo_root(), path)? {
            return Err(format!("stale applied transition source {key}"));
        }
    }
    let before = read(root, "before-project-operations-structure.json")?;
    if subject["protected_expectation"]["sha256"] != sha256(&before)
        || subject["protected_expectation"]["byte_length"] != before.len()
        || read(&repo_root(), EXPECTATION)? != read(root, "proposed-structure.json")?
    {
        return Err("approved before/after expectation association changed".into());
    }
    exact_pin_transition(
        &before,
        &read(&repo_root(), EXPECTATION)?,
        &read(root, "before-pdf_backend_qualification.rs")?,
        &read(&repo_root(), TEST_SOURCE)?,
    )?;
    Ok(Some(transition))
}
fn harness_bindings() -> Result<Vec<Value>> {
    ["tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json",
     "tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json",
     "tests/conformance/structure-profiles/canonical-pdf-structure-v1.json",
     "tests/conformance/comparator-profiles/canonical-visual-v1.json",
     "tools/pdf-harness/run-structure.sh","tools/pdf-harness/structure/inspect.py","tools/pdf-harness/structure/qpdf.lock.json",
     "tools/pdf-harness/run-pdfium.sh","tools/pdf-harness/pdfium/render.py","tools/pdf-harness/pdfium.lock.json",
     "tools/pdf-harness/run-pdfjs.sh","tools/pdf-harness/pdfjs/render.mjs","tools/pdf-harness/pdfjs/package-lock.json",
     "target/pdf-harness/qpdf/bin/qpdf","target/pdf-harness/qpdf/lib/libqpdf.so.30",
     "target/pdf-harness/pdfium/pypdfium2_raw/libpdfium.so",
     "tools/pdf-harness/pdfjs/node_modules/@napi-rs/canvas-linux-x64-gnu/skia.linux-x64-gnu.node",
     "tools/pdf-harness/pdfjs/node_modules/pdfjs-dist/legacy/build/pdf.mjs",
     "xtask/src/conformance_pdf_requalification.rs","xtask/src/conformance_candidate_native.rs","xtask/src/main.rs"]
        .iter().map(|p|descriptor(&repo_root(),p)).collect()
}
fn command(args: &[&str]) -> Result<Value> {
    let out = Command::new("bash")
        .current_dir(repo_root())
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "{}: {}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        ));
    }
    if out.stdout.is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
    }
}
fn put(root: &Path, name: &str, v: &Value) -> Result<()> {
    native::write_new(
        &root.join(name),
        &[canonical_bytes(v)?, b"\n".to_vec()].concat(),
    )
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
fn project_roi(rgba: &[u8], source: &[u8]) -> Result<Value> {
    use base64::Engine as _;
    let scene: Value = serde_json::from_slice(source).map_err(|e| e.to_string())?;
    let image = scene["elements"]
        .as_array()
        .ok_or("elements")?
        .iter()
        .find(|e| e["id"] == "png")
        .ok_or("source image")?;
    if image["x"] != 360
        || image["y"] != 20
        || image["width"] != 40
        || image["height"] != 40
        || image["opacity"] != 75
    {
        return Err("protected image geometry changed".into());
    }
    let encoded = scene["files"]["png-asset"]["dataURL"]
        .as_str()
        .and_then(|s| s.strip_prefix("data:image/png;base64,"))
        .ok_or("source PNG")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut pixels = vec![0; decoder.output_buffer_size()];
    let info = decoder.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if (info.width, info.height) != (1, 1)
        || info.color_type != png::ColorType::GrayscaleAlpha
        || pixels != [0, 255]
    {
        return Err("protected source is not the independently decoded opaque black texel".into());
    }
    // The source image lies at device [370,410] x [50.5,90.5]; avoid its AA
    // boundary. Opaque black at 75% over the authored white page gives 64.
    for y in 56..85 {
        for x in 375..405 {
            let at = (y * 520 + x) * 4;
            if rgba.get(at..at + 4) != Some(&[64, 64, 64, 255]) {
                return Err(format!(
                    "protected image ROI differs from source composition at {x},{y}"
                ));
            }
        }
    }
    Ok(
        json!({"source_size":[1,1],"source_gray_alpha":[0,255],"interior_roi":[375,56,30,29],"expected_rgba":[64,64,64,255],"passed":true,"orientation_observable":false}),
    )
}
fn orientation_control(
    root: &Path,
    directory: &str,
    old_executable: &Path,
    current_executable: &Path,
) -> Result<Value> {
    use base64::Engine as _;
    let samples = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
    ];
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&samples.concat())
            .map_err(|e| e.to_string())?;
    }
    native::write_new(&root.join("orientation-source.png"), &png_bytes)?;
    let scene = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","x":0,"y":0,"width":40,"height":40,"fileId":"source","status":"saved","scale":[1,1],"updated":1}],"appState":{},"files":{"source":{"id":"source","mimeType":"image/png","dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(&png_bytes)),"created":1}}});
    let source = canonical_bytes(&scene)?;
    native::write_new(&root.join("orientation-control.scene.json"), &source)?;
    let mut evidence = Vec::new();
    for (kind, executable) in [
        ("approved", old_executable),
        ("candidate", current_executable),
    ] {
        let mut prior = None;
        for suffix in ["", "-repeat"] {
            let rendered = native::render(
                executable,
                &source,
                "pdf",
                &[
                    "--padding",
                    "0",
                    "--no-background",
                    "--pdf-raster-fallback",
                    "forbid",
                ],
            )?;
            if prior.as_ref().is_some_and(|p| p != &rendered) {
                return Err("orientation control native bytes/reports do not repeat".into());
            }
            native::write_new(
                &root.join(format!("orientation-{kind}{suffix}.pdf")),
                &rendered.0,
            )?;
            native::write_new(
                &root.join(format!("orientation-{kind}{suffix}-report.json")),
                &rendered.1,
            )?;
            prior = Some(rendered);
        }
        for viewer in ["pdfium", "pdfjs"] {
            let mut prior = None;
            for suffix in ["", "-repeat"] {
                let path = format!("{directory}/orientation-{kind}-{viewer}{suffix}.rgba");
                let runtime = command(&[
                    &format!("tools/pdf-harness/run-{viewer}.sh"),
                    "render",
                    &format!("{directory}/orientation-{kind}.pdf"),
                    &path,
                    "40",
                    "40",
                ])?;
                put(
                    root,
                    &format!("orientation-{kind}-{viewer}{suffix}-runtime.json"),
                    &runtime,
                )?;
                let rgba = read(&repo_root(), &path)?;
                if prior.as_ref().is_some_and(|p| p != &rgba) {
                    return Err("orientation control viewer does not repeat".into());
                }
                prior = Some(rgba);
            }
            let rgba = prior.ok_or("orientation raster")?;
            let expected = if kind == "approved" && viewer == "pdfjs" {
                // Preserved, diagnosed old failure: unprimed /CS1 is parsed as
                // gray, consuming the first four RGB bytes, then Y-inverted.
                [
                    [0, 0, 0, 255],
                    [0, 0, 0, 255],
                    [255, 255, 255, 255],
                    [0, 0, 0, 255],
                ]
            } else if kind == "candidate" {
                samples
            } else {
                [samples[2], samples[3], samples[0], samples[1]]
            };
            let mut corners = Vec::new();
            for ((x, y), wanted) in [(5, 5), (34, 5), (5, 34), (34, 34)]
                .into_iter()
                .zip(expected)
            {
                let at = (y * 40 + x) * 4;
                let actual = rgba.get(at..at + 4).ok_or("orientation extent")?;
                if actual != wanted {
                    return Err(format!(
                        "{viewer} {kind} source corner mismatch at {x},{y}: {actual:?}"
                    ));
                }
                corners.push(json!({"x":x,"y":y,"rgba":actual}));
            }
            evidence.push(json!({"kind":kind,"viewer":viewer,"corners":corners,"matches_source_orientation":kind=="candidate","old_colour_defect_reproduced":kind=="approved"&&viewer=="pdfjs","native_and_viewer_repeat_runs":2}));
        }
    }
    Ok(
        json!({"purpose":"independent asymmetric source-image control; not a replacement for the protected scene","source_rgba":samples,"observations":evidence}),
    )
}
pub fn collect(directory: &str, baseline: &str, baseline_executable: &str) -> Result<Value> {
    if descriptor(&repo_root(), EXPECTATION)?["sha256"] != OLD_EXPECTATION {
        return Err("protected transition is already applied; verify its archived approved subject rather than recollecting over it".into());
    }
    crate::pdf_viewers::verify(true)?;
    let harness_before = harness_bindings()?;
    let root = candidate_root(directory)?;
    let old = read(&repo_root(), baseline)?;
    if sha256(&old) != APPROVED_PDF {
        return Err("baseline is not the exact protected approved PDF artifact".into());
    }
    let before = native::producers()?;
    let (executable, build) = native::build()?;
    let scene = read(&repo_root(), SCENE)?;
    let base_executable = Path::new(baseline_executable);
    let base_identity = sha256(&fs::read(base_executable).map_err(|e| e.to_string())?);
    let base_first = native::render(
        base_executable,
        &scene,
        "pdf",
        &["--pdf-raster-fallback", "forbid"],
    )?;
    let base_second = native::render(
        base_executable,
        &scene,
        "pdf",
        &["--pdf-raster-fallback", "forbid"],
    )?;
    if base_first != base_second || base_first.0 != old {
        return Err(
            "baseline executable does not reproduce the exact approved artifact/report twice"
                .into(),
        );
    }
    let first = native::render(
        &executable,
        &scene,
        "pdf",
        &["--pdf-raster-fallback", "forbid"],
    )?;
    let second = native::render(
        &executable,
        &scene,
        "pdf",
        &["--pdf-raster-fallback", "forbid"],
    )?;
    if first != second {
        return Err("PDF backend artifact/report do not repeat".into());
    }
    if sha256(&first.0) != CURRENT_PDF {
        return Err(
            "current protected PDF differs from the diagnosed image-only change; stop for review"
                .into(),
        );
    }
    let mut old_report: Value = serde_json::from_slice(&base_first.1).map_err(|e| e.to_string())?;
    let current_report: Value = serde_json::from_slice(&first.1).map_err(|e| e.to_string())?;
    old_report["artifact"] = current_report["artifact"].clone();
    if old_report != current_report {
        return Err("protected native diagnostics/report semantics changed".into());
    }
    native::write_new(&root.join("approved.pdf"), &old)?;
    native::write_new(&root.join("approved-repeat.pdf"), &base_second.0)?;
    native::write_new(&root.join("approved-report.json"), &base_first.1)?;
    native::write_new(&root.join("approved-repeat-report.json"), &base_second.1)?;
    native::write_new(&root.join("candidate.pdf"), &first.0)?;
    native::write_new(&root.join("candidate-repeat.pdf"), &second.0)?;
    native::write_new(&root.join("candidate-report.json"), &first.1)?;
    native::write_new(&root.join("candidate-repeat-report.json"), &second.1)?;
    let input = |name: &str| format!("{directory}/{name}");
    for (kind, expected) in [("approved", "pass"), ("candidate", "fail")] {
        for suffix in ["", "-repeat"] {
            command(&[
                "tools/pdf-harness/run-structure.sh",
                "inspect",
                &input(&format!("{kind}.pdf")),
                EXPECTATION,
                expected,
                &input(&format!("{kind}-structure{suffix}.json")),
            ])?;
        }
        if read(&root, &format!("{kind}-structure.json"))?
            != read(&root, &format!("{kind}-structure-repeat.json"))?
        {
            return Err("structural inspection does not repeat".into());
        }
    }
    let old_structure = value(&root, "approved-structure.json")?;
    let structure = value(&root, "candidate-structure.json")?;
    let delta = structural_delta(&old_structure, &structure)?;
    let failures: Vec<_> = structure["checks"]
        .as_array()
        .ok_or("structural checks")?
        .iter()
        .filter(|r| r["passed"] != true)
        .map(|r| r["id"].as_str().unwrap_or("unknown"))
        .collect();
    if failures != ["operation-sequence-sha256", "structural-inventory-sha256"] {
        return Err(format!(
            "unexpected protected structural drift: {failures:?}"
        ));
    }
    let mut proposal = value(&repo_root(), EXPECTATION)?;
    for (key, id) in [
        ("operation_sha256", "operation-sequence-sha256"),
        ("inventory_sha256", "structural-inventory-sha256"),
    ] {
        proposal[key] = structure["checks"]
            .as_array()
            .ok_or("checks")?
            .iter()
            .find(|r| r["id"] == id)
            .ok_or("missing structural digest")?["actual"]
            .clone();
    }
    put(&root, "proposed-structure.json", &proposal)?;
    command(&[
        "tools/pdf-harness/run-structure.sh",
        "inspect",
        &input("candidate.pdf"),
        &input("proposed-structure.json"),
        "pass",
        &input("proposed-structure-result.json"),
    ])?;
    let old_ops = old_structure["pages"][0]["operations"]
        .as_array()
        .ok_or("old operations")?;
    let new_ops = structure["pages"][0]["operations"]
        .as_array()
        .ok_or("new operations")?;
    if old_ops.len() != new_ops.len() {
        return Err("operation count changed".into());
    }
    let changes: Vec<_> = old_ops
        .iter()
        .zip(new_ops)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| json!({"index":i,"before":a,"after":b}))
        .collect();
    let mut visual = Vec::new();
    for viewer in ["pdfium", "pdfjs"] {
        for kind in ["approved", "candidate"] {
            for suffix in ["", "-repeat"] {
                let result = command(&[
                    &format!("tools/pdf-harness/run-{viewer}.sh"),
                    "render",
                    &input(&format!("{kind}.pdf")),
                    &input(&format!("{kind}-{viewer}{suffix}.rgba")),
                    "520",
                    "200",
                ])?;
                put(
                    &root,
                    &format!("{kind}-{viewer}{suffix}-runtime.json"),
                    &result,
                )?;
            }
            if read(&root, &format!("{kind}-{viewer}.rgba"))?
                != read(&root, &format!("{kind}-{viewer}-repeat.rgba"))?
            {
                return Err(format!("{viewer} does not repeat"));
            }
        }
        use excalidraw_render_compare::{CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass};
        let a = CanonicalRgba8::new(520, 200, read(&root, &format!("approved-{viewer}.rgba"))?)
            .map_err(|e| e.to_string())?;
        let b = CanonicalRgba8::new(520, 200, read(&root, &format!("candidate-{viewer}.rgba"))?)
            .map_err(|e| e.to_string())?;
        let c = excalidraw_render_compare::compare(
            &a,
            &b,
            VisualClass::GeometryImage,
            &[],
            if viewer == "pdfjs" {
                ComparisonMode::PdfJs
            } else {
                ComparisonMode::Canonical
            },
        )
        .map_err(|e| e.to_string())?;
        let a_bytes = read(&root, &format!("approved-{viewer}.rgba"))?;
        let b_bytes = read(&root, &format!("candidate-{viewer}.rgba"))?;
        if a_bytes != b_bytes {
            return Err(format!("{viewer}: protected scene changed visually beyond the constant-texel orientation/interpolation correction"));
        }
        let roi = project_roi(&b_bytes, &scene)?;
        put(&root, &format!("{viewer}-source-roi.json"), &roi)?;
        visual.push(json!({"viewer":viewer,"purpose":"measurement against the protected native PDF, not reference-renderer parity approval","changed_pixels":c.whole.changed_pixels,"ssim_bits":match c.whole.ssim{SsimOutcome::Score(s)=>Some(format!("{:016x}",s.to_bits())),_=>None},"within_canonical_budget_against_old_native":c.passed()}));
    }
    let orientation = orientation_control(&root, directory, base_executable, &executable)?;
    put(&root, "image-orientation-evidence.json", &orientation)?;
    put(&root, "structural-delta.json", &delta)?;
    native::check_producers(&before)?;
    if harness_before != harness_bindings()?
        || base_identity != sha256(&fs::read(base_executable).map_err(|e| e.to_string())?)
    {
        return Err("harness or baseline executable changed during collection".into());
    }
    let mut files = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .map(|e| {
            e.map_err(|e| e.to_string())
                .and_then(|e| e.file_name().into_string().map_err(|_| "file name".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    files.retain(|p| p != "README.md");
    files.sort();
    let subject = json!({"schema":"excalidraw-render/pdf-backend-requalification-subject/v1","originating_issue":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85","approval_required":true,"approved_artifact_sha256":APPROVED_PDF,"candidate_artifact_sha256":sha256(&first.0),"protected_expectation":descriptor(&repo_root(),EXPECTATION)?,"scene":descriptor(&repo_root(),SCENE)?,"build":build,"baseline":{"commit":"629930b4ce4e8d09d4f44a42c495278aab0ead15","executable_sha256":base_identity,"fresh_process_runs":2},"producer_sources":before,"harness_sources":harness_before,"changed_operations":changes,"structural_delta":delta,"visual_measurements":visual,"necessity":"Correct source-image vertical orientation, explicit image/mask interpolation, and standards-conforming direct image ICCBased references change the protected PDF bytes and exact operation/inventory digests. All other structure and protected-scene viewer bytes remain unchanged; fixed unprimed source colours pass both viewers. Old PDF.js alias failure is retained as a negative control. Protected files are not modified by collection.","files":files.iter().map(|p|descriptor(&root,p)).collect::<Result<Vec<_>>>()?});
    put(&root, "approval-subject.json", &subject)?;
    put(&root, "approval.json", &Value::Null)?;
    Ok(
        json!({"approval_subject_sha256":sha256(&canonical_bytes(&subject)?),"changed_operations":subject["changed_operations"],"visual_measurements":visual,"candidate_artifact_sha256":sha256(&first.0),"protected_files_updated":false}),
    )
}

pub fn verify(directory: &str) -> Result<String> {
    crate::pdf_viewers::verify(true)?;
    let root = candidate_root(directory)?;
    let subject = value(&root, "approval-subject.json")?;
    let transition = applied_transition(&root, &subject)?;
    let mut expected_harness = harness_bindings()?;
    if transition.is_some() {
        // Exactly one verifier implementation changed to consume its approved
        // history. Its original bytes remain pinned by the unchanged subject.
        let archived = read(&root, "before-conformance_pdf_requalification.rs")?;
        let recorded = subject["harness_sources"]
            .as_array()
            .ok_or("subject harness sources")?
            .iter()
            .find(|d| d["path"] == VERIFIER_SOURCE)
            .ok_or("missing original verifier binding")?;
        if recorded["sha256"] != sha256(&archived) || recorded["byte_length"] != archived.len() {
            return Err("historical verifier source differs from approved subject".into());
        }
        let current = expected_harness
            .iter_mut()
            .find(|d| d["path"] == VERIFIER_SOURCE)
            .ok_or("missing current verifier source")?;
        *current = recorded.clone();
    }
    if subject["schema"] != "excalidraw-render/pdf-backend-requalification-subject/v1"
        || subject["approved_artifact_sha256"] != APPROVED_PDF
        || subject["candidate_artifact_sha256"] != CURRENT_PDF
        || (transition.is_none()
            && subject["protected_expectation"] != descriptor(&repo_root(), EXPECTATION)?)
        || subject["scene"] != descriptor(&repo_root(), SCENE)?
        || subject["harness_sources"] != json!(expected_harness)
    {
        return Err("stale or invalid PDF requalification subject".into());
    }
    for d in subject["files"]
        .as_array()
        .ok_or("subject evidence files")?
    {
        let path = d["path"].as_str().ok_or("file path")?;
        if d != &descriptor(&root, path)? {
            return Err(format!("stale PDF requalification evidence {path}"));
        }
    }
    native::check_producers(&subject["producer_sources"])?;
    if sha256(&read(&root, "approved.pdf")?) != APPROVED_PDF
        || sha256(&read(&root, "candidate.pdf")?) != CURRENT_PDF
    {
        return Err("requalification PDF identity changed".into());
    }
    let delta = structural_delta(
        &value(&root, "approved-structure.json")?,
        &value(&root, "candidate-structure.json")?,
    )?;
    if subject["structural_delta"] != delta {
        return Err("structural delta claim changed".into());
    }
    let (executable, build) = native::build()?;
    if subject["build"] != build {
        return Err("requalification native build changed".into());
    }
    for _ in 0..2 {
        let render = native::render(
            &executable,
            &read(&repo_root(), SCENE)?,
            "pdf",
            &["--pdf-raster-fallback", "forbid"],
        )?;
        if render.0 != read(&root, "candidate.pdf")?
            || render.1 != read(&root, "candidate-report.json")?
        {
            return Err("requalification fresh native replay changed".into());
        }
    }
    Ok(sha256(&canonical_bytes(&subject)?))
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
