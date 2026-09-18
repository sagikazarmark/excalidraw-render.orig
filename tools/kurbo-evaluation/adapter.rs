fn include_cubic_bounds(
    bounds: &mut Option<(f64, f64, f64, f64)>,
    start: Point,
    control_1: Point,
    control_2: Point,
    end: Point,
) {
    use kurbo::{CubicBez, ParamCurveExtrema};
    let [p0, p1, p2, p3] = [start, control_1, control_2, end].map(|p| (p.x, p.y));
    let r = CubicBez::new(p0, p1, p2, p3).bounding_box();
    *bounds = Some(bounds.map_or((r.x0, r.y0, r.x1, r.y1), |b| {
        (b.0.min(r.x0), b.1.min(r.y0), b.2.max(r.x1), b.3.max(r.y1))
    }));
}
