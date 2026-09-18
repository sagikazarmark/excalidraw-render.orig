//! Bounded conversion experiment for #185; production has no library switch.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[allow(dead_code)]
enum ColorMode {
    Light,
    Dark,
}
mod adapter;
#[path = "../../../crates/excalidraw-render/src/css_syntax.rs"]
mod css_syntax;
#[allow(dead_code)]
#[path = "../../../crates/excalidraw-render/src/portable_math.rs"]
mod portable_math;
#[allow(dead_code, unused_imports)]
mod current {
    include!(concat!(env!("OUT_DIR"), "/current.rs"));
    pub fn inverse(kind: &str, v: [f64; 3]) -> [f64; 3] {
        match kind {
            "lab-inverse" => lab_d50_to_srgb(v),
            "oklab-inverse" => {
                let c =
                    csscolorparser::Color::from_oklaba(v[0] as f32, v[1] as f32, v[2] as f32, 1.0);
                [c.r, c.g, c.b].map(f64::from)
            }
            _ => unreachable!(),
        }
    }
}
#[allow(dead_code, unused_imports, clippy::unnecessary_cast)]
mod palette_kernel {
    include!(concat!(env!("OUT_DIR"), "/palette.rs"));
}
#[allow(dead_code, unused_imports, clippy::unnecessary_cast)]
mod linebender_kernel {
    include!(concat!(env!("OUT_DIR"), "/linebender.rs"));
}

fn bits(v: [f64; 3]) -> [String; 3] {
    v.map(|x| format!("{:016x}", x.to_bits()))
}

fn sample(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 32) as u32 as f64 / 4294967296.0
}

fn numeric() -> Value {
    use adapter::Space;
    let mut rows = Vec::new();
    let mut state = 185;
    for kind in [
        "xyz",
        "lab",
        "oklab",
        "decode",
        "encode",
        "d50-d65",
        "d65-d50",
        "lab-inverse",
        "oklab-inverse",
    ] {
        let mut hashes = [Sha256::new(), Sha256::new(), Sha256::new()];
        let mut changed = [0; 2];
        let mut maxima = [[0.0_f64; 3]; 2];
        let mut first = [Value::Null, Value::Null];
        for i in 0..4096 {
            let mut input = std::array::from_fn(|_| sample(&mut state) * 3.0 - 1.0);
            let outputs = match kind {
                "xyz" | "lab" | "oklab" => {
                    let rgb = if i < 256 {
                        [i as u8; 3]
                    } else {
                        input.map(|x| ((x + 1.0) * 85.0) as u8)
                    };
                    input = rgb.map(f64::from);
                    [
                        current::forward(kind, rgb),
                        palette_kernel::forward(kind, rgb),
                        linebender_kernel::forward(kind, rgb),
                    ]
                }
                "lab-inverse" | "oklab-inverse" => {
                    if kind == "lab-inverse" {
                        input = [(input[0] + 1.0) * 40.0, input[1] * 100.0, input[2] * 100.0];
                    }
                    let space = if kind == "lab-inverse" {
                        Space::Lab
                    } else {
                        Space::Oklab
                    };
                    [
                        current::inverse(kind, input),
                        adapter::palette(input, space, Space::Srgb),
                        adapter::linebender(input, space, Space::Srgb),
                    ]
                }
                _ => {
                    if matches!(kind, "encode" | "decode") && i < 24 {
                        let boundary: f64 = if kind == "decode" { 0.04045 } else { 0.0031308 };
                        let x = [
                            boundary.next_down(),
                            boundary,
                            boundary.next_up(),
                            0.0,
                            1.0,
                            -0.0,
                            1.000001,
                            2.0,
                        ][i % 8];
                        input = [x, -x, x];
                    }
                    [
                        current::primitive(kind, input),
                        palette_kernel::primitive(kind, input),
                        linebender_kernel::primitive(kind, input),
                    ]
                }
            };
            for (hash, output) in hashes.iter_mut().zip(outputs) {
                for x in input.into_iter().chain(output) {
                    hash.update(x.to_le_bytes());
                }
            }
            for c in 0..2 {
                let delta = std::array::from_fn::<_, 3, _>(|j| outputs[c + 1][j] - outputs[0][j]);
                for (max, d) in maxima[c].iter_mut().zip(delta) {
                    *max = max.max(d.abs());
                }
                if bits(outputs[0]) != bits(outputs[c + 1]) {
                    changed[c] += 1;
                    if first[c].is_null() {
                        first[c] = json!({"input":input,"input_bits":bits(input),"current":outputs[0],"candidate":outputs[c+1],"current_bits":bits(outputs[0]),"candidate_bits":bits(outputs[c+1]),"delta":delta});
                    }
                }
            }
        }
        rows.push(json!({"operation":kind,"samples":4096,"changed":changed,"max_abs_channel_delta":maxima,"first_difference":first,"stream_sha256":hashes.map(|h| format!("{:x}",h.finalize()))}));
    }
    json!(rows)
}

