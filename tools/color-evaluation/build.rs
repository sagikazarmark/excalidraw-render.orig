use std::{env, fs, path::PathBuf};

fn region(source: &str, name: &str) -> std::ops::Range<usize> {
    let start = source.find(&format!("fn {name}(")).unwrap();
    let end = start + source[start..].find("\n}\n").unwrap() + 3;
    start..end
}

fn replace_body(source: &mut String, name: &str, body: &str) {
    let range = region(source, name);
    let open = range.start + source[range.clone()].find('{').unwrap();
    source.replace_range(open + 1..range.end - 2, &format!("\n{body}\n"));
}

fn main() {
    let path = "../../crates/excalidraw-render/src/color.rs";
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rerun-if-changed=src/hooks.rs");
    println!("cargo:rerun-if-env-changed=COLOR_RECORD");
    let source = fs::read_to_string(path).unwrap();
    // Compile current implementation, not a hand-transcribed formula. The
    // production unit tests remain in the production suite.
    let source = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let hooks = fs::read_to_string("src/hooks.rs").unwrap();
    fs::write(out.join("current.rs"), format!("{source}\n{hooks}")).unwrap();
    for candidate in ["palette", "linebender"] {
        let mut text = source.to_string();
        for (name, body) in [
            ("decode_srgb", "convert([component, 0.0, 0.0], Space::Srgb, Space::Linear)[0]"),
            ("encode_srgb", "convert([component, 0.0, 0.0], Space::Linear, Space::Srgb)[0]"),
            ("adapt_d50_to_d65", "convert(xyz, Space::Xyz50, Space::Xyz65)"),
            ("adapt_d65_to_d50", "convert(xyz, Space::Xyz65, Space::Xyz50)"),
            ("srgb_to_xyz_d65", "convert(color.rgb_f64(), Space::Srgb, Space::Xyz65)"),
            ("srgb_to_lab", "convert(color.rgb_f64(), Space::Srgb, Space::Lab)"),
            ("canonical_lab_color", "let [r, g, b] = convert([lightness, a, b], Space::Lab, Space::Srgb);\ncanonical_css_color(csscolorparser::Color::new(r as f32, g as f32, b as f32, alpha as f32))"),
        ] {
            replace_body(&mut text, name, body);
        }
        // Paired XYZ -> linear RGB edge in color(), using the same library
        // constants as the forward edge instead of creating a hybrid round trip.
        let start = text.find("    let srgb_linear = multiply(").unwrap();
        let end = start
            + text[start..]
                .find("    Some(CanonicalColor::from_unit(")
                .unwrap();
        text.replace_range(
            start..end,
            "    let srgb_linear = convert(linear, Space::Xyz65, Space::Linear);\n",
        );
        // Only rectangular Oklab mix conversion changes. Gamut mapping and
        // polar Oklch's existing csscolorparser calls remain project policy.
        let start = text.find("        ColorMixSpace::Oklab => {").unwrap();
        let end = start + text[start..].find("        ColorMixSpace::XyzD50").unwrap();
        text.replace_range(start..end, "        ColorMixSpace::Oklab => convert(color.rgb_f64(), Space::Srgb, Space::Oklab),\n");
        let old = "ColorMixSpace::Oklab => Some(canonical_css_color(csscolorparser::Color::from_oklaba(\n            components[0] as f32,\n            components[1] as f32,\n            components[2] as f32,\n            alpha as f32,\n        ))),";
        assert_eq!(text.matches(old).count(), 1);
        text = text.replace(old, "ColorMixSpace::Oklab => {\nlet [r,g,b] = convert(components, Space::Oklab, Space::Srgb);\nSome(canonical_css_color(csscolorparser::Color::new(r as f32,g as f32,b as f32,alpha as f32)))\n},");
        fs::write(
            out.join(format!("{candidate}.rs")),
            format!("use crate::adapter::{{{candidate} as convert, Space}};\n{text}\n{hooks}"),
        )
        .unwrap();
    }
    // Raw Lab inverse, lifted before the existing f32/gamut boundary.
    let lab = &source[region(source, "canonical_lab_color")];
    let prefix = &lab[lab.find('{').unwrap() + 1..lab.find("    canonical_css_color(").unwrap()];
    fs::write(out.join("raw_lab.rs"), format!("fn raw_lab([lightness,a,b]: [f64;3]) -> [f64;3] {{\n{prefix}\nlinear.map(encode_srgb)\n}}\n")).unwrap();
}
