//! The raster codecs at the asset cache's own seam: `Cache::resolve` over one
//! scene file record with a transaction sink that admits everything, read off
//! the `Resolution` it returns — decoded RGBA, oriented dimensions, the
//! animation flag, the sniffed media kind and the typed `ResolveError` — rather
//! than off a rendered PNG's pixels or the diagnostics a session later maps
//! them to. The fixture builders are the ones the integration binaries use,
//! included by path from `tests/common`.

use std::sync::Arc;

use base64::Engine as _;

use super::*;

// The codec fixture builders live with the integration plumbing so the six
// `tests/fast/static_*_asset_render.rs` modules and this suite encode the same
// bytes; they are pure encoders with no dependency on either crate spelling,
// which is what lets one file serve both compilation units.
use crate::codec_fixtures as fixtures;

use fixtures::*;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const GREY_128: [u8; 4] = [128, 128, 128, 255];

/// A scene with no elements and one file record `asset` whose `mimeType` is
/// `mime_type` and whose data URL declares `data_url_media_type` over
/// `payload`; `Cache::resolve` reads only the record.
fn file_record_scene_with_claims(
    mime_type: &str,
    data_url_media_type: &str,
    payload: &[u8],
) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,"elements":[],"appState":{{}},
            "files":{{"asset":{{
                "id":"asset","mimeType":"{mime_type}",
                "dataURL":"data:{data_url_media_type};base64,{encoded}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("file record scene is admitted")
}

/// [`file_record_scene_with_claims`] with both claims naming `media_type`.
fn file_record_scene(media_type: &str, payload: &[u8]) -> SceneDocument {
    file_record_scene_with_claims(media_type, media_type, payload)
}

/// A scene with no elements and no file records.
fn empty_scene() -> SceneDocument {
    SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#,
    )
    .expect("empty scene is admitted")
}

/// Resolves the `asset` record of `scene` under `authority` for `format`
/// through `cache`, at revision 1 under default limits, sending every
/// transaction to `transact`.
fn resolve_with<F>(
    cache: &mut Cache,
    scene: &SceneDocument,
    supplemental: &SupplementalAssets,
    format: OutputFormat,
    authority: Authority,
    transact: F,
) -> Result<Resolution, ResolutionFailure>
where
    F: FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
{
    cache.resolve(
        scene,
        supplemental,
        CompatibilityRevision::REVISION_1,
        format,
        &ResourceLimits::default(),
        "asset",
        authority,
        transact,
    )
}

/// Resolves the scene's own `asset` record for `format` from a fresh cache,
/// admitting every transaction.
fn resolve_scene(
    scene: &SceneDocument,
    format: OutputFormat,
) -> Result<Resolution, ResolutionFailure> {
    resolve_with(
        &mut Cache::default(),
        scene,
        &SupplementalAssets::default(),
        format,
        Authority::Scene,
        |_| Ok(()),
    )
}

/// Resolves `payload` declared as `media_type` for the PNG target, which
/// derives the pixels.
fn resolve_for_png(media_type: &str, payload: &[u8]) -> Resolution {
    resolve_scene(&file_record_scene(media_type, payload), OutputFormat::Png)
        .expect("the payload decodes")
}

/// Resolves `payload` for the direct-SVG target, which validates without
/// deriving pixels.
fn resolve_for_svg(media_type: &str, payload: &[u8]) -> Resolution {
    resolve_scene(&file_record_scene(media_type, payload), OutputFormat::Svg)
        .expect("the payload validates")
}

/// The decoded asset of `payload` declared as `media_type`.
fn decode(media_type: &str, payload: &[u8]) -> Arc<ResolvedImageAsset> {
    resolve_for_png(media_type, payload).asset
}

/// The typed failure in `result`, which must be the asset's own refusal and
/// not the meter's.
fn asset_error(result: Result<Resolution, ResolutionFailure>) -> ResolveError {
    match result {
        Err(ResolutionFailure::Asset(error)) => error,
        Ok(_) => panic!("the payload is refused"),
        Err(other) => panic!("the payload is refused by the asset, not the meter: {other:?}"),
    }
}

/// The typed failure resolving `payload` for `format` produces.
fn failure_for(media_type: &str, payload: &[u8], format: OutputFormat) -> ResolveError {
    asset_error(resolve_scene(
        &file_record_scene(media_type, payload),
        format,
    ))
}

/// The typed failure resolving `payload` for the PNG target produces.
fn failure(media_type: &str, payload: &[u8]) -> ResolveError {
    failure_for(media_type, payload, OutputFormat::Png)
}

/// The straight-alpha pixel at (`x`, `y`) of a decoded asset.
fn pixel(asset: &ResolvedImageAsset, x: u32, y: u32) -> [u8; 4] {
    assert!(
        x < asset.width && y < asset.height,
        "({x}, {y}) is inside {}x{}",
        asset.width,
        asset.height
    );
    let rgba = asset.rgba.as_ref().expect("the PNG target derives pixels");
    let offset = usize::try_from((y * asset.width + x) * 4).expect("offset fits usize");
    rgba[offset..offset + 4].try_into().expect("four channels")
}

/// The only pixel of a one-pixel decoded asset.
fn only_pixel(asset: &ResolvedImageAsset) -> [u8; 4] {
    assert_eq!((asset.width, asset.height), (1, 1));
    pixel(asset, 0, 0)
}

/// Every pixel of a decoded asset, row-major.
fn pixels(asset: &ResolvedImageAsset) -> Vec<[u8; 4]> {
    (0..asset.height)
        .flat_map(|y| (0..asset.width).map(move |x| (x, y)))
        .map(|(x, y)| pixel(asset, x, y))
        .collect()
}

/// Asserts an opaque neutral grey within `tolerance` of `expected`, the shape
/// every colour-managed lossy or tagged fixture is read back in.
fn assert_grey_within(pixel: [u8; 4], expected: u8, tolerance: u8) {
    assert!(
        pixel[0].abs_diff(expected) <= tolerance,
        "grey {expected}: {pixel:?}"
    );
    assert_eq!(pixel[0], pixel[1], "grey has equal channels: {pixel:?}");
    assert_eq!(pixel[1], pixel[2], "grey has equal channels: {pixel:?}");
    assert_eq!(pixel[3], 255);
}

// --- PNG and APNG ----------------------------------------------------------

mod png_codec {
    use super::*;

    #[test]
    fn a_static_png_decodes_to_its_straight_rgba_and_reports_no_animation() {
        let resolution = resolve_for_png("image/png", &png_rgba(2, 1, &[RED, BLUE]));

        assert_eq!(resolution.media_kind, MediaKind::Png);
        assert!(!resolution.animated);
        assert_eq!(resolution.asset.media_type, "image/png");
        assert_eq!((resolution.asset.width, resolution.asset.height), (2, 1));
        assert_eq!(pixels(&resolution.asset), [RED, BLUE]);
    }

    #[test]
    fn partial_alpha_is_kept_straight_and_premultiplied_side_by_side() {
        let asset = decode("image/png", &png_rgba(1, 1, &[[200, 100, 50, 128]]));

        assert_eq!(only_pixel(&asset), [200, 100, 50, 128]);
        assert_eq!(
            asset.premultiplied_rgba.as_deref().map(Vec::as_slice),
            Some(&[100, 50, 25, 128][..])
        );
    }

    #[test]
    fn fully_transparent_pixels_lose_their_colour() {
        let asset = decode("image/png", &png_rgba(1, 1, &[[255, 17, 33, 0]]));

        assert_eq!(only_pixel(&asset), [0, 0, 0, 0]);
    }

    #[test]
    fn gama_normalizes_to_srgb_unless_srgb_signaling_is_present() {
        let source = png_rgba(1, 1, &[GREY_128]);
        let linear =
            png_with_chunks_before_idat(&source, &[(*b"gAMA", 100_000_u32.to_be_bytes().to_vec())]);
        let srgb = png_with_chunks_before_idat(
            &source,
            &[
                (*b"gAMA", 100_000_u32.to_be_bytes().to_vec()),
                (*b"sRGB", vec![0]),
            ],
        );

        assert_eq!(
            only_pixel(&decode("image/png", &linear)),
            [188, 188, 188, 255]
        );
        assert_eq!(only_pixel(&decode("image/png", &srgb)), GREY_128);
    }

    #[test]
    fn cicp_takes_precedence_over_legacy_color_chunks() {
        let tagged = png_with_chunks_before_idat(
            &png_rgba(1, 1, &[GREY_128]),
            &[
                (*b"gAMA", 100_000_u32.to_be_bytes().to_vec()),
                (*b"cICP", vec![1, 13, 0, 1]),
            ],
        );

        assert_eq!(only_pixel(&decode("image/png", &tagged)), GREY_128);
    }

    #[test]
    fn hdr_cicp_is_tone_mapped_rather_than_read_as_srgb() {
        let pq = png_with_chunks_before_idat(
            &png_rgba(1, 1, &[GREY_128]),
            &[(*b"cICP", vec![9, 16, 0, 1])],
        );

        assert_eq!(only_pixel(&decode("image/png", &pq)), [132, 132, 132, 255]);
    }

    #[test]
    fn the_gain_map_base_image_is_the_standard_sdr_projection() {
        let tagged = png_with_chunks_before_idat(
            &png_rgba(1, 1, &[[64, 128, 192, 255]]),
            &[
                (*b"gmAP", iso_gain_map_metadata()),
                (*b"gdAT", png_rgba(1, 1, &[[255, 255, 255, 255]])),
            ],
        );

        assert_eq!(
            only_pixel(&decode("image/png", &tagged)),
            [64, 128, 192, 255]
        );
    }

    #[test]
    fn exif_orientation_is_applied_from_either_chunk_position() {
        let source = png_rgba(2, 1, &[RED, BLUE]);
        let exif = exif_orientation_tiff(6);
        for oriented in [
            png_with_chunks_before_idat(&source, &[(*b"eXIf", exif.clone())]),
            png_with_chunk_before_iend(&source, *b"eXIf", &exif),
        ] {
            let asset = decode("image/png", &oriented);
            assert_eq!((asset.width, asset.height), (1, 2));
            assert_eq!(pixels(&asset), [RED, BLUE]);
        }
    }

    #[test]
    fn the_direct_svg_target_keeps_source_dimensions_and_derives_no_pixels() {
        let oriented = png_with_chunks_before_idat(
            &png_rgba(2, 1, &[RED, BLUE]),
            &[(*b"eXIf", exif_orientation_tiff(6))],
        );

        let resolution = resolve_for_svg("image/png", &oriented);

        assert_eq!((resolution.asset.width, resolution.asset.height), (2, 1));
        assert!(resolution.asset.rgba.is_none());
        assert!(resolution.asset.premultiplied_rgba.is_none());
        assert_eq!(*resolution.asset.source_bytes, oriented);
    }

    #[test]
    fn every_exif_orientation_maps_the_source_quadrants() {
        // Row-major 2x2 source: RED GREEN / BLUE YELLOW.
        let source = png_rgba(2, 2, &[RED, GREEN, BLUE, YELLOW]);
        let expected: [[[u8; 4]; 4]; 8] = [
            [RED, GREEN, BLUE, YELLOW],
            [GREEN, RED, YELLOW, BLUE],
            [YELLOW, BLUE, GREEN, RED],
            [BLUE, YELLOW, RED, GREEN],
            [RED, BLUE, GREEN, YELLOW],
            [BLUE, RED, YELLOW, GREEN],
            [YELLOW, GREEN, BLUE, RED],
            [GREEN, YELLOW, RED, BLUE],
        ];
        for (orientation, expected) in (1..=8).zip(expected) {
            let oriented = png_with_chunks_before_idat(
                &source,
                &[(*b"eXIf", exif_orientation_tiff(orientation))],
            );
            let asset = decode("image/png", &oriented);
            assert_eq!(pixels(&asset), expected, "orientation {orientation}");
        }
    }

    #[test]
    fn an_apng_flattens_to_frame_zero_and_reports_animation() {
        let resolution = resolve_for_png("image/png", &apng(&[RED], &[BLUE]));

        assert!(resolution.animated);
        assert_eq!(only_pixel(&resolution.asset), RED);
    }

    #[test]
    fn a_one_frame_actl_is_not_an_animation() {
        let apng = png_with_chunks_before_idat(
            &png_rgba(1, 1, &[RED]),
            &[
                (
                    *b"acTL",
                    [1_u32.to_be_bytes(), 0_u32.to_be_bytes()].concat(),
                ),
                (*b"fcTL", apng_frame_control(0, 1, 1)),
            ],
        );

        assert!(!resolve_for_png("image/png", &apng).animated);
    }

    #[test]
    fn animation_frame_zero_wins_over_a_separate_default_image() {
        let resolution = resolve_for_png("image/png", &apng_with_separate_default(false));

        assert!(!resolution.animated);
        assert_eq!(only_pixel(&resolution.asset), BLUE);
    }

    #[test]
    fn corrupt_image_data_behind_a_valid_crc_is_a_decode_failure() {
        let mut png = png_rgba(1, 1, &[RED]);
        mutate_first_png_chunk(&mut png, *b"IDAT", |data| data[0] ^= 0xff);

        assert_eq!(failure("image/png", &png), ResolveError::DecodeFailed);
        assert_eq!(
            failure_for("image/png", &png, OutputFormat::Svg),
            ResolveError::DecodeFailed
        );
    }

    #[test]
    fn every_apng_frame_is_validated_before_svg_passthrough() {
        let mut apng = apng(&[RED], &[BLUE]);
        mutate_first_png_chunk(&mut apng, *b"fdAT", |data| data[4] ^= 0xff);

        assert_eq!(
            failure_for("image/png", &apng, OutputFormat::Svg),
            ResolveError::DecodeFailed
        );
        assert_eq!(
            failure_for(
                "image/png",
                &apng_with_separate_default(true),
                OutputFormat::Svg
            ),
            ResolveError::DecodeFailed
        );
    }

    #[test]
    fn ihdr_must_be_the_first_chunk() {
        let malformed = png_with_chunk_before_ihdr(&png_rgba(1, 1, &[RED]));

        assert_eq!(failure("image/png", &malformed), ResolveError::DecodeFailed);
    }

    #[test]
    fn color_chunks_may_neither_split_nor_follow_image_data() {
        let source = png_rgba(1, 1, &[RED]);
        let split = png_with_chunk_splitting_idat(&source, *b"cICP", &[1, 13, 0, 1]);
        let after = png_with_chunk_before_iend(&source, *b"cICP", &[1, 13, 0, 1]);

        assert_eq!(failure("image/png", &split), ResolveError::DecodeFailed);
        assert_eq!(failure("image/png", &after), ResolveError::DecodeFailed);
    }
}

// --- JPEG ------------------------------------------------------------------

mod jpeg_codec {
    use super::*;

    fn assert_close(pixel: [u8; 4], expected: [u8; 3], tolerance: u8) {
        for (channel, (actual, wanted)) in pixel.iter().zip(expected).enumerate() {
            assert!(
                actual.abs_diff(wanted) <= tolerance,
                "channel {channel} of {pixel:?} is within {tolerance} of {expected:?}"
            );
        }
        assert_eq!(pixel[3], 255);
    }

    #[test]
    fn a_baseline_jpeg_decodes_to_opaque_srgb() {
        let resolution = resolve_for_png("image/jpeg", &jpeg_rgb(1, 1, &[JFIF_BLUE]));

        assert_eq!(resolution.media_kind, MediaKind::Jpeg);
        assert!(!resolution.animated);
        assert_eq!(resolution.asset.media_type, "image/jpeg");
        assert_close(only_pixel(&resolution.asset), JFIF_BLUE, 3);
    }

    #[test]
    fn the_sniffed_kind_wins_over_both_claims_which_are_kept_for_the_session() {
        let jpeg = jpeg_rgb(1, 1, &[JFIF_BLUE]);
        let resolution = resolve_scene(
            &file_record_scene_with_claims("image/png", "image/jfif", &jpeg),
            OutputFormat::Svg,
        )
        .expect("the JPEG validates whatever it is called");

        assert_eq!(resolution.media_kind, MediaKind::Jpeg);
        assert_eq!(resolution.asset.media_type, "image/jpeg");
        assert_eq!(resolution.claims.mime_type.as_deref(), Some("image/png"));
        assert_eq!(
            resolution.claims.data_url_media_type.as_deref(),
            Some("image/jfif")
        );
    }

    #[test]
    fn baseline_progressive_grayscale_and_raw_streams_decode_alike() {
        let baseline = jpeg_rgb(1, 1, &[[64, 64, 64]]);
        let raw = jpeg_without_app0(&baseline);

        for jpeg in [baseline, progressive_gray_jpeg(64), raw] {
            assert_grey_within(only_pixel(&decode("image/jpeg", &jpeg)), 64, 2);
        }
    }

    #[test]
    fn a_cmyk_jpeg_decodes_to_opaque_srgb() {
        // Full cyan and no other ink is cyan; full key alone is black.
        assert_eq!(
            only_pixel(&decode("image/jpeg", &cmyk_jpeg([255, 0, 0, 0]))),
            [0, 255, 255, 255]
        );
        assert_eq!(
            only_pixel(&decode("image/jpeg", &cmyk_jpeg([0, 0, 0, 255]))),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn an_embedded_icc_profile_takes_precedence_over_the_srgb_default() {
        let source = jpeg_rgb(1, 1, &[[128, 128, 128]]);
        let tagged = jpeg_with_icc_profile(&source, &linear_rgb_icc_profile());

        assert_grey_within(only_pixel(&decode("image/jpeg", &source)), 128, 2);
        assert_grey_within(only_pixel(&decode("image/jpeg", &tagged)), 188, 2);
    }

    #[test]
    fn malformed_multipart_icc_metadata_is_a_decode_failure() {
        let source = jpeg_rgb(1, 1, &[[128, 128, 128]]);
        let profile = linear_rgb_icc_profile();
        let mut missing_part = b"ICC_PROFILE\0\x01\x02".to_vec();
        missing_part.extend_from_slice(&profile);
        let missing_part = jpeg_with_app_segment(&source, 0xe2, &missing_part);
        let mut duplicate_part = b"ICC_PROFILE\0\x01\x01".to_vec();
        duplicate_part.extend_from_slice(&profile);
        let duplicate = jpeg_with_app_segment(&source, 0xe2, &duplicate_part);
        let duplicate = jpeg_with_app_segment(&duplicate, 0xe2, &duplicate_part);

        for malformed in [missing_part, duplicate] {
            assert_eq!(
                failure_for("image/jpeg", &malformed, OutputFormat::Svg),
                ResolveError::JpegDecodeFailed
            );
        }
    }

    #[test]
    fn exif_orientation_swaps_the_dimensions_and_turns_the_pixels() {
        // 2x1 red|blue turned by orientation 6 (90 degrees clockwise) reads
        // red over blue. The encoder keeps 4:4:4 chroma at quality 100, so
        // neighbouring pixels do not bleed into each other.
        let source = jpeg_rgb(2, 1, &[[240, 20, 20], [20, 20, 240]]);
        let asset = decode("image/jpeg", &jpeg_with_app1_exif(&source, 6));

        assert_eq!((asset.width, asset.height), (1, 2));
        let top = pixel(&asset, 0, 0);
        let bottom = pixel(&asset, 0, 1);
        assert!(top[0] > top[2], "top pixel: {top:?}");
        assert!(bottom[2] > bottom[0], "bottom pixel: {bottom:?}");
        let validated = resolve_for_svg("image/jpeg", &jpeg_with_app1_exif(&source, 6));
        assert_eq!((validated.asset.width, validated.asset.height), (2, 1));
    }

    #[test]
    fn a_signature_only_gain_map_index_is_not_a_valid_container() {
        let base = jpeg_rgb(1, 1, &[JFIF_BLUE]);
        let gain_map = jpeg_rgb(1, 1, &[[255, 255, 255]]);
        let xmp = b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta hdrgm:Version=\"1.0\"/>";
        let container = jpeg_with_app_segment(&base, 0xe2, b"MPF\0fixture-index");
        let mut container = jpeg_with_app_segment(&container, 0xe1, xmp);
        container.extend_from_slice(&gain_map);

        assert_eq!(
            failure_for("image/jpeg", &container, OutputFormat::Svg),
            ResolveError::JpegDecodeFailed
        );
    }

    #[test]
    fn unqualified_concatenated_streams_are_a_decode_failure() {
        let mut concatenated = jpeg_rgb(1, 1, &[JFIF_BLUE]);
        concatenated.extend_from_slice(&jpeg_rgb(1, 1, &[[255, 255, 255]]));

        assert_eq!(
            failure_for("image/jpeg", &concatenated, OutputFormat::Svg),
            ResolveError::JpegDecodeFailed
        );
    }

    #[test]
    fn truncated_and_malformed_streams_are_decode_failures() {
        let mut truncated = jpeg_rgb(1, 1, &[JFIF_BLUE]);
        truncated.truncate(truncated.len() - 2);
        let malformed_segment = b"\xff\xd8\xff\xe0\0\x01\xff\xd9".to_vec();

        for jpeg in [truncated, malformed_segment] {
            assert_eq!(
                failure_for("image/jpeg", &jpeg, OutputFormat::Svg),
                ResolveError::JpegDecodeFailed
            );
        }
    }
}

// --- GIF -------------------------------------------------------------------

mod gif_codec {
    use ::gif::{DisposalMethod, Frame, Repeat};

    use super::*;

    #[test]
    fn a_single_frame_gif_decodes_its_palette_and_is_not_animated() {
        let gif = gif(&[gif_frame(&[0, 1], 2, 1, &RED_BLUE_PALETTE, None)]);
        let resolution = resolve_for_png("image/gif", &gif);

        assert_eq!(resolution.media_kind, MediaKind::Gif);
        assert!(!resolution.animated);
        assert_eq!(resolution.asset.media_type, "image/gif");
        assert_eq!(pixels(&resolution.asset), [RED, BLUE]);
    }

    #[test]
    fn the_transparent_index_and_frame_offset_compose_onto_a_clear_screen() {
        let mut first = gif_frame(&[1], 1, 1, &[0, 0, 255, 255, 0, 0], Some(0));
        first.left = 1;
        let gif = gif_with_screen(2, 1, &[], None, &[first]);

        let asset = decode("image/gif", &gif);

        assert_eq!((asset.width, asset.height), (2, 1));
        assert_eq!(pixels(&asset), [[0, 0, 0, 0], RED]);
    }

    #[test]
    fn a_gif87a_with_a_frame_local_palette_decodes() {
        let mut gif = gif(&[gif_frame(&[1], 1, 1, &[0, 255, 0, 255, 0, 255], None)]);
        let control = gif_graphic_control_offset(&gif);
        gif.drain(control..control + 8);
        gif[..6].copy_from_slice(b"GIF87a");

        assert_eq!(only_pixel(&decode("image/gif", &gif)), [255, 0, 255, 255]);
    }

    #[test]
    fn an_animated_gif_flattens_to_its_first_frame_and_reports_animation() {
        let gif = gif(&[
            gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None),
            gif_frame(&[1], 1, 1, &RED_BLUE_PALETTE, None),
        ]);
        let resolution = resolve_for_png("image/gif", &gif);

        assert!(resolution.animated);
        assert_eq!(only_pixel(&resolution.asset), RED);
    }

    #[test]
    fn loop_metadata_alone_does_not_make_a_gif_animated() {
        let gif = gif_with_screen(
            1,
            1,
            &[],
            Some(Repeat::Infinite),
            &[gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None)],
        );

        assert!(!resolve_for_png("image/gif", &gif).animated);
    }

    #[test]
    fn a_global_palette_interlacing_and_every_disposal_method_are_accepted() {
        let frames: Vec<Frame<'static>> = [
            DisposalMethod::Any,
            DisposalMethod::Keep,
            DisposalMethod::Background,
            DisposalMethod::Previous,
        ]
        .into_iter()
        .map(|dispose| {
            let mut frame = gif_global_palette_frame(&[0, 1, 1, 0], 2, 2);
            frame.dispose = dispose;
            frame.interlaced = true;
            frame
        })
        .collect();
        let gif = gif_with_screen(2, 2, &RED_BLUE_PALETTE, None, &frames);

        let resolution = resolve_for_png("image/gif", &gif);

        assert!(resolution.animated);
        assert_eq!(pixels(&resolution.asset), [RED, BLUE, BLUE, RED]);
        resolve_for_svg("image/gif", &gif);
    }

    #[test]
    fn every_frame_is_validated_before_svg_passthrough() {
        let mut gif = gif(&[
            gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None),
            gif_frame(&[1], 1, 1, &RED_BLUE_PALETTE, None),
        ]);
        gif.truncate(gif.len() - 3);

        assert_eq!(
            failure_for("image/gif", &gif, OutputFormat::Svg),
            ResolveError::GifDecodeFailed
        );
    }

    #[test]
    fn palette_references_outside_the_palette_and_misdeclared_extensions_are_rejected() {
        let mut invalid_background = gif_with_screen(
            1,
            1,
            &RED_BLUE_PALETTE,
            None,
            &[gif_global_palette_frame(&[0], 1, 1)],
        );
        invalid_background[11] = 2;

        let mut invalid_transparency = gif(&[gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, Some(0))]);
        let control = gif_graphic_control_offset(&invalid_transparency);
        invalid_transparency[control + 6] = 2;

        let mut malformed_application = gif_with_screen(
            1,
            1,
            &[],
            Some(Repeat::Infinite),
            &[gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None)],
        );
        let application = gif_application_extension_offset(&malformed_application);
        malformed_application[application + 2] = 10;

        for malformed in [
            invalid_background,
            invalid_transparency,
            malformed_application,
        ] {
            assert_eq!(
                failure_for("image/gif", &malformed, OutputFormat::Svg),
                ResolveError::GifDecodeFailed
            );
        }
    }

    #[test]
    fn a_reserved_disposal_method_is_rejected() {
        let mut gif = gif(&[gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None)]);
        let control = gif_graphic_control_offset(&gif);
        gif[control + 3] = 4 << 2;

        assert_eq!(
            failure_for("image/gif", &gif, OutputFormat::Svg),
            ResolveError::GifDecodeFailed
        );
    }

    #[test]
    fn reserved_image_descriptor_bits_are_rejected() {
        let mut gif = gif(&[gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None)]);
        let descriptor = gif_image_descriptor_offset(&gif);
        gif[descriptor + 9] |= 0x08;

        assert_eq!(
            failure_for("image/gif", &gif, OutputFormat::Svg),
            ResolveError::GifDecodeFailed
        );
    }
}

