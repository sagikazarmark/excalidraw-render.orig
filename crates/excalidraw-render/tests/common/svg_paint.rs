//! The paint and text attributes of an SVG document as the SVG parity harness
//! compares them: what each painted shape and each text line is drawn with,
//! resolved through the group structure, so that two documents that group
//! their elements differently but paint the same compare equal.
//!
//! A painted shape is a `<path>` or `<rect>` outside definitions, clip paths
//! and masks. Its record is the fill, the stroke and, for a stroked shape, the
//! stroke width, line cap and dash array, each as inherited from the nearest
//! ancestor that sets it (or the SVG initial value), the product of every
//! ancestor's `opacity`, and the composed `transform`. Attributes on `<g>`
//! are judged this way, through the shapes and lines they paint, because the
//! two exporters group differently. The stroke's width, cap and dashes are
//! not compared on an unstroked shape, where they draw nothing and where the
//! official export writes `stroke-width="0"` on every fill-only path. A shape
//! that paints nothing (fill `none` or `transparent` and stroke `none`, as
//! the renderer's embeddable and link hit areas are) has no record.
//!
//! A text line is the run or runs of one `<text>` element that share a
//! baseline. Its record is its orientation (the linear part of the composed
//! transform), its baseline (the document position of its origin across the
//! baseline direction), the `font-size`, the fill and the opacity, and, when
//! the line is one run with one anchor position, its `x` (the origin's
//! position along the baseline) and `text-anchor`. Positions are compared in
//! the document frame because the two exporters reach the same place through
//! different group translations and local coordinates. A line whose glyphs
//! the document positions one by one, or that is split into font-fallback or
//! bidirectional runs, has no single anchor, so its `x` and `text-anchor` are
//! not compared: the renderer resolves the official anchor into glyph
//! positions there (spec 8.3), and where along the line they land is the
//! raster dimension's to judge.
//!
//! Numbers compare after rounding to six decimals, which absorbs the
//! reference's binary-float spellings (`6.900000000000006`) and nothing a
//! viewer could show; lengths drop a `px` unit; colours compare as lowercase
//! six-digit hex when they are hex. `font-family`, `clip-path` and `mask` are
//! never compared: the renderer spells its families as the spec's fallback
//! list (spec 8.4), and clip and mask references are ids, which differ.

use roxmltree::{Document, Node, ParsingOptions};

/// Elements whose content is not painted where it stands but referenced.
const UNPAINTED_CONTAINERS: &[&str] = &["defs", "clipPath", "mask", "symbol", "pattern", "marker"];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

type Matrix = [f64; 6];

/// One painted shape or text line and its compared attributes, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintRecord {
    /// `path`, `rect` or `text line`.
    pub kind: &'static str,
    pub attributes: Vec<(&'static str, String)>,
    /// Whether this is a text line without a single anchor position, whose
    /// `x` and `text-anchor` are not compared.
    pub resolved_line: bool,
}

/// Every painted shape and text line of `svg`, in document order.
pub fn paint_records(svg: &str) -> Vec<PaintRecord> {
    let document = Document::parse_with_options(
        svg,
        ParsingOptions {
            allow_dtd: true,
            nodes_limit: u32::MAX,
            ..ParsingOptions::default()
        },
    )
    .expect("SVG parses as XML");
    let mut records = Vec::new();
    for node in document.descendants().filter(Node::is_element) {
        if node
            .ancestors()
            .any(|ancestor| UNPAINTED_CONTAINERS.contains(&ancestor.tag_name().name()))
        {
            continue;
        }
        match node.tag_name().name() {
            "path" => records.extend(shape_record(node, "path")),
            "rect" => records.extend(shape_record(node, "rect")),
            "text" => records.extend(text_lines(node)),
            _ => {}
        }
    }
    records
}

/// The first differing record of each kind between `candidate` and
/// `official`, if any: a count mismatch, or the first attribute that differs.
pub fn attribute_divergence(candidate: &str, official: &str) -> Option<String> {
    let candidate = paint_records(candidate);
    let official = paint_records(official);
    let divergences: Vec<String> = ["path", "rect", "text line"]
        .into_iter()
        .filter_map(|kind| {
            let ours: Vec<_> = candidate.iter().filter(|r| r.kind == kind).collect();
            let theirs: Vec<_> = official.iter().filter(|r| r.kind == kind).collect();
            if ours.len() != theirs.len() {
                return Some(format!(
                    "paints {} {kind}s, official has {}",
                    ours.len(),
                    theirs.len()
                ));
            }
            ours.iter()
                .zip(&theirs)
                .enumerate()
                .find_map(|(index, (ours, theirs))| {
                    first_difference(ours, theirs).map(|(name, a, b)| {
                        format!("{kind} {index} {name} {a:?} against official {b:?}")
                    })
                })
        })
        .collect();
    (!divergences.is_empty()).then(|| divergences.join("; "))
}

