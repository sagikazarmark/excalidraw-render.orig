//! Differential measurements for #184, not a production geometry switch.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

mod current {
    fn square(x: f64) -> f64 {
        x * x
    }
    fn cube(x: f64) -> f64 {
        x * x * x
    }
    include!(concat!(env!("OUT_DIR"), "/current.rs"));
}
mod candidate {
    include!(concat!(env!("OUT_DIR"), "/candidate.rs"));
}

type Curve = [[f64; 2]; 4];
type Bounds = [f64; 4];

fn bounds(curve: Curve) -> (Bounds, Bounds) {
    let [a, b, c, d] = curve.map(|[x, y]| current::Point { x, y });
    let old = current::Path {
        operations: vec![
            current::PathOperation::Move(a),
            current::PathOperation::Cubic {
                control_1: b,
                control_2: c,
                end: d,
            },
        ],
    }
    .cubic_bounds()
    .unwrap();
    let [a, b, c, d] = curve.map(|[x, y]| candidate::Point { x, y });
    let new = candidate::Path {
        operations: vec![
            candidate::PathOperation::Move(a),
            candidate::PathOperation::Cubic {
                control_1: b,
                control_2: c,
                end: d,
            },
        ],
    }
    .cubic_bounds()
    .unwrap();
    ([old.0, old.1, old.2, old.3], [new.0, new.1, new.2, new.3])
}

fn bits(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect()
}

fn grid(b: Bounds) -> Bounds {
    [b[0].floor(), b[1].floor(), b[2].ceil(), b[3].ceil()]
}

fn ordered(x: f64) -> u64 {
    let bits = x.to_bits();
    if bits >> 63 != 0 {
        !bits
    } else {
        bits | (1 << 63)
    }
}

fn witness(curve: Curve, old: Bounds, new: Bounds) -> Value {
    json!({"curve":curve,"input_bits":curve.map(|p| bits(&p)),
        "current":old,"candidate":new,"current_bits":bits(&old),"candidate_bits":bits(&new),
        "current_grid":grid(old),"candidate_grid":grid(new)})
}

fn next(state: &mut u64) -> f64 {
    // Fixed integer PRNG; dyadic samples avoid host-dependent generation.
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as i32 as f64) / 2147483648.0
}

fn numeric() -> Value {
    let mut result = Vec::new();
    let mut state = 184_u64;
    for family in [
        "degenerate",
        "near-linear",
        "near-linear-derivative",
        "repeated-control",
        "rotated",
        "large-coordinate",
        "rounding-boundary",
        "general",
    ] {
        let mut digest = Sha256::new();
        let (mut changed, mut grid_changed, mut max_ulp) = (0, 0, 0);
        let mut max_absolute = 0.0_f64;
        let (mut first, mut worst, mut first_grid) = (Value::Null, Value::Null, Value::Null);
        for i in 0..4096 {
            let mut p = std::array::from_fn::<_, 4, _>(|_| {
                [next(&mut state) * 256.0, next(&mut state) * 256.0]
            });
            match family {
                "degenerate" => p = [p[0]; 4],
                "near-linear" => {
                    let start = p[0];
                    let end = p[3];
                    p[1] =
                        std::array::from_fn(|axis| start[axis] + (end[axis] - start[axis]) / 3.0);
                    p[2] = std::array::from_fn(|axis| {
                        start[axis]
                            + 2.0 * (end[axis] - start[axis]) / 3.0
                            + (i % 3) as f64 * f64::EPSILON * 256.0
                    });
                }
                "near-linear-derivative" => {
                    // Near-quadratic cubics: cancellation in the leading derivative coefficient.
                    p[3] = std::array::from_fn(|axis| {
                        p[0][axis] - 3.0 * p[1][axis]
                            + 3.0 * p[2][axis]
                            + (i % 3) as f64 * f64::EPSILON * 256.0
                    });
                }
                "repeated-control" => {
                    p[1] = p[0];
                    if i % 2 == 0 {
                        p[2] = p[3];
                    }
                }
                "rotated" => {
                    let angle = next(&mut state) * 8.0;
                    let (s, c) = (libm::sin(angle), libm::cos(angle));
                    p = p.map(|[x, y]| [x * c - y * s, x * s + y * c]);
                }
                "large-coordinate" => {
                    let offset = if i % 2 == 0 {
                        999_999_000.0
                    } else {
                        999_999_999_000.0
                    };
                    p = p.map(|[x, y]| [offset + x, -offset + y]);
                }
                "rounding-boundary" => {
                    let (b, _) = bounds(p);
                    let shift = (i % 3) as f64 * 0.5 - b[2];
                    p = p.map(|[x, y]| [x + shift, y]);
                }
                _ => {}
            }
            assert!(p.iter().flatten().all(|x| x.is_finite() && x.abs() <= 1e12));
            let (old, new) = bounds(p);
            assert!(old.iter().chain(&new).all(|v| v.is_finite()));
            for v in p.iter().flatten().chain(&old).chain(&new) {
                digest.update(v.to_bits().to_le_bytes());
            }
            if bits(&old) != bits(&new) {
                changed += 1;
                if first.is_null() {
                    first = witness(p, old, new);
                }
            }
            if grid(old) != grid(new) {
                grid_changed += 1;
                if first_grid.is_null() {
                    first_grid = witness(p, old, new);
                }
            }
            for (a, b) in old.into_iter().zip(new) {
                max_ulp = max_ulp.max(ordered(a).abs_diff(ordered(b)));
                if (a - b).abs() > max_absolute {
                    max_absolute = (a - b).abs();
                    worst = witness(p, old, new);
                }
            }
        }
        result.push(json!({"family":family,"samples":4096,"changed":changed,"grid_changed":grid_changed,
            "max_ulp":max_ulp,"max_absolute":max_absolute,"sha256":format!("{:x}",digest.finalize()),
            "first_difference":first,"largest_absolute_difference":worst,"first_grid_difference":first_grid}));
    }
    json!(result)
}