// --- WebP ------------------------------------------------------------------

mod webp_codec {
    use super::*;

    const TRANSLUCENT_BLUE: [u8; 4] = [24, 96, 208, 128];

    #[test]
    fn a_lossless_webp_keeps_its_alpha_straight() {
        let webp = webp_rgba(2, 1, &[TRANSLUCENT_BLUE, [200, 100, 50, 0]]);
        let resolution = resolve_for_png("image/webp", &webp);

        assert_eq!(resolution.media_kind, MediaKind::WebP);
        assert!(!resolution.animated);
        assert_eq!(resolution.asset.media_type, "image/webp");
        assert_eq!(pixels(&resolution.asset), [TRANSLUCENT_BLUE, [0, 0, 0, 0]]);
    }

    #[test]
    fn untransformed_webp_truncates_only_the_draw_time_copy() {
        let source = [[1, 128, 128, 1], [255, 129, 65, 128]];
        // The EXIF orientation is ignored, as the reference ignores it.
        let webp = webp_with_metadata(2, 1, &source, None, Some(exif_orientation_with_header(6)));
        let asset = decode("image/webp", &webp);
        assert_eq!((asset.width, asset.height), (2, 1));
        assert_eq!(pixels(&asset), source);
        assert_eq!(
            asset.premultiplied_rgba.as_deref().unwrap().as_slice(),
            &[0, 0, 0, 1, 128, 64, 32, 128]
        );
        // A real color transform remains on the established rounded path.
        let tagged = webp_with_metadata(
            1,
            1,
            &[[128, 128, 128, 1]],
            Some(linear_rgb_icc_profile()),
            None,
        );
        let asset = decode("image/webp", &tagged);
        let pixel = only_pixel(&asset);
        assert_eq!(pixel[3], 1);
        assert!(pixel[..3].iter().all(|c| c.abs_diff(188) <= 2));
        assert_eq!(
            asset.premultiplied_rgba.as_deref().unwrap().as_slice(),
            &[1, 1, 1, 1]
        );
    }

