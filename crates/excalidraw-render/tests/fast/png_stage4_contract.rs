use crate::common::lower_hex;
use crate::common::png_stage4::{self as vectors, Case, Outcome};
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::json;

#[test]
fn review_adam7_narrow_multiline_subframes_preserve_rows_and_validate_later_indexes() {
    for selected in [false, true] {
        for bad in [false, true] {
            let mut chunks = vec![
                (*b"IHDR", vectors::header(3, 2, 3, 8, true)),
                (*b"PLTE", vec![255, 0, 0, 0, 255, 0]),
                (
                    *b"acTL",
                    [
                        if selected { 1_u32 } else { 2 }.to_be_bytes(),
                        0_u32.to_be_bytes(),
                    ]
                    .concat(),
                ),
            ];
            if !selected {
                chunks.push((*b"fcTL", vectors::control(0, 3, 2, 0, 0, 0, 0)));
            }
            chunks.push((*b"IDAT", vectors::compressed(&[0; 10])));
            chunks.push((
                *b"fcTL",
                vectors::control(u32::from(!selected), 1, 2, 1, 0, 0, 0),
            ));
            chunks.push((
                *b"fdAT",
                [
                    (1 + u32::from(!selected)).to_be_bytes().as_slice(),
                    &vectors::compressed(&[0, 0, 0, if bad { 2 } else { 1 }]),
                ]
                .concat(),
            ));
            chunks.push((*b"IEND", vec![]));
            let mut row = vectors::case(
                format!("narrow-adam7-{selected}-{bad}"),
                &chunks,
                if bad {
                    Outcome::DecodeFailed
                } else {
                    Outcome::Accepted(if selected { 1 } else { 2 })
                },
            );
            row.pixels = if selected {
                vec![
                    0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0,
                ]
            } else {
                [255, 0, 0, 255].repeat(6)
            };
            check(vec![row]);
        }
    }
}

#[test]
fn review_icc_declaration_bounds_both_tag_table_and_tag_data() {
    for size in [132_u32, 144, 215, 216, 296, 308, 312] {
        let mut profile = crate::common::codec_fixtures::linear_rgb_icc_profile();
        profile[..4].copy_from_slice(&size.to_be_bytes());
        let mut chunks = vectors::base();
        chunks.insert(
            1,
            (
                *b"iCCP",
                [b"authored\0\0".as_slice(), &vectors::compressed(&profile)].concat(),
            ),
        );
        check(vec![vectors::case(
            format!("icc-declared-{size}"),
            &chunks,
            if size == 312 {
                Outcome::Accepted(1)
            } else {
                Outcome::Unsupported
            },
        )]);
    }
}

#[test]
fn review_icc_common_headers_are_checked_on_every_path() {
    check(crate::common::png_icc_safety::headers());
}
#[test]
fn sampled_icc_curves_and_descriptive_metadata_have_independent_pixel_contracts() {
    check_evidence(
        crate::common::png_icc_safety::sampled_evidence(),
        "icc-sampled",
        64,
    );
}

#[test]
fn review_lower_depth_transparency_keys_never_alias_after_truncation() {
    check(crate::common::png_stage4_review::lower_keys());
    for (color, depth) in [(0, 1), (0, 2), (0, 4), (0, 8), (2, 8)] {
        let maximum = (1_u16 << depth) - 1;
        for key in [0, maximum, maximum + 1, 0x100, 0xffff] {
            let data = if color == 0 {
                key.to_be_bytes().to_vec()
            } else {
                [key.to_be_bytes(), 0_u16.to_be_bytes(), 0_u16.to_be_bytes()].concat()
            };
            let raw = vec![
                0;
                if color == 0 {
                    2 * (1 + (3 * depth as usize).div_ceil(8))
                } else {
                    20
                }
            ];
            let chunks = vec![
                (*b"IHDR", vectors::header(3, 2, color, depth, false)),
                (*b"tRNS", data),
                (*b"IDAT", vectors::compressed(&raw)),
                (*b"IEND", vec![]),
            ];
            let mut row = vectors::case(
                format!("key-{color}-{depth}-{key}"),
                &chunks,
                if key > maximum {
                    Outcome::DecodeFailed
                } else {
                    Outcome::Accepted(1)
                },
            );
            row.pixels = if key == 0 {
                vec![0; 24]
            } else {
                [0, 0, 0, 255].repeat(6)
            };
            check(vec![row]);
        }
    }
}

