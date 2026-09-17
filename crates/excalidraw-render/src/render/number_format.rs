//! The three policies by which an `f64` becomes artifact text, kept together
//! so a reviewer can compare them:
//!
//! - `push_rough_number`: Rough.js's path serialization, a fixed two-decimal
//!   spelling with trailing zeros and a vanishing sign dropped, using the
//!   pinned ECMAScript formatter's `toFixed(2)` rounding.
//!   Both the SVG and the PDF backend spell Rough path coordinates with it.
//! - `ecmascript_number`: ECMAScript `Number#toString`, the shortest
//!   round-trip digits with exponent notation from 1e21 and below 1e-6 and an
//!   explicit plus sign on positive exponents. The freedraw port reduces its
//!   coordinates through it the way the reference's `toFixed`-free path
//!   builder does.
//! - `push_svg_number` and `format_pdf_number`: Rust `Display`, which never
//!   uses exponent notation, with negative zero canonicalized to zero. The PDF
//!   variant additionally expands an exponent should one ever appear, since
//!   PDF numbers have no exponent syntax.
//!
//! `ecmascript_round` (`Math.round`) stays in `geometry`: it is a rounding
//! policy applied to coordinates, not a spelling.

use std::fmt::Write as _;

use super::FormatEncodeError;

pub(super) fn push_rough_number(
    output: &mut (impl std::fmt::Write + ?Sized),
    value: f64,
) -> Result<(), FormatEncodeError> {
    if !value.is_finite() || value.abs() >= 1e21 {
        return Err(FormatEncodeError::Encoding(
            "Rough path number is outside fixed-decimal range".to_owned(),
        ));
    }
    let mut buffer = ryu_js::Buffer::new();
    // Below 1e21 toFixed always includes the two fractional digits, so trimming
    // stops at the decimal point and cannot consume zeros in the integer part.
    let token = buffer.format_to_fixed(value, 2);
    let token = token.trim_end_matches('0').trim_end_matches('.');
    let token = if token == "-0" { "0" } else { token };
    output
        .write_str(token)
        .map_err(|_| FormatEncodeError::Encoding("failed to write Rough path number".to_owned()))
}

pub(super) fn ecmascript_number(value: f64) -> Result<String, FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(
            "ECMAScript number must be finite".to_owned(),
        ));
    }
    Ok(ryu_js::Buffer::new().format_finite(value).to_owned())
}

pub(super) fn push_svg_number<W: std::fmt::Write + ?Sized>(
    output: &mut W,
    value: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite"
        )));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    write!(output, "{value}")
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to write {subject}")))
}

pub(super) fn format_pdf_number(value: f64, subject: &str) -> Result<String, FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite"
        )));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    let mut rendered = String::new();
    rendered
        .try_reserve_exact(32)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_number",
        })?;
    write!(rendered, "{value}")
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to write {subject}")))?;
    let Some(exponent_marker) = rendered.find(['e', 'E']) else {
        return Ok(rendered);
    };
    expand_exponent(&rendered, exponent_marker, subject)
}