    #[test]
    fn a_lossy_webp_decodes_to_opaque_srgb() {
        let asset = decode("image/webp", &lossy_red_webp());

        assert_eq!((asset.width, asset.height), (2, 2));
        assert_eq!(pixels(&asset), [[255, 0, 2, 255]; 4]);
    }

    #[test]
    fn an_animated_webp_flattens_to_its_first_frame_and_reports_animation() {
        let webp = animated_webp(
            1,
            1,
            &[
                (0, 0, webp_rgba(1, 1, &[RED])),
                (0, 0, webp_rgba(1, 1, &[BLUE])),
            ],
        );
        let resolution = resolve_for_png("image/webp", &webp);

        assert!(resolution.animated);
        assert_eq!(only_pixel(&resolution.asset), RED);
    }

    #[test]
    fn the_first_animation_frame_keeps_its_alpha() {
        let webp = animated_webp(
            1,
            1,
            &[
                (0, 0, webp_rgba(1, 1, &[TRANSLUCENT_BLUE])),
                (0, 0, webp_rgba(1, 1, &[BLUE])),
            ],
        );
        let resolution = resolve_for_png("image/webp", &webp);

        assert!(resolution.animated);
        assert_eq!(only_pixel(&resolution.asset), TRANSLUCENT_BLUE);
    }