#[test]
fn authored_case_cardinalities_are_explicit() {
    for (name, actual, expected) in [
        ("orientation", vectors::orientations().len(), 32),
        (
            "invalid-orientation",
            vectors::invalid_orientation().len(),
            149,
        ),
        ("ihdr", vectors::ihdr_cases().len(), 66292),
        (
            "chunks",
            vectors::chunk_cases().len(),
            vectors::encode(&vectors::base()).len() + 5,
        ),
        ("scanlines", vectors::scanline_cases().len(), 271),
        ("palette", vectors::palette_cases().len(), 50),
        ("apng-controls", vectors::apng_controls().len(), 74),
        ("apng-malformed", vectors::apng_malformed().len(), 2078),
        ("invalid-cicp", vectors::invalid_cicp().len(), 994),
        ("large-streams", vectors::large_streams().len(), 4),
        (
            "composition-per-signal",
            vectors::composition(&[], false).len(),
            720,
        ),
        (
            "hdr-composition-per-transfer",
            vectors::hdr_composition(16).len(),
            720,
        ),
    ] {
        assert_eq!(actual, expected, "{name}");
        println!("{name}: {actual}");
    }
}

fn check(rows: Vec<Case>) {
    for row in rows {
        let source = vectors::scene(&row);
        let scene = SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
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
                row.id
            );
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
            vectors::report(
                &row,
                format.as_str(),
                serde_json::to_value(first.report()).unwrap(),
            );
            if row.outcome == Outcome::DecodeFailed
                || (row.outcome == Outcome::Unsupported && format != OutputFormat::Svg)
            {
                let mut removed = source.clone();
                removed["files"] = json!({});
                let removed = SceneDocument::parse(&serde_json::to_vec(&removed).unwrap()).unwrap();
                let recovery = Renderer::new().render(&removed, &request).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    recovery.artifact().bytes(),
                    "{} recovery {format:?}",
                    row.id
                );
                continue;
            }
            match format {
                OutputFormat::Svg => {
                    let doc = roxmltree::Document::parse(
                        std::str::from_utf8(first.artifact().bytes()).unwrap(),
                    )
                    .unwrap();
                    let image = doc.descendants().find(|n| n.has_tag_name("image")).unwrap();
                    assert_eq!(
                        image.attribute("href").unwrap(),
                        source["files"]["asset"]["dataURL"].as_str().unwrap()
                    );
                }
                OutputFormat::Png => {
                    let raster = crate::common::raster::decode_png(first.artifact().bytes());
                    assert_eq!(
                        (raster.width as usize, raster.height as usize),
                        (row.width, row.height),
                        "{}",
                        row.id
                    );
                    assert_eq!(raster.rgba, row.pixels, "{} PNG", row.id);
                    super::png_profile_contract::assert_untagged_png(first.artifact().bytes());
                }
                OutputFormat::Pdf => super::png_profile_contract::assert_pdf_pixels(
                    first.artifact().bytes(),
                    row.width,
                    row.height,
                    &row.pixels,
                ),
                _ => unreachable!(),
            }
        }
    }
}
#[test]
fn all_orientation_cells_preserve_source_and_permute_pixels() {
    check(vectors::orientations());
    check(vectors::invalid_orientation());
}
#[test]
fn invalid_orientation_is_ignored_but_duplicate_chunks_fail() {
    check(vectors::invalid_orientation());
}
#[test]
fn malformed_png_headers_chunks_and_streams_have_complete_recovery() {
    review_adam7_narrow_multiline_subframes_preserve_rows_and_validate_later_indexes();
    review_icc_declaration_bounds_both_tag_table_and_tag_data();
    review_lower_depth_transparency_keys_never_alias_after_truncation();
    check(vectors::ihdr_cases());
    check(vectors::chunk_cases());
    check(vectors::palette_cases());
    check(vectors::scanline_cases());
    check(vectors::apng_malformed());
    check(vectors::invalid_orientation());
    check_icc("icc-malformed", 36);
    check_icc("icc-malformed-extra", 20);
}
#[test]
fn malformed_chunks_fail_and_unknown_ancillary_survives() {
    check(vectors::chunk_cases());
}
#[test]
fn scanline_streams_and_adam7_extents_are_fully_validated() {
    check(vectors::scanline_cases());
    check(vectors::large_streams());
}
#[test]
fn streaming_validation_drains_bounded_output_across_split_input() {
    check(vectors::large_streams());
}
#[test]
fn reserved_cicp_fields_preserve_svg_and_recover_projections() {
    check(vectors::invalid_cicp());
}
#[test]
fn sample_color_orientation_animation_axes_compose() {
    check(vectors::composition(
        &[(*b"gAMA", 100000_u32.to_be_bytes().to_vec())],
        true,
    ));
    check(vectors::composition(&[(*b"cICP", vec![1, 8, 0, 1])], true));
    let p = crate::common::codec_fixtures::linear_rgb_icc_profile();
    let data = [b"authored\0\0".as_slice(), &vectors::compressed(&p)].concat();
    check(vectors::composition(&[(*b"iCCP", data)], true));
    check(vectors::hdr_composition(16));
    check(vectors::hdr_composition(18));
}
#[test]
fn palettes_and_transparency_chunks_have_strict_sample_contracts() {
    review_lower_depth_transparency_keys_never_alias_after_truncation();
    check(vectors::palette_cases());
}
#[test]
fn apng_one_frame_subframes_blend_and_disposal_are_qualified() {
    review_adam7_narrow_multiline_subframes_preserve_rows_and_validate_later_indexes();
    check(vectors::apng_controls());
    animation_occurrences();
    check(vectors::apng_malformed());
}

