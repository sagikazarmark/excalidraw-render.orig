//! Rendering a scene the way most tests want it: the default request for one
//! format, a panic naming the format when the render is refused. A test with a
//! non-default request builds it and hands it to [`render_with_request`].

use excalidraw_render::{
    ErrorCode, ErrorDetails, OutputFormat, RenderOutcome, RenderRequest, RenderRequestBuilder,
    Renderer, ResourceLimits, ResourceMeasurement, ResourceValue, SceneDocument,
};

/// Renders `scene` with the default request for `format`.
pub fn render(scene: &SceneDocument, format: OutputFormat) -> RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("default request is valid");
    render_with_request(scene, &request)
}

/// Parses `source` and renders it with the default request for `format`.
pub fn render_source(source: &[u8], format: OutputFormat) -> RenderOutcome {
    let scene = SceneDocument::parse(source).expect("scene is admitted");
    render(&scene, format)
}

/// Renders `scene` with the default request for `format` minus the background
/// fill, so unpainted pixels stay transparent.
pub fn render_without_background(scene: &SceneDocument, format: OutputFormat) -> RenderOutcome {
    let request = RenderRequest::builder(format)
        .include_background(false)
        .build()
        .expect("transparent request is valid");
    render_with_request(scene, &request)
}

/// Renders `scene` with `request`, refusing to continue when the render fails.
pub fn render_with_request(scene: &SceneDocument, request: &RenderRequest) -> RenderOutcome {
    Renderer::new()
        .render(scene, request)
        .unwrap_or_else(|error| panic!("{:?} render succeeds: {error:?}", request.format()))
}

/// The artifact bytes of a default render of `scene` that raised no diagnostic.
pub fn render_artifact(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    artifact_without_diagnostics(&render(scene, format))
}

/// The artifact bytes of a background-free render of `scene` that raised no
/// diagnostic.
pub fn render_artifact_without_background(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    artifact_without_diagnostics(&render_without_background(scene, format))
}

/// The artifact bytes of an outcome that raised no diagnostic.
pub fn artifact_without_diagnostics(outcome: &RenderOutcome) -> Vec<u8> {
    assert!(
        outcome.report().is_empty(),
        "unexpected diagnostics: {:?}",
        outcome.report().diagnostics()
    );
    outcome.artifact().bytes().to_vec()
}

/// The smallest `max_total_work_units` at which `request_builder`'s request
/// renders `scene` with the report it has under default limits: the scene's
/// canonical total work. Each per-item work ceiling is lowered with the
/// aggregate it must fit inside, so below the boundary the render either fails
/// on the aggregate or omits something locally and reports it, and at or
/// above it neither happens; the predicate is monotone and a bisection settles
/// on the exact value. The render one unit below must be the fatal
/// `resource.total-work-exceeded` attempting exactly the boundary, which the
/// probe checks before returning.
pub fn total_work_boundary(
    scene: &SceneDocument,
    request_builder: impl Fn() -> RenderRequestBuilder,
) -> u64 {
    let baseline = render_with_request(
        scene,
        &request_builder().build().expect("probe request is valid"),
    );
    let clean = |total: u64| {
        Renderer::new()
            .render(scene, &work_probe_request(request_builder(), total))
            .is_ok_and(|outcome| outcome.report() == baseline.report())
    };
    let (mut refused, mut admitted) = (0, ResourceLimits::default().max_total_work_units());
    assert!(clean(admitted), "the scene renders under default work");
    while admitted - refused > 1 {
        let probe = refused + (admitted - refused) / 2;
        if clean(probe) {
            admitted = probe;
        } else {
            refused = probe;
        }
    }
    let error = Renderer::new()
        .render(scene, &work_probe_request(request_builder(), admitted - 1))
        .expect_err("one unit below the total-work boundary is fatal");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED);
    match error.details() {
        Some(ErrorDetails::ResourceLimit {
            field,
            measurement:
                ResourceMeasurement::Attempted {
                    value: ResourceValue::Integer(attempted),
                },
            ..
        }) => {
            assert_eq!(*field, "max_total_work_units");
            assert_eq!(*attempted, admitted, "the crossing attempts the boundary");
        }
        details => panic!("total-work crossing carries its attempt: {details:?}"),
    }
    admitted
}

/// One total-work charge of a render, as the fatal crossing it causes reports
/// it: the units the event adds, and the element and pointer the crossing is
/// attributed to (`None` and `""` for a canvas-wide event).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkCharge {
    pub units: u64,
    pub element_id: Option<String>,
    pub json_pointer: String,
}

/// The last `count` total-work charges of `request_builder`'s render of
/// `scene`, latest first, read through the public total-work ceiling alone.
/// Work is cumulative, so every ceiling between the total before an event and
/// one unit below the total after it fails at that event with the total after
/// it as the attempt; the smallest such ceiling is the total before, and the
/// difference is the event's charge. Each charge is one bisection, starting
/// from [`total_work_boundary`].
pub fn trailing_work_charges(
    scene: &SceneDocument,
    request_builder: impl Fn() -> RenderRequestBuilder,
    count: usize,
) -> Vec<WorkCharge> {
    let crossing = |total: u64| {
        let error = Renderer::new()
            .render(scene, &work_probe_request(request_builder(), total))
            .err()?;
        let attempted = match error.details() {
            Some(ErrorDetails::ResourceLimit {
                field: "max_total_work_units",
                measurement:
                    ResourceMeasurement::Attempted {
                        value: ResourceValue::Integer(attempted),
                    },
                ..
            }) => *attempted,
            _ => return None,
        };
        assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED);
        Some((attempted, error))
    };
    let mut after = total_work_boundary(scene, &request_builder);
    let mut charges = Vec::with_capacity(count);
    while charges.len() < count && after > 0 {
        let (mut refused, mut crosses_here) = (0, after - 1);
        while crosses_here - refused > 1 {
            let probe = refused + (crosses_here - refused) / 2;
            if crossing(probe).is_some_and(|(attempted, _)| attempted == after) {
                crosses_here = probe;
            } else {
                refused = probe;
            }
        }
        let before = if crossing(refused).is_some_and(|(attempted, _)| attempted == after) {
            refused
        } else {
            crosses_here
        };
        let (_, error) = crossing(before).expect("the event's own crossing is fatal");
        charges.push(WorkCharge {
            units: after - before,
            element_id: error.element_id().map(str::to_owned),
            json_pointer: error.json_pointer().to_owned(),
        });
        after = before;
    }
    charges
}

/// `builder` under a `total` work ceiling with every per-item work ceiling
/// lowered to fit inside it.
fn work_probe_request(builder: RenderRequestBuilder, total: u64) -> RenderRequest {
    let defaults = ResourceLimits::default();
    builder
        .resource_limits(
            ResourceLimits::builder()
                .max_element_work_units(defaults.max_element_work_units().min(total))
                .max_asset_work_units(defaults.max_asset_work_units().min(total))
                .max_total_work_units(total)
                .build()
                .expect("total-work probe limits are valid"),
        )
        .build()
        .expect("total-work probe request is valid")
}