    #[test]
    fn exif_orientation_is_ignored_as_the_reference_ignores_it() {
        // Blink's WebP decoder never applies EXIF orientation.
        let webp = webp_with_metadata(
            4,
            1,
            &[RED, RED, BLUE, BLUE],
            None,
            Some(exif_orientation_with_header(6)),
        );

        let asset = decode("image/webp", &webp);
        assert_eq!((asset.width, asset.height), (4, 1));
        assert_eq!(pixels(&asset), [RED, RED, BLUE, BLUE]);

        let validated = resolve_for_svg("image/webp", &webp);
        assert_eq!((validated.asset.width, validated.asset.height), (4, 1));
        assert!(validated.asset.rgba.is_none());
    }

    #[test]
    fn an_embedded_icc_profile_takes_precedence_over_the_srgb_default() {
        let untagged = webp_rgba(1, 1, &[GREY_128]);
        let tagged = webp_with_metadata(1, 1, &[GREY_128], Some(linear_rgb_icc_profile()), None);

        assert_grey_within(only_pixel(&decode("image/webp", &untagged)), 128, 1);
        assert_grey_within(only_pixel(&decode("image/webp", &tagged)), 188, 2);
    }

    #[test]
    fn truncated_and_malformed_containers_are_decode_failures() {
        let mut truncated = webp_rgba(1, 1, &[[24, 96, 208, 255]]);
        truncated.truncate(truncated.len() - 2);
        let malformed = b"RIFF\x04\0\0\0WEBP".to_vec();

        for webp in [truncated, malformed] {
            assert_eq!(
                failure_for("image/webp", &webp, OutputFormat::Svg),
                ResolveError::WebPDecodeFailed
            );
        }
    }