fn expand_exponent(
    rendered: &str,
    exponent_marker: usize,
    subject: &str,
) -> Result<String, FormatEncodeError> {
    let (mantissa, exponent) = rendered.split_at(exponent_marker);
    let exponent = exponent[1..]
        .parse::<i32>()
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to expand {subject} exponent")))?;
    let (negative, unsigned) = mantissa
        .strip_prefix('-')
        .map_or((false, mantissa), |unsigned| (true, unsigned));
    let decimal = unsigned.find('.').unwrap_or(unsigned.len());
    let mut digits = String::new();
    digits
        .try_reserve_exact(unsigned.len())
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_number",
        })?;
    digits.extend(unsigned.chars().filter(|character| *character != '.'));
    let decimal = i32::try_from(decimal)
        .map_err(|_| FormatEncodeError::Encoding(format!("{subject} is too long")))?
        + exponent;
    let mut output = String::new();
    output
        .try_reserve(
            rendered
                .len()
                .saturating_add(exponent.unsigned_abs() as usize),
        )
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_number",
        })?;
    if negative {
        output.push('-');
    }
    if decimal <= 0 {
        output.push_str("0.");
        for _ in 0..decimal.unsigned_abs() {
            output.push('0');
        }
        output.push_str(&digits);
    } else if usize::try_from(decimal).is_ok_and(|decimal| decimal >= digits.len()) {
        output.push_str(&digits);
        for _ in digits.len()..usize::try_from(decimal).unwrap_or(digits.len()) {
            output.push('0');
        }
    } else {
        let decimal = usize::try_from(decimal)
            .map_err(|_| FormatEncodeError::Encoding(format!("{subject} is too long")))?;
        output.push_str(&digits[..decimal]);
        output.push('.');
        output.push_str(&digits[decimal..]);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{
        ecmascript_number, expand_exponent, format_pdf_number, push_rough_number, push_svg_number,
    };

    fn rough(value: f64) -> String {
        let mut output = String::new();
        push_rough_number(&mut output, value).expect("finite Rough coordinate");
        output
    }

    fn svg(value: f64) -> String {
        let mut output = String::new();
        push_svg_number(&mut output, value, "test number").expect("finite SVG number");
        output
    }

    #[test]
    fn rough_numbers_carry_at_most_two_decimals_and_round_ties_up() {
        assert_eq!(rough(1.0), "1");
        assert_eq!(rough(1.5), "1.5");
        assert_eq!(rough(1.25), "1.25");
        assert_eq!(rough(-2.5), "-2.5");
        // `toFixed(2)` on the exact tie 60.125 rounds upward.
        assert_eq!(rough(60.125), "60.13");
        // A negative value that rounds to zero loses its sign.
        assert_eq!(rough(-0.001), "0");
        assert!(push_rough_number(&mut String::new(), f64::NAN).is_err());
        assert!(push_rough_number(&mut String::new(), 1e21).is_err());
    }

    #[test]
    fn ecmascript_numbers_spell_like_number_to_string() {
        assert_eq!(ecmascript_number(0.0).unwrap(), "0");
        assert_eq!(ecmascript_number(-0.0).unwrap(), "0");
        assert_eq!(ecmascript_number(1.5).unwrap(), "1.5");
        assert_eq!(ecmascript_number(0.1 + 0.2).unwrap(), "0.30000000000000004");
        // Exponent notation begins at 1e21 and below 1e-6, with an explicit
        // plus sign on positive exponents.
        assert_eq!(ecmascript_number(1e21).unwrap(), "1e+21");
        assert_eq!(
            ecmascript_number(123_456_789_012_345_680_000.0).unwrap(),
            "123456789012345680000"
        );
        assert_eq!(ecmascript_number(1e-7).unwrap(), "1e-7");
        assert_eq!(ecmascript_number(0.000_001).unwrap(), "0.000001");
    }

    #[test]
    fn ecmascript_spelling_matches_pinned_chromium_vectors() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/number-format-vectors.json"
        ))
        .unwrap();
        for vector in corpus["vectors"].as_array().unwrap() {
            let bits = u64::from_str_radix(vector["bits"].as_str().unwrap(), 16).unwrap();
            let value = f64::from_bits(bits);
            if let Some(expected) = vector["shortest"].as_str() {
                assert_eq!(ecmascript_number(value).unwrap(), expected, "{vector}");
            } else {
                assert!(ecmascript_number(value).is_err(), "{vector}");
            }
        }
    }

    #[test]
    fn rough_fixed_decimals_match_pinned_chromium_vectors() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/number-format-vectors.json"
        ))
        .unwrap();
        for vector in corpus["vectors"].as_array().unwrap() {
            let bits = u64::from_str_radix(vector["bits"].as_str().unwrap(), 16).unwrap();
            let value = f64::from_bits(bits);
            if let Some(expected) = vector["rough"].as_str() {
                assert_eq!(rough(value), expected, "{vector}");
                assert_eq!(
                    ryu_js::Buffer::new().format_to_fixed(value, 2),
                    vector["fixed"].as_str().unwrap(),
                    "{vector}"
                );
            } else {
                let mut output = String::from("unchanged");
                assert!(push_rough_number(&mut output, value).is_err(), "{vector}");
                assert_eq!(output, "unchanged");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    mod wasm_tests {
        #[wasm_bindgen_test::wasm_bindgen_test]
        fn number_formatting_matches_pinned_chromium_vectors() {
            super::ecmascript_spelling_matches_pinned_chromium_vectors();
            super::rough_fixed_decimals_match_pinned_chromium_vectors();
        }
    }

    #[test]
    fn display_numbers_canonicalize_negative_zero_and_reject_non_finite_values() {
        assert_eq!(svg(-0.0), "0");
        assert_eq!(svg(0.5), "0.5");
        assert_eq!(svg(1e-7), "0.0000001");
        assert!(push_svg_number(&mut String::new(), f64::INFINITY, "test number").is_err());

        assert_eq!(format_pdf_number(-0.0, "test number").unwrap(), "0");
        assert_eq!(format_pdf_number(12.75, "test number").unwrap(), "12.75");
        assert!(format_pdf_number(f64::NAN, "test number").is_err());
    }

    #[test]
    fn pdf_exponent_expansion_writes_plain_decimals() {
        assert_eq!(expand_exponent("1.5e3", 3, "test number").unwrap(), "1500");
        assert_eq!(expand_exponent("1e2", 1, "test number").unwrap(), "100");
        assert_eq!(
            expand_exponent("-2.5e-3", 4, "test number").unwrap(),
            "-0.0025"
        );
        assert_eq!(
            expand_exponent("12.5e-1", 4, "test number").unwrap(),
            "1.25"
        );
    }
}
