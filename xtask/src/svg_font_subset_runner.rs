use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};
use viewer_harfrust as harfrust;
use viewer_skrifa::instance::{LocationRef, Size};
use viewer_skrifa::outline::{DrawSettings, OutlinePen};
use viewer_skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::conformance::{canonical_bytes, repo_root, sha256};

pub fn generate(inputs: &str, viewer: &str, output: &str) -> Result<(), String> {
    let inputs_path = repository_path(inputs)?;
    let viewer_path = repository_path(viewer)?;
    let output_path = repository_output_path(output)?;
    let bytes = fs::read(&inputs_path)
        .map_err(|error| format!("failed to read viewer inputs {inputs}: {error}"))?;
    let inputs: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid viewer input manifest: {error}"))?;
    if inputs["schema"] != json!("excalidraw-render/svg-font-subset-viewer-inputs/v1") {
        return Err("unsupported SVG font subset viewer input manifest".to_owned());
    }
    let cases = inputs["cases"]
        .as_array()
        .ok_or_else(|| "viewer input cases must be an array".to_owned())?;
    if cases.len() != crate::svg_font_subsets::EXECUTIONS {
        return Err(format!(
            "expected {} viewer input cases, got {}",
            crate::svg_font_subsets::EXECUTIONS,
            cases.len()
        ));
    }

    let staging = output_path
        .parent()
        .ok_or_else(|| "viewer report output has no parent".to_owned())?
        .join("pure-rust-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("failed to clear {}: {error}", staging.display()))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|error| format!("failed to create {}: {error}", staging.display()))?;

    let mut results = Vec::with_capacity(cases.len());
    for (index, case) in cases.iter().enumerate() {
        results.push(run_case(case, &viewer_path, &staging).map_err(|error| {
            format!(
                "pure-Rust viewer case {}/{} failed: {error}",
                index + 1,
                cases.len()
            )
        })?);
    }
    fs::remove_dir_all(&staging)
        .map_err(|error| format!("failed to remove {}: {error}", staging.display()))?;

    let report = json!({
        "environment": {
            "fontdb": "0.24.0",
            "host": "linux-amd64",
            "harfrust": "0.12.0",
            "resvg": "0.48.1",
            "rustc": rustc_version()?,
            "skrifa": "0.44.0",
            "target": "x86_64-unknown-linux-gnu",
            "tiny-skia": "0.12.0",
            "usvg": "0.48.1",
        },
        "fresh_state_per_role": true,
        "inputs_sha256": sha256(&bytes),
        "results": results,
        "samples_per_role": 2,
        "schema": "excalidraw-render/svg-font-subset-viewer-report/v1",
        "viewer": "pure_rust",
    });
    write_json(&output_path, &report)
}

fn run_case(case: &Value, viewer: &Path, staging: &Path) -> Result<Value, String> {
    let id = string(
        &case["semantic_subset_identity"],
        "semantic subset identity",
    )?;
    verify_sha256(id, "semantic subset identity")?;
    let dimensions = case["dimensions"].clone();
    let expected_length = dimensions["byte_length"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("case {id} has invalid raster byte length"))?;
    let mapped_scalars = case["mapped_scalars"]
        .as_array()
        .ok_or_else(|| format!("case {id} mapped scalars must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| format!("case {id} has an invalid mapped scalar"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_sfnt = read_case_file(case, "source_sfnt", "source_sfnt_sha256")?;
    let subset_sfnt = read_case_file(case, "subset_sfnt", "subset_sfnt_sha256")?;
    let source_svg = read_case_file(case, "source_svg", "source_svg_sha256")?;
    let subset_svg = read_case_file(case, "subset_svg", "subset_svg_sha256")?;
    let source_semantic = semantic_report(&source_sfnt, &source_svg, &mapped_scalars)?;
    let subset_semantic = semantic_report(&subset_sfnt, &subset_svg, &mapped_scalars)?;
    let source_semantic_bytes = canonical_bytes(&source_semantic)?;
    let subset_semantic_bytes = canonical_bytes(&subset_semantic)?;
    let semantic_equal = source_semantic == subset_semantic;

    let mut role_hashes = Vec::with_capacity(2);
    let mut canonical = Vec::new();
    for (role, svg_field, assets_field) in [
        ("source", "source_svg", "source_assets"),
        ("subset", "subset_svg", "subset_assets"),
    ] {
        let mut hashes = Vec::with_capacity(2);
        for run in 0..2 {
            let output = staging.join(format!("{id}-{role}-{run}.rgba"));
            let status = Command::new(viewer)
                .arg("render")
                .arg(repository_path(string(&case[svg_field], svg_field)?)?)
                .arg(&output)
                .arg(repository_path(string(&case[assets_field], assets_field)?)?)
                .output()
                .map_err(|error| format!("failed to start canonical viewer: {error}"))?;
            if !status.status.success() {
                return Err(format!(
                    "canonical viewer rejected {role}: {}",
                    String::from_utf8_lossy(&status.stderr)
                ));
            }
            let rgba = fs::read(&output)
                .map_err(|error| format!("failed to read {}: {error}", output.display()))?;
            fs::remove_file(&output)
                .map_err(|error| format!("failed to remove {}: {error}", output.display()))?;
            validate_raster(&rgba, expected_length)?;
            hashes.push(sha256(&rgba));
            if run == 0 {
                canonical.push(rgba);
            }
        }
        if hashes[0] != hashes[1] {
            return Err(format!("{role} raster is not deterministic"));
        }
        role_hashes.push(hashes);
    }
    let raster_equal = canonical[0] == canonical[1];
    let passed = semantic_equal && raster_equal;
    let checks = json!({
        "advances_offsets": pass(advances_and_offsets(&source_semantic) == advances_and_offsets(&subset_semantic)),
        "glyph_choice": pass(glyph_choices(&source_semantic) == glyph_choices(&subset_semantic)),
        "line_layout": pass(source_semantic["line_layout"] == subset_semantic["line_layout"]),
        "metrics": pass(source_semantic["metrics"] == subset_semantic["metrics"]),
        "rasterized_typography": pass(raster_equal),
        "selectable_text": pass(source_semantic["selectable_text"] == subset_semantic["selectable_text"]),
        "shaping": pass(source_semantic["shaping"] == subset_semantic["shaping"]),
    });
    Ok(json!({
        "checks": checks,
        "comparison": {
            "changed_pixels": if raster_equal { 0 } else { 1 },
            "height": dimensions["height"],
            "passed": raster_equal,
            "ssim_bits": if raster_equal { "3ff0000000000000" } else { "0000000000000000" },
            "visual_class": "typography",
            "width": dimensions["width"],
        },
        "compatibility_record": Value::Null,
        "decision": if passed { "pass" } else { "fail" },
        "dimensions": dimensions,
        "semantic_subset_identity": id,
        "source_raster_sha256": role_hashes[0][0],
        "source_run_sha256": role_hashes[0],
        "source_semantic_sha256": sha256(&source_semantic_bytes),
        "subset_raster_sha256": role_hashes[1][0],
        "subset_run_sha256": role_hashes[1],
        "subset_semantic_sha256": sha256(&subset_semantic_bytes),
    }))
}

fn glyph_choices(report: &Value) -> Value {
    json!({
        "mapped": report["glyphs"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|glyph| json!({"outline": glyph["outline"], "scalar": glyph["scalar"]}))
            .collect::<Vec<_>>(),
        "shaped": report["shaping"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|probe| probe["glyphs"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|glyph| json!({"cluster": glyph["cluster"], "outline": glyph["outline"]}))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
    })
}

fn advances_and_offsets(report: &Value) -> Value {
    json!({
        "mapped_advances": report["glyphs"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|glyph| glyph["advance"].clone())
            .collect::<Vec<_>>(),
        "shaped_positions": report["shaping"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|probe| probe["glyphs"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|glyph| json!({
                    "x_advance": glyph["x_advance"],
                    "x_offset": glyph["x_offset"],
                    "y_advance": glyph["y_advance"],
                    "y_offset": glyph["y_offset"],
                }))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
    })
}

/// The viewer's usvg 0.48.1 reads fonts with skrifa (charmap, unscaled
/// metrics, unhinted outlines at the default location) and shapes with
/// HarfRust 0.12; the report reads and shapes each face the same way.
fn semantic_report(sfnt: &[u8], svg: &[u8], scalars: &[u32]) -> Result<Value, String> {
    let face = FontRef::from_index(sfnt, 0)
        .map_err(|error| format!("qualification sfnt could not be parsed: {error}"))?;
    let text = std::str::from_utf8(svg).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| format!("qualification SVG is invalid: {error}"))?;
    let selected_lines = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "tspan")
        .map(|node| {
            node.text()
                .unwrap_or_default()
                .chars()
                .map(u32::from)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let selected = selected_lines.iter().flatten().copied().collect::<Vec<_>>();
    if selected != scalars {
        return Err("selectable SVG text does not match mapped corpus scalars".to_owned());
    }

    let charmap = face.charmap();
    let glyph_metrics = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let glyphs = scalars
        .iter()
        .map(|scalar| {
            let glyph = charmap
                .map(*scalar)
                .ok_or_else(|| format!("U+{scalar:04X} is not mapped"))?;
            Ok(json!({
                "advance": glyph_metrics.advance_width(glyph),
                "outline": outline_identity(&face, glyph),
                "scalar": scalar,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let shaper_font = harfrust::FontRef::from_index(sfnt, 0)
        .map_err(|error| format!("qualification sfnt could not be shaped: {error}"))?;
    let shaper_data = harfrust::ShaperData::new(&shaper_font);
    let shaper = shaper_data.shaper(&shaper_font).build();
    let shape = |scalars: &[u32], direction| shape_report(&shaper, &face, scalars, direction);
    let line_layout = selected_lines
        .iter()
        .map(|line| shape(line, None))
        .collect::<Result<Vec<_>, _>>()?;
    let mut probes = vec![shape(scalars, None)?];
    for scalar in scalars {
        let repeated = [*scalar, *scalar, *scalar];
        probes.push(shape(&repeated, Some(harfrust::Direction::LeftToRight))?);
        probes.push(shape(&repeated, Some(harfrust::Direction::RightToLeft))?);
    }
    for probe in ["fi", "fl", "ffi", "office", "لا"] {
        let values = probe.chars().map(u32::from).collect::<Vec<_>>();
        if values.iter().all(|scalar| scalars.contains(scalar)) {
            probes.push(shape(&values, None)?);
        }
    }
    let metrics = face.metrics(Size::unscaled(), LocationRef::default());
    Ok(json!({
        "glyphs": glyphs,
        "line_layout": line_layout,
        "metrics": {
            "ascender": metrics.ascent,
            "descender": metrics.descent,
            "line_gap": metrics.leading,
            "units_per_em": metrics.units_per_em,
        },
        "selectable_text": selected,
        "shaping": probes,
    }))
}

fn shape_report(
    shaper: &harfrust::Shaper<'_>,
    face: &FontRef<'_>,
    scalars: &[u32],
    direction: Option<harfrust::Direction>,
) -> Result<Value, String> {
    let direction_name = direction.map_or("guessed", |direction| match direction {
        harfrust::Direction::LeftToRight => "left-to-right",
        harfrust::Direction::RightToLeft => "right-to-left",
        harfrust::Direction::TopToBottom => "top-to-bottom",
        harfrust::Direction::BottomToTop => "bottom-to-top",
        _ => "invalid",
    });
    let text = scalars
        .iter()
        .map(|scalar| char::from_u32(*scalar).ok_or_else(|| "invalid shaping scalar".to_owned()))
        .collect::<Result<String, _>>()?;
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(&text);
    if let Some(direction) = direction {
        buffer.set_direction(direction);
    }
    buffer.guess_segment_properties();
    let shaped = shaper.shape(buffer, harfrust::ShapeOptions::new());
    let glyphs = shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(info, position)| {
            json!({
                "cluster": info.cluster,
                "outline": outline_identity(face, GlyphId::new(info.glyph_id)),
                "x_advance": position.x_advance,
                "x_offset": position.x_offset,
                "y_advance": position.y_advance,
                "y_offset": position.y_offset,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "direction": direction_name,
        "glyphs": glyphs,
        "scalars": scalars,
    }))
}

#[derive(Default)]
struct OutlineDigest {
    bytes: Vec<u8>,
}

impl OutlineDigest {
    fn coordinate(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
}

impl OutlinePen for OutlineDigest {
    fn move_to(&mut self, x: f32, y: f32) {
        self.bytes.push(b'M');
        self.coordinate(x);
        self.coordinate(y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.bytes.push(b'L');
        self.coordinate(x);
        self.coordinate(y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.bytes.push(b'Q');
        for value in [x1, y1, x, y] {
            self.coordinate(value);
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.bytes.push(b'C');
        for value in [x1, y1, x2, y2, x, y] {
            self.coordinate(value);
        }
    }

    fn close(&mut self) {
        self.bytes.push(b'Z');
    }
}

fn outline_identity(face: &FontRef<'_>, glyph: GlyphId) -> Value {
    let mut outline = OutlineDigest::default();
    let drawn = face.outline_glyphs().get(glyph).is_some_and(|glyph| {
        glyph
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut outline,
            )
            .is_ok()
    });
    let bounds = face
        .glyph_metrics(Size::unscaled(), LocationRef::default())
        .bounds(glyph)
        .map(|bounds| json!([bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]));
    json!({
        "bounds": bounds,
        "commands_sha256": sha256(&outline.bytes),
        "drawn": drawn,
    })
}

fn read_case_file(case: &Value, path_field: &str, hash_field: &str) -> Result<Vec<u8>, String> {
    let path = string(&case[path_field], path_field)?;
    let bytes = fs::read(repository_path(path)?)
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    if case[hash_field] != json!(sha256(&bytes)) {
        return Err(format!("stale {hash_field} for {path}"));
    }
    Ok(bytes)
}

fn validate_raster(bytes: &[u8], expected_length: usize) -> Result<(), String> {
    if bytes.len() != expected_length {
        return Err(format!(
            "viewer raster has {} bytes, expected {expected_length}",
            bytes.len()
        ));
    }
    if bytes
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[3] == 0 && pixel[..3] != [0, 0, 0])
    {
        return Err("viewer raster has nonzero RGB below zero alpha".to_owned());
    }
    Ok(())
}

fn pass(value: bool) -> &'static str {
    if value {
        "pass"
    } else {
        "fail"
    }
}

fn rustc_version() -> Result<String, String> {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to execute rustc --version: {error}"))?;
    if !output.status.success() {
        return Err("rustc --version failed".to_owned());
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("rustc version is not UTF-8: {error}"))
}

fn repository_path(relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    let root = fs::canonicalize(repo_root())
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let resolved = fs::canonicalize(root.join(path))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("path escapes repository: {relative}"));
    }
    Ok(resolved)
}

fn repository_output_path(relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "unsafe repository-relative output path {relative:?}"
        ));
    }
    let root = fs::canonicalize(repo_root())
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "output path has no parent".to_owned())?;
    let parent = fs::canonicalize(root.join(parent))
        .map_err(|error| format!("failed to resolve output parent: {error}"))?;
    if !parent.starts_with(&root) {
        return Err(format!("output path escapes repository: {relative}"));
    }
    Ok(parent.join(path.file_name().expect("output path has a file name")))
}

fn string<'a>(value: &'a Value, subject: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{subject} must be a string"))
}

fn verify_sha256(value: &str, subject: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{subject} must be a lowercase SHA-256 digest"));
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = canonical_bytes(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}