fn animation_occurrences() {
    for separate in [false, true] {
        for frames in [0, 1, 2] {
            let chunks = if frames == 0 {
                vectors::base()
            } else {
                vectors::animation(separate, frames, false, 0, 0)
            };
            let row = vectors::case(
                "animation-occurrences",
                &chunks,
                Outcome::Accepted(frames.max(1)),
            );
            for active in 0..=2 {
                let mut source = vectors::scene(&row);
                let original = source["elements"][0].clone();
                source["elements"] = json!([]);
                for i in 0..3 {
                    let mut image = original.clone();
                    image["id"] = json!(format!("image-{i}"));
                    image["x"] = json!(i * 3);
                    image["isDeleted"] = json!(i >= active);
                    source["elements"].as_array_mut().unwrap().push(image);
                }
                let scene = SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
                for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
                    let request = RenderRequest::builder(format)
                        .padding(0.0)
                        .include_background(false)
                        .build()
                        .unwrap();
                    let first = Renderer::new().render(&scene, &request).unwrap();
                    let second = Renderer::new().render(&scene, &request).unwrap();
                    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                    assert_eq!(first.report(), second.report());
                    let mut actual = serde_json::to_value(first.report()).unwrap();
                    for d in actual["diagnostics"].as_array_mut().unwrap() {
                        assert!(!d["message"].as_str().unwrap().is_empty());
                        d.as_object_mut().unwrap().remove("message");
                    }
                    let diagnostics: Vec<_> = if frames > 1 && format != OutputFormat::Svg {
                        (0..active).map(|i|json!({"code":"image.animation-flattened","severity":"info","format":format.as_str(),"effects":["visual"],"element_id":format!("image-{i}"),"json_pointer":format!("/elements/{i}/fileId"),"details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/png","recovery":"flattened","recipe_id":null}})).collect()
                    } else {
                        vec![]
                    };
                    assert_eq!(
                        actual,
                        json!({"schema_version":1,"diagnostics":diagnostics})
                    );
                    if active != 0 && format == OutputFormat::Png {
                        let actual = crate::common::raster::decode_png(first.artifact().bytes());
                        assert_eq!((actual.width, actual.height), (active * 3, 2));
                        let expected: Vec<_> = vectors::PIXELS
                            .as_chunks::<3>()
                            .0
                            .iter()
                            .flat_map(|pixels| {
                                (0..active).flat_map(move |_| pixels.as_flattened().iter().copied())
                            })
                            .collect();
                        assert_eq!(actual.rgba, expected);
                    }
                    if active != 0 && format == OutputFormat::Svg {
                        let doc = roxmltree::Document::parse(
                            std::str::from_utf8(first.artifact().bytes()).unwrap(),
                        )
                        .unwrap();
                        assert_eq!(
                            doc.descendants()
                                .filter(|n| n.has_tag_name("image"))
                                .count(),
                            1
                        );
                        assert_eq!(
                            doc.descendants().filter(|n| n.has_tag_name("use")).count(),
                            active as usize
                        );
                        assert_eq!(
                            doc.descendants()
                                .find(|n| n.has_tag_name("image"))
                                .unwrap()
                                .attribute("href")
                                .unwrap(),
                            source["files"]["asset"]["dataURL"].as_str().unwrap()
                        );
                    }
                    if active != 0 && format == OutputFormat::Pdf {
                        super::png_profile_contract::assert_pdf_pixels(
                            first.artifact().bytes(),
                            3,
                            2,
                            vectors::PIXELS.as_flattened(),
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn apng_sequence_counts_and_later_streams_fail_before_passthrough() {
    review_adam7_narrow_multiline_subframes_preserve_rows_and_validate_later_indexes();
    check(vectors::apng_malformed());
}

fn check_icc(group: &str, count: usize) {
    check_evidence(
        crate::common::png_stage4_reference::evidence(),
        group,
        count,
    );
}

#[test]
fn complete_profile_composition_matches_independent_reference() {
    check_evidence(
        crate::common::png_stage4_review::gray_lut_equivalence(),
        "icc-gray-lut",
        12,
    );
    check_evidence(
        crate::common::png_stage4_composition::evidence(),
        "composition",
        1968,
    );
    review_adam7_narrow_multiline_subframes_preserve_rows_and_validate_later_indexes();
    review_lower_depth_transparency_keys_never_alias_after_truncation();
}

/// PNG alpha and color-profile umbrella facets (#85): every accepted signal
/// family crossed with every sample layout, transparency form (intrinsic,
/// key, palette, none), interlace and APNG container against the pinned
/// reference, including PDF's grayscale soft mask and transparent RGB zero.
/// The frozen matrix covers the gray ICC, LUT, gAMA, cHRM, v4 ICC and PQ/HLG
/// families; the supplement adds the sRGB chunk, v2 matrix/TRC ICC, SDR,
/// linear, BT.709 and Display-P3 cICP, ignored narrow-range cICP, and gray
/// sRGB/gAMA/cICP/PQ.
#[test]
fn png_alpha_and_profiles_compose_across_every_layout() {
    complete_profile_composition_matches_independent_reference();
    check_evidence(
        crate::common::png_stage4_composition::supplemental_evidence(),
        "composition",
        840,
    );
}

/// PNG limits umbrella facet (#85): byte, dimension, pixel and work equality
/// and first crossing for every layout, interlace and container, and the
/// multi-crossing precedence of spec section 13.2.
#[test]
fn png_limits_have_exact_boundaries_and_precedence() {
    profile_validation_and_decode_limits_have_exact_boundaries();
    super::png_limit_crossing_contract::raster_crossings_order_width_then_height_then_area();
}

fn check_evidence(evidence: serde_json::Value, group: &str, count: usize) {
    use crate::common::{png_color_reference as reference, png_stage4_reference};
    use sha2::{Digest, Sha256};
    for source in evidence["sources"].as_array().unwrap() {
        if crate::common::is_oracle_producer(source["path"].as_str().unwrap()) {
            continue;
        }
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
        if group == "composition" {
            let r = reference::png(&reference::base64(&row["reference"]["png"]));
            let alpha: Vec<_> = r.rgba().as_chunks::<4>().0.iter().map(|p| p[3]).collect();
            assert_eq!(json!(alpha), row["alpha"], "{} reference alpha", row["id"]);
        }
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
                "{}",
                row["id"]
            );
            assert_eq!(first.report(), second.report());
            if group == "composition" {
                if format != OutputFormat::Svg {
                    let late = SceneDocument::parse(
                        &serde_json::to_vec(&crate::common::png_stage4_review::late_scene(row))
                            .unwrap(),
                    )
                    .unwrap();
                    let late = Renderer::new().render(&late, &request).unwrap();
                    assert_eq!(first.artifact().bytes(), late.artifact().bytes());
                    assert_eq!(first.report(), late.report());
                }
                crate::common::png_stage4_composition::report(
                    row,
                    format.as_str(),
                    serde_json::to_value(first.report()).unwrap(),
                );
            } else {
                reference::report(
                    row,
                    format.as_str(),
                    serde_json::to_value(first.report()).unwrap(),
                );
            }
            if reference::recovery(row, format.as_str()).is_some() {
                let mut removed = row["scene"].clone();
                removed["files"] = json!({});
                let removed = SceneDocument::parse(&serde_json::to_vec(&removed).unwrap()).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    Renderer::new()
                        .render(&removed, &request)
                        .unwrap()
                        .artifact()
                        .bytes()
                );
                continue;
            }
            match format {
                OutputFormat::Svg => {
                    let doc = roxmltree::Document::parse(
                        std::str::from_utf8(first.artifact().bytes()).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        doc.descendants()
                            .find(|n| n.has_tag_name("image"))
                            .unwrap()
                            .attribute("href")
                            .unwrap(),
                        row["scene"]["files"]["asset"]["dataURL"].as_str().unwrap()
                    );
                }
                OutputFormat::Png => {
                    super::png_profile_contract::assert_untagged_png(first.artifact().bytes());
                    png_stage4_reference::anchor(
                        row,
                        reference::png(first.artifact().bytes()).rgba(),
                    );
                    reference::appearance(row, reference::png(first.artifact().bytes()), false);
                }
                OutputFormat::Pdf => {
                    let (w, h) = (
                        row["width"].as_u64().unwrap() as usize,
                        row["height"].as_u64().unwrap() as usize,
                    );
                    let pixels =
                        super::png_profile_contract::pdf_rgba(first.artifact().bytes(), w, h);
                    png_stage4_reference::anchor(row, &pixels);
                    reference::appearance(
                        row,
                        excalidraw_render_compare::CanonicalRgba8::new(w as u32, h as u32, pixels)
                            .unwrap(),
                        true,
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
#[test]
fn gray_icc_profiles_transform_expanded_samples() {
    sampled_icc_curves_and_descriptive_metadata_have_independent_pixel_contracts();
    check_icc("icc-gray", 8);
    check_evidence(
        crate::common::png_stage4_review::gray_lut_equivalence(),
        "icc-gray-lut",
        12,
    );
}
#[test]
fn lut_profiles_select_tables_and_recover_unsupported_classes() {
    check_icc("icc-lut", 62);
    check_icc("icc-lut-extra", 16);
}
#[test]
fn lut_lab_pcs_missing_intent_tables_and_mab_stages_are_qualified() {
    check_icc("icc-lut-extra", 16);
}
#[test]
fn incompatible_icc_models_preserve_svg_and_recover_projections() {
    check_icc("icc-model", 40);
    check_icc("icc-model-extra", 4);
}
#[test]
fn malformed_iccp_has_explicit_ignore_or_recovery_contracts() {
    review_icc_common_headers_are_checked_on_every_path();
    review_icc_declaration_bounds_both_tag_table_and_tag_data();
    check_icc("icc-malformed", 36);
    check_icc("icc-malformed-extra", 20);
}

#[test]
fn profile_validation_and_decode_limits_have_exact_boundaries() {
    use excalidraw_render::ResourceLimits;
    for row in vectors::composition(&[], false)
        .into_iter()
        .filter(|r| r.id.ends_with("-1-false"))
    {
        let source = vectors::scene(&row);
        let scene = SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
        let bytes = row.bytes.len() as u64;
        let extra = if row.id.contains("-2-1-false") {
            48
        } else if row.id.contains("-1-1-false") {
            24
        } else {
            0
        };
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            for field in ["bytes", "dimension", "pixels", "work"] {
                for crossing in [false, true] {
                    let (n, field_name, code, kind) = match field {
                        "bytes" => (
                            bytes,
                            "max_asset_bytes",
                            "resource.asset-bytes-exceeded",
                            "bytes",
                        ),
                        "dimension" => (
                            3,
                            "max_raster_dimension",
                            "resource.image-dimensions-exceeded",
                            "dimension",
                        ),
                        "pixels" => (
                            6,
                            "max_decoded_pixels_per_asset",
                            "resource.image-pixels-exceeded",
                            "pixels",
                        ),
                        "work" => (
                            if format == OutputFormat::Svg {
                                bytes + extra
                            } else {
                                5 * bytes + 24 + extra
                            },
                            "max_asset_work_units",
                            "resource.asset-work-exceeded",
                            "work_units",
                        ),
                        _ => unreachable!(),
                    };
                    let configured = n - u64::from(crossing);
                    let limits = match field {
                        "bytes" => ResourceLimits::builder().max_asset_bytes(configured),
                        "dimension" => {
                            ResourceLimits::builder().max_raster_dimension(configured as u32)
                        }
                        "pixels" => {
                            ResourceLimits::builder().max_decoded_pixels_per_asset(configured)
                        }
                        "work" => ResourceLimits::builder().max_asset_work_units(configured),
                        _ => unreachable!(),
                    }
                    .build()
                    .unwrap();
                    let request = RenderRequest::builder(format)
                        .padding(0.0)
                        .include_background(false)
                        .resource_limits(limits)
                        .build()
                        .unwrap();
                    let first = Renderer::new().render(&scene, &request).unwrap();
                    let second = Renderer::new().render(&scene, &request).unwrap();
                    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
                    assert_eq!(first.report(), second.report());
                    let fails = crossing
                        && !(format == OutputFormat::Svg
                            && matches!(field, "dimension" | "pixels"));
                    if fails {
                        let mut actual = serde_json::to_value(first.report()).unwrap();
                        for r in actual["diagnostics"].as_array_mut().unwrap() {
                            assert!(!r["message"].as_str().unwrap().is_empty());
                            r.as_object_mut().unwrap().remove("message");
                        }
                        assert_eq!(
                            actual,
                            json!({"schema_version":1,"diagnostics":[{"code":code,"severity":"warning","format":format.as_str(),"effects":["visual"],"element_id":if matches!(field,"dimension"|"pixels") {serde_json::Value::Null} else {json!("image")},"json_pointer":"/files/asset/dataURL","details":{"kind":"resource_limit","field":field_name,"configured":configured,"value_kind":kind,"measurement":{"kind":"attempted","value":n},"recovery":if format==OutputFormat::Png {"placeholder"} else {"omitted"}}}]}),
                            "{} {field} {format:?}",
                            row.id
                        );
                        let mut removed = source.clone();
                        removed["files"] = json!({});
                        let removed =
                            SceneDocument::parse(&serde_json::to_vec(&removed).unwrap()).unwrap();
                        let control = Renderer::new()
                            .render(
                                &removed,
                                &RenderRequest::builder(format)
                                    .padding(0.0)
                                    .include_background(false)
                                    .build()
                                    .unwrap(),
                            )
                            .unwrap();
                        assert_eq!(first.artifact().bytes(), control.artifact().bytes());
                    } else {
                        vectors::report(
                            &row,
                            format.as_str(),
                            serde_json::to_value(first.report()).unwrap(),
                        );
                        let control = Renderer::new()
                            .render(
                                &scene,
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
    }
}

#[test]
fn profile_projection_shares_identities_but_charges_occurrences_tracer() {
    use crate::common::png_color_reference as reference;
    use base64::Engine as _;
    use excalidraw_render::{ErrorCode, ResourceLimits};
    let evidence = crate::common::png_stage4_reference::evidence();
    for id in [
        "gray-v4-g1-c4-d16",
        "lut-lab-mAB-c6-d8",
        "lut-mab-matrix-m-curves-aligned-c6-d16",
    ] {
        let row = evidence["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let bytes = reference::base64(&row["png"]);
        for distinct in [false, true] {
            let mut source = row["scene"].clone();
            source["files"]["second"] = source["files"]["asset"].clone();
            source["files"]["second"]["id"] = json!("second");
            let second = if distinct {
                crate::common::codec_fixtures::png_with_chunk_before_iend(
                    &bytes,
                    *b"vpAg",
                    b"distinct semantic identity",
                )
            } else {
                bytes.clone()
            };
            source["files"]["second"]["dataURL"] = json!(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&second)
            ));
            let mut image = source["elements"][0].clone();
            image["id"] = json!("second-image");
            image["fileId"] = json!("second");
            image["x"] = json!(15);
            source["elements"].as_array_mut().unwrap().push(image);
            let scene = SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
            for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
                let pixels = if distinct { 450 } else { 225 };
                let limits = ResourceLimits::builder()
                    .max_asset_bytes(second.len().max(bytes.len()) as u64)
                    .max_total_asset_bytes((bytes.len() + second.len()) as u64)
                    .max_decoded_pixels_per_asset(225)
                    .max_total_decoded_pixels(pixels)
                    .build()
                    .unwrap();
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .include_background(false)
                    .resource_limits(limits)
                    .build()
                    .unwrap();
                let first = Renderer::new().render(&scene, &request).unwrap();
                let second_render = Renderer::new().render(&scene, &request).unwrap();
                assert_eq!(
                    serde_json::to_value(first.report()).unwrap(),
                    json!({"schema_version":1,"diagnostics":[]})
                );
                assert_eq!(first.report(), second_render.report());
                assert_eq!(first.artifact().bytes(), second_render.artifact().bytes());
                if format == OutputFormat::Png {
                    let p = reference::png(first.artifact().bytes());
                    assert_eq!((p.width(), p.height()), (30, 15));
                    for x in [0, 15] {
                        let pixels = (0..15)
                            .flat_map(|y| {
                                p.rgba()[(y * 30 + x) * 4..(y * 30 + x + 15) * 4]
                                    .iter()
                                    .copied()
                            })
                            .collect();
                        reference::appearance(
                            row,
                            excalidraw_render_compare::CanonicalRgba8::new(15, 15, pixels).unwrap(),
                            false,
                        );
                    }
                }
                let limits = ResourceLimits::builder()
                    .max_asset_bytes(second.len().max(bytes.len()) as u64)
                    .max_total_asset_bytes((bytes.len() + second.len() - 1) as u64)
                    .build()
                    .unwrap();
                let request = RenderRequest::builder(format)
                    .padding(0.0)
                    .include_background(false)
                    .resource_limits(limits)
                    .build()
                    .unwrap();
                let error = Renderer::new().render(&scene, &request).unwrap_err();
                assert_eq!(
                    error.descriptor().code(),
                    ErrorCode::RESOURCE_TOTAL_ASSET_BYTES_EXCEEDED
                );
                assert_eq!(
                    serde_json::to_value(error.descriptor().details()).unwrap(),
                    json!({"kind":"resource_limit","field":"max_total_asset_bytes","configured":bytes.len()+second.len()-1,"value_kind":"bytes","measurement":{"kind":"attempted","value":bytes.len()+second.len()},"recovery":null})
                );
                let repeated = Renderer::new().render(&scene, &request).unwrap_err();
                assert_eq!(error.descriptor(), repeated.descriptor());
                assert_eq!(error.report(), repeated.report());
                assert_eq!(
                    serde_json::to_value(error.report()).unwrap(),
                    json!({"schema_version":1,"diagnostics":[]})
                );
                if distinct && format != OutputFormat::Svg {
                    let request = RenderRequest::builder(format)
                        .padding(0.0)
                        .include_background(false)
                        .resource_limits(
                            ResourceLimits::builder()
                                .max_decoded_pixels_per_asset(225)
                                .max_total_decoded_pixels(449)
                                .build()
                                .unwrap(),
                        )
                        .build()
                        .unwrap();
                    let error = Renderer::new().render(&scene, &request).unwrap_err();
                    assert_eq!(
                        error.descriptor().code(),
                        ErrorCode::RESOURCE_TOTAL_DECODED_PIXELS_EXCEEDED
                    );
                    assert_eq!(
                        serde_json::to_value(error.descriptor().details()).unwrap(),
                        json!({"kind":"resource_limit","field":"max_total_decoded_pixels","configured":449,"value_kind":"pixels","measurement":{"kind":"attempted","value":450},"recovery":null})
                    );
                    assert_eq!(
                        serde_json::to_value(error.report()).unwrap(),
                        json!({"schema_version":1,"diagnostics":[]})
                    );
                }
            }
        }
    }
}
