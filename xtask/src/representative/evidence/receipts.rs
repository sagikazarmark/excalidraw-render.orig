use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PdfiumAdapter {
    Python,
    Rust,
}

impl PdfiumAdapter {
    pub(super) fn from_receipt(receipt: &Value) -> Result<Self> {
        require(receipt["pdfium"] == "152.0.7947.0", "PDFium identity")?;
        match (
            receipt.get("pypdfium2"),
            receipt.get("adapter"),
            receipt.get("library_sha256"),
        ) {
            (Some(version), None, None) if version == "5.12.1" => Ok(Self::Python),
            (None, Some(adapter), Some(library))
                if adapter == "pdfium-render/0.9.4"
                    && library
                        == "61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33" =>
            {
                Ok(Self::Rust)
            }
            _ => Err("PDFium adapter identity".into()),
        }
    }
}

pub(super) fn dimensions(value: &Value) -> Result<(u32, u32)> {
    let n = |key| {
        value[key]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or("invalid viewer dimensions")
    };
    Ok((n("width")?, n("height")?))
}

fn length(width: u32, height: u32) -> Result<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("raster size overflow".into())
}

fn clip(raw: &[u8], w: u32, h: u32, width: u32, height: u32, rust: bool) -> Result<Vec<u8>> {
    require(raw.len() == length(w, h)?, "invalid receipt dimensions")?;
    if rust {
        require(
            w >= width && w - width <= 1 && h >= height && h - height <= 1,
            "invalid Rust grid",
        )?;
    } else {
        require((w, h) == (width, height), "viewer grid differs")?;
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length(width, height)?)
        .map_err(|e| e.to_string())?;
    for y in 0..height as usize {
        pixels.extend_from_slice(&raw[y * w as usize * 4..y * w as usize * 4 + width as usize * 4]);
    }
    Ok(pixels)
}

