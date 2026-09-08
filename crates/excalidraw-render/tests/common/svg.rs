//! Reading the geometry out of a rendered SVG document.

/// Every `<path d>` value in document order: the path vectors, which are what
/// a geometry pin holds while the rest of the document is free to move.
pub fn path_data(svg: &str) -> Vec<&str> {
    svg.split("<path d=\"")
        .skip(1)
        .map(|tail| tail.split_once('"').expect("path data terminates").0)
        .collect()
}
