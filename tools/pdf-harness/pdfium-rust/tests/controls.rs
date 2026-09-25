//! Independent PDF objects/xrefs and pixel expectations, ported from the Python
//! alpha controls and prototype's fractional adapter controls. No producer code.
use excalidraw_render_pdfium::canonicalize_rgba;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let parent =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/pdf-harness/pdfium-rust");
        fs::create_dir_all(&parent).unwrap();
        loop {
            let path = parent.join(format!(
                "controls-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create scratch directory: {error}"),
            }
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn source(&self, bytes: &[u8]) -> PathBuf {
        let path = self.path("control.pdf");
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn worker(source: &Path, output: &Path, width: &str, height: &str) -> Output {
    // PDFium is never initialized in the (possibly parallel) test process.
    Command::new(env!("CARGO_BIN_EXE_excalidraw-render-pdfium"))
        .arg(source)
        .arg(output)
        .args([width, height])
        .output()
        .unwrap()
}

fn pdf(objects: &[Vec<u8>], version: &str) -> Vec<u8> {
    let mut data = format!("%PDF-{version}\n").into_bytes();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(data.len());
        data.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        data.extend(object);
        data.extend(b"\nendobj\n");
    }
    let start = data.len();
    data.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        data.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    data.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    data
}

fn stream(data: &[u8], dictionary: &str) -> Vec<u8> {
    let mut output = format!("<< {dictionary} /Length {} >>\nstream\n", data.len()).into_bytes();
    output.extend(data);
    output.extend(b"\nendstream");
    output
}

fn alpha_control(soft_mask: bool) -> Vec<u8> {
    // Four 12x12 device-pixel cells, alpha 0/1/128/255; exact RGB 64/128/192.
    let (resources, content) = if soft_mask {
        ("/XObject << /I 5 0 R >>", "q 27 0 0 9 9 0 cm /I Do Q")
    } else {
        (
            "/ExtGState << /A << /ca 0.003921569 >> /B << /ca 0.501960785 >> >>",
            "0.250980392 0.501960784 0.752941176 rg q /A gs 9 0 9 9 re f Q q /B gs 18 0 9 9 re f Q 27 0 9 9 re f",
        )
    };
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 36 9] /Resources << {resources} >> /Contents 4 0 R >>").into_bytes(),
        stream(content.as_bytes(), ""),
    ];
    if soft_mask {
        objects.push(stream(
            &[64, 128, 192].repeat(3),
            "/Type /XObject /Subtype /Image /Width 3 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate false /SMask 6 0 R",
        ));
        objects.push(stream(
            &[1, 128, 255],
            "/Type /XObject /Subtype /Image /Width 3 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Interpolate false",
        ));
    }
    pdf(&objects, "1.7")
}

fn rectangle_control(
    width: f64,
    height: f64,
    rotation: u16,
    version: &str,
    pages: usize,
) -> Vec<u8> {
    let content = format!("0 0 0 rg 15 {:.2} 30 30 re f", height - 45.0);
    pdf(
        &[
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", "3 0 R ".repeat(pages)).into_bytes(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Rotate {rotation} /Resources << >> /Contents 4 0 R >>").into_bytes(),
            stream(content.as_bytes(), ""),
        ],
        version,
    )
}

fn assert_render_repeats(pdf: &[u8], width: usize, height: usize, expected: &[u8]) {
    let scratch = Scratch::new();
    let source = scratch.source(pdf);
    let mut previous = None;
    for repeat in 0..2 {
        let output = scratch.path(&format!("{repeat}.rgba"));
        let result = worker(&source, &output, &width.to_string(), &height.to_string());
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty(), "{:?}", result.stderr);
        let pixels = fs::read(output).unwrap();
        assert_eq!(pixels, expected);
        assert_eq!(pixels.len(), width * height * 4);
        let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
        // Independent literals intentionally bind the native identity and entire
        // receipt schema, including absence of Python-wrapper identity.
        assert_eq!(
            receipt,
            json!({
                "adapter": "pdfium-render/0.9.4",
                "pdfium": "152.0.7947.0",
                "library_sha256": "61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33",
                "byte_length": width * height * 4,
                "width": width,
                "height": height,
                "layout": "top-left-row-major",
                "pixel_format": "straight-alpha-srgb-rgba8",
                "sha256": format!("{:x}", Sha256::digest(&pixels)),
            })
        );
        assert_eq!(result.stdout.iter().filter(|&&b| b == b'\n').count(), 1);
        let current = (pixels, result.stdout);
        if let Some(previous) = &previous {
            assert_eq!(&current, previous, "fresh-process output/receipt drift");
        }
        previous = Some(current);
    }
}

#[test]
fn canonicalization_preserves_every_alpha_and_clears_hidden_color() {
    let mut pixels = (0..=255)
        .map(|alpha| [1, 127, 254, alpha])
        .collect::<Vec<_>>();
    let mut expected = pixels.clone();
    expected[0] = [0; 4];
    canonicalize_rgba(&mut pixels);
    assert_eq!(pixels, expected);
}

