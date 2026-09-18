//! Unclamped, unpremultiplied conversions only. Alpha never enters a library.
use palette::chromatic_adaptation::{AdaptFrom, Method};
use palette::convert::FromColorUnclamped;
use palette::white_point::{D50, D65};
use palette::{Lab, LinSrgb, Oklab, Srgb, Xyz};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Space {
    Srgb,
    Linear,
    Xyz65,
    Xyz50,
    Lab,
    Oklab,
}

pub fn linebender(v: [f64; 3], from: Space, to: Space) -> [f64; 3] {
    let tag = |s| match s {
        Space::Srgb => color::ColorSpaceTag::Srgb,
        Space::Linear => color::ColorSpaceTag::LinearSrgb,
        Space::Xyz65 => color::ColorSpaceTag::XyzD65,
        Space::Xyz50 => color::ColorSpaceTag::XyzD50,
        Space::Lab => color::ColorSpaceTag::Lab,
        Space::Oklab => color::ColorSpaceTag::Oklab,
    };
    if from == to {
        v
    } else {
        tag(from)
            .convert(tag(to), v.map(|x| x as f32))
            .map(f64::from)
    }
}

pub fn palette(v: [f64; 3], from: Space, to: Space) -> [f64; 3] {
    if from == to {
        return v;
    }
    let [a, b, c] = v;
    // Adapt XYZ directly: do not add an avoidable RGB round trip.
    if from == Space::Xyz50 && to == Space::Xyz65 {
        let x = Xyz::<D65, f64>::adapt_from_using(Xyz::<D50, f64>::new(a, b, c), Method::Bradford);
        return [x.x, x.y, x.z];
    }
    if from == Space::Xyz65 && to == Space::Xyz50 {
        let x = Xyz::<D50, f64>::adapt_from_using(Xyz::<D65, f64>::new(a, b, c), Method::Bradford);
        return [x.x, x.y, x.z];
    }
    let from_d50 = |x: Xyz<D50, f64>| {
        LinSrgb::from_color_unclamped(Xyz::<D65, f64>::adapt_from_using(x, Method::Bradford))
    };
    let linear: LinSrgb<f64> = match from {
        Space::Srgb => Srgb::new(a, b, c).into_linear(),
        Space::Linear => LinSrgb::new(a, b, c),
        Space::Xyz65 => LinSrgb::from_color_unclamped(Xyz::<D65, f64>::new(a, b, c)),
        Space::Xyz50 => from_d50(Xyz::new(a, b, c)),
        Space::Lab => from_d50(Xyz::from_color_unclamped(Lab::<D50, f64>::new(a, b, c))),
        Space::Oklab => LinSrgb::from_color_unclamped(Oklab::new(a, b, c)),
    };
    match to {
        Space::Srgb => {
            let x: Srgb<f64> = Srgb::from_linear(linear);
            [x.red, x.green, x.blue]
        }
        Space::Linear => [linear.red, linear.green, linear.blue],
        Space::Xyz65 => {
            let x: Xyz<D65, f64> = Xyz::from_color_unclamped(linear);
            [x.x, x.y, x.z]
        }
        Space::Xyz50 | Space::Lab => {
            let d65: Xyz<D65, f64> = Xyz::from_color_unclamped(linear);
            let x: Xyz<D50, f64> = Xyz::adapt_from_using(d65, Method::Bradford);
            if to == Space::Xyz50 {
                [x.x, x.y, x.z]
            } else {
                let lab = Lab::from_color_unclamped(x);
                [lab.l, lab.a, lab.b]
            }
        }
        Space::Oklab => {
            let x = Oklab::from_color_unclamped(linear);
            [x.l, x.a, x.b]
        }
    }
}
