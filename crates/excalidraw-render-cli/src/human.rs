//! The human report: error lines with their fixes, and compatibility
//! diagnostics grouped for reading. The JSON report carries every occurrence;
//! this view only arranges them.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io;

use excalidraw_render::{
    CompatibilityDiagnostic, DiagnosticDetails, DiagnosticSeverity, ErrorDescriptor, OutputFormat,
};
use serde::de::IgnoredAny;
use serde::Deserialize;

/// How many element IDs and characters a grouped diagnostic names before it
/// summarizes the rest.
const SHOWN_ELEMENTS: usize = 3;
const SHOWN_CHARACTERS: usize = 5;
const SHOWN_PROPERTIES: usize = 3;

/// The library's message for official text drawn with the `?` replacement glyph.
const OFFICIAL_MISSING_GLYPH: &str =
    "official text used the deterministic basic-Latin replacement glyph";

/// Human-report choices that do not change the JSON report.
#[derive(Clone, Copy)]
pub(crate) struct HumanReport {
    /// Print informational diagnostics too.
    pub(crate) verbose: bool,
    /// `--max-dimension` was given for SVG output, where it does nothing.
    pub(crate) ignored_max_dimension: bool,
}

impl HumanReport {
    pub(crate) const PLAIN: Self = Self {
        verbose: false,
        ignored_max_dimension: false,
    };

    pub(crate) fn new(
        verbose: bool,
        max_dimension: Option<f64>,
        format: Option<OutputFormat>,
    ) -> Self {
        Self {
            verbose,
            ignored_max_dimension: max_dimension.is_some() && format == Some(OutputFormat::Svg),
        }
    }

    /// Warnings about the command line itself, printed before diagnostics.
    pub(crate) fn notes(self) -> &'static str {
        if self.ignored_max_dimension {
            "warning: --max-dimension applies only to PNG and PDF output and was ignored; use --scale to resize SVG\n"
        } else {
            ""
        }
    }
}

/// One `error: MESSAGE [CODE]` line.
pub(crate) fn error_line(message: &str, code: &str) -> String {
    format!("error: {message} [{code}]\n")
}

/// `hint:` lines for `hints`, or, when there are none, for the advice `code`
/// implies: advice specific to one failure replaces its code's general advice.
pub(crate) fn hint_lines<'a>(code: &str, hints: impl IntoIterator<Item = &'a str>) -> String {
    let mut hints = hints.into_iter().peekable();
    let general = hints.peek().is_none().then(|| code_hint(code)).flatten();
    hints
        .chain(general)
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
        // A PNG canvas gets advice sized from the error; this is for the
        // rest, such as content a PDF rasterizes.
        "resource.raster-surface-exceeded" => {
            "shrink the output with a lower --max-dimension or --scale, or render to SVG instead"
        }
        "resource.svg-dimensions-exceeded" => "use a lower --scale",
        "pdf.raster-fallback-forbidden" => {
            "use --pdf-raster-fallback allow-isolated to rasterize what vector PDF cannot express"
        }
        "revision.unsupported" => "run `excalidraw-render --list-revisions` to see the supported revisions",
        _ => return None,
    })
}

/// The fix for a PNG or PDF whose sizing rounds a side to zero. The smallest
/// valid sizing depends on the scene, so the library reports it as an
/// encoding failure that only its message distinguishes.
pub(crate) fn sizing_hint(code: &str, message: &str) -> Option<&'static str> {
    (matches!(code, "encode.png-failed" | "encode.pdf-failed")
        && message.contains("rounds to zero"))
    .then_some("use a larger --scale or --max-dimension")
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
    let unsupported = error.code().as_str() == "scene.unsupported-input";
    let image = if !unsupported {
        None
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
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

    if error.code().as_str() == "scene.invalid-json" && is_blank(bytes) {
        return (format!("{subject}: it is empty"), Vec::new());
    }

    let mut message = format!("{subject}: {}", error.message());
    let mut hints = Vec::new();
    match error.code().as_str() {
        "scene.unsupported-input" => hints.extend(envelope_hint(bytes).map(str::to_owned)),
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
        "scene.invalid-envelope" => hints.extend(
            missing_members_hint(bytes).or_else(|| envelope_hint(bytes).map(str::to_owned)),
        ),
        _ => {
            if let Some(pointer) = error.json_pointer().filter(|pointer| !pointer.is_empty()) {
                let _ = write!(message, " at {pointer}");
            }
        }
    }
    (message, hints)
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let start = without_bom(bytes).trim_ascii_start();
    start.starts_with(b"<svg") || start.starts_with(b"<?xml") || start.starts_with(b"<!DOCTYPE svg")
}