    #[test]
    fn later_frames_outside_the_canvas_are_rejected_but_reserved_frame_bits_and_the_alpha_flag_are_not(
    ) {
        let animated = animated_webp(
            1,
            1,
            &[
                (0, 0, webp_rgba(1, 1, &[RED])),
                (0, 0, webp_rgba(1, 1, &[[0, 0, 255, 128]])),
            ],
        );
        let second_frame = webp_second_frame_payload_offset(&animated);

        let mut outside_canvas = animated.clone();
        outside_canvas[second_frame] = 1;
        assert_eq!(
            failure_for("image/webp", &outside_canvas, OutputFormat::Svg),
            ResolveError::WebPDecodeFailed
        );
        // libwebp's demuxer ignores reserved ANMF bits, and Blink draws an
        // animation whatever its VP8X alpha flag says.
        let mut reserved_frame_flag = animated.clone();
        reserved_frame_flag[second_frame + 15] |= 0x80;
        let mut missing_alpha_flag = animated;
        missing_alpha_flag[20] &= !0x10;
        for webp in [reserved_frame_flag, missing_alpha_flag] {
            let resolution = resolve_for_png("image/webp", &webp);
            assert!(resolution.animated);
            assert_eq!(only_pixel(&resolution.asset), RED);
        }
    }
}

