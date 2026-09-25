//! Judgment decisions at the command boundary over real, small raster fixtures.
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
struct Fixture {
    root: PathBuf,
    case: Value,
}
impl Fixture {
    fn new(control_noise: bool, candidate_noise: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "representative-judge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        copy(
            &root().join("tests/conformance/cases/representative/excali-local-slides"),
            &directory.join("case"),
        );
        let mut case: Value =
            serde_json::from_slice(&fs::read(directory.join("case/case.json")).unwrap()).unwrap();
        case["approval"] = Value::Null;
        case["exceptions"] = json!([]);
        write_json(&directory.join("case/case.json"), &case);
        fs::create_dir(directory.join("collection")).unwrap();
        let f = Self {
            root: directory,
            case,
        };
        let mut judgments = Vec::new();
        for variant in ["light", "dark"] {
            fs::create_dir(f.root.join("collection").join(variant)).unwrap();
            let plan = json!({"width":32,"height":32,"whole_class":"mixed-representative","crops":[{"id":"avatar","class":"geometry/image","x":0,"y":0,"width":16,"height":16}]});
            write_json(
                &f.root.join("collection").join(variant).join("regions.json"),
                &plan,
            );
            for name in [
                "official-chromium",
                "official-rust",
                "official-png",
                "native-chromium",
                "native-rust",
                "native-pdfium",
                "native-pdfjs",
            ] {
                f.raster(
                    variant,
                    name,
                    if name == "official-rust" {
                        control_noise
                    } else {
                        name == "native-pdfium" || name == "native-pdfjs"
                    } && candidate_noise,
                );
            }
            // Keep native Rust matched to its reference, including control noise.
            f.raster(variant, "native-rust", control_noise);
            f.png(variant);
            for comparison in f.case["comparisons"].as_array().unwrap() {
                // Deliberately forged pass flags: judgment must measure pixels.
                judgments.push(json!({"variant":variant,"comparison":comparison,"result":{"passed":true,"whole":{"changed_pixels_pass":true,"ssim_pass":true},"crops":[]}}));
            }
        }
        write_json(
            &f.root.join("collection/viewers.json"),
            &json!({"judgments":judgments}),
        );
        f
    }
    fn image(noise: bool) -> Vec<u8> {
        let mut pixels = Vec::new();
        for y in 0..32 {
            for x in 0..32 {
                let value = if noise && x < 16 && y < 16 && (x + y) % 4 == 0 {
                    120
                } else if (8..24).contains(&x) && (8..24).contains(&y) {
                    0
                } else {
                    255
                };
                pixels.extend([value, value, value, 255]);
            }
        }
        pixels
    }
    fn raster(&self, variant: &str, name: &str, noise: bool) {
        fs::write(
            self.root
                .join("collection")
                .join(variant)
                .join(format!("{name}.rgba.zlib")),
            miniz_oxide::deflate::compress_to_vec_zlib(&Self::image(noise), 6),
        )
        .unwrap();
    }
    fn png(&self, variant: &str) {
        let file = fs::File::create(
            self.root
                .join("collection")
                .join(variant)
                .join("native.png"),
        )
        .unwrap();
        let mut encoder = png::Encoder::new(file, 32, 32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&Self::image(false))
            .unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("representative")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap()
    }
    fn judge(&self, success: bool) -> Value {
        let output = self.run(&["case", "judge", "case", "collection"]);
        assert_eq!(
            output.status.success(),
            success,
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if success {
            serde_json::from_slice(&output.stdout).unwrap()
        } else {
            json!(String::from_utf8(output.stderr).unwrap())
        }
    }
    fn controls(&self, comparison: &str) -> Value {
        let out = self.run(&["controls", "collection/light", "--comparison", comparison]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn ceilings(&mut self) {
        let mut exceptions = Vec::new();
        for comparison in ["pdf-pdfium", "pdf-pdfjs"] {
            let report = self.controls(comparison);
            for row in report["failing_regions"].as_array().unwrap() {
                for variant in ["light", "dark"] {
                    exceptions.push(json!({"variant":variant,"comparison_id":comparison,"region":row["region"],"max_changed_pixels":row["changed_pixels"],"minimum_ssim_bits":format!("{:016x}",row["ssim"].as_f64().unwrap().to_bits()),"reason":"fixture exact measurement","approving_issue":"https://example.invalid","viewer_profile":if comparison == "pdf-pdfium" {"canonical-pdf-pdfium-v1"} else {"canonical-pdf-pdfjs-v1"}}));
                }
            }
        }
        assert!(!exceptions.is_empty());
        self.case["exceptions"] = json!(exceptions);
        self.save();
    }
    fn save(&self) {
        write_json(&self.root.join("case/case.json"), &self.case);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn canonical_budget_and_identical_input_controls_pass_without_exceptions() {
    let exact = Fixture::new(false, false);
    let summary = exact.judge(true);
    assert_eq!(summary["within_controls"], 0);
    assert_eq!(summary["exceptions_applied"], 0);
    let controlled = Fixture::new(true, true);
    let summary = controlled.judge(true);
    assert!(summary["within_controls"].as_u64().unwrap() > 0);
    assert_eq!(summary["exceptions_applied"], 0);
    let report = controlled.controls("pdf-pdfium");
    let rows = report["failing_regions"].as_array().unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r["verdict"] == "within"));
}

#[test]
fn forged_pass_flags_cannot_hide_out_of_budget_rasters() {
    let f = Fixture::new(false, true);
    assert!(f
        .judge(false)
        .as_str()
        .unwrap()
        .contains("comparisons fail"));
    let report = f.controls("pdf-pdfium");
    let rows = report["failing_regions"].as_array().unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r["verdict"] == "beyond"));
}

