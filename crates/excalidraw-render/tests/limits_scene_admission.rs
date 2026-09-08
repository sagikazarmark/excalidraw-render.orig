//! The fixed parser ceilings that need a ceiling-sized input to cross: the
//! million-entry JSON structure ceiling, the 64 MiB input-byte ceiling and the
//! 256 MiB parser-memory ceiling, each inclusive at equality and rejected one
//! unit over. Building these scenes is what makes the tests cost seconds, so
//! they run in the render tier rather than the fast one.

mod common;

use common::parse_limits::assert_parse_limit;
use excalidraw_render::{ErrorCode, Measurement, SceneDocument, ValueKind};

const CANONICAL_EMPTY_SCENE: &[u8] =
    br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;

#[test]
fn fixed_structure_and_input_byte_boundaries_are_enforced() {
    const MAX_STRUCTURE_ENTRIES: usize = 1_048_576;
    const ENVELOPE_STRUCTURE_ENTRIES: usize = 11;

    let scene_at_structure_limit =
        scene_with_null_elements(MAX_STRUCTURE_ENTRIES - ENVELOPE_STRUCTURE_ENTRIES);
    SceneDocument::parse(scene_at_structure_limit.as_bytes())
        .expect("the fixed JSON structure ceiling is inclusive");

    let scene_over_structure_limit =
        scene_with_null_elements(MAX_STRUCTURE_ENTRIES - ENVELOPE_STRUCTURE_ENTRIES + 1);
    assert_parse_limit(
        SceneDocument::parse(scene_over_structure_limit.as_bytes())
            .expect_err("one structural entry over the ceiling is rejected"),
        ErrorCode::SCENE_JSON_STRUCTURE_EXCEEDED,
        Some("/elements/1048565"),
        "max_json_structure_entries",
        1_048_576,
        ValueKind::Count,
        Measurement::Attempted { value: 1_048_577 },
    );

    const MAX_INPUT_BYTES: usize = 67_108_864;
    let mut scene_at_input_limit = CANONICAL_EMPTY_SCENE.to_vec();
    scene_at_input_limit.resize(MAX_INPUT_BYTES, b' ');
    SceneDocument::parse(&scene_at_input_limit)
        .expect("the fixed parser input-byte ceiling is inclusive");

    scene_at_input_limit.push(b' ');
    assert_parse_limit(
        SceneDocument::parse(&scene_at_input_limit)
            .expect_err("one input byte over the ceiling is rejected"),
        ErrorCode::SCENE_INPUT_TOO_LARGE,
        None,
        "max_input_bytes",
        67_108_864,
        ValueKind::Bytes,
        Measurement::Attempted { value: 67_108_865 },
    );
}

#[test]
fn fixed_canonical_parser_memory_boundary_is_enforced() {
    // The retained number records plus the live and retained string records total exactly 256 MiB.
    let mut scene_at_memory_limit =
        scene_with_number_elements_and_future_string(720_000, 65_096_768);
    SceneDocument::parse(scene_at_memory_limit.as_bytes())
        .expect("the canonical parser live-byte ceiling is inclusive");

    scene_at_memory_limit.insert(scene_at_memory_limit.len() - 2, 'x');
    assert_parse_limit(
        SceneDocument::parse(scene_at_memory_limit.as_bytes())
            .expect_err("the first parser-memory allocation unit over the ceiling is rejected"),
        ErrorCode::SCENE_PARSE_MEMORY_EXCEEDED,
        Some("/future"),
        "max_parse_working_bytes",
        268_435_456,
        ValueKind::Bytes,
        Measurement::Attempted { value: 268_435_584 },
    );
}

fn scene_with_null_elements(element_count: usize) -> String {
    let mut scene = String::from(
        "{\"type\":\"excalidraw\",\"version\":2,\"appState\":{},\"files\":{},\"elements\":[",
    );
    for index in 0..element_count {
        if index != 0 {
            scene.push(',');
        }
        scene.push_str("null");
    }
    scene.push_str("]}");
    scene
}

fn scene_with_number_elements_and_future_string(
    element_count: usize,
    string_bytes: usize,
) -> String {
    let mut scene = String::with_capacity(element_count * 2 + string_bytes + 96);
    scene.push_str(
        "{\"type\":\"excalidraw\",\"version\":2,\"appState\":{},\"files\":{},\"elements\":[",
    );
    for index in 0..element_count {
        if index != 0 {
            scene.push(',');
        }
        scene.push('0');
    }
    scene.push_str("],\"future\":\"");
    scene.extend(std::iter::repeat_n('x', string_bytes));
    scene.push_str("\"}");
    scene
}
