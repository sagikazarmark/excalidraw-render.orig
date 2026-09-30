//! Representative-candidate asset preparation and diagnostic comparisons.
//! Approval readiness is checked by `conformance_candidate_gate`.
use crate::conformance::{repo_root, sha256};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Component, Path},
};

fn read(root: &Path, name: &str) -> Result<Vec<u8>, String> {
    if Path::new(name)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("candidate path must be relative".into());
    }
    let path = root.join(name).canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("candidate path escapes its root".into());
    }
    fs::read(path).map_err(|e| e.to_string())
}
pub fn prepare_assets(directory: &str) -> Result<(), String> {
    derived_assets(directory, false)
}
/// Recompute from the original exports, not from the stored descriptor claims.
/// Verification never creates, repairs, or overwrites a derived resource.
pub fn verify_assets(directory: &str) -> Result<(), String> {
    derived_assets(directory, true)
}
fn derived_assets(directory: &str, verify: bool) -> Result<(), String> {
    let root = crate::conformance_candidate_gate::candidate_root(directory)?;
    let output = std::process::Command::new("node")
        .current_dir(repo_root())
        .args([
            "tools/reference-oracle/scripts/stage4-viewer-assets.mjs",
            directory,
            "verify",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "SVG resource derivation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let emit = |path: &Path, bytes: &[u8]| -> Result<(), String> {
        if verify || path.exists() {
            if fs::read(path).map_err(|e| e.to_string())? != bytes {
                return Err(format!("SVG derived artifact differs: {}", path.display()));
            }
            Ok(())
        } else {
            crate::conformance_candidate_native::write_new(path, bytes)
        }
    };
    let plans: Value = serde_json::from_slice(&read(&root, "viewer-assets-plan.json")?)
        .map_err(|e| e.to_string())?;
    for plan in plans.as_array().ok_or("asset plan must be an array")? {
        let kind = plan["kind"].as_str().ok_or("missing artifact kind")?;
        if !matches!(kind, "official" | "native") {
            return Err("invalid artifact kind".into());
        }
        let mut fonts = Vec::new();
        for (i, font) in plan["fonts"]
            .as_array()
            .ok_or("missing fonts")?
            .iter()
            .enumerate()
        {
            let woff = read(
                &repo_root(),
                font["woff2"].as_str().ok_or("missing WOFF2 path")?,
            )?;
            if font["source"]["sha256"] != json!(sha256(&woff)) {
                return Err("stale WOFF2".into());
            }
            let sfnt = wuff::decompress_woff2(&woff).map_err(|e| format!("WOFF2 decode: {e:?}"))?;
            let face = ttf_parser::Face::parse(&sfnt, 0)
                .ok()
                .ok_or("invalid decoded SFNT")?;
            let family = [16, 1]
                .into_iter()
                .find_map(|id| {
                    face.names()
                        .into_iter()
                        .filter(|n| n.name_id == id)
                        .find_map(|n| n.to_string())
                })
                .ok_or("missing font family")?;
            {
                if plan["fonts"].as_array().ok_or("missing font faces")?.len() != 1 {
                    return Err(
                        "this candidate requires one fully covering declared text face".into(),
                    );
                }
                let original = String::from_utf8(read(&root, &format!("{kind}-view.svg"))?)
                    .map_err(|e| e.to_string())?;
                let xml = roxmltree::Document::parse(&original).map_err(|e| e.to_string())?;
                let mut replacements = Vec::new();
                for text in xml.descendants().filter(|n| n.has_tag_name("text")) {
                    let full = text
                        .ancestors()
                        .find_map(|n| n.attribute("font-family"))
                        .ok_or("missing official text family")?;
                    let primary = full
                        .split(',')
                        .next()
                        .ok_or("empty font family")?
                        .trim()
                        .trim_matches(['\'', '"']);
                    if primary != font["css"]["family"].as_str().ok_or("missing CSS family")? {
                        return Err("text is not covered by the declared primary face".into());
                    }
                    for node in text.descendants().filter(|n| n.is_text()) {
                        for ch in node.text().unwrap_or("").chars() {
                            if face.glyph_index(ch).is_none() {
                                return Err(format!("official primary face does not cover {ch:?}"));
                            }
                        }
                    }
                    replacements.push((
                        format!("font-family=\"{full}\""),
                        format!("font-family=\"{primary}\""),
                    ));
                }
                let mut normalized = original;
                for (before, after) in replacements {
                    normalized = normalized.replace(&before, &after);
                }
                emit(
                    &root.join(format!("{kind}-rust.svg")),
                    normalized.as_bytes(),
                )?;
            }
            let stretch = match face.width().to_number() {
                1 => "ultra-condensed",
                2 => "extra-condensed",
                3 => "condensed",
                4 => "semi-condensed",
                5 => "normal",
                6 => "semi-expanded",
                7 => "expanded",
                8 => "extra-expanded",
                9 => "ultra-expanded",
                _ => return Err("invalid font width".into()),
            };
            let path = format!("{directory}/assets/{kind}-font-{i}.sfnt");
            emit(&repo_root().join(&path), &sfnt)?;
            fonts.push(json!({"source":font["source"],"css":font["css"],"sfnt":{"path":path,"media_type":if sfnt.starts_with(b"OTTO"){"font/otf"}else{"font/ttf"},"sha256":sha256(&sfnt),"face_index":0,"family":family,"style":if face.is_italic(){"italic"}else{"normal"},"weight":face.weight().to_number(),"stretch":stretch}}));
        }
        let declaration = json!({"schema":"excalidraw-render/svg-viewer-asset-declaration/v1","fonts":fonts,"images":plan["images"]});
        emit(
            &root.join(format!("{kind}-assets.json")),
            &serde_json::to_vec_pretty(&declaration).map_err(|e| e.to_string())?,
        )?;
    }
    Ok(())
}
pub fn comparisons(collection: &str) -> Result<Value, String> {
    comparisons_with_scopes(collection, &[])
}
/// Internal proposed-limit measurement. Callers must validate the exact scoped
/// proposal first; this function never confers approval or reads stored judgments.
pub(crate) fn comparisons_with_scopes(collection: &str, scopes: &[Value]) -> Result<Value, String> {
    use excalidraw_render_compare::{
        CanonicalRgba8, ComparisonLimits, ComparisonMode, Crop, RegionReport, SsimOutcome,
        VisualClass,
    };
    let record: Value =
        serde_json::from_slice(&read(&repo_root(), collection)?).map_err(|e| e.to_string())?;
    let captures = record["captures"].as_array().ok_or("missing captures")?;
    let image = |id: &str| -> Result<CanonicalRgba8, String> {
        let row = captures
            .iter()
            .find(|r| r["id"] == id)
            .ok_or(format!("missing capture {id}"))?;
        let d = &row["raster"];
        let bytes = read(
            &repo_root(),
            d["path"].as_str().ok_or("missing raster path")?,
        )?;
        if d["sha256"] != json!(sha256(&bytes)) {
            return Err("stale raster".into());
        }
        CanonicalRgba8::new(200, 120, bytes).map_err(|e| e.to_string())
    };
    let dir = record["candidate_directory"]
        .as_str()
        .unwrap_or("tests/conformance/candidates/stage4-frames-text-assets");
    let plan: Value =
        serde_json::from_slice(&read(&repo_root(), &format!("{dir}/judgment-plan.json"))?)
            .map_err(|e| e.to_string())?;
    if plan["whole_class"] != "geometry-image" || plan["exceptions"] != json!([]) {
        return Err("candidate budgets must remain canonical".into());
    }
    let mut crops = Vec::new();
    for c in plan["crops"].as_array().ok_or("missing focused crops")? {
        let class = match c["class"].as_str() {
            Some("typography") => VisualClass::Typography,
            Some("geometry-image") => VisualClass::GeometryImage,
            _ => return Err("invalid crop class".into()),
        };
        let n = |k| {
            c[k].as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("invalid crop coordinate")
        };
        crops.push(
            Crop::new(
                c["id"].as_str().ok_or("missing crop ID")?,
                n("x")?,
                n("y")?,
                n("width")?,
                n("height")?,
                class,
            )
            .map_err(|e| e.to_string())?,
        );
    }
    let region = |r: &RegionReport| json!({"width":r.width,"height":r.height,"area":r.area,"changed_pixels":r.changed_pixels,"changed_mask_sha256":sha256(&r.changed_mask.iter().map(|b|u8::from(*b)).collect::<Vec<_>>()),"ssim_bits":match r.ssim{SsimOutcome::Score(v)=>Some(format!("{:016x}",v.to_bits())),_=>None},"changed_pixels_pass":r.changed_pixels_pass,"ssim_pass":r.ssim_pass});
    let mut rows = Vec::new();
    for (id, a, b, mode) in [
        (
            "svg-chromium",
            "official-chromium",
            "native-chromium",
            ComparisonMode::Canonical,
        ),
        (
            "svg-rust",
            "official-rust",
            "native-rust",
            ComparisonMode::PureRustSvg,
        ),
        (
            "pdf-pdfium",
            "official-chromium",
            "native-pdfium",
            ComparisonMode::Canonical,
        ),
        (
            "pdf-pdfjs",
            "official-chromium",
            "native-pdfjs",
            ComparisonMode::PdfJs,
        ),
    ] {
        let mut scoped_crops = crops.clone();
        let mut whole = VisualClass::GeometryImage.limits(mode);
        for scope in scopes.iter().filter(|s| s["comparison_id"] == id) {
            let bits = u64::from_str_radix(
                scope["minimum_ssim_bits"]
                    .as_str()
                    .ok_or("exception SSIM bits")?,
                16,
            )
            .map_err(|e| e.to_string())?;
            let limits = ComparisonLimits::tolerance_exception(
                scope["max_changed_pixels"]
                    .as_u64()
                    .ok_or("exception pixel ceiling")?,
                f64::from_bits(bits),
            )
            .map_err(|e| e.to_string())?;
            if scope["region"] == "whole" {
                whole = limits;
            } else {
                let region = scope["region"].as_str().ok_or("exception region")?;
                let plans = plan["crops"].as_array().ok_or("crop plan")?;
                let index = plans
                    .iter()
                    .position(|c| c["id"] == region)
                    .ok_or("exception names unknown crop")?;
                let crop = &plans[index];
                let n = |key| {
                    crop[key]
                        .as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .ok_or("crop coordinate")
                };
                scoped_crops[index] =
                    Crop::with_limits(region, n("x")?, n("y")?, n("width")?, n("height")?, limits)
                        .map_err(|e| e.to_string())?;
            }
        }
        let comparison = excalidraw_render_compare::compare_with_limits(
            &image(a)?,
            &image(b)?,
            whole,
            &scoped_crops,
            mode,
        )
        .map_err(|e| e.to_string())?;
        rows.push(json!({"id":id,"reference":a,"candidate":b,"passed":comparison.passed(),"whole":region(&comparison.whole),"crops":comparison.crops.iter().map(|c|json!({"id":c.id,"region":region(&c.region)})).collect::<Vec<_>>()}));
    }
    let reference = CanonicalRgba8::new(
        200,
        120,
        read(&repo_root(), &format!("{dir}/official-png.rgba"))?,
    )
    .map_err(|e| e.to_string())?;
    let bytes = read(&repo_root(), &format!("{dir}/native.png"))?;
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut rgba = vec![0; reader.output_buffer_size().ok_or("PNG buffer overflows")?];
    let info = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("non-RGBA8 candidate".into());
    }
    let candidate =
        CanonicalRgba8::new(info.width, info.height, rgba).map_err(|e| e.to_string())?;
    let c = excalidraw_render_compare::compare(
        &reference,
        &candidate,
        VisualClass::GeometryImage,
        &crops,
        ComparisonMode::Canonical,
    )
    .map_err(|e| e.to_string())?;
    rows.push(json!({"id":"png-canvas","passed":c.passed(),"whole":region(&c.whole),"crops":c.crops.iter().map(|c|json!({"id":c.id,"region":region(&c.region)})).collect::<Vec<_>>()}));
    Ok(json!({"schema":"excalidraw-render/representative-comparison-results/v1","rows":rows}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(root: &Path, at: &Path, files: &mut Vec<(String, Vec<u8>)>) {
            for e in fs::read_dir(at).unwrap() {
                let e = e.unwrap();
                if e.file_type().unwrap().is_dir() {
                    walk(root, &e.path(), files);
                } else {
                    files.push((
                        e.path()
                            .strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_owned(),
                        fs::read(e.path()).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        walk(root, root, &mut files);
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }
    #[test]
    fn provenance_derivation_rejects_mutations_without_repairing_evidence() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = format!(
            "target/stage4-derivation-test-{}-{nonce}",
            std::process::id()
        );
        let root = repo_root().join(&directory);
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        let original =
            repo_root().join("tests/conformance/candidates/stage4-frames-text-assets-v4");
        for name in ["official.svg", "native.svg"] {
            fs::write(root.join(name), read(&original, name).unwrap()).unwrap();
        }
        let output = std::process::Command::new("node")
            .current_dir(repo_root())
            .args([
                "tools/reference-oracle/scripts/stage4-viewer-assets.mjs",
                &directory,
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        prepare_assets(&directory).unwrap();
        let baseline = snapshot(&root);
        verify_assets(&directory).unwrap();
        assert_eq!(snapshot(&root), baseline);
        for name in [
            "official-view.svg",
            "native-rust.svg",
            "assets/native-font-0.woff2",
            "assets/native-font-0.sfnt",
            "assets/native-image-0.png",
            "viewer-assets-plan.json",
            "native-assets.json",
        ] {
            let path = root.join(name);
            let before = fs::read(&path).unwrap();
            let changed = if name.ends_with(".json") {
                // Keep valid JSON, and mutate claims rather than just syntax.
                let mut v: Value = serde_json::from_slice(&before).unwrap();
                if name == "viewer-assets-plan.json" {
                    v[0]["fonts"][0]["source"]["sha256"] = json!("rehashed-but-detached");
                } else {
                    v["fonts"][0]["sfnt"]["family"] = json!("Detached family");
                }
                serde_json::to_vec_pretty(&v).unwrap()
            } else {
                let mut v = before.clone();
                v.push(b' ');
                v
            };
            fs::write(&path, changed).unwrap();
            let mutated = snapshot(&root);
            let error = verify_assets(&directory).unwrap_err();
            assert!(
                error.contains("derivation") || error.contains("derived artifact"),
                "{name}: {error}"
            );
            assert_eq!(
                snapshot(&root),
                mutated,
                "verification must not repair {name}"
            );
            fs::write(&path, before).unwrap();
        }
        let missing = root.join("native-rust.svg");
        let before = fs::read(&missing).unwrap();
        fs::remove_file(&missing).unwrap();
        assert!(verify_assets(&directory).is_err());
        assert!(
            !missing.exists(),
            "verification must not recreate a missing derivative"
        );
        fs::write(missing, before).unwrap();
        assert_eq!(snapshot(&root), baseline);
    }
}