#[test]
fn exact_exception_ceilings_pass_but_pixel_and_ssim_overruns_fail() {
    let mut f = Fixture::new(false, true);
    f.ceilings();
    assert_eq!(
        f.judge(true)["exceptions_applied"].as_u64().unwrap() as usize,
        f.case["exceptions"].as_array().unwrap().len()
    );
    let original = f.case.clone();
    let pixels = f.case["exceptions"][0]["max_changed_pixels"]
        .as_u64()
        .unwrap();
    assert!(pixels > 0);
    f.case["exceptions"][0]["max_changed_pixels"] = json!(pixels - 1);
    f.save();
    assert!(f
        .judge(false)
        .as_str()
        .unwrap()
        .contains("comparisons fail"));
    f.case = original.clone();
    let bits = u64::from_str_radix(
        f.case["exceptions"][0]["minimum_ssim_bits"]
            .as_str()
            .unwrap(),
        16,
    )
    .unwrap();
    f.case["exceptions"][0]["minimum_ssim_bits"] = json!(format!("{:016x}", bits + 1));
    f.save();
    assert!(f
        .judge(false)
        .as_str()
        .unwrap()
        .contains("comparisons fail"));
    f.case = original;
    f.case["exceptions"][0]["minimum_ssim_bits"] = json!("7ff8000000000000");
    f.save();
    assert!(f.judge(false).as_str().unwrap().contains("non-finite"));
}

#[test]
fn exceptions_become_dead_when_budget_or_controls_cover_them() {
    let mut f = Fixture::new(false, true);
    f.ceilings();
    for variant in ["light", "dark"] {
        f.raster(variant, "native-pdfium", false);
        f.raster(variant, "native-pdfjs", false);
    }
    assert!(f.judge(false).as_str().unwrap().contains("dead exceptions"));
    for variant in ["light", "dark"] {
        f.raster(variant, "native-pdfium", true);
        f.raster(variant, "native-pdfjs", true);
        f.raster(variant, "official-rust", true);
        f.raster(variant, "native-rust", true);
    }
    assert!(f.judge(false).as_str().unwrap().contains("dead exceptions"));
}

#[test]
fn missing_duplicate_and_unknown_judgments_are_rejected() {
    let f = Fixture::new(false, false);
    let path = f.root.join("collection/viewers.json");
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for mutation in 0..4 {
        let mut record = original.clone();
        match mutation {
            0 => {
                record["judgments"] = json!([]);
            }
            1 => {
                let row = record["judgments"][0].clone();
                record["judgments"].as_array_mut().unwrap().push(row);
            }
            2 => record["judgments"][0]["comparison"] = json!("unknown"),
            _ => record["judgments"][0]["variant"] = json!("../escape"),
        }
        write_json(&path, &record);
        f.judge(false);
    }
}

#[test]
fn corrupt_rasters_regions_and_wrong_png_dimensions_fail() {
    let f = Fixture::new(false, false);
    fs::write(
        f.root.join("collection/light/native-pdfium.rgba.zlib"),
        b"not zlib",
    )
    .unwrap();
    f.judge(false);
    f.raster("light", "native-pdfium", false);
    let path = f.root.join("collection/light/regions.json");
    let plan: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for mutation in 0..3 {
        let mut changed = plan.clone();
        match mutation {
            0 => changed["crops"][0]["id"] = json!("whole"),
            1 => changed["crops"][0]["width"] = json!(33),
            _ => changed["width"] = json!(31),
        }
        write_json(&path, &changed);
        f.judge(false);
    }
    write_json(&path, &plan);
    let png = f.root.join("collection/light/native.png");
    let file = fs::File::create(png).unwrap();
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0, 0, 0, 255])
        .unwrap();
    assert!(f.judge(false).as_str().unwrap().contains("PNG dimensions"));
}

#[test]
fn controls_tiles_report_and_outputs_are_create_only() {
    let f = Fixture::new(false, true);
    let args = [
        "controls",
        "collection/light",
        "--comparison",
        "pdf-pdfium",
        "--tiles",
        "16",
        "--output",
        "report.json",
    ];
    let out = f.run(&args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = fs::read(f.root.join("report.json")).unwrap();
    assert_eq!(bytes, out.stdout);
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(report["failing_regions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["region"] == "tile-0-0"));
    assert!(!f.run(&args).status.success());
    assert_eq!(fs::read(f.root.join("report.json")).unwrap(), bytes);
    // A remainder below the 11-pixel SSIM window extends the preceding tile.
    // An oversized tile likewise covers the entire 32x32 raster.
    for size in ["24", "256"] {
        let out = f.run(&[
            "controls",
            "collection/light",
            "--comparison",
            "pdf-pdfium",
            "--tiles",
            size,
        ]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        let rows = report["failing_regions"].as_array().unwrap();
        let tiles: Vec<_> = rows
            .iter()
            .filter(|r| r["region"].as_str().unwrap().starts_with("tile-"))
            .collect();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0]["region"], "tile-0-0");
        let whole = rows.iter().find(|r| r["region"] == "whole").unwrap();
        for field in [
            "changed_pixels",
            "ssim",
            "control_changed_pixels",
            "control_ssim",
        ] {
            assert_eq!(tiles[0][field], whole[field], "tile size {size}: {field}");
        }
    }
    for size in ["0", "10"] {
        assert!(!f
            .run(&["controls", "collection/light", "--tiles", size])
            .status
            .success());
    }
}
