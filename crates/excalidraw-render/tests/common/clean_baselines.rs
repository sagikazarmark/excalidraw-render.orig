//! Memoized clean baselines for the field-contract matrices. A matrix cell
//! compares every recovered value of a field against one clean scene, so the
//! clean scene and its frames render once per test thread rather than once per
//! value. Only the clean side is shared: every source value still renders fresh.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use excalidraw_render::{
    FrameRenderRequest, OutputFormat, RenderError, RenderOutcome, RenderRequest, Renderer,
    SceneDocument,
};
use serde_json::Value;

/// A clean scene rendered with the default request, and each of its frames,
/// in source order, with the default frame request. A render that fails is
/// kept as its error, so only the callers that compare against it fail.
pub struct Baseline {
    scene: Result<RenderOutcome, RenderError>,
    frames: Vec<Result<RenderOutcome, RenderError>>,
}

impl Baseline {
    pub fn scene(&self) -> &RenderOutcome {
        self.scene.as_ref().unwrap()
    }

    pub fn frames(&self) -> impl Iterator<Item = &RenderOutcome> {
        self.frames.iter().map(|frame| frame.as_ref().unwrap())
    }

    pub fn last_frame(&self) -> &RenderOutcome {
        self.frames.last().unwrap().as_ref().unwrap()
    }
}

/// Clean scene bytes and the format they render to.
type Key = (Vec<u8>, OutputFormat);

thread_local! {
    static BASELINES: RefCell<HashMap<Key, Rc<Baseline>>> = RefCell::default();
}

pub fn baseline(clean: &Value, format: OutputFormat) -> Rc<Baseline> {
    let key = (serde_json::to_vec(clean).unwrap(), format);
    if let Some(baseline) = BASELINES.with_borrow(|cache| cache.get(&key).cloned()) {
        return baseline;
    }
    let document = SceneDocument::parse(&key.0).unwrap();
    let renderer = Renderer::new();
    let request = RenderRequest::builder(format).build().unwrap();
    let frame_request = FrameRenderRequest::builder(format).build().unwrap();
    let baseline = Rc::new(Baseline {
        scene: renderer.render(&document, &request),
        frames: document
            .frames()
            .map(|frame| renderer.render_frame(frame, &frame_request))
            .collect(),
    });
    BASELINES.with_borrow_mut(|cache| cache.insert(key, Rc::clone(&baseline)));
    baseline
}
