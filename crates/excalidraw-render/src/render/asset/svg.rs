use std::sync::Arc;

use base64::Engine as _;

use super::ResolveError;
use crate::ResourceLimits;

#[derive(Clone)]
pub(crate) struct Removal {
    pub(crate) node_ordinal: u64,
    pub(crate) attribute_ordinal: Option<u64>,
    pub(crate) name: String,
    pub(crate) changes_appearance: bool,
}

pub(super) struct SanitizedSvg {
    pub(super) bytes: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) removals: Vec<Removal>,
    pub(super) work_units: u64,
    pub(super) working_bytes: u64,
    pub(super) nested_asset_entries: u64,
    pub(super) nested_asset_bytes: u64,
    pub(super) nested_decoded_pixels: u64,
    pub(super) tree: Arc<usvg::Tree>,
}

pub(super) struct DecodedSvg {
    pub(super) rgba: Arc<Vec<u8>>,
    pub(super) premultiplied_rgba: Arc<Vec<u8>>,
}

pub(super) fn looks_like_svg(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<')
}

pub(super) fn sanitize(
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<SanitizedSvg, ResolveError> {
    sanitize_at(bytes, limits, 0)
}

fn sanitize_at(
    bytes: &[u8],
    limits: &ResourceLimits,
    depth: u32,
) -> Result<SanitizedSvg, ResolveError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ResolveError::SvgDecodeFailed)?;
    let (text, mut removals) = remove_dtd_and_entities(text)?;
    let document = roxmltree::Document::parse(&text).map_err(|_| ResolveError::SvgDecodeFailed)?;
    for instruction in document.descendants().filter_map(|node| node.pi()) {
        if instruction.target != "xml" {
            removals.push(Removal {
                node_ordinal: 0,
                attribute_ordinal: None,
                name: instruction.target.to_owned(),
                changes_appearance: true,
            });
        }
    }
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err(ResolveError::SvgDecodeFailed);
    }
    let nodes = document
        .descendants()
        .filter(|node| node.is_element() || node.is_text())
        .count();
    if nodes > usize::try_from(limits.max_nested_svg_nodes()).unwrap_or(usize::MAX) {
        return Err(ResolveError::SvgComplexity {
            attempted: u64::try_from(nodes).unwrap_or(u64::MAX),
        });
    }
    let mut serialized = String::new();
    let mut node_ordinal = 0;
    let mut nested_asset_entries = 0;
    let mut nested_asset_bytes = 0;
    let mut nested_decoded_pixels = 0;
    serialize_node(
        root,
        true,
        &mut node_ordinal,
        &mut serialized,
        &mut removals,
        limits,
        depth,
        &mut nested_asset_entries,
        &mut nested_asset_bytes,
        &mut nested_decoded_pixels,
    )?;
    let options = viewer_options()?;
    let tree =
        usvg::Tree::from_str(&serialized, &options).map_err(|_| ResolveError::SvgDecodeFailed)?;
    let size = tree.size().to_int_size();
    let width = size.width();
    let height = size.height();
    let attributes = document
        .descendants()
        .filter(|node| node.is_element())
        .map(|node| node.attributes().len())
        .sum::<usize>()
        .saturating_add(removals.len());
    let tokens = nodes.saturating_add(attributes);
    let local_references = document
        .descendants()
        .filter(|node| node.is_element())
        .flat_map(|node| node.attributes())
        .filter(|attribute| {
            matches!(attribute.name(), "href" | "xlink:href") || attribute.value().contains("url(")
        })
        .count();
    let semantic_records = u64::try_from(tokens)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(nodes).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(attributes).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(local_references).unwrap_or(u64::MAX));
    let work_units = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .saturating_add(semantic_records.saturating_mul(16));
    let working_bytes = 512_u64
        .saturating_add(u64::try_from(nodes).unwrap_or(u64::MAX).saturating_mul(128))
        .saturating_add(
            u64::try_from(
                attributes
                    .saturating_add(tokens)
                    .saturating_add(local_references),
            )
            .unwrap_or(u64::MAX)
            .saturating_mul(64),
        );
    Ok(SanitizedSvg {
        bytes: serialized.into_bytes(),
        width,
        height,
        removals,
        work_units,
        working_bytes,
        nested_asset_entries,
        nested_asset_bytes,
        nested_decoded_pixels,
        tree: Arc::new(tree),
    })
}

fn remove_dtd_and_entities(text: &str) -> Result<(String, Vec<Removal>), ResolveError> {
    let mut without_dtd = String::new();
    let mut removals = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = text[cursor..].find("<!DOCTYPE") {
        let start = cursor + relative;
        without_dtd.push_str(&text[cursor..start]);
        let end = doctype_end(&text[start..]).ok_or(ResolveError::SvgDecodeFailed)? + start;
        removals.push(Removal {
            node_ordinal: 0,
            attribute_ordinal: None,
            name: "DOCTYPE".to_owned(),
            changes_appearance: false,
        });
        cursor = end;
    }
    without_dtd.push_str(&text[cursor..]);

    let mut sanitized = String::new();
    let mut rest = without_dtd.as_str();
    while let Some(start) = rest.find('&') {
        sanitized.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';') else {
            sanitized.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        if matches!(entity, "amp" | "lt" | "gt" | "quot" | "apos")
            || entity
                .strip_prefix('#')
                .is_some_and(|number| valid_numeric_entity(number))
        {
            sanitized.push_str(&rest[..=end]);
        } else {
            removals.push(Removal {
                node_ordinal: 0,
                attribute_ordinal: None,
                name: entity.to_owned(),
                changes_appearance: true,
            });
        }
        rest = &rest[end + 1..];
    }
    sanitized.push_str(rest);
    Ok((sanitized, removals))
}

