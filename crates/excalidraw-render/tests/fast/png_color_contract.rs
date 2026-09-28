//! PNG color contracts: canonical visual budgets plus independent exact
//! semantic anchors, complete reports, source preservation and recovery.
use crate::common::lower_hex;
use crate::common::png_color_reference as reference;
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::CanonicalRgba8;
use serde_json::json;
use sha2::{Digest, Sha256};

fn check_group(group: &str, count: usize) {
    exact_transfer_and_metadata_anchors();
    let evidence = reference::evidence();
    // These reference pairs must fail the same visual gate used below. They
    // prove the white-point and priority fixtures distinguish the wrong path.
    let reference_image = |signal: &str| {
        let id = format!("{signal}-c6-d8");
        let row = evidence["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        reference::png(&reference::base64(&row["reference"]["png"]))
    };
    let assert_discriminates = |right: &str, wrong: &str| {
        let result = excalidraw_render_compare::compare(
            &reference_image(right),
            &reference_image(wrong),
            excalidraw_render_compare::VisualClass::GeometryImage,
            &[],
            excalidraw_render_compare::ComparisonMode::Canonical,
        )
        .unwrap();
        assert!(!result.passed(), "{right} does not discriminate {wrong}");
    };
    for (right, wrong) in [
        ("chrm-srgb-d50-100000", "chrm-srgb-100000"),
        ("srgb-over-chrm", "chrm-p3-100000"),
        ("icc-over-chrm", "chrm-p3-100000"),
        ("cicp-over-chrm", "chrm-p3-100000"),
    ] {
        assert_discriminates(right, wrong);
    }
    if group == "cicp" {
        for (primary, wrong_primary) in [(4, 1), (5, 1), (6, 1), (8, 1), (10, 1), (22, 1)] {
            let right = format!("cicp-p{primary}-t8-r1");
            let wrong = format!("cicp-p{wrong_primary}-t8-r1");
            assert_discriminates(&right, &wrong);
        }
        // H.273 defines primary codes 6 and 7 as equivalent, unlike the
        // distinct white points of cinema-P3 (11) and display-P3 (12).
        for transfer in [1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18] {
            assert_eq!(
                reference_image(&format!("cicp-p6-t{transfer}-r1")),
                reference_image(&format!("cicp-p7-t{transfer}-r1"))
            );
        }
    }
    assert_eq!(evidence["sources"].as_array().unwrap().len(), 6);
    check_rows(&evidence, group, count);
}

fn check_rows(evidence: &serde_json::Value, group: &str, count: usize) {
    for source in evidence["sources"].as_array().unwrap() {
        let bytes =
            std::fs::read(crate::common::repository_root().join(source["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(source["sha256"], lower_hex(&Sha256::digest(bytes)));
    }
    let rows: Vec<_> = evidence["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["group"] == group)
        .collect();
    assert_eq!(rows.len(), count);
    for row in rows {
        let scene = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap();
            let first = Renderer::new().render(&scene, &request).unwrap();
            let second = Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(
                first.artifact().bytes(),
                second.artifact().bytes(),
                "{} {format:?}",
                row["id"]
            );
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
            reference::report(
                row,
                format.as_str(),
                serde_json::to_value(first.report()).unwrap(),
            );
            if format != OutputFormat::Svg {
                if let Some(equivalent) = reference::bt1886_equivalent(evidence, row)
                    .or_else(|| reference::ignored_cicp_equivalent(evidence, row))
                    .or_else(|| reference::gain_map_equivalent(evidence, row))
                    .or_else(|| reference::hdr_metadata_equivalent(evidence, row))
                {
                    let scene =
                        SceneDocument::parse(&serde_json::to_vec(&equivalent["scene"]).unwrap())
                            .unwrap();
                    let control = Renderer::new().render(&scene, &request).unwrap();
                    assert_eq!(
                        first.artifact().bytes(),
                        control.artifact().bytes(),
                        "{} {format:?} exact metadata equivalence",
                        row["id"]
                    );
                    assert_eq!(first.report(), control.report());
                }
            }
            if reference::recovery(row, format.as_str()).is_some() {
                let mut removed = row["scene"].clone();
                removed["files"] = json!({});
                let removed = SceneDocument::parse(&serde_json::to_vec(&removed).unwrap()).unwrap();
                let control = Renderer::new().render(&removed, &request).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    control.artifact().bytes(),
                    "{} recovery {format:?}",
                    row["id"]
                );
                continue;
            }
            match format {
                OutputFormat::Svg => {
                    let xml = roxmltree::Document::parse(
                        std::str::from_utf8(first.artifact().bytes()).unwrap(),
                    )
                    .unwrap();
                    let image = xml.descendants().find(|n| n.has_tag_name("image")).unwrap();
                    assert_eq!(
                        image.attribute("href").unwrap(),
                        row["scene"]["files"]["asset"]["dataURL"].as_str().unwrap()
                    );
                }
                OutputFormat::Png => {
                    super::png_profile_contract::assert_untagged_png(first.artifact().bytes());
                    reference::appearance(row, reference::png(first.artifact().bytes()), false);
                }
                OutputFormat::Pdf => {
                    let (w, h) = (
                        row["width"].as_u64().unwrap() as u32,
                        row["height"].as_u64().unwrap() as u32,
                    );
                    let rgba = super::png_profile_contract::pdf_rgba(
                        first.artifact().bytes(),
                        w as usize,
                        h as usize,
                    );
                    assert!(rgba
                        .chunks_exact(4)
                        .all(|p| p[3] != 0 || p[..3] == [0, 0, 0]));
                    reference::appearance(row, CanonicalRgba8::new(w, h, rgba).unwrap(), true);
                }
                _ => unreachable!(),
            }
        }
    }
}
#[test]
fn srgb_intents_and_recovery_match_the_contract() {
    check_group("srgb-intents", 40);
}
#[test]
fn linear_gamma_matches_the_reference() {
    check_group("gama-linear", 4);
}
#[test]
fn nonlinear_gamma_and_recovery_match_the_reference() {
    check_group("gama-nonlinear", 48);
}
#[test]
fn chromaticities_and_recovery_match_the_reference() {
    check_group("chrm", 60);
}
#[test]
fn rgb_icc_v2_matrix_profiles_match_the_reference() {
    check_group("icc-v2", 12);
}
#[test]
fn rgb_icc_v4_parametric_profiles_match_the_reference() {
    check_group("icc-v4", 32);
}
#[test]
fn metadata_precedence_and_recovery_match_the_contract() {
    check_group("precedence", 84);
}
#[test]
fn sdr_cicp_candidates_match_the_reference() {
    primary_space_exact_anchors();
    bt1886_transfer_exact_anchors();
    remaining_transfer_exact_anchors();
    ignored_transfer_metadata_anchors();
    hdr_transfer_exact_anchors();
    check_group("cicp", 704);
}

#[test]
fn hdr_transfer_exact_anchors() {
    // Independently calculated BT.2100 -> 203-nit reference white -> SDR
    // RWTMO (gain 1/2 below its knee) -> sRGB. Full white exercises the cap.
    for (transfer, anchors) in [
        (
            16,
            [(0, 0), (8, 0), (32, 5), (64, 30), (128, 132), (255, 255)],
        ),
        (
            18,
            [(0, 0), (8, 1), (32, 14), (64, 43), (128, 99), (255, 255)],
        ),
    ] {
        for (input, output) in anchors {
            assert_eq!(
                pixel(&[(*b"cICP", vec![1, transfer, 0, 1])], input),
                [output, output, output, 255]
            );
        }
        assert_eq!(
            pixel(&[(*b"cICP", vec![9, transfer, 0, 0])], 128),
            [128, 128, 128, 255]
        );
        assert_eq!(
            pixel(
                &[
                    (*b"cICP", vec![9, transfer, 0, 0]),
                    (*b"gAMA", 100000_u32.to_be_bytes().to_vec())
                ],
                128
            ),
            [188, 188, 188, 255]
        );
    }
    // Negative luminance under D50-adapted XYZ needs Skia's signed OOTF.
    assert_eq!(
        pixel_rgb(&[(*b"cICP", vec![10, 18, 0, 1])], [255, 16, 16]),
        [0, 255, 0, 255]
    );
}

#[test]
fn hdr_adam7_preserves_reference_samples() {
    use crate::common::codec_fixtures::append_png_chunk as chunk;
    let evidence = reference::evidence();
    for (primary, transfer) in [(9, 16), (10, 18)] {
        for depth in [8, 16] {
            let id = format!("cicp-p{primary}-t{transfer}-r1-c6-d{depth}");
            let row = evidence["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap();
            let original = reference::base64(&row["png"]);
            let mut reader = png::Decoder::new(std::io::Cursor::new(&original))
                .read_info()
                .unwrap();
            let mut samples = vec![0; reader.output_buffer_size().expect("PNG buffer fits")];
            reader.next_frame(&mut samples).unwrap();
            let bpp = 4 * depth / 8;
            let mut scanlines = Vec::new();
            for (x0, y0, dx, dy) in [
                (0, 0, 8, 8),
                (4, 0, 8, 8),
                (0, 4, 4, 8),
                (2, 0, 4, 4),
                (0, 2, 2, 4),
                (1, 0, 2, 2),
                (0, 1, 1, 2),
            ] {
                for y in (y0..15).step_by(dy) {
                    scanlines.push(0);
                    for x in (x0..15).step_by(dx) {
                        let at = (y * 15 + x) * bpp;
                        scanlines.extend_from_slice(&samples[at..at + bpp]);
                    }
                }
            }
            let mut source = original[..8].to_vec();
            let mut header = original[16..29].to_vec();
            header[12] = 1;
            chunk(&mut source, *b"IHDR", &header);
            chunk(&mut source, *b"cICP", &[primary, transfer, 0, 1]);
            chunk(
                &mut source,
                *b"IDAT",
                &miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 6),
            );
            chunk(&mut source, *b"IEND", &[]);
            let mut scene = row["scene"].clone();
            scene["files"]["asset"]["dataURL"] = json!(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(source)
            ));
            let scene = SceneDocument::parse(&serde_json::to_vec(&scene).unwrap()).unwrap();
            for format in [OutputFormat::Png, OutputFormat::Pdf] {
                let result = Renderer::new()
                    .render(
                        &scene,
                        &RenderRequest::builder(format)
                            .padding(0.0)
                            .include_background(false)
                            .build()
                            .unwrap(),
                    )
                    .unwrap();
                reference::report(
                    row,
                    format.as_str(),
                    serde_json::to_value(result.report()).unwrap(),
                );
                let actual = if format == OutputFormat::Png {
                    reference::png(result.artifact().bytes())
                } else {
                    CanonicalRgba8::new(
                        15,
                        15,
                        super::png_profile_contract::pdf_rgba(result.artifact().bytes(), 15, 15),
                    )
                    .unwrap()
                };
                reference::appearance(row, actual, format == OutputFormat::Pdf);
            }
        }
    }
}

/// Every PNG HDR tone-mapping path of the pinned reference: PQ and HLG over
/// two primaries, 8/16-bit, with RWTMO driven by the content peak (a positive
/// cLLI MaxCLL, else a positive mDCV maximum, else 1000 nits; no tone map at or
/// below reference white), including malformed, duplicate, late and zero
/// metadata, ignored gain-map signaling and SDR controls. The pinned
/// SkPngRustCodec has no AGTM source, so no other tone map is reachable. Also
/// runs the gain-map base, admission and precedence evidence.
#[test]
fn hdr_peak_luminance_and_tone_mapping_paths_match_the_reference() {
    exact_transfer_and_metadata_anchors();
    hdr_transfer_exact_anchors();
    let evidence = reference::hdr_metadata_evidence();
    assert_eq!(evidence["sources"].as_array().unwrap().len(), 8);
    check_rows(&evidence, "hdr-metadata-controls", 8);
    check_rows(&evidence, "hdr-metadata", 162);
    gain_map_base_admission_and_precedence_match_the_reference();
}

#[test]
fn gain_map_base_admission_and_precedence_match_the_reference() {
    exact_transfer_and_metadata_anchors();
    hdr_transfer_exact_anchors();
    let evidence = reference::gain_map_evidence();
    assert_eq!(evidence["sources"].as_array().unwrap().len(), 8);
    check_rows(&evidence, "gain-map-controls", 24);
    check_rows(&evidence, "gain-map", 84);
    gain_map_payload_resource_boundaries();
}

#[test]
fn gain_map_payload_resource_boundaries() {
    use excalidraw_render::ResourceLimits;
    let evidence = reference::gain_map_evidence();
    for name in [
        "huge-inner-dimensions",
        "recursive-payload",
        "bad-inner-crc",
    ] {
        let id = format!("gain-{name}-c6-d8");
        let row = evidence["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let bytes = reference::base64(&row["png"]).len() as u64;
        let source = SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            for admitted in [true, false] {
                let configured = bytes - u64::from(!admitted);
                let limits = ResourceLimits::builder()
                    .max_asset_bytes(configured)
                    .max_nested_asset_depth(1)
                    .max_raster_dimension(15)
                    .max_decoded_pixels_per_asset(225)
                    .build()
                    .unwrap();
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .include_background(false)
                    .resource_limits(limits)
                    .build()
                    .unwrap();
                let first = Renderer::new().render(&source, &request).unwrap();
                let second = Renderer::new().render(&source, &request).unwrap();
                assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                assert_eq!(first.report(), second.report());
                let mut actual = serde_json::to_value(first.report()).unwrap();
                for diagnostic in actual["diagnostics"].as_array_mut().unwrap() {
                    assert!(!diagnostic["message"].as_str().unwrap().is_empty());
                    diagnostic.as_object_mut().unwrap().remove("message");
                }
                let diagnostics = if admitted {
                    json!([])
                } else {
                    json!([{
                        "code":"resource.asset-bytes-exceeded","severity":"warning","format":format.as_str(),"effects":["visual"],
                        "element_id":"image","json_pointer":"/files/asset/dataURL",
                        "details":{"kind":"resource_limit","field":"max_asset_bytes","configured":configured,"value_kind":"bytes","measurement":{"kind":"attempted","value":bytes},"recovery":if format==OutputFormat::Png {"placeholder"} else {"omitted"}}
                    }])
                };
                assert_eq!(
                    actual,
                    json!({"schema_version":1,"diagnostics":diagnostics}),
                    "{id} {format:?}"
                );
                let mut control = row["scene"].clone();
                if !admitted {
                    control["files"] = json!({});
                }
                let control = SceneDocument::parse(&serde_json::to_vec(&control).unwrap()).unwrap();
                let control = Renderer::new()
                    .render(
                        &control,
                        &RenderRequest::builder(format)
                            .padding(0.0)
                            .include_background(false)
                            .build()
                            .unwrap(),
                    )
                    .unwrap();
                assert_eq!(first.artifact().bytes(), control.artifact().bytes());
            }
        }
    }
}

#[test]
fn remaining_transfer_exact_anchors() {
    // Independent SMPTE240 and cinema EOTF -> sRGB calculations. Include
    // the SMPTE240 toe on both sides and cinema's scale/clipping, not just
    // an undifferentiated gamma or the already-qualified BT.1886 mapping.
    for (transfer, anchors) in [
        (7, [(8, 22), (23, 41), (24, 42), (128, 141), (255, 255)]),
        (17, [(8, 0), (23, 7), (24, 8), (128, 118), (255, 255)]),
    ] {
        for (input, output) in anchors {
            assert_eq!(
                pixel(&[(*b"cICP", vec![1, transfer, 0, 1])], input),
                [output, output, output, 255],
                "transfer {transfer}, input {input}"
            );
        }
        assert_eq!(
            pixel(&[(*b"cICP", vec![1, transfer, 0, 0])], 128),
            [128, 128, 128, 255]
        );
        assert_eq!(
            pixel(
                &[
                    (*b"cICP", vec![1, transfer, 0, 0]),
                    (*b"gAMA", 100000_u32.to_be_bytes().to_vec())
                ],
                128
            ),
            [188, 188, 188, 255]
        );
    }
}

#[test]
fn ignored_transfer_metadata_anchors() {
    // SkColorSpace::MakeCICP does not support these valid H.273 codes.
    // SkPngRustCodec falls through; this is not logarithmic conversion or
    // unsupported-format recovery. A non-sRGB primary must also be ignored.
    for transfer in [9, 10, 12] {
        for range in [0, 1] {
            let cicp = (*b"cICP", vec![9, transfer, 0, range]);
            assert_eq!(
                pixel(std::slice::from_ref(&cicp), 128),
                [128, 128, 128, 255]
            );
            assert_eq!(
                pixel(
                    &[cicp.clone(), (*b"gAMA", 100000_u32.to_be_bytes().to_vec())],
                    128
                ),
                [188, 188, 188, 255]
            );
            assert_eq!(
                pixel(
                    &[
                        cicp,
                        (*b"sRGB", vec![0]),
                        (*b"gAMA", 100000_u32.to_be_bytes().to_vec())
                    ],
                    128
                ),
                [128, 128, 128, 255]
            );
        }
    }
}

#[test]
fn bt1886_transfer_exact_anchors() {
    // Independently authored neutral anchors, corroborated by the collected
    // field in the shared helper. Range zero falls through to lower metadata.
    for transfer in [6, 11, 14, 15] {
        assert_eq!(
            pixel(&[(*b"cICP", vec![1, transfer, 0, 1])], 128),
            [121, 121, 121, 255]
        );
        assert_eq!(
            pixel(&[(*b"cICP", vec![1, transfer, 0, 0])], 128),
            [128, 128, 128, 255]
        );
        assert_eq!(
            pixel(
                &[
                    (*b"cICP", vec![1, transfer, 0, 0]),
                    (*b"gAMA", 100000_u32.to_be_bytes().to_vec())
                ],
                128
            ),
            [188, 188, 188, 255]
        );
        assert_eq!(
            pixel(
                &[
                    (*b"cICP", vec![1, transfer, 0, 1]),
                    (*b"gAMA", 100000_u32.to_be_bytes().to_vec())
                ],
                128
            ),
            [121, 121, 121, 255]
        );
    }
}

#[test]
fn primary_space_exact_anchors() {
    let evidence = reference::evidence();
    let image = |id: &str, opaque: bool| {
        let row = evidence["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        reference::png(&reference::base64(
            &row["reference"][if opaque { "opaque_png" } else { "png" }],
        ))
    };
    let observed = image("cicp-p10-t1-r1-c6-d8", false);
    // The authored field's (9,6) pixel has [128,128,128,255].
    assert_eq!(&observed.rgba()[99 * 4..100 * 4], &[121, 121, 121, 255]);
    assert_eq!(
        pixel(&[(*b"cICP", vec![10, 1, 0, 1])], 128),
        [121, 121, 121, 255]
    );
    // Cinema-P3 and display-P3 differ only by white point. Their difference
    // here is inside the image budget, so pin a discriminating opaque anchor
    // instead: authored pixel (2,0) is [2,2,64,255] in the companion.
    for (primary, expected) in [(11, [20, 20, 143, 255]), (12, [22, 22, 143, 255])] {
        let observed = image(&format!("cicp-p{primary}-t8-r1-c6-d8"), true);
        assert_eq!(&observed.rgba()[8..12], &expected);
        assert_eq!(
            pixel_rgb(&[(*b"cICP", vec![primary, 8, 0, 1])], [2, 2, 64]),
            expected
        );
    }
}

fn pixel(chunks: &[([u8; 4], Vec<u8>)], value: u8) -> Vec<u8> {
    pixel_rgb(chunks, [value; 3])
}
fn pixel_rgb(chunks: &[([u8; 4], Vec<u8>)], rgb: [u8; 3]) -> Vec<u8> {
    let source = crate::common::codec_fixtures::png_with_chunks_before_idat(
        &crate::common::codec_fixtures::png_rgba(1, 1, &[[rgb[0], rgb[1], rgb[2], 255]]),
        chunks,
    );
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(source)
    );
    let scene = SceneDocument::parse(&serde_json::to_vec(&json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":1,"height":1,"fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},"files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":data,"created":1}}})).unwrap()).unwrap();
    let result = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Png)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(result.report()).unwrap(),
        json!({"schema_version":1,"diagnostics":[]})
    );
    crate::common::raster::decode_png(result.artifact().bytes()).rgba
}
#[test]
fn exact_transfer_and_metadata_anchors() {
    assert_eq!(
        lower_hex(&Sha256::digest(include_bytes!(
            "../common/png_color_reference.rs"
        ))),
        "6d2cba3ddfa2fcf017ea9972715aee3b0a6f002cb2afce950274059a1b64b83f"
    );
    let evidence = reference::evidence();
    let icc = evidence["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "icc-v4-srgb-c6-d8")
        .unwrap();
    let tags: Vec<_> = icc["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["kind"].as_str().unwrap().as_bytes().try_into().unwrap(),
                t["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect(),
            )
        })
        .collect();
    assert_eq!(pixel(&tags, 1), [1, 1, 1, 255]);
    assert_eq!(pixel(&tags, 2), [2, 2, 2, 255]);
    for value in [1, 2, 8, 16, 32, 128, 255] {
        assert_eq!(
            pixel(&[(*b"cICP", vec![1, 13, 0, 1])], value),
            [value, value, value, 255]
        );
    }
    assert_eq!(
        pixel(&[(*b"cICP", vec![1, 1, 0, 1])], 128),
        // Above the sRGB toe: 255*(1.055*(128/255)-0.055) rounds to 121.
        [121, 121, 121, 255]
    );
    assert_eq!(
        pixel(&[(*b"cICP", vec![9, 8, 0, 0])], 128),
        [128, 128, 128, 255]
    );
    assert_eq!(
        pixel(&[(*b"gAMA", 45455_u32.to_be_bytes().to_vec())], 16),
        [16, 16, 16, 255]
    );
    for (gamma, expected) in [(43181_u32, 5), (43182, 16), (47727, 16), (47728, 10)] {
        assert_eq!(
            pixel(&[(*b"gAMA", gamma.to_be_bytes().to_vec())], 16),
            [expected, expected, expected, 255]
        );
    }
    assert_eq!(
        pixel(&[(*b"gAMA", 100000_u32.to_be_bytes().to_vec())], 128),
        [188, 188, 188, 255]
    );
    assert_eq!(
        pixel(&[(*b"gAMA", 1_u32.to_be_bytes().to_vec())], 254),
        [0, 0, 0, 255]
    );
    assert_eq!(
        pixel(&[(*b"gAMA", u32::MAX.to_be_bytes().to_vec())], 0),
        [0, 0, 0, 255]
    );
    assert_eq!(
        pixel(
            &[
                (*b"gAMA", 100000_u32.to_be_bytes().to_vec()),
                (*b"sRGB", vec![0])
            ],
            128
        ),
        [128, 128, 128, 255]
    );
}
