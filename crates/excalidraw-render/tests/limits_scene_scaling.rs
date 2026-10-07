//! Scene shapes that a per-element lookup by linear scan turns quadratic: many
//! long identities sharing a prefix and repeating late, many containers with
//! bound text, and one text of a million distinct scalars. Each scene is sized
//! so the linear pipeline renders it in seconds while the quadratic lookups
//! these tests guard took minutes, and each test asserts the render finishes
//! inside a ceiling between the two. A ceiling leaves the linear render several
//! times its own duration, so contention does not fail it: this is a
//! regression guard, not a benchmark. Building these scenes is what makes the
//! tests cost seconds, so they run in the render tier rather than the fast one.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use excalidraw_render::{
    DiagnosticCode, ErrorCode, OutputFormat, RenderError, RenderOutcome, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};

#[test]
fn long_shared_prefix_identities_and_late_duplicates_render_in_linear_time() {
    // 49,152 rectangles with 1,200-byte identities that differ only at their
    // end, so every equality comparison reads the whole shared prefix. The
    // second third repeats the last identity of the first two thirds, so each
    // duplicate's first occurrence is as late as the scene allows. Comparing
    // each identity against every earlier one took over a minute.
    const COUNT: usize = 49_152;
    const UNIQUE: usize = COUNT / 3 * 2;
    let prefix = "A".repeat(1_200);
    let scene = scene(COUNT, |scene, index| {
        let id = format!("{prefix}{}", index.min(UNIQUE - 1));
        push_element(scene, &id, "rectangle", index, "");
    });

    let outcome = render_within(&scene, Duration::from_secs(20)).expect("the scene renders");
    let duplicates = outcome
        .report()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code() == DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID
        })
        .count();
    assert_eq!(duplicates, COUNT - UNIQUE);
}

#[test]
fn many_bound_texts_render_in_linear_time() {
    // 22,528 rectangles, each bound to its own text label: as many as the
    // fixed JSON structure ceiling admits. Finding each label's container and
    // its frame by scanning the relationship graph took over a minute.
    const PAIRS: usize = 22_528;
    let scene = scene(PAIRS * 2, |scene, index| {
        let pair = index / 2;
        if index % 2 == 0 {
            push_element(
                scene,
                &format!("c{pair}"),
                "rectangle",
                index,
                &format!(r#","boundElements":[{{"id":"t{pair}","type":"text"}}]"#),
            );
        } else {
            push_element(
                scene,
                &format!("t{pair}"),
                "text",
                index,
                &format!(r#","text":"a","containerId":"c{pair}""#),
            );
        }
    });

    let outcome = render_within(&scene, Duration::from_secs(20)).expect("the scene renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(svg.matches("<text").count(), PAIRS);
}

#[test]
fn a_text_of_a_million_distinct_scalars_resolves_in_linear_time() {
    // One text element of every scalar from U+0020 up, a million of them,
    // each resolved to a face once. Its glyph outlines exceed the total
    // geometry limit, which is only known once every scalar is resolved.
    // Looking each scalar up in a list of the ones resolved before it took
    // three minutes.
    const SCALARS: usize = 1_000_000;
    let mut text = String::new();
    for character in (0x20..=u32::from(char::MAX))
        .filter_map(char::from_u32)
        .take(SCALARS)
    {
        if matches!(character, '"' | '\\') {
            text.push('\\');
        }
        text.push(character);
    }
    let scene = scene(1, |scene, index| {
        push_element(
            scene,
            "text",
            "text",
            index,
            &format!(r#","text":"{text}","fontSize":20,"fontFamily":1"#),
        );
    });

    let error = render_within(&scene, Duration::from_secs(60))
        .expect_err("the text's outlines exceed the total geometry limit");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
}

/// Admits and renders `scene` to SVG, asserting both finish inside `ceiling`.
/// The scene size limits are raised to their maxima so a scene can be large
/// enough for a quadratic lookup to show.
fn render_within(scene: &str, ceiling: Duration) -> Result<RenderOutcome, RenderError> {
    let limits = ResourceLimits::builder()
        .max_scene_bytes(67_108_864)
        .max_elements(65_536)
        .max_text_scalars_per_element(1_048_576)
        .build()
        .expect("the limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("the request is valid");
    let started = Instant::now();
    let scene = SceneDocument::parse(scene.as_bytes()).expect("the scene is admitted");
    let result = Renderer::new().render(&scene, &request);
    let elapsed = started.elapsed();
    assert!(
        elapsed < ceiling,
        "rendering took {elapsed:?}, over the {ceiling:?} ceiling: \
         a per-element lookup has likely turned quadratic"
    );
    result
}

fn scene(count: usize, mut push: impl FnMut(&mut String, usize)) -> String {
    let mut scene =
        String::from(r#"{"type":"excalidraw","version":2,"appState":{},"files":{},"elements":["#);
    for index in 0..count {
        if index != 0 {
            scene.push(',');
        }
        push(&mut scene, index);
    }
    scene.push_str("]}");
    scene
}

/// One element record on a 128-wide grid of 10-unit cells. Only the members
/// that would otherwise be repaired with a diagnostic are spelled out, which
/// keeps the larger scenes inside the fixed JSON structure ceiling.
fn push_element(scene: &mut String, id: &str, kind: &str, index: usize, fields: &str) {
    let (x, y) = ((index % 128) * 10, (index / 128) * 10);
    write!(
        scene,
        r#"{{"id":"{id}","type":"{kind}","x":{x},"y":{y},"width":8,"height":8,"seed":1,"updated":1{fields}}}"#
    )
    .expect("writing to a String cannot fail");
}