fn validate_receipt(receipt: &Value, raw: &[u8], viewer: &str) -> Result<()> {
    require(VIEWERS.contains(&viewer), "unknown viewer receipt identity")?;
    let expected: &[&str] = if viewer.ends_with("-chromium") {
        &[
            "width",
            "height",
            "browser_version",
            "browser_process_id",
            "container_namespace",
            "lifecycle_updates",
            "animation_offset_microseconds",
            "platform_fonts",
            "requests",
            "rgba_sha256",
            "virtual_time_ticks_base",
        ]
    } else if viewer.ends_with("-rust") {
        // The standalone Rust adapter emits dimensions only, not a version or
        // digest. Its historical identity lives in the collection's sources.
        &["width", "height"]
    } else if viewer == "native-pdfium" {
        &[
            "width",
            "height",
            "byte_length",
            "sha256",
            "layout",
            "pixel_format",
            "pdfium",
        ]
    } else {
        &[
            "width",
            "height",
            "byte_length",
            "sha256",
            "layout",
            "pixel_format",
            "pdfjs",
        ]
    };
    let mut expected = expected.to_vec();
    if viewer == "native-pdfium" {
        expected.extend(match PdfiumAdapter::from_receipt(receipt)? {
            PdfiumAdapter::Python => &["pypdfium2"][..],
            PdfiumAdapter::Rust => &["adapter", "library_sha256"][..],
        });
    }
    // Collection adds this field after validating adapter stdout; verification
    // validates the retained envelope and separately binds the indexed filename.
    if let Some(capture) = receipt.get("raw_capture") {
        expected.push("raw_capture");
        fields(capture, &["file", "bytes", "sha256"], "invalid raw receipt")?;
        require(
            !text(capture, "file")?.is_empty()
                && capture["bytes"].as_u64() == Some(raw.len() as u64)
                && capture["sha256"] == sha256(raw),
            "raw receipt detached",
        )?;
    }
    fields(receipt, &expected, "invalid viewer receipt fields")?;
    let (width, height) = dimensions(receipt)?;
    require(
        raw.len() == length(width, height)?,
        "invalid receipt dimensions",
    )?;
    if viewer.ends_with("-chromium") {
        require(
            receipt["browser_version"] == "149.0.7827.55"
                && receipt["lifecycle_updates"].as_u64() == Some(2)
                && receipt["animation_offset_microseconds"].as_u64() == Some(0),
            "browser receipt identity",
        )?;
        let fonts = array(receipt, "platform_fonts")?;
        require(!fonts.is_empty(), "font audit failed")?;
        for font in fonts {
            fields(
                font,
                &["familyName", "postScriptName", "glyphCount", "isCustomFont"],
                "invalid font audit fields",
            )?;
            require(
                font["isCustomFont"] == true
                    && !text(font, "familyName")?.trim().is_empty()
                    && !text(font, "postScriptName")?.trim().is_empty()
                    && font["glyphCount"].as_u64().is_some(),
                "font audit failed",
            )?;
        }
        require(
            receipt["rgba_sha256"] == sha256(raw),
            "browser receipt digest",
        )?;
        require(
            !text(receipt, "container_namespace")?.trim().is_empty()
                && receipt["browser_process_id"]
                    .as_u64()
                    .is_some_and(|id| id > 0)
                && receipt["virtual_time_ticks_base"]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n >= 0.0),
            "missing browser process identity",
        )?;
        require(
            receipt["requests"] == json!(["http://reference.invalid/input.svg"]),
            "browser request inventory differs",
        )?;
    } else if !viewer.ends_with("-rust") {
        require(
            receipt["sha256"] == sha256(raw)
                && receipt["byte_length"].as_u64() == Some(raw.len() as u64),
            "PDF receipt digest",
        )?;
        require(
            receipt["pixel_format"] == "straight-alpha-srgb-rgba8"
                && receipt["layout"] == "top-left-row-major",
            "PDF pixel profile",
        )?;
        if viewer == "native-pdfjs" {
            // The release is provenance (ADR-0019); fresh rasters are judged
            // against the expectations, not against a pinned PDF.js version.
            require(
                receipt["pdfjs"].as_str().is_some_and(|v| !v.is_empty()),
                "PDF.js identity",
            )?;
        }
    }
    Ok(())
}