/// Whether `bytes` hold nothing but whitespace (or nothing at all).
fn is_blank(bytes: &[u8]) -> bool {
    without_bom(bytes).trim_ascii().is_empty()
}

fn without_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(bytes)
}

/// The members a version-2 scene document must have, with the empty value
/// that satisfies each. `appState` and `files` may be absent.
const SCENE_MEMBERS: [(&str, &str); 3] = [
    ("type", "\"excalidraw\""),
    ("version", "2"),
    ("elements", "[]"),
];

/// Every scene member a would-be scene lacks, in one hint, so a hand-written
/// or generated document is fixed in one pass rather than one member at a
/// time. `None` for a document that is something else, or lacks nothing.
fn missing_members_hint(bytes: &[u8]) -> Option<String> {
    let root = serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(bytes).ok()?;
    if root.get("type").is_some_and(|kind| kind != "excalidraw")
        || root.get("version").is_some_and(|version| version != 2)
    {
        return None;
    }
    let missing = SCENE_MEMBERS
        .iter()
        .filter(|(name, _)| !root.contains_key(*name))
        .map(|(name, value)| format!("\"{name}\": {value}"))
        .collect::<Vec<_>>();
    match missing.as_slice() {
        [] => None,
        [member] => Some(format!("add the missing member {member} to the scene")),
        members => Some(format!(
            "add the missing members {} to the scene",
            members.join(", ")
        )),
    }
}

/// What a JSON document that is not a version-2 scene probably is, when the
/// error does not already say: the library names libraries, clipboard data and
/// unsupported versions itself.
fn envelope_hint(bytes: &[u8]) -> Option<&'static str> {
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
    let version = envelope
        .as_ref()
        .and_then(|envelope| envelope.version.as_ref());
    match kind.as_deref() {
        Some("excalidrawlib" | "excalidraw/clipboard") => None,
        Some("excalidraw") if version.is_some_and(|version| version != 2) => version
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|version| version > 2)
            .then_some(
                "the file may come from a newer Excalidraw than this renderer supports; \
                 check for a newer excalidraw-render",
            ),
        _ => Some(
            "an Excalidraw scene is a JSON object with \"type\": \"excalidraw\" and \"version\": 2",
        ),
    }
}