fn doctype_end(doctype: &str) -> Option<usize> {
    let mut quote = None;
    let mut subset_depth = 0_u32;
    for (index, character) in doctype.char_indices() {
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' => subset_depth = subset_depth.saturating_add(1),
            ']' => subset_depth = subset_depth.saturating_sub(1),
            '>' if subset_depth == 0 => return Some(index + 1),
            _ => {}
        }
    }
    None
}

fn valid_numeric_entity(number: &str) -> bool {
    number.strip_prefix(['x', 'X']).map_or_else(
        || !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()),
        |hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
    )
}

fn serialize_node(
    node: roxmltree::Node<'_, '_>,
    is_root: bool,
    node_ordinal: &mut u64,
    output: &mut String,
    removals: &mut Vec<Removal>,
    limits: &ResourceLimits,
    depth: u32,
    nested_asset_entries: &mut u64,
    nested_asset_bytes: &mut u64,
    nested_decoded_pixels: &mut u64,
) -> Result<(), ResolveError> {
    let ordinal = *node_ordinal;
    *node_ordinal = node_ordinal.saturating_add(1);
    if node.is_text() {
        escape_text(output, node.text().unwrap_or_default());
        return Ok(());
    }
    if !node.is_element() {
        return Ok(());
    }
    let name = node.tag_name().name();
    let svg_namespace = node.tag_name().namespace() == Some("http://www.w3.org/2000/svg");
    if !svg_namespace || !allowed_element(name) {
        removals.push(Removal {
            node_ordinal: ordinal,
            attribute_ordinal: None,
            name: name.to_owned(),
            changes_appearance: true,
        });
        count_descendants(node, node_ordinal);
        return Ok(());
    }

    output.push('<');
    output.push_str(name);
    if is_root {
        output.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    for (attribute_ordinal, attribute) in node.attributes().enumerate() {
        let attribute_name = attribute.name();
        let value = attribute.value();
        if unsafe_attribute(name, attribute_name, attribute.namespace(), value) {
            removals.push(Removal {
                node_ordinal: ordinal,
                attribute_ordinal: Some(u64::try_from(attribute_ordinal).unwrap_or(u64::MAX)),
                name: attribute_name.to_owned(),
                changes_appearance: attribute_name != "href" && attribute_name != "xlink:href",
            });
            continue;
        }
        let nested_value = if matches!(attribute_name, "href" | "xlink:href") {
            sanitize_nested_data_image(
                value,
                limits,
                depth,
                removals,
                nested_asset_entries,
                nested_asset_bytes,
                nested_decoded_pixels,
            )?
        } else {
            None
        };
        let value = nested_value.as_deref().unwrap_or(value);
        output.push(' ');
        if attribute.namespace() == Some("http://www.w3.org/1999/xlink") {
            output.push_str("href");
        } else {
            output.push_str(attribute_name);
        }
        output.push_str("=\"");
        escape_attribute(output, value);
        output.push('"');
    }
    output.push('>');
    for child in node.children() {
        if child.is_element() || child.is_text() {
            serialize_node(
                child,
                false,
                node_ordinal,
                output,
                removals,
                limits,
                depth,
                nested_asset_entries,
                nested_asset_bytes,
                nested_decoded_pixels,
            )?;
        }
    }
    output.push_str("</");
    output.push_str(name);
    output.push('>');
    Ok(())
}

fn sanitize_nested_data_image(
    value: &str,
    limits: &ResourceLimits,
    depth: u32,
    removals: &mut Vec<Removal>,
    nested_asset_entries: &mut u64,
    nested_asset_bytes: &mut u64,
    nested_decoded_pixels: &mut u64,
) -> Result<Option<String>, ResolveError> {
    let Some(data) = value.strip_prefix("data:") else {
        return Ok(None);
    };
    let Some((claimed_media_type, encoded)) = data.split_once(";base64,") else {
        return Err(ResolveError::SvgDecodeFailed);
    };
    let attempted_depth = depth.saturating_add(1);
    if attempted_depth > limits.max_nested_asset_depth() {
        return Err(ResolveError::SvgDepth {
            attempted: u64::from(attempted_depth),
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| ResolveError::SvgDecodeFailed)?;
    let decoded_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if decoded_bytes > limits.max_asset_bytes() {
        return Err(ResolveError::AssetBytes {
            attempted: decoded_bytes,
        });
    }
    let (media_type, canonical_bytes, child_entries, child_bytes, child_pixels) =
        if looks_like_svg(&bytes) {
            let nested = sanitize_at(&bytes, limits, attempted_depth)?;
            removals.extend(nested.removals);
            (
                "image/svg+xml",
                nested.bytes,
                nested.nested_asset_entries,
                nested.nested_asset_bytes,
                nested.nested_decoded_pixels,
            )
        } else {
            let (kind, pixels) = super::validate_nested_raster(&bytes, limits)?;
            (kind.media_type(), bytes, 0, 0, pixels)
        };
    if claimed_media_type != media_type {
        removals.push(Removal {
            node_ordinal: 0,
            attribute_ordinal: None,
            name: "media-type".to_owned(),
            changes_appearance: false,
        });
    }
    *nested_asset_entries = nested_asset_entries
        .saturating_add(1)
        .saturating_add(child_entries);
    *nested_asset_bytes = nested_asset_bytes
        .saturating_add(decoded_bytes)
        .saturating_add(child_bytes);
    *nested_decoded_pixels = nested_decoded_pixels.saturating_add(child_pixels);
    let mut canonical = format!("data:{media_type};base64,");
    base64::engine::general_purpose::STANDARD.encode_string(canonical_bytes, &mut canonical);
    Ok(Some(canonical))
}

fn count_descendants(node: roxmltree::Node<'_, '_>, node_ordinal: &mut u64) {
    let descendants = node
        .descendants()
        .skip(1)
        .filter(|node| node.is_element() || node.is_text())
        .count();
    *node_ordinal = node_ordinal.saturating_add(u64::try_from(descendants).unwrap_or(u64::MAX));
}

fn allowed_element(name: &str) -> bool {
    matches!(
        name,
        "svg"
            | "g"
            | "defs"
            | "symbol"
            | "use"
            | "title"
            | "desc"
            | "metadata"
            | "path"
            | "rect"
            | "circle"
            | "ellipse"
            | "line"
            | "polyline"
            | "polygon"
            | "linearGradient"
            | "radialGradient"
            | "stop"
            | "pattern"
            | "clipPath"
            | "mask"
            | "marker"
            | "image"
            | "text"
            | "tspan"
            | "textPath"
            | "a"
    )
}

fn unsafe_attribute(element_name: &str, name: &str, namespace: Option<&str>, value: &str) -> bool {
    if name.len() >= 2 && name[..2].eq_ignore_ascii_case("on") {
        return true;
    }
    if namespace.is_some_and(|namespace| {
        namespace != "http://www.w3.org/1999/xlink"
            && namespace != "http://www.w3.org/XML/1998/namespace"
    }) {
        return true;
    }
    if matches!(
        name,
        "href" | "xlink:href" | "src" | "action" | "formaction"
    ) {
        return !safe_href(element_name, value);
    }
    if name == "filter" {
        return true;
    }
    let lowercase = value.to_ascii_lowercase();
    lowercase.contains("@import")
        || lowercase.contains("javascript:")
        || lowercase.contains("expression(")
        || lowercase.contains("-moz-binding")
        || lowercase.contains("behavior:")
        || contains_nonlocal_url(&lowercase)
}

fn safe_href(element_name: &str, value: &str) -> bool {
    let value = value.trim();
    value.starts_with('#')
        || matches!(element_name, "image" | "feImage")
            && value.starts_with("data:image/")
            && value.contains(";base64,")
}

fn contains_nonlocal_url(value: &str) -> bool {
    let mut rest = value;
    while let Some(index) = rest.find("url(") {
        rest = &rest[index + 4..];
        let Some(end) = rest.find(')') else {
            return true;
        };
        let target = rest[..end].trim().trim_matches(['\'', '"']);
        if !target.starts_with('#') {
            return true;
        }
        rest = &rest[end + 1..];
    }
    false
}

fn escape_text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

fn escape_attribute(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '"' => output.push_str("&quot;"),
            '\r' => output.push_str("&#13;"),
            '\n' => output.push_str("&#10;"),
            '\t' => output.push_str("&#9;"),
            _ => output.push(character),
        }
    }
}

pub(super) fn render(svg: &SanitizedSvg) -> Result<DecodedSvg, ResolveError> {
    let mut pixmap = tiny_skia::Pixmap::new(svg.width, svg.height).ok_or(ResolveError::Memory)?;
    resvg::render(
        &svg.tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let premultiplied = pixmap.data().to_vec();
    let mut rgba = premultiplied.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha == 0 {
            pixel[..3].fill(0);
        } else {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Ok(DecodedSvg {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied),
    })
}

fn viewer_options() -> Result<usvg::Options<'static>, ResolveError> {
    let font = excalidraw_render_assets_base::load_excalifont_basic_latin_sfnt()
        .map_err(|_| ResolveError::SvgDecodeFailed)?;
    let mut options = usvg::Options::default();
    options.resources_dir = None;
    options.font_family = "Excalifont".to_owned();
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(font.bytes().to_vec());
    options.fontdb = Arc::new(fontdb);
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    Ok(options)
}