fn traversal() -> Value {
    // Run the SAME extracted traversal on both sides, including absent start and
    // ignored contributors. A whole BezPath bounding_box fails these checks.
    let sequences: Vec<Vec<(char, [f64; 2])>> = vec![
        vec![],
        vec![('M', [900.0, 900.0])],
        vec![('M', [900.0, 900.0]), ('L', [0.0, 0.0])],
        vec![('C', [1.0, 1.0])],
        vec![
            ('M', [900.0, 900.0]),
            ('L', [0.0, 0.0]),
            ('C', [1.0, 1.0]),
            ('L', [999.0, 999.0]),
        ],
        vec![('M', [2.0, 2.0]), ('C', [2.0, 2.0])],
        vec![
            ('M', [0.0, 0.0]),
            ('C', [1.0, 1.0]),
            ('M', [-2.0, -2.0]),
            ('C', [-1.0, -1.0]),
        ],
    ];
    sequences
        .into_iter()
        .map(|seq| {
            let old = current::Path {
                operations: seq
                    .iter()
                    .map(|&(kind, [x, y])| {
                        let p = current::Point { x, y };
                        match kind {
                            'M' => current::PathOperation::Move(p),
                            'L' => current::PathOperation::Line(p),
                            'C' => current::PathOperation::Cubic {
                                control_1: p,
                                control_2: p,
                                end: p,
                            },
                            _ => panic!("unknown traversal fixture operation: {kind}"),
                        }
                    })
                    .collect(),
            }
            .cubic_bounds();
            let new = candidate::Path {
                operations: seq
                    .iter()
                    .map(|&(kind, [x, y])| {
                        let p = candidate::Point { x, y };
                        match kind {
                            'M' => candidate::PathOperation::Move(p),
                            'L' => candidate::PathOperation::Line(p),
                            'C' => candidate::PathOperation::Cubic {
                                control_1: p,
                                control_2: p,
                                end: p,
                            },
                            _ => panic!("unknown traversal fixture operation: {kind}"),
                        }
                    })
                    .collect(),
            }
            .cubic_bounds();
            assert_eq!(old, new);
            json!({"operations":seq,"bounds":old})
        })
        .collect()
}

mod artifacts;

pub fn report() -> Value {
    json!({"numeric":numeric(),"traversal":traversal(),"artifacts":artifacts::report()})
}

pub fn verify(report: &Value) {
    let expected: Value =
        serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/expected.json"))).unwrap();
    // Normalize both through the same JSON reader. Binary64 identity is held
    // separately by explicit hex bits and stream digests, not decimal parsing.
    let actual: Value = serde_json::from_slice(&serde_json::to_vec(report).unwrap()).unwrap();
    assert_eq!(
        actual, expected,
        "differential observations changed; investigate before recording"
    );
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn exact_observation() {
        let report = super::report();
        if option_env!("KURBO_RECORD") == Some("1") {
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_test::console_log!("KURBO_REPORT={}", report);
        } else {
            super::verify(&report);
        }
    }
}
