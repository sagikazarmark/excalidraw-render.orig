use super::Point;

pub(super) fn bounds(
    start: Point,
    control_1: Point,
    control_2: Point,
    end: Point,
) -> (f64, f64, f64, f64) {
    use kurbo::{CubicBez, ParamCurveExtrema};
    let [p0, p1, p2, p3] = [start, control_1, control_2, end].map(|p| (p.x, p.y));
    let r = CubicBez::new(p0, p1, p2, p3).bounding_box();
    (r.x0, r.y0, r.x1, r.y1)
}