pub(super) fn collect(
    directory: &Path,
    variant: &str,
    viewer: &str,
    width: u32,
    height: u32,
) -> Result<Value> {
    let mut runs = Vec::new();
    let mut first_raw = None;
    let mut pixels = Vec::new();
    for index in 0..2 {
        let output = directory.join(format!("{viewer}-{index}.rgba"));
        // Output has no bytes yet, but its parent is known to be in the repo.
        let output_arg = format!(
            "{}/{}",
            relative(directory)?,
            output
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("output filename")?
        );
        let (program, args) = if viewer.ends_with("-chromium") {
            let kind = viewer.strip_suffix("-chromium").ok_or("viewer role")?;
            let input = viewer_assets::chromium_input(directory, kind);
            let mut args = docker_args()?;
            args.extend([
                "node".into(),
                "../representative-intake/svg-viewer.mjs".into(),
                "render".into(),
                relative(&input)?,
                output_arg,
                relative(&directory.join(format!("{kind}-assets.json")))?,
            ]);
            ("docker".to_owned(), args)
        } else if viewer.ends_with("-rust") {
            let kind = viewer.strip_suffix("-rust").ok_or("viewer role")?;
            (
                crate::conformance::repo_root()
                    .join("target/representative-svg-viewer/release/excalidraw-render-svg-viewer")
                    .to_string_lossy()
                    .into_owned(),
                vec![
                    "render".into(),
                    relative(&directory.join(format!("{kind}-rust.svg")))?,
                    output_arg,
                    relative(&directory.join(format!("{kind}-assets.json")))?,
                ],
            )
        } else if viewer == "native-pdfjs" {
            let mut args = docker_args()?;
            // The PDF.js adapter accepts workspace-absolute file paths.
            args.extend([
                "node".into(),
                "../pdf-harness/pdfjs/render.mjs".into(),
                format!("/workspace/{}", relative(&directory.join("native.pdf"))?),
                format!("/workspace/{output_arg}"),
                width.to_string(),
                height.to_string(),
            ]);
            ("docker".to_owned(), args)
        } else {
            let kind = viewer.strip_prefix("native-").ok_or("viewer role")?;
            (
                "bash".into(),
                vec![
                    format!("tools/pdf-harness/run-{kind}.sh"),
                    "render".into(),
                    relative(&directory.join("native.pdf"))?,
                    output_arg,
                    width.to_string(),
                    height.to_string(),
                ],
            )
        };
        let mut receipt = decode(&command(&program, &args, 600)?)?;
        let raw = read(
            directory,
            output
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("output filename")?,
        )?;
        let (w, h) = dimensions(&receipt)?;
        validate_receipt(&receipt, &raw, viewer)?;
        let canonical = clip(&raw, w, h, width, height, viewer.ends_with("-rust"))?;
        if let Some(first) = &first_raw {
            require(
                *first == raw && pixels == canonical,
                "viewer raster does not repeat",
            )?;
        } else {
            first_raw = Some(raw.clone());
            pixels = canonical;
        }
        let name = format!("{viewer}-{index}-raw.rgba.zlib");
        write_new(
            &directory.join(&name),
            &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9),
        )?;
        receipt["raw_capture"] = json!({"file":name,"bytes":raw.len(),"sha256":sha256(&raw)});
        runs.push(receipt);
        fs::remove_file(output).map_err(|e| e.to_string())?;
    }
    if viewer.ends_with("-chromium") {
        require(
            runs[0]["container_namespace"] != runs[1]["container_namespace"],
            "browser process isolation missing",
        )?;
    }
    let raster = directory.join(format!("{viewer}.rgba.zlib"));
    write_new(
        &raster,
        &miniz_oxide::deflate::compress_to_vec_zlib(&pixels, 9),
    )?;
    Ok(
        json!({"variant":variant,"viewer":viewer,"raster":bind(&raster)?,"canonical_pixels":descriptor(&pixels),"runs":runs}),
    )
}

