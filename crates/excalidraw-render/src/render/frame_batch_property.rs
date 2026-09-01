//! Batch-versus-standalone frame equivalence.
//!
//! Rendering one frame through a shared [`PreparedFrameRender`] must produce
//! the same artifact bytes, the same compatibility report, and the same
//! canonical resource trace as rendering it with no batch preparation at all.
//! That equivalence is what lets the batch path replay a journal of
//! normalization and relationship events instead of re-deriving them, and it is
//! the property any simplification of that replay has to keep.
//!
//! Unlike a fixed scene list, the corpus here is generated: scenes vary in
//! frame count, element mix, placement class, and text demand, and each scene
//! is exercised against resource limits probed at the `N-1`, `N`, and `N+1`
//! boundaries of the values it actually consumes. Near-limit scenes matter
//! because `resource.*-exceeded` diagnostics are embedded in the artifact, so a
//! change in *when* a charge lands can change output bytes even when the total
//! charge is unchanged.

use super::*;
use crate::{FrameRenderRequest, ResourceLimits};

const FIXED_SEED: u64 = 0x2545_f491_4f6c_dd1d;
/// Kept small enough to gate every pull request; `nightly_frame_batch_property`
/// runs the same generator far deeper.
const PR_CASES: usize = 16;
const MAX_FRAMES: usize = 4;
const MAX_ELEMENTS: usize = 12;

/// Deterministic xorshift generator; the corpus must be reproducible on every
/// target, so no environment or clock input is admitted.
struct Generator(u64);

impl Generator {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound.max(1)
    }

    fn coordinate(&mut self, span: f64) -> f64 {
        f64::from(u32::try_from(self.below(1_000)).unwrap_or(0)) / 1_000.0 * span
    }
}

fn assert_generated_cases(seed: u64, cases: usize) {
    let mut generator = Generator(seed);
    for case in 0..cases {
        let source = generate_scene(&mut generator);
        let id = format!("seed {seed:016x} case {case}");
        let scene = SceneDocument::parse(source.as_bytes())
            .unwrap_or_else(|error| panic!("{id} is admissible: {error:?}\n{source}"));
        let frames = scene.frames().collect::<Vec<_>>();
        if frames.is_empty() {
            continue;
        }
        for format in [OutputFormat::Svg, OutputFormat::Png] {
            for limits in limit_ladder(format, &frames, &id) {
                assert_batch_matches_standalone(&scene, &frames, format, limits, &id);
            }
        }
    }
}