// --- BMP -------------------------------------------------------------------

mod bmp_codec {
    use super::*;

    #[test]
    fn bottom_up_rows_come_out_top_down() {
        let resolution = resolve_for_png("image/bmp", &bmp24(1, 2, &[[255, 0, 0], [0, 0, 255]]));

        assert_eq!(resolution.media_kind, MediaKind::Bmp);
        assert!(!resolution.animated);
        assert_eq!(resolution.asset.media_type, "image/bmp");
        assert_eq!((resolution.asset.width, resolution.asset.height), (1, 2));
        assert_eq!(pixels(&resolution.asset), [RED, BLUE]);
    }

    #[test]
    fn legacy_bi_rgb_32_ignores_its_reserved_byte_while_alpha_bitfields_read_it() {
        let legacy = bmp32([24, 96, 208, 0], false);
        let bitfields = bmp32([24, 96, 208, 128], true);

        assert_eq!(
            only_pixel(&decode("image/bmp", &legacy)),
            [24, 96, 208, 255]
        );
        assert_eq!(
            only_pixel(&decode("image/bmp", &bitfields)),
            [24, 96, 208, 128]
        );
    }

    #[test]
    fn rle8_delta_skips_leave_palette_index_zero() {
        let asset = decode("image/bmp", &bmp_rle8());

        assert_eq!((asset.width, asset.height), (2, 1));
        assert_eq!(pixels(&asset), [RED, BLUE]);
    }

    #[test]
    fn alpha_bitfields_need_storage_for_the_alpha_mask() {
        assert_eq!(
            failure_for(
                "image/bmp",
                &bmp32_without_alpha_mask_storage(),
                OutputFormat::Svg
            ),
            ResolveError::BmpDecodeFailed
        );
    }

    #[test]
    fn a_truncated_bmp_is_a_decode_failure_under_its_sniffed_kind() {
        let mut bmp = bmp24(1, 1, &[[24, 96, 208]]);
        bmp.truncate(bmp.len() - 1);

        assert_eq!(
            failure_for("image/png", &bmp, OutputFormat::Svg),
            ResolveError::BmpDecodeFailed
        );
    }
}

// --- ICO -------------------------------------------------------------------

mod ico_codec {
    use super::*;

    #[test]
    fn the_largest_entry_is_selected_before_the_deepest() {
        let ico = ico(&[
            (1, 1, 32, solid_png(1, 1, BLUE)),
            (2, 1, 8, solid_png(2, 1, RED)),
        ]);
        let resolution = resolve_for_png("image/x-icon", &ico);

        assert_eq!(resolution.media_kind, MediaKind::Ico);
        assert_eq!(resolution.asset.media_type, "image/x-icon");
        assert_eq!((resolution.asset.width, resolution.asset.height), (2, 1));
        assert_eq!(pixels(&resolution.asset), [RED, RED]);
    }

    #[test]
    fn equal_areas_prefer_depth_and_exact_ties_the_first_directory_entry() {
        let ico = ico(&[
            (1, 1, 8, solid_png(1, 1, BLUE)),
            (1, 1, 32, solid_png(1, 1, RED)),
            (1, 1, 32, solid_png(1, 1, GREEN)),
        ]);

        assert_eq!(only_pixel(&decode("image/x-icon", &ico)), RED);
    }

