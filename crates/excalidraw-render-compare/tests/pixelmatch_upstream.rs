use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use excalidraw_render_compare::pixelmatch::{compare, Options};

#[test]
fn pixelmatch_matches_the_pinned_upstream_counts_and_diff_bytes() {
    let cases = [
        Case::new(
            "1a",
            "1b",
            "1diff",
            143,
            Options {
                threshold: 0.05,
                ..Options::default()
            },
        ),
        Case::new("1a", "1b", "1diffdefaultthreshold", 106, Options::default()),
        Case::new(
            "1a",
            "1b",
            "1diffmask",
            143,
            Options {
                threshold: 0.05,
                diff_mask: true,
                ..Options::default()
            },
        ),
        Case::new(
            "1a",
            "1a",
            "1emptydiffmask",
            0,
            Options {
                threshold: 0.0,
                diff_mask: true,
                ..Options::default()
            },
        ),
        Case::new(
            "2a",
            "2b",
            "2diff",
            12_437,
            Options {
                threshold: 0.05,
                alpha: 0.5,
                aa_color: [0, 192, 0],
                diff_color: [255, 0, 255],
                ..Options::default()
            },
        ),
        Case::new(
            "3a",
            "3b",
            "3diff",
            212,
            Options {
                threshold: 0.05,
                ..Options::default()
            },
        ),
        Case::new(
            "4a",
            "4b",
            "4diff",
            36_049,
            Options {
                threshold: 0.05,
                ..Options::default()
            },
        ),
        Case::new(
            "5a",
            "5b",
            "5diff",
            6,
            Options {
                threshold: 0.05,
                ..Options::default()
            },
        ),
        Case::new(
            "6a",
            "6b",
            "6diff",
            51,
            Options {
                threshold: 0.05,
                ..Options::default()
            },
        ),
        Case::new(
            "6a",
            "6a",
            "6empty",
            0,
            Options {
                threshold: 0.0,
                ..Options::default()
            },
        ),
        Case::new(
            "7a",
            "7b",
            "7diff",
            2_448,
            Options {
                diff_color_alt: Some([0, 255, 0]),
                ..Options::default()
            },
        ),
        Case::new(
            "8a",
            "5b",
            "8diff",
            32_896,
            Options {
                threshold: 0.05,
                ..Options::default()
            },
        ),
    ];

    for case in cases {
        let (width, height, first) = read_fixture(case.first);
        let (second_width, second_height, second) = read_fixture(case.second);
        assert_eq!(
            (second_width, second_height),
            (width, height),
            "{}",
            case.diff
        );
        let (_, _, expected_diff) = read_fixture(case.diff);

        let actual = compare(&first, &second, width, height, case.options).unwrap();
        assert_eq!(
            actual.mismatch_count(),
            case.mismatches,
            "{} count",
            case.diff
        );
        assert_eq!(actual.diff_rgba, expected_diff, "{} bytes", case.diff);
    }
}

#[derive(Clone, Copy)]
struct Case {
    first: &'static str,
    second: &'static str,
    diff: &'static str,
    mismatches: usize,
    options: Options,
}

impl Case {
    const fn new(
        first: &'static str,
        second: &'static str,
        diff: &'static str,
        mismatches: usize,
        options: Options,
    ) -> Self {
        Self {
            first,
            second,
            diff,
            mismatches,
            options,
        }
    }
}

fn read_fixture(name: &str) -> (u32, u32, Vec<u8>) {
    let path = fixture_root().join(format!("{name}.png"));
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut decoded = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut decoded).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "{}", path.display());
    decoded.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => decoded,
        png::ColorType::Rgb => decoded
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => decoded
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Grayscale => decoded
            .into_iter()
            .flat_map(|value| [value, value, value, 255])
            .collect(),
        png::ColorType::Indexed => panic!("{} remained indexed after expansion", path.display()),
    };
    (info.width, info.height, rgba)
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/comparator-vectors/pixelmatch-v7.2.0/fixtures")
}
