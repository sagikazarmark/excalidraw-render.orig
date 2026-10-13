//! A PNG too large for the renderer's pixel limit, described in the reader's
//! terms: its size in pixels and megapixels, and the largest `--max-dimension`
//! or `--scale` that fits. The JSON report keeps the library's error as it is.

use excalidraw_render::{
    DiagnosticDetails, ErrorCode, ErrorDescriptor, Measurement, OutputFormat, ResourceValue, Sizing,
};

/// The main PNG canvas of a render needing more pixels than the limit allows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PixelLimit {
    /// The pixels the canvas needs; `None` when a side overflowed and only a
    /// lower bound is known.
    needed: Option<u64>,
    /// The most pixels a canvas may have.
    limit: u64,
}

/// The size of a scene before sizing (padding included) and the sizing the
/// render asked for: enough to work out the canvas of any other sizing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SceneSize {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) sizing: Sizing,
}

impl PixelLimit {
    /// The crossing `error` reports, when it is the main PNG canvas's: a PDF's
    /// rasterized element has a crossing of its own, which this advice does
    /// not fit.
    pub(crate) fn of(error: &ErrorDescriptor) -> Option<Self> {
        if error.code() != ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED
            || error.format() != Some(OutputFormat::Png)
            || error.element_id().is_some()
        {
            return None;
        }
        let Some(DiagnosticDetails::ResourceLimit {
            field: "max_raster_surface_pixels",
            configured: ResourceValue::Integer(limit),
            measurement,
            ..
        }) = error.details()
        else {
            return None;
        };
        let needed = match measurement {
            Measurement::Attempted {
                value: ResourceValue::Integer(needed),
            } => Some(*needed),
            Measurement::AtLeast { .. } => None,
            _ => return None,
        };
        Some(Self {
            needed,
            limit: *limit,
        })
    }

    /// `scene`, when it accounts for the canvas the error measured, so that
    /// sizes worked out from it are the renderer's.
    fn confirmed(self, scene: Option<SceneSize>) -> Option<SceneSize> {
        let scene = scene?;
        let pixels = canvas(scene, scene.sizing).map(|(width, height)| width * height);
        match self.needed {
            Some(needed) => (pixels == Some(needed)).then_some(scene),
            // An overflowing side has no exact area to compare.
            None => pixels
                .is_none_or(|pixels| pixels > self.limit)
                .then_some(scene),
        }
    }

    /// The error line's message, sized in pixels when `scene` is known.
    pub(crate) fn message(self, scene: Option<SceneSize>) -> String {
        let Some(needed) = self.needed else {
            let (_, limit) = megapixels(self.limit, self.limit);
            return format!("the PNG would be over the {limit}-megapixel limit");
        };
        let (needed, limit) = megapixels(needed, self.limit);
        let size = match self
            .confirmed(scene)
            .and_then(|scene| canvas(scene, scene.sizing))
        {
            Some((width, height)) => format!("{width} × {height} pixels ({needed} megapixels)"),
            None => format!("{needed} megapixels"),
        };
        format!("the PNG would be {size}, over the {limit}-megapixel limit")
    }

    /// The fix: the largest `--max-dimension` that fits (or `--scale`, for a
    /// scene that fits unscaled), or a bound that fits any scene of the size
    /// when `scene` is unknown.
    pub(crate) fn hint(self, scene: Option<SceneSize>) -> String {
        const ALTERNATIVE: &str = "or render to SVG or PDF instead";
        let Some(scene) = self.confirmed(scene) else {
            return format!(
                "use --max-dimension {} or lower, {ALTERNATIVE}",
                square_side(self.limit)
            );
        };
        if let Some(maximum) = largest_max_dimension(scene, self.limit) {
            format!("use --max-dimension {maximum}, the largest size that fits, {ALTERNATIVE}")
        } else if let Some(hundredths) = largest_scale(scene, self.limit) {
            format!(
                "use --scale {}, the largest scale that fits, {ALTERNATIVE}",
                decimal_hundredths(hundredths)
            )
        } else {
            format!(
                "use --max-dimension {} or lower, {ALTERNATIVE}",
                square_side(self.limit)
            )
        }
    }
}

/// The PNG canvas the renderer derives for `scene` at `sizing`, in whole
/// pixels, as it does: the scale shrinks the longer side to a maximum
/// dimension below it, and each side is truncated. `None` for a side past
/// `u32`, which the renderer rejects as too large whatever the limit.
fn canvas(scene: SceneSize, sizing: Sizing) -> Option<(u64, u64)> {
    let largest = scene.width.max(scene.height);
    let scale = match sizing {
        Sizing::Scale(scale) => scale,
        Sizing::MaxDimension(maximum) if maximum < largest => maximum / largest,
        Sizing::MaxDimension(_) => 1.0,
        _ => return None,
    };
    let side = |logical: f64| {
        let value = logical * scale;
        (value.is_finite() && value < f64::from(u32::MAX) + 1.0).then(|| value.trunc() as u64)
    };
    Some((side(scene.width)?, side(scene.height)?))
}