pub(super) fn verify(
    directory: &Path,
    capture: &Value,
    viewer: &str,
    width: u32,
    height: u32,
) -> Result<()> {
    let raster = directory.join(format!("{viewer}.rgba.zlib"));
    require(
        capture["raster"] == bind(&raster)?,
        "invalid raster capture binding",
    )?;
    let inflate = |bytes: &[u8], limit| {
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(bytes, limit)
            .map_err(|e| format!("raster inflate: {e:?}"))
    };
    let pixels = inflate(
        &read(directory, &format!("{viewer}.rgba.zlib"))?,
        length(width, height)?,
    )?;
    require(
        pixels.len() == length(width, height)?
            && capture["canonical_pixels"] == descriptor(&pixels),
        "decoded raster digest changed",
    )?;
    let runs = array(capture, "runs")?;
    require(runs.len() == 2, "invalid raster capture repeats")?;
    let mut first = None;
    for (index, receipt) in runs.iter().enumerate() {
        let (w, h) = dimensions(receipt)?;
        require(
            w >= width && w - width <= 1 && h >= height && h - height <= 1,
            "invalid viewer grid",
        )?;
        let name = format!("{viewer}-{index}-raw.rgba.zlib");
        let raw = inflate(&read(directory, &name)?, length(w, h)?)?;
        require(
            receipt["raw_capture"] == json!({"file":name,"bytes":raw.len(),"sha256":sha256(&raw)}),
            "raw receipt detached",
        )?;
        validate_receipt(receipt, &raw, viewer)?;
        require(
            clip(&raw, w, h, width, height, viewer.ends_with("-rust"))? == pixels,
            "Rust clip or viewer pixels differ",
        )?;
        if let Some(first) = &first {
            require(*first == raw, "raw repeats differ")?;
        } else {
            first = Some(raw);
        }
    }
    if viewer.ends_with("-chromium") {
        require(
            runs[0]["container_namespace"] != runs[1]["container_namespace"],
            "browser process isolation missing",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retained(viewer: &str) -> (Value, Vec<u8>) {
        let root = crate::conformance::repo_root()
            .join("tests/conformance/proposals/velox84-evidence-v1/viewers");
        let record = decode(&read(&root, "viewers.json").unwrap()).unwrap();
        let capture = array(&record, "captures")
            .unwrap()
            .iter()
            .find(|c| c["variant"] == "light" && c["viewer"] == viewer)
            .unwrap();
        let receipt = capture["runs"][0].clone();
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(
            &read(&root.join("light"), &format!("{viewer}-0-raw.rgba.zlib")).unwrap(),
        )
        .unwrap();
        (receipt, raw)
    }

    #[test]
    fn retained_adapter_receipts_and_stdout_shapes_are_valid() {
        for viewer in VIEWERS {
            let (mut receipt, raw) = retained(viewer);
            validate_receipt(&receipt, &raw, viewer).unwrap();
            receipt.as_object_mut().unwrap().remove("raw_capture");
            validate_receipt(&receipt, &raw, viewer).unwrap();
        }
    }

    fn rust_pdfium() -> (Value, Vec<u8>) {
        let raw = vec![12, 34, 56, 255];
        let receipt = json!({
            "width": 1, "height": 1, "byte_length": raw.len(), "sha256": sha256(&raw),
            "layout": "top-left-row-major", "pixel_format": "straight-alpha-srgb-rgba8",
            "pdfium": "152.0.7947.0", "adapter": "pdfium-render/0.9.4",
            "library_sha256": "61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33"
        });
        (receipt, raw)
    }

    #[test]
    fn rust_pdfium_stdout_and_retained_envelope_are_valid() {
        let (mut receipt, raw) = rust_pdfium();
        validate_receipt(&receipt, &raw, "native-pdfium").unwrap();
        receipt["raw_capture"] =
            json!({"file":"native-pdfium-0-raw.rgba.zlib","bytes":raw.len(),"sha256":sha256(&raw)});
        validate_receipt(&receipt, &raw, "native-pdfium").unwrap();
        receipt["raw_capture"]["sha256"] = json!("0".repeat(64));
        assert!(validate_receipt(&receipt, &raw, "native-pdfium").is_err());
    }

    #[test]
    fn pdfium_receipts_reject_missing_fields_corruption_and_conflicting_identities() {
        let viewer = "native-pdfium";
        for (receipt, raw) in [retained(viewer), rust_pdfium()] {
            for key in receipt
                .as_object()
                .unwrap()
                .keys()
                .filter(|key| key.as_str() != "raw_capture")
            {
                let mut changed = receipt.clone();
                changed.as_object_mut().unwrap().remove(key);
                assert!(
                    validate_receipt(&changed, &raw, viewer).is_err(),
                    "missing {key}"
                );
            }
            for (key, value) in [
                ("pdfium", json!("152.0.7947.1")),
                ("pypdfium2", json!("5.12.2")),
                ("adapter", json!("pdfium-render/0.9.5")),
                ("library_sha256", json!("0".repeat(64))),
                ("byte_length", json!(raw.len() + 4)),
                ("sha256", json!("0".repeat(64))),
                ("layout", json!("bottom-up")),
                ("pixel_format", json!("premultiplied-rgba8")),
                ("width", json!(0)),
                ("height", json!("1")),
                ("unknown", json!(true)),
            ] {
                let mut changed = receipt.clone();
                changed[key] = value;
                assert!(
                    validate_receipt(&changed, &raw, viewer).is_err(),
                    "accepted {key}"
                );
            }
            // Even correct values must not combine the two identity schemas;
            // null identity fields are also present fields, not an absent key.
            for value in [json!("5.12.1"), Value::Null] {
                let mut changed = receipt.clone();
                changed["adapter"] = json!("pdfium-render/0.9.4");
                changed["library_sha256"] = rust_pdfium().0["library_sha256"].clone();
                changed["pypdfium2"] = value;
                assert!(validate_receipt(&changed, &raw, viewer).is_err());
            }
            let mut changed_raw = raw.clone();
            changed_raw[0] ^= 1;
            assert!(validate_receipt(&receipt, &changed_raw, viewer).is_err());
        }
    }

    #[test]
    fn browser_receipts_reject_missing_fields_and_invalid_observations() {
        let viewer = "official-chromium";
        let (receipt, raw) = retained(viewer);
        for key in receipt
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| k.as_str() != "raw_capture")
        {
            let mut changed = receipt.clone();
            changed.as_object_mut().unwrap().remove(key);
            assert!(
                validate_receipt(&changed, &raw, viewer).is_err(),
                "missing {key}"
            );
        }
        for (key, value) in [
            ("browser_process_id", json!(0)),
            ("browser_process_id", json!("20")),
            ("browser_process_id", json!(1.5)),
            ("container_namespace", json!(" ")),
            ("virtual_time_ticks_base", json!("123")),
            ("virtual_time_ticks_base", json!(-1)),
            (
                "virtual_time_ticks_base",
                serde_json::from_str("1e400").unwrap(),
            ),
            ("requests", json!([])),
            ("requests", json!(["https://external.invalid/"])),
            ("requests", json!([12])),
            ("browser_version", json!("other")),
            ("lifecycle_updates", json!("2")),
            ("animation_offset_microseconds", json!(1)),
            ("platform_fonts", json!([{"isCustomFont":true}])),
            ("platform_fonts", json!([])),
            ("rgba_sha256", json!("0".repeat(64))),
            ("height", json!(0)),
            ("unknown", json!(true)),
        ] {
            let mut changed = receipt.clone();
            changed[key] = value;
            assert!(
                validate_receipt(&changed, &raw, viewer).is_err(),
                "accepted {key}"
            );
        }
        for key in ["familyName", "postScriptName", "glyphCount", "isCustomFont"] {
            let mut changed = receipt.clone();
            changed["platform_fonts"][0]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                validate_receipt(&changed, &raw, viewer).is_err(),
                "missing font {key}"
            );
        }
        for (key, value) in [
            ("familyName", json!("")),
            ("postScriptName", json!(false)),
            ("glyphCount", json!(-1)),
            ("glyphCount", json!(1.5)),
            ("glyphCount", json!("1")),
            ("isCustomFont", json!(false)),
        ] {
            let mut changed = receipt.clone();
            changed["platform_fonts"][0][key] = value;
            assert!(
                validate_receipt(&changed, &raw, viewer).is_err(),
                "invalid font {key}"
            );
        }
    }

    #[test]
    fn rust_receipts_require_actual_adapter_shape_and_matching_raw_grid() {
        let viewer = "official-rust";
        let (receipt, raw) = retained(viewer);
        for key in ["width", "height"] {
            let mut changed = receipt.clone();
            changed.as_object_mut().unwrap().remove(key);
            assert!(validate_receipt(&changed, &raw, viewer).is_err());
            for value in [json!(0), json!("1"), json!(1.5)] {
                changed[key] = value;
                assert!(validate_receipt(&changed, &raw, viewer).is_err());
            }
        }
        let mut changed = receipt.clone();
        changed["browser_version"] = json!("149.0.7827.55");
        assert!(validate_receipt(&changed, &raw, viewer).is_err());
        assert!(validate_receipt(&receipt, &raw[..raw.len() - 4], viewer).is_err());
        assert!(validate_receipt(&receipt, &raw, "unknown-rust").is_err());
        assert!(validate_receipt(&receipt, &raw, "official-chromium").is_err());
        let raw = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        assert_eq!(clip(&raw, 2, 2, 1, 1, true).unwrap(), raw[..4]);
        assert!(clip(&raw, 2, 2, 1, 1, false).is_err());
    }
}