fn css_inputs() -> Vec<String> {
    let mut inputs: Vec<String> = [
        "RGB(30 30 30)",
        "hwb(120 0% 0% / .5)",
        "lab(50% 0 0)",
        "oklab(50% 0 0)",
        "color(display-p3 1 0 0)",
        "color(srgb 0.25 0 0.75)",
        "color(srgb-linear 0.21586050011389926 0.21586050011389926 0.21586050011389926)",
        "color-mix(IN SRGB, red 25%, blue)",
        "color-mix(in oklab, red, blue)",
        "color-mix(in hsl, red, blue)",
        "color-mix(in hsl longer hue, red, blue)",
        "color-mix(in hwb, red, blue)",
        "color-mix(in display-p3, red, blue)",
        "color-mix(in xyz, red, blue)",
        "currentcolor",
        "rebeccapurple",
        "  red  ",
        "Highlight",
        "Canvas",
        "color(srgb none 0.2 0.3 / none)",
        "lab(none none none / none)",
        "color-mix(in srgb, red 49.9999999%, blue)",
        "color-mix(in srgb, red 50.0000001%, blue)",
        "color-mix(in srgb, currentcolor/*(,)*/, blue)",
        "color-mix(in srgb, currentcolor/*,*/, blue)",
        "color-mix(in srgb, red 0%, blue 0%)",
        "color-mix(in srgb, red, blue",
        "color(invalid 1 1 1)",
        "color-mix(in srgb, red calc(25%), blue)",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    for space in [
        "srgb",
        "srgb-linear",
        "display-p3",
        "a98-rgb",
        "prophoto-rgb",
        "rec2020",
        "xyz-d50",
        "xyz-d65",
    ] {
        for components in ["-0.5 0.2 1.2", "0 0 0", "1 1 1", "0.04045 0.0031308 0.5"] {
            inputs.push(format!("color({space} {components})"));
        }
    }
    for space in [
        "srgb",
        "srgb-linear",
        "display-p3",
        "a98-rgb",
        "prophoto-rgb",
        "rec2020",
        "xyz-d50",
        "xyz-d65",
        "lab",
        "oklab",
        "hsl",
        "hwb",
        "lch",
        "oklch",
    ] {
        for stops in [
            "red, blue",
            "rgb(255 0 0 / 0), rgb(0 0 255 / 0)",
            "rgb(255 0 0 / .004) 20%, rgb(0 0 255 / .5) 30%",
        ] {
            inputs.push(format!("color-mix(in {space}, {stops})"));
        }
    }
    for lab in [
        "0 0 0",
        "100 0 0",
        "50 125 -125",
        "50 -125 125",
        "8 0 0",
        "50 .01 -.01",
    ] {
        for alpha in ["0", "0.0019607842", "0.0019607844", "0.5", "1"] {
            inputs.push(format!("lab({lab} / {alpha})"));
        }
    }
    inputs
}

fn parsed(s: &str) -> [Option<[u8; 4]>; 3] {
    [
        current::parse(s),
        palette_kernel::parse(s),
        linebender_kernel::parse(s),
    ]
}

fn css() -> Value {
    // Existing pinned-Chromium unit witnesses, in the same order as css_inputs.
    let pinned = [
        "1e1e1e", "00ff00", "777777", "636363", "ff0000", "4000bf", "808080", "4000bf", "8c53a2",
        "ff00ff", "00ff00", "ff00ff", "800a91", "bc00bc", "000000", "663399", "ff0000",
    ];
    json!(css_inputs()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let rgba = parsed(s);
            if i < pinned.len() {
                let c = rgba[0].unwrap();
                assert_eq!(format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2]), pinned[i]);
            }
            json!({"css":s,"rgba":rgba,"pinned_rgb":pinned.get(i)})
        })
        .collect::<Vec<_>>())
}