/// Whether `sizing` gives `scene` a canvas within `limit` pixels.
fn within(scene: SceneSize, sizing: Sizing, limit: u64) -> bool {
    canvas(scene, sizing).is_some_and(|(width, height)| width * height <= limit)
}

/// Whether `sizing` gives `scene` a canvas within `limit` pixels and at least
/// one pixel on each side, which the renderer needs.
fn fits(scene: SceneSize, sizing: Sizing, limit: u64) -> bool {
    within(scene, sizing, limit)
        && canvas(scene, sizing).is_some_and(|(width, height)| width >= 1 && height >= 1)
}

/// The largest whole `--max-dimension` that fits, or `None` when the scene
/// fits unscaled, where a maximum dimension cannot help.
fn largest_max_dimension(scene: SceneSize, limit: u64) -> Option<u64> {
    if within(scene, Sizing::Scale(1.0), limit) {
        return None;
    }
    // The canvas grows with the maximum, and a maximum at or above the longer
    // side leaves the scene unscaled, which is too large: search below it.
    let within_at = |maximum: u64| within(scene, Sizing::MaxDimension(maximum as f64), limit);
    let (mut low, mut high) = (0_u64, scene.width.max(scene.height).ceil() as u64);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if within_at(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    fits(scene, Sizing::MaxDimension(low as f64), limit).then_some(low)
}

/// The largest `--scale` in hundredths that fits, for a scene that fits
/// unscaled.
fn largest_scale(scene: SceneSize, limit: u64) -> Option<u64> {
    let estimate = (limit as f64 / (scene.width * scene.height)).sqrt() * 100.0;
    let mut hundredths = estimate.floor() as u64 + 1;
    while hundredths > 0 && !fits(scene, Sizing::Scale(hundredths as f64 / 100.0), limit) {
        hundredths -= 1;
    }
    (hundredths > 0).then_some(hundredths)
}

/// The longest side of a square within `limit` pixels: a maximum dimension
/// that fits any scene, since neither side can exceed it.
fn square_side(limit: u64) -> u64 {
    let mut side = (limit as f64).sqrt() as u64;
    while side * side > limit {
        side -= 1;
    }
    while (side + 1) * (side + 1) <= limit {
        side += 1;
    }
    side
}

/// `hundredths` / 100 as the shortest decimal: `1.5`, `2`, `0.05`.
fn decimal_hundredths(hundredths: u64) -> String {
    let (whole, fraction) = (hundredths / 100, hundredths % 100);
    match fraction {
        0 => whole.to_string(),
        fraction if fraction % 10 == 0 => format!("{whole}.{}", fraction / 10),
        fraction => format!("{whole}.{fraction:02}"),
    }
}

/// `value` and `other` in megapixels, with the fewest decimals (at least one)
/// that tell them apart.
fn megapixels(value: u64, other: u64) -> (String, String) {
    let format = |pixels: u64, decimals: usize| format!("{:.decimals$}", pixels as f64 / 1e6);
    (1..=6)
        .map(|decimals| (format(value, decimals), format(other, decimals)))
        .find(|(value, other)| value != other)
        .unwrap_or_else(|| (format(value, 1), format(other, 1)))
}

/// The scene's size before sizing, from the `viewBox` of an SVG export of it.
pub(crate) fn svg_logical_size(svg: &[u8]) -> Option<(f64, f64)> {
    let head = std::str::from_utf8(&svg[..svg.len().min(512)]).ok()?;
    let view_box = head.split_once("viewBox=\"0 0 ")?.1.split_once('"')?.0;
    let (width, height) = view_box.split_once(' ')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::{
        canvas, decimal_hundredths, fits, largest_max_dimension, largest_scale, megapixels,
        square_side, svg_logical_size, PixelLimit, SceneSize,
    };
    use excalidraw_render::Sizing;

    const LIMIT: u64 = 33_554_432;

    fn scene(width: f64, height: f64, sizing: Sizing) -> SceneSize {
        SceneSize {
            width,
            height,
            sizing,
        }
    }

    /// The renderer's own sizing rule, as its plan tests pin it.
    #[test]
    fn the_canvas_follows_the_renderer_sizing_rule() {
        let box_scene = scene(40.0, 30.0, Sizing::Scale(1.0));
        for (sizing, expected) in [
            (Sizing::Scale(1.0), (40, 30)),
            (Sizing::Scale(1.5), (60, 45)),
            (Sizing::MaxDimension(20.0), (20, 15)),
            (Sizing::MaxDimension(100.0), (40, 30)),
        ] {
            assert_eq!(canvas(box_scene, sizing), Some(expected), "{sizing:?}");
        }
        assert_eq!(canvas(box_scene, Sizing::Scale(1e300)), None);
    }

    #[test]
    fn the_largest_max_dimension_fits_and_one_more_does_not() {
        for (width, height) in [
            (6524.45062991657, 5993.189064640477),
            (7020.0, 5020.0),
            (5020.0, 7020.0),
            (100_000.0, 1000.0),
            (8000.0, 8000.0),
            (5793.0, 5793.0),
        ] {
            let scene = scene(width, height, Sizing::Scale(1.0));
            let maximum = largest_max_dimension(scene, LIMIT).expect("a maximum fits");
            assert!(fits(scene, Sizing::MaxDimension(maximum as f64), LIMIT));
            assert!(!fits(
                scene,
                Sizing::MaxDimension(maximum as f64 + 1.0),
                LIMIT
            ));
        }
        assert_eq!(
            largest_max_dimension(
                scene(6524.45062991657, 5993.189064640477, Sizing::Scale(1.0)),
                LIMIT
            ),
            Some(6044)
        );
        // A scene that fits unscaled needs a lower scale, not a maximum.
        assert_eq!(
            largest_max_dimension(scene(4020.0, 3020.0, Sizing::Scale(2.0)), LIMIT),
            None
        );
    }

    #[test]
    fn the_largest_scale_is_a_whole_hundredth_that_fits() {
        let scene = scene(4020.0, 3020.0, Sizing::Scale(2.0));
        assert_eq!(largest_scale(scene, LIMIT), Some(166));
        assert!(fits(scene, Sizing::Scale(1.66), LIMIT));
        assert!(!fits(scene, Sizing::Scale(1.67), LIMIT));
    }

    #[test]
    fn a_square_side_bounds_every_aspect_ratio() {
        assert_eq!(square_side(LIMIT), 5792);
        assert_eq!(square_side(16), 4);
        assert_eq!(square_side(15), 3);
    }

    #[test]
    fn megapixels_use_the_decimals_that_tell_them_apart() {
        assert_eq!(
            megapixels(39_098_332, LIMIT),
            ("39.1".to_owned(), "33.6".to_owned())
        );
        assert_eq!(
            megapixels(33_563_049, LIMIT),
            ("33.56".to_owned(), "33.55".to_owned())
        );
        assert_eq!(
            megapixels(33_555_000, LIMIT),
            ("33.555".to_owned(), "33.554".to_owned())
        );
    }

    #[test]
    fn scales_print_as_short_decimals() {
        assert_eq!(decimal_hundredths(166), "1.66");
        assert_eq!(decimal_hundredths(150), "1.5");
        assert_eq!(decimal_hundredths(200), "2");
        assert_eq!(decimal_hundredths(5), "0.05");
    }

    #[test]
    fn the_logical_size_is_read_from_the_svg_view_box() {
        assert_eq!(
            svg_logical_size(
                br#"<svg version="1.1" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6524.45062991657 5993.189064640477" width="13048.9" height="11986.3">"#
            ),
            Some((6524.45062991657, 5993.189064640477))
        );
        assert_eq!(svg_logical_size(b"<svg>"), None);
    }

    #[test]
    fn a_size_that_does_not_account_for_the_error_is_not_used() {
        let limit = PixelLimit {
            needed: Some(39_098_332),
            limit: LIMIT,
        };
        let varmq = scene(6524.45062991657, 5993.189064640477, Sizing::Scale(1.0));
        assert_eq!(
            limit.message(Some(varmq)),
            "the PNG would be 6524 × 5993 pixels (39.1 megapixels), over the 33.6-megapixel limit"
        );
        assert_eq!(
            limit.hint(Some(varmq)),
            "use --max-dimension 6044, the largest size that fits, or render to SVG or PDF instead"
        );
        let other = scene(7020.0, 5020.0, Sizing::Scale(1.0));
        assert_eq!(
            limit.message(Some(other)),
            "the PNG would be 39.1 megapixels, over the 33.6-megapixel limit"
        );
        assert_eq!(
            limit.hint(Some(other)),
            "use --max-dimension 5792 or lower, or render to SVG or PDF instead"
        );
        let unknown = PixelLimit {
            needed: None,
            limit: LIMIT,
        };
        assert_eq!(
            unknown.message(None),
            "the PNG would be over the 33.6-megapixel limit"
        );
    }
}