fn first_difference(
    ours: &PaintRecord,
    theirs: &PaintRecord,
) -> Option<(&'static str, String, String)> {
    let unanchored = ours.resolved_line || theirs.resolved_line;
    let compared = |record: &PaintRecord| -> Vec<(&'static str, String)> {
        record
            .attributes
            .iter()
            .filter(|(name, _)| !(unanchored && matches!(*name, "x" | "text-anchor")))
            .cloned()
            .collect()
    };
    let ours = compared(ours);
    let theirs = compared(theirs);
    let names: Vec<&'static str> =
        ours.iter()
            .chain(&theirs)
            .map(|(name, _)| *name)
            .fold(Vec::new(), |mut names, name| {
                if !names.contains(&name) {
                    names.push(name);
                }
                names
            });
    let value = |record: &[(&'static str, String)], name: &str| {
        record
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map_or_else(|| "(unset)".to_owned(), |(_, value)| value.clone())
    };
    names.into_iter().find_map(|name| {
        let (a, b) = (value(&ours, name), value(&theirs, name));
        (a != b).then_some((name, a, b))
    })
}

fn shape_record(node: Node<'_, '_>, kind: &'static str) -> Option<PaintRecord> {
    let fill = colour(inherited(node, "fill").unwrap_or("black"));
    let stroke = colour(inherited(node, "stroke").unwrap_or("none"));
    let unpainted = |paint: &str| matches!(paint, "none" | "transparent");
    if unpainted(&fill) && unpainted(&stroke) {
        return None;
    }
    let mut attributes = vec![("fill", fill), ("stroke", stroke.clone())];
    if !unpainted(&stroke) {
        attributes.push((
            "stroke-width",
            length(inherited(node, "stroke-width").unwrap_or("1")),
        ));
        attributes.push((
            "stroke-linecap",
            inherited(node, "stroke-linecap")
                .unwrap_or("butt")
                .to_owned(),
        ));
        attributes.push((
            "stroke-dasharray",
            dash_array(inherited(node, "stroke-dasharray").unwrap_or("none")),
        ));
    }
    attributes.push(("opacity", number(opacity(node))));
    attributes.push(("transform", matrix_text(transform(node))));
    Some(PaintRecord {
        kind,
        attributes,
        resolved_line: false,
    })
}

/// The lines of one `<text>` element: its text-bearing runs (itself or its
/// `<tspan>`s with character data) grouped by baseline in document order.
fn text_lines(text: Node<'_, '_>) -> Vec<PaintRecord> {
    let runs: Vec<Node<'_, '_>> = text
        .descendants()
        .filter(|node| {
            matches!(node.tag_name().name(), "text" | "tspan")
                && node
                    .children()
                    .any(|child| child.is_text() && !child.text().unwrap_or("").trim().is_empty())
        })
        .collect();
    let mut lines: Vec<(String, Vec<Node<'_, '_>>)> = Vec::new();
    for run in runs {
        let baseline = number(first_coordinate(positioned(run, "y")));
        match lines.last_mut() {
            Some((y, members)) if *y == baseline => members.push(run),
            _ => lines.push((baseline, vec![run])),
        }
    }
    lines
        .into_iter()
        .map(|(_, members)| {
            let first = members[0];
            let matrix = transform(first);
            let (along, across) = baseline_frame(
                matrix,
                first_coordinate(positioned(first, "x")),
                first_coordinate(positioned(first, "y")),
            );
            let [a, b, c, d, ..] = matrix;
            let anchored = members.len() == 1
                && positioned(first, "x").is_none_or(|x| coordinates(x).len() <= 1);
            let joined = |value: &dyn Fn(Node<'_, '_>) -> String| {
                let mut values: Vec<String> = members.iter().map(|run| value(*run)).collect();
                values.dedup();
                values.join("|")
            };
            PaintRecord {
                kind: "text line",
                attributes: vec![
                    ("orientation", [a, b, c, d].map(number).join(" ")),
                    ("x", number(along)),
                    ("baseline", number(across)),
                    (
                        "font-size",
                        joined(&|run| length(inherited(run, "font-size").unwrap_or("medium"))),
                    ),
                    (
                        "text-anchor",
                        joined(&|run| inherited(run, "text-anchor").unwrap_or("start").to_owned()),
                    ),
                    (
                        "fill",
                        joined(&|run| colour(inherited(run, "fill").unwrap_or("black"))),
                    ),
                    ("opacity", joined(&|run| number(opacity(run)))),
                ],
                resolved_line: !anchored,
            }
        })
        .collect()
}

/// The value of an inherited presentation attribute at `node`.
fn inherited<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.ancestors()
        .find_map(|ancestor| ancestor.attribute(name))
}