/// Diagnostics with identical code and message folded into one line each, in
/// first-occurrence order. Informational ones are left out unless `verbose`.
pub(crate) fn diagnostics(diagnostics: &[CompatibilityDiagnostic], verbose: bool) -> String {
    struct Group<'a> {
        first: &'a CompatibilityDiagnostic,
        count: usize,
        elements: Vec<&'a str>,
        characters: Vec<u32>,
        properties: Vec<String>,
        // Membership only, never iterated: the lists above keep first-occurrence
        // order, and these keep a report of many (long) ids from comparing each
        // occurrence against every one already listed.
        seen_elements: HashSet<&'a str>,
        seen_characters: HashSet<u32>,
        seen_properties: HashSet<String>,
    }
    let mut groups: Vec<Group<'_>> = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.severity() == DiagnosticSeverity::Info && !verbose {
            continue;
        }
        let index = groups
            .iter()
            .position(|group| {
                group.first.code() == diagnostic.code()
                    && group.first.message() == diagnostic.message()
            })
            .unwrap_or_else(|| {
                groups.push(Group {
                    first: diagnostic,
                    count: 0,
                    elements: Vec::new(),
                    characters: Vec::new(),
                    properties: Vec::new(),
                    seen_elements: HashSet::new(),
                    seen_characters: HashSet::new(),
                    seen_properties: HashSet::new(),
                });
                groups.len() - 1
            });
        let group = &mut groups[index];
        group.count += 1;
        if let Some(element) = diagnostic.element_id() {
            if group.seen_elements.insert(element) {
                group.elements.push(element);
            }
        }
        if let Some(DiagnosticDetails::MissingGlyph { codepoint, .. }) = diagnostic.details() {
            if group.seen_characters.insert(*codepoint) {
                group.characters.push(*codepoint);
            }
        }
        if let Some(property) = property_path(diagnostic) {
            if group.seen_properties.insert(property.clone()) {
                group.properties.push(property);
            }
        }
    }

    let mut text = String::new();
    for group in groups {
        let severity = group.first.severity();
        // The library's message names the mechanism; say what the reader sees.
        let replaced_characters = group.first.code().as_str() == "font.missing-glyph"
            && group.first.message() == OFFICIAL_MISSING_GLYPH;
        let message = if replaced_characters {
            match group.count {
                1 => "1 character has no glyph in the bundled fonts and was drawn as \"?\""
                    .to_owned(),
                count => format!(
                    "{count} characters have no glyph in the bundled fonts and were drawn as \"?\""
                ),
            }
        } else if group.properties.is_empty() {
            group.first.message().to_owned()
        } else {
            format!(
                "{}: {}",
                group.first.message(),
                listed(&group.properties, SHOWN_PROPERTIES)
            )
        };
        let mut facts = Vec::new();
        if group.count > 1 && !replaced_characters {
            facts.push(format!("{} occurrences", group.count));
        }
        match group.elements.as_slice() {
            [] => {}
            [element] => facts.push(format!("element {element}")),
            elements => facts.push(format!("elements {}", listed(elements, SHOWN_ELEMENTS))),
        }
        if !group.characters.is_empty() {
            let characters = group
                .characters
                .iter()
                .map(|codepoint| format!("U+{codepoint:04X}"))
                .collect::<Vec<_>>();
            facts.push(format!(
                "characters {}",
                listed(&characters, SHOWN_CHARACTERS)
            ));
        }
        let _ = write!(text, "{severity}: {message} [{}]", group.first.code());
        if !facts.is_empty() {
            let _ = write!(text, " ({})", facts.join("; "));
        }
        text.push('\n');
    }
    text
}

/// The property a diagnostic is about, from its JSON pointer: the path within
/// its element (`startBinding.fixedPoint`), which the line names by ID, or
/// the whole path for anything outside the elements
/// (`appState.lockedMultiSelections`). `None` for a diagnostic about a whole
/// element or the whole scene.
fn property_path(diagnostic: &CompatibilityDiagnostic) -> Option<String> {
    let mut segments = diagnostic
        .json_pointer()
        .strip_prefix('/')?
        .split('/')
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    if diagnostic.element_id().is_some()
        && segments
            .first()
            .is_some_and(|segment| segment == "elements")
        && segments
            .get(1)
            .is_some_and(|index| index.bytes().all(|byte| byte.is_ascii_digit()))
    {
        segments.drain(..2);
    }
    (!segments.is_empty()).then(|| segments.join("."))
}

/// `a, b, c and 2 more`, naming at most `shown` items.
fn listed(items: &[impl AsRef<str>], shown: usize) -> String {
    let named = items
        .iter()
        .take(shown)
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ");
    match items.len().saturating_sub(shown) {
        0 => named,
        rest => format!("{named} and {rest} more"),
    }
}

#[cfg(test)]
mod tests {
    use super::{clap_error, listed, os_reason};
    use std::io;

    #[test]
    fn a_list_names_a_few_items_and_counts_the_rest() {
        assert_eq!(listed(&["a"], 3), "a");
        assert_eq!(listed(&["a", "b", "c"], 3), "a, b, c");
        assert_eq!(listed(&["a", "b", "c", "d", "e"], 3), "a, b, c and 2 more");
    }

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