    #[test]
    fn a_dib_entry_keeps_meaningful_alpha_and_falls_back_to_the_and_mask() {
        let translucent = ico(&[(1, 1, 32, dib32([24, 96, 208, 128], false))]);
        let masked = ico(&[(1, 1, 32, dib32([24, 96, 208, 0], true))]);

        assert_eq!(
            only_pixel(&decode("image/x-icon", &translucent)),
            [24, 96, 208, 128]
        );
        assert_eq!(only_pixel(&decode("image/x-icon", &masked)), [0, 0, 0, 0]);
    }

    #[test]
    fn the_selected_entry_sets_the_dimensions_for_the_svg_target_too() {
        let ico = ico(&[(2, 1, 32, solid_png(2, 1, RED))]);
        let resolution = resolve_for_svg("image/x-icon", &ico);

        assert_eq!((resolution.asset.width, resolution.asset.height), (2, 1));
        assert!(resolution.asset.rgba.is_none());
    }

    #[test]
    fn a_malformed_entry_fails_the_icon_even_when_it_is_not_selected() {
        let mut malformed = solid_png(1, 1, BLUE);
        let last = malformed.len() - 1;
        malformed[last] ^= 1;
        let ico = ico(&[(2, 1, 32, solid_png(2, 1, RED)), (1, 1, 8, malformed)]);

        assert_eq!(
            failure_for("image/x-icon", &ico, OutputFormat::Svg),
            ResolveError::IcoDecodeFailed
        );
    }

    #[test]
    fn overlapping_dib_bitfield_masks_are_rejected() {
        let ico = ico(&[(1, 1, 32, dib_with_overlapping_masks())]);

        assert_eq!(
            failure_for("image/x-icon", &ico, OutputFormat::Svg),
            ResolveError::IcoDecodeFailed
        );
    }
}

// --- Payload authority and sniffing ---------------------------------------

mod payload_authority {
    use super::*;

    /// Resolves the `asset` record under `authority` with `supplemental`
    /// beside `scene`, for the PNG target from a fresh cache.
    fn resolve_authority(
        scene: &SceneDocument,
        supplemental: &SupplementalAssets,
        authority: Authority,
    ) -> Result<Resolution, ResolutionFailure> {
        resolve_with(
            &mut Cache::default(),
            scene,
            supplemental,
            OutputFormat::Png,
            authority,
            |_| Ok(()),
        )
    }

    #[test]
    fn payloads_without_a_known_signature_are_unsupported() {
        let avif_ftyp = b"\x00\x00\x00\x1cftypavif\x00\x00\x00\x00avifmif1miaf";

        assert_eq!(failure("image/avif", avif_ftyp), ResolveError::Unsupported);
        assert_eq!(
            failure("image/png", b"not a png"),
            ResolveError::Unsupported
        );
    }

    #[test]
    fn a_supplemental_record_carries_only_its_advisory_media_type_as_a_claim() {
        let supplemental = SupplementalAssets::builder()
            .asset_with_media_type("asset", png_rgba(1, 1, &[RED]), "image/jpeg")
            .build()
            .expect("supplemental mapping is valid");

        let resolution = resolve_authority(&empty_scene(), &supplemental, Authority::Supplemental)
            .expect("the supplemental PNG decodes");

        assert_eq!(resolution.media_kind, MediaKind::Png);
        assert_eq!(resolution.claims.mime_type.as_deref(), Some("image/jpeg"));
        assert_eq!(resolution.claims.data_url_media_type, None);
        assert_eq!(only_pixel(&resolution.asset), RED);
    }

    #[test]
    fn a_missing_authority_or_record_is_missing() {
        let supplemental = SupplementalAssets::default();

        assert_eq!(
            asset_error(resolve_authority(
                &empty_scene(),
                &supplemental,
                Authority::Missing
            )),
            ResolveError::Missing
        );
        assert_eq!(
            asset_error(resolve_authority(
                &empty_scene(),
                &supplemental,
                Authority::Scene
            )),
            ResolveError::Missing
        );
        assert_eq!(
            asset_error(resolve_authority(
                &empty_scene(),
                &supplemental,
                Authority::Supplemental
            )),
            ResolveError::Missing
        );
    }

    #[test]
    fn malformed_payloads_and_forbidden_locations_have_distinct_recovery() {
        let null_record = SceneDocument::parse(
            br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{"asset":null}}"#,
        )
        .expect("null file record remains admitted scene content");
        let bad_base64 = SceneDocument::parse(
            br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{
                "asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,not-base64","created":1}
            }}"#,
        )
        .expect("scene is admitted");
        let no_scheme = SceneDocument::parse(
            br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{
                "asset":{"id":"asset","mimeType":"image/png","dataURL":"https://example.com/a.png","created":1}
            }}"#,
        )
        .expect("scene is admitted");

        for scene in [null_record, bad_base64] {
            assert_eq!(
                asset_error(resolve_authority(
                    &scene,
                    &SupplementalAssets::default(),
                    Authority::Scene
                )),
                ResolveError::Malformed
            );
        }
        let rejected =
            resolve_authority(&no_scheme, &SupplementalAssets::default(), Authority::Scene)
                .unwrap();
        assert_eq!(rejected.svg_failures.len(), 1);
        assert_eq!(rejected.svg_failures[0].error, ResolveError::Forbidden);
        assert!(
            !rejected.svg_failures[0].placeholder,
            "the cache-only call supplies no safe viewport"
        );
    }
}

// --- The transaction sink ----------------------------------------------------

mod transactions {
    //! The shape of what a resolution asks the meter for: which events are
    //! acquired, released and aborted, in what order, and that a cache hit
    //! asks for nothing. Events are named through `MediaKind`'s own accessors
    //! and no byte or work amount is pinned, so this holds the protocol stable
    //! without pinning the trace itself (ADR-0002).

    use super::*;