fn boundaries() -> Value {
    let mut hash = Sha256::new();
    let mut changed = [0; 2];
    let mut first = [Value::Null, Value::Null];
    let mut samples = 0;
    for n in 0..255 {
        let half = (n as f64 + 0.5) / 255.0;
        for x in [half.next_down(), half, half.next_up()] {
            let linear = if x <= 0.04045 {
                x / 12.92
            } else {
                libm::pow((x + 0.055) / 1.055, 2.4)
            };
            for s in [
                format!("color(srgb {x} {x} {x} / {x})"),
                format!(
                    "color-mix(in srgb-linear, black {}%, white)",
                    (1.0 - linear) * 100.0
                ),
            ] {
                let rgba = parsed(&s);
                samples += 1;
                hash.update((s.len() as u64).to_le_bytes());
                hash.update(s.as_bytes());
                for c in rgba {
                    hash.update(c.unwrap());
                }
                for c in 0..2 {
                    if rgba[0] != rgba[c + 1] {
                        changed[c] += 1;
                        if first[c].is_null() {
                            first[c] = json!({"css":s,"rgba":rgba});
                        }
                    }
                }
            }
        }
    }
    json!({"samples":samples,"changed":changed,"first_difference":first,"sha256":format!("{:x}",hash.finalize())})
}

fn sweep() -> Value {
    let mut state = 185;
    let mut changed = [0; 2];
    let mut maxima = [[0_i16; 4]; 2];
    let mut witnesses = [Value::Null, Value::Null];
    let mut hash = Sha256::new();
    for _ in 0..4096 {
        let s = format!(
            "lab({} {} {} / {})",
            sample(&mut state) * 100.0,
            sample(&mut state) * 250.0 - 125.0,
            sample(&mut state) * 250.0 - 125.0,
            sample(&mut state)
        );
        let rgba = parsed(&s).map(Option::unwrap);
        hash.update((s.len() as u64).to_le_bytes());
        hash.update(s.as_bytes());
        for c in rgba {
            hash.update(c);
        }
        for c in 0..2 {
            let delta = std::array::from_fn::<_, 4, _>(|j| {
                i16::from(rgba[c + 1][j]) - i16::from(rgba[0][j])
            });
            for (m, d) in maxima[c].iter_mut().zip(delta) {
                *m = (*m).max(d.abs());
            }
            if rgba[c + 1] != rgba[0] {
                changed[c] += 1;
                if witnesses[c].is_null() {
                    witnesses[c] = json!({"css":s,"rgba":rgba,"delta":delta});
                }
            }
        }
    }
    json!({"samples":4096,"changed":changed,"max_abs_rgba_delta":maxima,"first_difference":witnesses,"sha256":format!("{:x}",hash.finalize())})
}

mod artifacts;

pub fn report() -> Value {
    let numeric = numeric();
    let css = css();
    let boundaries = boundaries();
    let sweep = sweep();
    let artifacts = artifacts::report(&css, &boundaries, &sweep);
    json!({"numeric":numeric,"css":css,"half_byte":boundaries,"lab_sweep":sweep,"artifacts":artifacts})
}

pub fn verify(report: &Value, expected: &str) {
    let expected: Value = serde_json::from_str(expected).unwrap();
    let actual: Value = serde_json::from_slice(&serde_json::to_vec(report).unwrap()).unwrap();
    assert_eq!(
        actual, expected,
        "observations changed; investigate before recording"
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
        #[cfg(target_arch = "wasm32")]
        if option_env!("COLOR_RECORD") == Some("1") {
            wasm_bindgen_test::console_log!("COLOR_REPORT={}", report);
            return;
        }
        super::verify(
            &report,
            if cfg!(target_arch = "wasm32") {
                include_str!("../expected-wasm.json")
            } else {
                include_str!("../expected-native.json")
            },
        );
    }
}
