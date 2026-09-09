//! Rendering is a pure function of the scene and the request: the same input
//! rendered twice, by two renderers, yields the same artifact bytes and the
//! same report values. One property test over every registered scene and
//! typography case and export variant, in every output format, holds this for
//! the corpus as a whole; the feature tests no longer each render twice to
//! prove it for their own scene. Each case manifest declares the fresh-process
//! runs its evidence was checked against (`determinism.fresh_process_runs`);
//! this is the in-process half of that claim, covering PDF as well, which has
//! no official evidence to compare against. The registry's one frame case is
//! exported through the frame API and has no export variants; its repeat
//! render stays in `parity_frame_export`.

use excalidraw_render::{OutputFormat, Renderer};

use crate::common::registered_cases::{
    case_count, judge_each, registered_variants_of_kinds, variant_request, RegisteredVariant,
};
use crate::common::repository_root;

#[test]
fn every_registered_case_renders_identically_twice_in_every_format() {
    let variants = registered_variants_of_kinds(&repository_root(), &["scene", "typography"]);
    let failures: Vec<String> = judge_each(&variants, divergent_repeat)
        .into_iter()
        .flatten()
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} registered cases did not render identically twice:\n  {}",
        failures.len(),
        case_count(&variants),
        failures.join("\n  ")
    );
}

/// Renders one variant twice per format with independent renderers and
/// reports the first format whose artifact or report moved, if any.
fn divergent_repeat(variant: &RegisteredVariant) -> Option<String> {
    let context = variant.context();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = variant_request(format, variant.options(), &context);
        let first = Renderer::new()
            .render(&variant.scene, &request)
            .unwrap_or_else(|error| panic!("{context} renders as {format:?}: {error:?}"));
        let second = Renderer::new()
            .render(&variant.scene, &request)
            .unwrap_or_else(|error| panic!("{context} renders as {format:?} again: {error:?}"));
        if first.artifact().bytes() != second.artifact().bytes() {
            return Some(format!(
                "{context}: {format:?} artifact bytes differ between renders"
            ));
        }
        if first.report() != second.report() {
            return Some(format!(
                "{context}: {format:?} report differs between renders"
            ));
        }
    }
    None
}
