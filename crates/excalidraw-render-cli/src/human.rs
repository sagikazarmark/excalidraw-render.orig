//! The human report: error lines with their fixes, and compatibility
//! diagnostics grouped for reading. The JSON report carries every occurrence;
//! this view only arranges them.

use std::fmt::Write as _;
use std::io;

use excalidraw_render::{CompatibilityDiagnostic, ErrorDescriptor};
use serde::de::IgnoredAny;
use serde::Deserialize;

/// One `error: MESSAGE [CODE]` line.
pub(crate) fn error_line(message: &str, code: &str) -> String {
    format!("error: {message} [{code}]\n")
}

/// `hint:` lines for `hints`, followed by the advice `code` implies.
pub(crate) fn hint_lines<'a>(code: &str, hints: impl IntoIterator<Item = &'a str>) -> String {
    hints
        .into_iter()
        .chain(code_hint(code))
        .map(|hint| format!("hint: {hint}\n"))
        .collect()
}

/// The fix a failure code implies whatever its context.
pub(crate) fn code_hint(code: &str) -> Option<&'static str> {
    Some(match code {
        "cli.destination-exists" => "use --force to overwrite it",
        "cli.format-required" => {
            "pass --format svg, png, or pdf, or name the output with a .svg, .png, or .pdf extension"
        }
        "cli.frame-no-match" => "run `excalidraw-render frames INPUT --list` to see the frames",
        "resource.raster-surface-exceeded" => {
            "use --max-dimension N (for example 4096) or a lower --scale to shrink the image"
        }
        "resource.svg-dimensions-exceeded" => "use a lower --scale",
        "pdf.raster-fallback-forbidden" => {
            "use --pdf-raster-fallback allow-isolated to rasterize what vector PDF cannot express"
        }
        "revision.unsupported" => "run `excalidraw-render --list-revisions` to see the supported revisions",
        _ => return None,
    })
}

/// A clap grammar error with `code` closing its first paragraph, the error
/// itself.
pub(crate) fn clap_error(rendered: &str, code: &str) -> String {
    let rendered = rendered.trim_end();
    let (error, rest) = match rendered.split_once("\n\n") {
        Some((error, rest)) => (error, Some(rest)),
        None => (rendered, None),
    };
    let mut text = format!("{error} [{code}]\n");
    if let Some(rest) = rest {
        text.push('\n');
        text.push_str(rest);
        text.push('\n');
    }
    text
}

/// The operating-system reason for an I/O failure, without the temporary
/// paths a wrapping layer may have appended.
pub(crate) fn os_reason(error: &io::Error) -> String {
    match error.raw_os_error() {
        Some(code) => io::Error::from_raw_os_error(code).to_string(),
        None => {
            let text = error.to_string();
            match text.split_once(" at path \"") {
                Some((reason, _)) => reason.to_owned(),
                None => text,
            }
        }
    }
}

/// How `path` is named in a message.
pub(crate) fn input_name(path: &str) -> String {
    if path == "-" {
        "standard input".to_owned()
    } else {
        format!("'{path}'")
    }
}

/// The human message and hints for a scene that failed to parse, recognizing
/// common files that are not scene documents.
pub(crate) fn scene_parse_context(
    input: &str,
    bytes: &[u8],
    error: &ErrorDescriptor,
) -> (String, Vec<String>) {
    let subject = format!("cannot parse {}", input_name(input));
    let image = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("a PNG")
    } else if looks_like_svg(bytes) {
        Some("an SVG")
    } else if bytes.starts_with(b"%PDF-") {
        Some("a PDF")
    } else {
        None
    };
    if let Some(kind) = image {
        return (
            format!("{subject}: it is {kind} image, not an Excalidraw scene"),
            vec![
                "PNG and SVG input is not supported; render the .excalidraw scene file instead \
                 (an image exported with an embedded scene can be opened in Excalidraw and saved \
                 as .excalidraw)"
                    .to_owned(),
            ],
        );
    }

    let mut message = format!("{subject}: {}", error.message());
    let mut hints = Vec::new();
    match error.code().as_str() {
        "scene.invalid-json" => {
            if let Err(syntax) = serde_json::from_slice::<IgnoredAny>(bytes) {
                if syntax.line() > 0 {
                    let _ = write!(
                        message,
                        " at line {}, column {}",
                        syntax.line(),
                        syntax.column()
                    );
                }
            }
        }
        "scene.invalid-envelope" => hints.push(envelope_hint(bytes).to_owned()),
        _ => {
            if let Some(pointer) = error.json_pointer().filter(|pointer| !pointer.is_empty()) {
                let _ = write!(message, " at {pointer}");
            }
        }
    }
    (message, hints)
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let start = bytes
        .strip_prefix("\u{feff}".as_bytes())
        .unwrap_or(bytes)
        .trim_ascii_start();
    start.starts_with(b"<svg") || start.starts_with(b"<?xml") || start.starts_with(b"<!DOCTYPE svg")
}

/// What a JSON document that is not a version-2 scene probably is.
fn envelope_hint(bytes: &[u8]) -> &'static str {
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(rename = "type")]
        kind: Option<serde_json::Value>,
        version: Option<serde_json::Value>,
    }
    let envelope = serde_json::from_slice::<Envelope>(bytes).ok();
    let kind = envelope
        .as_ref()
        .and_then(|envelope| envelope.kind.as_ref()?.as_str().map(str::to_owned));
    match kind.as_deref() {
        Some("excalidrawlib") => {
            "this is an Excalidraw library; only scene documents can be rendered"
        }
        Some("excalidraw/clipboard") => {
            "this is Excalidraw clipboard data; paste it into Excalidraw and save the scene as .excalidraw"
        }
        Some("excalidraw")
            if envelope
                .as_ref()
                .and_then(|envelope| envelope.version.as_ref())
                .is_some_and(|version| version != 2) =>
        {
            "only version 2 scene documents are supported"
        }
        _ => "an Excalidraw scene is a JSON object with \"type\": \"excalidraw\" and \"version\": 2",
    }
}

/// One `CODE: MESSAGE` line per diagnostic.
pub(crate) fn diagnostics(diagnostics: &[CompatibilityDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}\n", diagnostic.code(), diagnostic.message()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{clap_error, os_reason};
    use std::io;

    #[test]
    fn a_clap_error_carries_the_code_after_the_error_paragraph() {
        assert_eq!(
            clap_error("error: bad\n\nUsage: x\n", "cli.invalid-arguments"),
            "error: bad [cli.invalid-arguments]\n\nUsage: x\n"
        );
        assert_eq!(
            clap_error("error: missing:\n  --format <F>\n\nUsage: x", "c"),
            "error: missing:\n  --format <F> [c]\n\nUsage: x\n"
        );
        assert_eq!(clap_error("error: bad", "c"), "error: bad [c]\n");
    }

    #[test]
    fn os_reasons_drop_appended_temporary_paths() {
        let wrapped = io::Error::new(
            io::ErrorKind::NotFound,
            "No such file or directory (os error 2) at path \"/x/.a.svg.tmp\"",
        );
        assert_eq!(
            os_reason(&wrapped),
            "No such file or directory (os error 2)"
        );
        assert_eq!(
            os_reason(&io::Error::from_raw_os_error(2)),
            io::Error::from_raw_os_error(2).to_string()
        );
    }
}