    /// One transaction as `(kind, event)`, so a sequence reads as a ledger.
    fn describe(transaction: &IntrinsicTransaction) -> (&'static str, &'static str) {
        match transaction {
            IntrinsicTransaction::Acquire { event, .. } => ("acquire", event),
            IntrinsicTransaction::ReleaseTemporary { event, .. } => ("release", event),
            IntrinsicTransaction::Abort { event, .. } => ("abort", event),
            IntrinsicTransaction::RejectLocalWork { event, .. } => ("reject", event),
            IntrinsicTransaction::PreflightNested { .. } => ("preflight", "nested"),
            IntrinsicTransaction::DiscoverNestedOccurrence { .. } => ("discover", "occurrence"),
            IntrinsicTransaction::DiscoverNestedPixels { .. } => ("discover", "pixels"),
        }
    }

    /// Resolves `scene` for `format` through `cache`, collecting every
    /// transaction the resolution issues.
    fn resolve_collecting(
        cache: &mut Cache,
        scene: &SceneDocument,
        format: OutputFormat,
    ) -> (
        Result<Resolution, ResolutionFailure>,
        Vec<(&'static str, &'static str)>,
    ) {
        let mut events = Vec::new();
        let result = resolve_with(
            cache,
            scene,
            &SupplementalAssets::default(),
            format,
            Authority::Scene,
            |transaction| {
                events.push(describe(&transaction));
                Ok(())
            },
        );
        (result, events)
    }

    /// The kinds of `events` in order, without their event names.
    fn kinds(events: &[(&'static str, &'static str)]) -> Vec<&'static str> {
        events.iter().map(|(kind, _)| *kind).collect()
    }

    /// The event names `events` acquired, in order.
    fn acquired(events: &[(&'static str, &'static str)]) -> Vec<&'static str> {
        events
            .iter()
            .filter(|(kind, _)| *kind == "acquire")
            .map(|(_, event)| *event)
            .collect()
    }

    #[test]
    fn the_direct_svg_target_acquires_only_the_validated_source() {
        let scene = file_record_scene("image/png", &png_rgba(1, 1, &[RED]));

        let (result, events) = resolve_collecting(&mut Cache::default(), &scene, OutputFormat::Svg);

        result.expect("the PNG validates");
        assert_eq!(kinds(&events), ["acquire", "release"]);
        assert_eq!(acquired(&events), [MediaKind::Png.source_event()]);
    }

    #[test]
    fn the_png_target_acquires_source_colour_and_decoded_pixels_in_turn() {
        let scene = file_record_scene("image/jpeg", &jpeg_rgb(1, 1, &[JFIF_BLUE]));

        let (result, events) = resolve_collecting(&mut Cache::default(), &scene, OutputFormat::Png);

        result.expect("the JPEG decodes");
        assert_eq!(
            kinds(&events),
            ["acquire", "release", "acquire", "release", "acquire", "release"]
        );
        assert_eq!(
            acquired(&events),
            [
                MediaKind::Jpeg.source_event(),
                MediaKind::Jpeg.color_event(),
                MediaKind::Jpeg.decoded_event(),
            ]
        );
    }

    #[test]
    fn identical_bytes_resolve_from_the_cache_without_a_transaction() {
        let mut cache = Cache::default();
        let scene = file_record_scene(
            "image/gif",
            &gif(&[gif_frame(&[0], 1, 1, &RED_BLUE_PALETTE, None)]),
        );

        let (first, _) = resolve_collecting(&mut cache, &scene, OutputFormat::Png);
        let (second, events) = resolve_collecting(&mut cache, &scene, OutputFormat::Png);

        let first = first.expect("the GIF decodes");
        let second = second.expect("the GIF resolves again");
        assert!(events.is_empty(), "cached resolution issued {events:?}");
        assert_eq!(first.asset.identity, second.asset.identity);
        assert!(Arc::ptr_eq(
            first.asset.rgba.as_ref().expect("first decode"),
            second.asset.rgba.as_ref().expect("second decode")
        ));
    }

    #[test]
    fn a_validation_failure_aborts_what_it_acquired() {
        let mut png = png_rgba(1, 1, &[RED]);
        mutate_first_png_chunk(&mut png, *b"IDAT", |data| data[0] ^= 0xff);
        let scene = file_record_scene("image/png", &png);

        let (result, events) = resolve_collecting(&mut Cache::default(), &scene, OutputFormat::Png);

        assert_eq!(asset_error(result), ResolveError::DecodeFailed);
        assert_eq!(
            events,
            [
                ("acquire", MediaKind::Png.source_event()),
                ("abort", MediaKind::Png.source_event()),
            ]
        );
    }

    #[test]
    fn a_header_that_does_not_parse_fails_before_anything_is_acquired() {
        let mut truncated = webp_rgba(1, 1, &[RED]);
        truncated.truncate(truncated.len() - 2);
        let scene = file_record_scene("image/webp", &truncated);

        let (result, events) = resolve_collecting(&mut Cache::default(), &scene, OutputFormat::Png);

        assert_eq!(asset_error(result), ResolveError::WebPDecodeFailed);
        assert!(events.is_empty(), "header failure issued {events:?}");
    }

    #[test]
    fn a_refused_transaction_stops_the_resolution_with_the_meter_error() {
        let scene = file_record_scene("image/bmp", &bmp24(1, 1, &[[24, 96, 208]]));
        let refuse = |_| {
            Err(RenderError {
                category: crate::RenderErrorCategory::ResourceLimit,
                code: crate::ErrorCode::REQUEST_INVALID_RESOURCE_LIMIT,
                message: "refused by the test".into(),
                format: OutputFormat::Png,
                element_id: None,
                json_pointer: "".into(),
                details: None,
                report: crate::CompatibilityReport::empty(),
            })
        };

        let result = resolve_with(
            &mut Cache::default(),
            &scene,
            &SupplementalAssets::default(),
            OutputFormat::Png,
            Authority::Scene,
            refuse,
        );

        match result {
            Err(ResolutionFailure::Transaction(error)) => {
                assert_eq!(error.message.as_ref(), "refused by the test");
            }
            Err(other) => panic!("the meter's refusal is passed through: {other:?}"),
            Ok(_) => panic!("the meter's refusal stops the resolution"),
        }
    }
}