/// A text position attribute (`x`, `y`) as the nearest run or text element
/// that sets it.
fn positioned<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.ancestors()
        .take_while(|ancestor| matches!(ancestor.tag_name().name(), "text" | "tspan"))
        .find_map(|ancestor| ancestor.attribute(name))
}

fn opacity(node: Node<'_, '_>) -> f64 {
    node.ancestors()
        .filter_map(|ancestor| ancestor.attribute("opacity"))
        .map(|value| value.trim().parse::<f64>().expect("opacity is a number"))
        .product()
}

fn transform(node: Node<'_, '_>) -> Matrix {
    let mut chain: Vec<&str> = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect();
    chain.reverse();
    chain
        .into_iter()
        .map(parse_transform)
        .fold(IDENTITY, multiply)
}

fn multiply(left: Matrix, right: Matrix) -> Matrix {
    let [a, b, c, d, e, f] = left;
    let [g, h, i, j, k, l] = right;
    [
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ]
}

/// An SVG `transform` list as one matrix.
pub fn parse_transform(value: &str) -> Matrix {
    let mut matrix = IDENTITY;
    let mut rest = value.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim_matches(|c: char| c == ',' || c.is_whitespace());
        let close = rest.find(')').expect("transform function closes");
        let arguments: Vec<f64> = coordinates(&rest[open + 1..close]);
        let step = match (name, arguments.as_slice()) {
            ("translate", [x]) => [1.0, 0.0, 0.0, 1.0, *x, 0.0],
            ("translate", [x, y]) => [1.0, 0.0, 0.0, 1.0, *x, *y],
            ("scale", [s]) => [*s, 0.0, 0.0, *s, 0.0, 0.0],
            ("scale", [x, y]) => [*x, 0.0, 0.0, *y, 0.0, 0.0],
            ("rotate", [angle, centre @ ..]) => {
                let (sin, cos) = libm::sincos(angle.to_radians());
                let rotation = [cos, sin, -sin, cos, 0.0, 0.0];
                match centre {
                    [] => rotation,
                    [x, y] => multiply(
                        multiply([1.0, 0.0, 0.0, 1.0, *x, *y], rotation),
                        [1.0, 0.0, 0.0, 1.0, -x, -y],
                    ),
                    _ => panic!("rotate takes one or three arguments: {value}"),
                }
            }
            ("matrix", [a, b, c, d, e, f]) => [*a, *b, *c, *d, *e, *f],
            _ => panic!("unsupported transform {name}({arguments:?}) in {value:?}"),
        };
        matrix = multiply(matrix, step);
        rest = &rest[close + 1..];
    }
    matrix
}

fn coordinates(list: &str) -> Vec<f64> {
    list.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().expect("coordinate is a number"))
        .collect()
}

fn first_coordinate(list: Option<&str>) -> f64 {
    list.and_then(|list| coordinates(list).first().copied())
        .unwrap_or(0.0)
}

fn matrix_text(matrix: Matrix) -> String {
    matrix.map(number).join(" ")
}

/// A number rounded to six decimals, without trailing zeros or a negative zero.
pub fn number(value: f64) -> String {
    let rounded = format!("{value:.6}");
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn length(value: &str) -> String {
    let value = value.trim();
    value
        .strip_suffix("px")
        .unwrap_or(value)
        .parse()
        .map_or_else(|_| value.to_owned(), number)
}

fn dash_array(value: &str) -> String {
    if value.trim() == "none" {
        return "none".to_owned();
    }
    coordinates(value)
        .into_iter()
        .map(number)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A colour as lowercase six-digit hex when it is three- or six-digit hex.
pub fn colour(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    match value.strip_prefix('#') {
        Some(hex) if hex.len() == 3 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => hex
            .chars()
            .flat_map(|c| [c, c])
            .fold(String::from("#"), |mut out, c| {
                out.push(c);
                out
            }),
        _ => value,
    }
}

/// The document position of a text origin `(x, y)` under `matrix`, split into
/// its distance along the text's baseline direction and across it. Two
/// documents that place a line through different group translations and
/// local coordinates agree on both; a line whose glyphs are positioned one by
/// one agrees on the second, its baseline, whatever its first glyph's
/// position along the line.
fn baseline_frame(matrix: Matrix, x: f64, y: f64) -> (f64, f64) {
    let [a, b, c, d, e, f] = matrix;
    let (origin_x, origin_y) = (a * x + c * y + e, b * x + d * y + f);
    let norm = libm::hypot(a, b);
    let (direction_x, direction_y) = (a / norm, b / norm);
    (
        origin_x * direction_x + origin_y * direction_y,
        origin_y * direction_x - origin_x * direction_y,
    )
}