#[test]
fn vector_and_soft_mask_preserve_straight_samples_and_repeat_receipts() {
    let row = [0, 1, 128, 255]
        .into_iter()
        .flat_map(|alpha| {
            let pixel = if alpha == 0 {
                [0; 4]
            } else {
                [64, 128, 192, alpha]
            };
            pixel.repeat(12)
        })
        .collect::<Vec<_>>();
    for soft_mask in [false, true] {
        assert_render_repeats(&alpha_control(soft_mask), 48, 12, &row.repeat(12));
    }
}

#[test]
fn fractional_page_preserves_exact_scale_and_top_left_origin() {
    let pdf = rectangle_control(84.09, 62.88, 0, "1.7", 1);
    for (width, height) in [(112, 84), (113, 83)] {
        let expected = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    if (20..60).contains(&x) && (20..60).contains(&y) {
                        [0, 0, 0, 255]
                    } else {
                        [0; 4]
                    }
                })
            })
            .collect::<Vec<_>>();
        assert_render_repeats(&pdf, width, height, &expected);
    }
}

fn assert_rejected(result: &Output) {
    assert_eq!(
        result.status.code(),
        Some(1),
        "worker crashed or succeeded: {result:?}"
    );
    assert!(result.stdout.is_empty(), "rejection emitted receipt");
    assert!(String::from_utf8_lossy(&result.stderr).starts_with("error: "));
}

fn rejects_without_touching_output(pdf: &[u8], width: &str, height: &str, diagnostic: &str) {
    let scratch = Scratch::new();
    let source = scratch.source(pdf);
    let output = scratch.path("output.rgba");
    for existing in [false, true] {
        if existing {
            fs::write(&output, b"untouched output").unwrap();
        }
        let result = worker(&source, &output, width, height);
        assert_rejected(&result);
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(diagnostic),
            "{result:?}"
        );
        if existing {
            assert_eq!(fs::read(&output).unwrap(), b"untouched output");
        } else {
            assert!(
                !output.try_exists().unwrap(),
                "rejected render created output"
            );
        }
    }
}

#[test]
fn rejects_pdf_version_page_count_rotation_and_malformed_input() {
    for (version, pages, rotation, diagnostic) in [
        ("1.6", 1, 0, "PDF 1.7 only"),
        ("1.7", 2, 0, "exactly one page"),
        ("1.7", 1, 90, "rejects page rotation"),
        ("1.7", 1, 180, "rejects page rotation"),
        ("1.7", 1, 270, "rejects page rotation"),
    ] {
        rejects_without_touching_output(
            &rectangle_control(84.09, 62.88, rotation, version, pages),
            "112",
            "84",
            diagnostic,
        );
    }
    for malformed in [b"".as_slice(), b"not a PDF", b"%PDF-1.7\ntruncated"] {
        rejects_without_touching_output(malformed, "112", "84", "error:");
    }
}

#[test]
fn rejects_invalid_dimensions_and_exact_one_pixel_boundary() {
    let fractional = rectangle_control(84.09, 62.88, 0, "1.7", 1);
    for (width, height) in [("111", "84"), ("114", "84"), ("112", "82"), ("112", "85")] {
        rejects_without_touching_output(&fractional, width, height, "px at 96 DPI");
    }
    for (width, height) in [
        ("0", "84"),
        ("-1", "84"),
        ("112", "0"),
        ("112", "-1"),
        ("1.5", "84"),
        ("112", "NaN"),
        ("", "84"),
        ("2147483648", "84"),
        ("2147483647", "84"),
        ("112", "2147483648"),
    ] {
        rejects_without_touching_output(&fractional, width, height, "error:");
    }
    let integer = rectangle_control(150.0, 90.0, 0, "1.7", 1);
    for (width, height) in [
        ("199", "120"),
        ("201", "120"),
        ("200", "119"),
        ("200", "121"),
    ] {
        rejects_without_touching_output(&integer, width, height, "px at 96 DPI");
    }
}

#[test]
#[cfg(unix)]
fn rejects_live_and_dangling_output_symlinks() {
    let scratch = Scratch::new();
    let source = scratch.source(&alpha_control(false));
    let output = scratch.path("output.rgba");
    let target = scratch.path("target.rgba");
    std::os::unix::fs::symlink(&target, &output).unwrap();
    for existing in [false, true] {
        if existing {
            fs::write(&target, b"untouched symlink target").unwrap();
        }
        let result = worker(&source, &output, "48", "12");
        assert_rejected(&result);
        assert!(String::from_utf8_lossy(&result.stderr).contains("symlink"));
        assert_eq!(fs::read_link(&output).unwrap(), target);
        if existing {
            assert_eq!(fs::read(&target).unwrap(), b"untouched symlink target");
        } else {
            assert!(!target.try_exists().unwrap());
        }
    }
}

#[test]
fn missing_input_and_wrong_argument_count_fail_cleanly() {
    let scratch = Scratch::new();
    let output = scratch.path("output.rgba");
    fs::write(&output, b"untouched output").unwrap();
    assert_rejected(&worker(&scratch.path("missing.pdf"), &output, "48", "12"));
    assert_eq!(fs::read(output).unwrap(), b"untouched output");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render-pdfium"))
        .output()
        .unwrap();
    assert_rejected(&result);
    assert!(String::from_utf8_lossy(&result.stderr).contains("usage:"));
}