/// Renders every frame twice — once from a shared batch preparation, once cold
/// — and requires the two to be indistinguishable in artifact, report, and
/// trace, whether they succeed or fail.
fn assert_batch_matches_standalone(
    scene: &SceneDocument,
    frames: &[crate::FrameRef<'_>],
    format: OutputFormat,
    limits: ResourceLimits,
    id: &str,
) {
    let request = FrameRenderRequest::builder(format)
        .resource_limits(limits)
        .build()
        .unwrap_or_else(|error| panic!("{id} frame request is valid: {error:?}"));
    let prepared = PreparedFrameRender::new(
        crate::catalog::REVISION_1.frame_profile(),
        crate::catalog::REVISION_1.scene_surface(),
        scene,
        request.as_render_request(),
    );
    let Ok(prepared) = prepared else {
        // Preparation failure is a batch-only outcome with no standalone
        // counterpart; the batch API surfaces it before any frame renders.
        return;
    };

    for frame in frames {
        let (batch, batch_trace) = render_frame_with_trace(
            &crate::catalog::REVISION_1,
            *frame,
            &request,
            Some(&prepared),
        );
        let (cold, cold_trace) =
            render_frame_with_trace(&crate::catalog::REVISION_1, *frame, &request, None);
        let ordinal = frame.ordinal();
        match (batch, cold) {
            (Ok(batch), Ok(cold)) => {
                assert_eq!(
                    batch.artifact().bytes(),
                    cold.artifact().bytes(),
                    "{id} frame {ordinal} {format:?} artifact bytes diverge"
                );
                assert_eq!(
                    batch.report(),
                    cold.report(),
                    "{id} frame {ordinal} {format:?} report diverges"
                );
            }
            (Err(batch), Err(cold)) => {
                assert_eq!(
                    batch.code(),
                    cold.code(),
                    "{id} frame {ordinal} {format:?} failure code diverges"
                );
                // The record a crossing lands on is contract-visible, so the
                // first crossing must be identical and not merely the same
                // code: a charge that moves earlier or later would change it.
                assert_eq!(
                    (batch.element_id(), batch.json_pointer()),
                    (cold.element_id(), cold.json_pointer()),
                    "{id} frame {ordinal} {format:?} first crossing location diverges"
                );
                assert_eq!(
                    batch.report(),
                    cold.report(),
                    "{id} frame {ordinal} {format:?} failure report diverges"
                );
            }
            (batch, cold) => panic!(
                "{id} frame {ordinal} {format:?} outcome kind diverges: batch {:?}, cold {:?}",
                batch.map(|outcome| outcome.artifact().bytes().len()).err(),
                cold.map(|outcome| outcome.artifact().bytes().len()).err(),
            ),
        }
        assert_eq!(
            batch_trace, cold_trace,
            "{id} frame {ordinal} {format:?} canonical resource trace diverges"
        );
    }
}

/// Probes what the first frame actually consumes under default limits, then
/// returns limits sitting exactly on, just under, and just over those values.
/// A scene whose measurements cannot be observed contributes only the default
/// profile.
fn limit_ladder(
    format: OutputFormat,
    frames: &[crate::FrameRef<'_>],
    id: &str,
) -> Vec<ResourceLimits> {
    let mut ladder = vec![ResourceLimits::default()];
    let probe = FrameRenderRequest::builder(format)
        .build()
        .unwrap_or_else(|error| panic!("{id} probe request is valid: {error:?}"));
    let (outcome, trace) =
        render_frame_with_trace(&crate::catalog::REVISION_1, frames[0], &probe, None);
    let Ok(outcome) = outcome else {
        return ladder;
    };
    let output_bytes = outcome.artifact().bytes().len() as u64;
    let working_bytes = peak_working_bytes(&trace);

    for boundary in [
        output_bytes.saturating_sub(1),
        output_bytes,
        output_bytes + 1,
    ] {
        if let Ok(limits) = ResourceLimits::builder()
            .max_output_bytes(boundary.max(1))
            .build()
        {
            ladder.push(limits);
        }
    }
    if let Some(working_bytes) = working_bytes {
        for boundary in [
            working_bytes.saturating_sub(1),
            working_bytes,
            working_bytes + 1,
        ] {
            if let Ok(limits) = ResourceLimits::builder()
                .max_working_bytes(boundary.max(1))
                .build()
            {
                ladder.push(limits);
            }
        }
    }
    ladder
}

/// Reads the highest committed `working_bytes` value out of a canonical trace.
fn peak_working_bytes(trace: &[u8]) -> Option<u64> {
    let trace: serde_json::Value = serde_json::from_slice(trace).ok()?;
    trace["entries"]
        .as_array()?
        .iter()
        .filter_map(|entry| entry["after"]["working_bytes"].as_u64())
        .max()
}

/// Builds one scene of frames plus elements distributed over the placement
/// classes that frame selection distinguishes: inside, crossing, outside, and
/// bound to a frame by `frameId`.
fn generate_scene(generator: &mut Generator) -> String {
    let frame_count = 1 + generator.below(MAX_FRAMES as u64) as usize;
    let mut elements = String::new();
    let mut updated = 0_u32;
    for index in 0..frame_count {
        let x = 200.0 * index as f64;
        let kind = if generator.below(4) == 0 {
            "magicframe"
        } else {
            "frame"
        };
        let name = match generator.below(3) {
            0 => "null".to_owned(),
            1 => format!("\"Frame {index}\""),
            _ => "\"\"".to_owned(),
        };
        updated += 1;
        push_element(
            &mut elements,
            &format!(
                r#"{{"id":"frame-{index}","type":"{kind}","name":{name},"x":{x},"y":0,"width":160,"height":120,"updated":{updated}}}"#
            ),
        );
    }

    let element_count = generator.below(MAX_ELEMENTS as u64 + 1) as usize;
    for index in 0..element_count {
        let owner = generator.below(frame_count as u64) as usize;
        let origin = 200.0 * owner as f64;
        let (x, y) = match generator.below(4) {
            // Inside the frame box.
            0 => (origin + 10.0, 10.0),
            // Crossing the left edge.
            1 => (origin - 20.0, generator.coordinate(100.0)),
            // Entirely outside every frame.
            2 => (origin + 400.0, 400.0),
            // Randomly placed near the frame.
            _ => (
                origin + generator.coordinate(180.0),
                generator.coordinate(140.0),
            ),
        };
        let frame_id = if generator.below(2) == 0 {
            format!(r#","frameId":"frame-{owner}""#)
        } else {
            String::new()
        };
        updated += 1;
        let element = match generator.below(4) {
            0 => format!(
                r#"{{"id":"rect-{index}","type":"rectangle","x":{x},"y":{y},"width":40,"height":30,"updated":{updated}{frame_id}}}"#
            ),
            1 => format!(
                r#"{{"id":"ellipse-{index}","type":"ellipse","x":{x},"y":{y},"width":50,"height":20,"updated":{updated}{frame_id}}}"#
            ),
            2 => format!(
                r#"{{"id":"text-{index}","type":"text","x":{x},"y":{y},"width":80,"height":25,"fontSize":20,"fontFamily":{},"text":{},"updated":{updated}{frame_id}}}"#,
                1 + generator.below(6),
                text_literal(generator),
            ),
            _ => format!(
                r#"{{"id":"diamond-{index}","type":"diamond","x":{x},"y":{y},"width":36,"height":36,"updated":{updated}{frame_id}}}"#
            ),
        };
        push_element(&mut elements, &element);
    }

    format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{elements}],"appState":{{}},"files":{{}}}}"#
    )
}

/// Text demand drives font subsetting, which is the costliest charge a frame
/// makes; the alternatives span Latin, CJK, and emoji chunk selection.
fn text_literal(generator: &mut Generator) -> &'static str {
    match generator.below(5) {
        0 => "\"A\"",
        1 => "\"Roadmap\"",
        2 => "\"\\u4e2d\\u6587\"",
        3 => "\"\\ud83d\\ude00\"",
        _ => "\"One two\\nthree\"",
    }
}

fn push_element(elements: &mut String, element: &str) {
    if !elements.is_empty() {
        elements.push(',');
    }
    elements.push_str(element);
}

#[test]
fn frame_batch_rendering_matches_standalone_around_resource_limits() {
    assert_generated_cases(FIXED_SEED, PR_CASES);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "scheduled deterministic frame batch expansion"]
fn nightly_frame_batch_property_is_exhaustive() {
    let cases = std::env::var("EXCALIDRAW_FRAME_BATCH_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2_048)
        .min(16_384);
    assert_generated_cases(FIXED_SEED, cases);
}
