use excalidraw_render_compare::pixelmatch::{compare, Options};

#[test]
fn pixelmatch_uses_the_pinned_checkerboard_alpha_composite() {
    let transparent_black = [0, 0, 0, 128];
    let opaque_gray = [127, 127, 127, 255];

    let checkerboard = compare(&transparent_black, &opaque_gray, 1, 1, Options::default()).unwrap();
    assert_eq!(checkerboard.changed_mask, [true]);

    let white = compare(
        &transparent_black,
        &opaque_gray,
        1,
        1,
        Options {
            checkerboard: false,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(white.changed_mask, [false]);
}

#[test]
fn pixelmatch_threshold_is_strict_at_the_binary64_boundary() {
    let black = [0, 0, 0, 255];
    let blue_three = [0, 0, 3, 255];
    let equality_threshold = f64::from_bits(0x3f6f_2368_1f0f_cfd3);

    let at_equality = compare(
        &black,
        &blue_three,
        1,
        1,
        Options {
            threshold: equality_threshold,
            include_aa: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(at_equality.changed_mask, [false]);

    let below = compare(
        &black,
        &blue_three,
        1,
        1,
        Options {
            threshold: f64::from_bits(equality_threshold.to_bits() - 1),
            include_aa: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(below.changed_mask, [true]);

    let above = compare(
        &black,
        &blue_three,
        1,
        1,
        Options {
            threshold: f64::from_bits(equality_threshold.to_bits() + 1),
            include_aa: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(above.changed_mask, [false]);
}

#[test]
fn pixelmatch_suppresses_antialiasing_only_after_color_comparison() {
    let mut first = Vec::new();
    let mut second = Vec::new();
    for y in 0..5 {
        for x in 0..5 {
            let value = match y {
                0 | 1 => 0,
                2 if x == 2 => 128,
                2 => 64,
                _ => 255,
            };
            first.extend_from_slice(&[value, value, value, 255]);
            let other = if x == 2 && y == 2 { 129 } else { value };
            second.extend_from_slice(&[other, other, other, 255]);
        }
    }

    let suppressed = compare(
        &first,
        &second,
        5,
        5,
        Options {
            threshold: 0.0,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(suppressed.mismatch_count(), 0);

    let included = compare(
        &first,
        &second,
        5,
        5,
        Options {
            threshold: 0.0,
            include_aa: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(included.mismatch_count(), 1);
    assert!(included.changed_mask[12]);
}

#[test]
fn pixelmatch_keeps_the_first_darkest_and_brightest_neighbors_on_ties() {
    let mut first = Vec::new();
    let mut second = Vec::new();
    for y in 0..3 {
        for x in 0..3 {
            let value = match y {
                0 => 0,
                1 if x == 1 => 128,
                1 => 64,
                _ => 255,
            };
            first.extend_from_slice(&[value, value, value, 255]);
            let other = if x == 1 && y == 1 { 129 } else { value };
            second.extend_from_slice(&[other, other, other, 255]);
        }
    }

    let result = compare(
        &first,
        &second,
        3,
        3,
        Options {
            threshold: 0.0,
            ..Options::default()
        },
    )
    .unwrap();

    // x-outer/y-inner traversal retains the tied corner extrema. Their clipped
    // neighborhoods lack enough equal siblings, so the center is not AA.
    assert_eq!(result.mismatch_count(), 1);
    assert!(result.changed_mask[4]);
}

#[test]
fn pixelmatch_checkerboard_uses_ecmascript_position_arithmetic() {
    let first = [0, 0, 0, 64].repeat(8);
    let second = [0, 0, 0, 128].repeat(8);

    let result = compare(
        &first,
        &second,
        8,
        1,
        Options {
            include_aa: true,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(
        result.changed_mask,
        [false, false, false, true, true, false, false, true]
    );
}

#[test]
fn pixelmatch_counts_exact_equal_luma_chroma_differences_as_equal_siblings() {
    const DARK: [u8; 4] = [0, 0, 0, 255];
    const LIGHT: [u8; 4] = [255, 255, 255, 255];
    const CENTER: [u8; 4] = [3, 11, 64, 20];
    const EQUAL_LUMA: [u8; 4] = [35, 68, 8, 69];
    let first_pixels = [
        DARK, DARK, EQUAL_LUMA, LIGHT, DARK, CENTER, EQUAL_LUMA, LIGHT, LIGHT, LIGHT, EQUAL_LUMA,
        LIGHT,
    ];
    let second_pixels = [
        DARK, DARK, EQUAL_LUMA, LIGHT, DARK, EQUAL_LUMA, EQUAL_LUMA, LIGHT, LIGHT, LIGHT,
        EQUAL_LUMA, LIGHT,
    ];
    let first: Vec<u8> = first_pixels.into_iter().flatten().collect();
    let second: Vec<u8> = second_pixels.into_iter().flatten().collect();

    let result = compare(
        &first,
        &second,
        4,
        3,
        Options {
            threshold: 0.0,
            diff_mask: true,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(result.mismatch_count(), 1);
    assert_eq!(
        result.changed_mask,
        [false, false, false, false, false, true, false, false, false, false, false, false]
    );
}
