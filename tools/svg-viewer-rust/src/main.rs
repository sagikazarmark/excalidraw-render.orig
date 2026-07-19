use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use base64::Engine;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const ASSET_DECLARATION_ID: &str = "excalidraw-render/svg-viewer-asset-declaration/v1";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    if args.next().as_deref() != Some("render") {
        return Err(usage());
    }
    let input = PathBuf::from(args.next().ok_or_else(usage)?);
    let output = PathBuf::from(args.next().ok_or_else(usage)?);
    let declaration = args.next().map(PathBuf::from);
    if let Some(argument) = args.next() {
        return Err(format!("unexpected argument {argument:?}; {}", usage()));
    }

    let catalog = match declaration {
        Some(path) => AssetCatalog::read(&path)?,
        None => AssetCatalog::default(),
    };
    let svg =
        fs::read(&input).map_err(|error| format!("failed to read {}: {error}", input.display()))?;
    let used = validate_svg_resources(&svg, &catalog, 0)?;
    let fontdb = prepare_font_database(&catalog, &used.fonts)?;
    let empty_options = viewer_options(fontdb.clone(), Arc::new(BTreeMap::new()));
    let images = prepare_images(&catalog, &used.images, &empty_options)?;
    let options = viewer_options(fontdb, Arc::new(images));
    let tree = usvg::Tree::from_data(&svg, &options)
        .map_err(|error| format!("failed to parse SVG: {error}"))?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| "SVG dimensions cannot be rasterized".to_owned())?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let rgba = straight_alpha_rgba8(pixmap.data());
    fs::write(&output, rgba)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    println!(r#"{{"width":{},"height":{}}}"#, size.width(), size.height());
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AssetIdentity {
    media_type: String,
    sha256: [u8; 32],
}

impl AssetIdentity {
    fn new(media_type: &str, bytes: &[u8]) -> Self {
        Self {
            media_type: media_type.to_owned(),
            sha256: sha256(bytes),
        }
    }

    fn label(&self) -> String {
        format!("{}:{}", self.media_type, hex(&self.sha256))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DeclaredStyle {
    Normal,
    Italic,
    Oblique,
}

impl DeclaredStyle {
    fn parse(value: &str, label: &str) -> Result<Self, String> {
        match value {
            "normal" => Ok(Self::Normal),
            "italic" => Ok(Self::Italic),
            "oblique" => Ok(Self::Oblique),
            _ => Err(format!("{label} is not an admitted font style")),
        }
    }

    fn fontdb(self) -> usvg::fontdb::Style {
        match self {
            Self::Normal => usvg::fontdb::Style::Normal,
            Self::Italic => usvg::fontdb::Style::Italic,
            Self::Oblique => usvg::fontdb::Style::Oblique,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DeclaredStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl DeclaredStretch {
    fn parse(value: &str, label: &str) -> Result<Self, String> {
        match value {
            "ultra-condensed" => Ok(Self::UltraCondensed),
            "extra-condensed" => Ok(Self::ExtraCondensed),
            "condensed" => Ok(Self::Condensed),
            "semi-condensed" => Ok(Self::SemiCondensed),
            "normal" => Ok(Self::Normal),
            "semi-expanded" => Ok(Self::SemiExpanded),
            "expanded" => Ok(Self::Expanded),
            "extra-expanded" => Ok(Self::ExtraExpanded),
            "ultra-expanded" => Ok(Self::UltraExpanded),
            _ => Err(format!("{label} is not an admitted font stretch")),
        }
    }

    fn fontdb(self) -> usvg::fontdb::Stretch {
        match self {
            Self::UltraCondensed => usvg::fontdb::Stretch::UltraCondensed,
            Self::ExtraCondensed => usvg::fontdb::Stretch::ExtraCondensed,
            Self::Condensed => usvg::fontdb::Stretch::Condensed,
            Self::SemiCondensed => usvg::fontdb::Stretch::SemiCondensed,
            Self::Normal => usvg::fontdb::Stretch::Normal,
            Self::SemiExpanded => usvg::fontdb::Stretch::SemiExpanded,
            Self::Expanded => usvg::fontdb::Stretch::Expanded,
            Self::ExtraExpanded => usvg::fontdb::Stretch::ExtraExpanded,
            Self::UltraExpanded => usvg::fontdb::Stretch::UltraExpanded,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FaceProperties {
    family: String,
    style: DeclaredStyle,
    weight: u16,
    stretch: DeclaredStretch,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct UnicodeRange {
    start: u32,
    end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CssFace {
    properties: FaceProperties,
    unicode_range: Vec<UnicodeRange>,
}

#[derive(Clone, Debug)]
struct FileDescriptor {
    path: String,
    media_type: String,
    sha256: [u8; 32],
}

#[derive(Clone, Debug)]
struct SfntDescriptor {
    file: FileDescriptor,
    face_index: u32,
    metadata: FaceProperties,
}

#[derive(Clone, Debug)]
struct DeclaredFont {
    source: AssetIdentity,
    css: CssFace,
    sfnt: SfntDescriptor,
}

#[derive(Clone, Debug)]
struct DeclaredImage {
    source: AssetIdentity,
    canonical: FileDescriptor,
}

#[derive(Default)]
struct AssetCatalog {
    fonts: Vec<DeclaredFont>,
    images: Vec<DeclaredImage>,
}

impl AssetCatalog {
    fn read(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "failed to read asset declaration {}: {error}",
                path.display()
            )
        })?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("invalid asset declaration JSON: {error}"))?;
        let object = exact_object(&value, "asset declaration", &["schema", "fonts", "images"])?;
        if string_field(object, "schema", "asset declaration")? != ASSET_DECLARATION_ID {
            return Err("unsupported asset declaration schema".to_owned());
        }
        let font_values = array_field(object, "fonts", "asset declaration")?;
        let image_values = array_field(object, "images", "asset declaration")?;
        let mut fonts = Vec::with_capacity(font_values.len());
        for (index, value) in font_values.iter().enumerate() {
            fonts.push(parse_declared_font(value, index)?);
        }
        let mut images = Vec::with_capacity(image_values.len());
        for (index, value) in image_values.iter().enumerate() {
            images.push(parse_declared_image(value, index)?);
        }

        reject_duplicate(
            fonts.iter().map(|font| font.source.clone()),
            "font source identity",
        )?;
        reject_duplicate(
            fonts.iter().map(|font| font.css.properties.clone()),
            "font CSS face",
        )?;
        reject_duplicate(
            images.iter().map(|image| image.source.clone()),
            "image source identity",
        )?;
        Ok(Self { fonts, images })
    }

    fn font(&self, identity: &AssetIdentity) -> Option<&DeclaredFont> {
        self.fonts.iter().find(|font| &font.source == identity)
    }

    fn image(&self, identity: &AssetIdentity) -> Option<&DeclaredImage> {
        self.images.iter().find(|image| &image.source == identity)
    }
}

fn parse_declared_font(value: &Value, index: usize) -> Result<DeclaredFont, String> {
    let label = format!("fonts[{index}]");
    let object = exact_object(value, &label, &["source", "css", "sfnt"])?;
    let source = parse_identity(
        field(object, "source", &label)?,
        &format!("{label}.source"),
        &["font/woff2"],
    )?;
    let css = parse_css_face(field(object, "css", &label)?, &format!("{label}.css"))?;
    let sfnt = parse_sfnt_descriptor(field(object, "sfnt", &label)?, &format!("{label}.sfnt"))?;
    Ok(DeclaredFont { source, css, sfnt })
}

fn parse_declared_image(value: &Value, index: usize) -> Result<DeclaredImage, String> {
    let label = format!("images[{index}]");
    let object = exact_object(value, &label, &["source", "canonical"])?;
    let source = parse_identity(
        field(object, "source", &label)?,
        &format!("{label}.source"),
        &[
            "image/png",
            "image/apng",
            "image/jpeg",
            "image/jfif",
            "image/gif",
            "image/webp",
            "image/bmp",
            "image/x-icon",
            "image/avif",
            "image/svg+xml",
        ],
    )?;
    let canonical = parse_file_descriptor(
        field(object, "canonical", &label)?,
        &format!("{label}.canonical"),
        &["image/png", "image/svg+xml"],
    )?;
    let expected = if source.media_type == "image/svg+xml" {
        "image/svg+xml"
    } else {
        "image/png"
    };
    if canonical.media_type != expected {
        return Err(format!(
            "{label}.canonical.media_type must be {expected} for {}",
            source.media_type
        ));
    }
    Ok(DeclaredImage { source, canonical })
}

fn parse_identity(
    value: &Value,
    label: &str,
    media_types: &[&str],
) -> Result<AssetIdentity, String> {
    let object = exact_object(value, label, &["media_type", "sha256"])?;
    let media_type = string_field(object, "media_type", label)?;
    if !media_types.contains(&media_type) {
        return Err(format!("{label}.media_type is not admitted"));
    }
    Ok(AssetIdentity {
        media_type: media_type.to_owned(),
        sha256: parse_sha256(
            string_field(object, "sha256", label)?,
            &format!("{label}.sha256"),
        )?,
    })
}

fn parse_css_face(value: &Value, label: &str) -> Result<CssFace, String> {
    let object = exact_object(
        value,
        label,
        &["family", "style", "weight", "stretch", "unicode_range"],
    )?;
    let properties = parse_face_properties(object, label)?;
    let values = array_field(object, "unicode_range", label)?;
    if values.is_empty() {
        return Err(format!("{label}.unicode_range must not be empty"));
    }
    let mut text = Vec::with_capacity(values.len());
    for value in values {
        text.push(
            value
                .as_str()
                .ok_or_else(|| format!("{label}.unicode_range entries must be strings"))?,
        );
    }
    Ok(CssFace {
        properties,
        unicode_range: parse_unicode_ranges(&text.join(","), label)?,
    })
}

fn parse_sfnt_descriptor(value: &Value, label: &str) -> Result<SfntDescriptor, String> {
    let object = exact_object(
        value,
        label,
        &[
            "path",
            "media_type",
            "sha256",
            "face_index",
            "family",
            "style",
            "weight",
            "stretch",
        ],
    )?;
    let file = parse_file_descriptor_object(object, label, &["font/ttf", "font/otf"])?;
    let face_index = integer_field(object, "face_index", label)?;
    let face_index =
        u32::try_from(face_index).map_err(|_| format!("{label}.face_index must be a u32"))?;
    Ok(SfntDescriptor {
        file,
        face_index,
        metadata: parse_face_properties(object, label)?,
    })
}

fn parse_file_descriptor(
    value: &Value,
    label: &str,
    media_types: &[&str],
) -> Result<FileDescriptor, String> {
    let object = exact_object(value, label, &["path", "media_type", "sha256"])?;
    parse_file_descriptor_object(object, label, media_types)
}

fn parse_file_descriptor_object(
    object: &Map<String, Value>,
    label: &str,
    media_types: &[&str],
) -> Result<FileDescriptor, String> {
    let path = string_field(object, "path", label)?;
    validate_repository_path(path).map_err(|error| format!("{label}.path {error}"))?;
    let media_type = string_field(object, "media_type", label)?;
    if !media_types.contains(&media_type) {
        return Err(format!("{label}.media_type is not admitted"));
    }
    Ok(FileDescriptor {
        path: path.to_owned(),
        media_type: media_type.to_owned(),
        sha256: parse_sha256(
            string_field(object, "sha256", label)?,
            &format!("{label}.sha256"),
        )?,
    })
}

fn parse_face_properties(
    object: &Map<String, Value>,
    label: &str,
) -> Result<FaceProperties, String> {
    let family = string_field(object, "family", label)?;
    if family.is_empty() || family.chars().any(char::is_control) {
        return Err(format!(
            "{label}.family must be a non-empty printable string"
        ));
    }
    let weight = integer_field(object, "weight", label)?;
    let weight = u16::try_from(weight)
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| format!("{label}.weight must be an integer from 1 through 1000"))?;
    Ok(FaceProperties {
        family: family.to_owned(),
        style: DeclaredStyle::parse(
            string_field(object, "style", label)?,
            &format!("{label}.style"),
        )?,
        weight,
        stretch: DeclaredStretch::parse(
            string_field(object, "stretch", label)?,
            &format!("{label}.stretch"),
        )?,
    })
}

fn exact_object<'a>(
    value: &'a Value,
    label: &str,
    expected: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))?;
    let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    let mut expected = expected.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    if actual != expected {
        return Err(format!(
            "{label} must contain exactly: {}",
            expected.join(", ")
        ));
    }
    Ok(object)
}

fn field<'a>(object: &'a Map<String, Value>, name: &str, label: &str) -> Result<&'a Value, String> {
    object
        .get(name)
        .ok_or_else(|| format!("{label}.{name} is required"))
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    label: &str,
) -> Result<&'a str, String> {
    field(object, name, label)?
        .as_str()
        .ok_or_else(|| format!("{label}.{name} must be a string"))
}

fn integer_field(object: &Map<String, Value>, name: &str, label: &str) -> Result<u64, String> {
    field(object, name, label)?
        .as_u64()
        .ok_or_else(|| format!("{label}.{name} must be a non-negative integer"))
}

fn array_field<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    label: &str,
) -> Result<&'a [Value], String> {
    field(object, name, label)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label}.{name} must be an array"))
}

fn reject_duplicate<T: Ord>(
    values: impl IntoIterator<Item = T>,
    label: &str,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("duplicate {label}"));
        }
    }
    Ok(())
}

#[derive(Default)]
struct UsedAssets {
    fonts: BTreeSet<AssetIdentity>,
    images: BTreeSet<AssetIdentity>,
}

fn validate_svg_resources(
    svg: &[u8],
    catalog: &AssetCatalog,
    depth: usize,
) -> Result<UsedAssets, String> {
    if depth > 8 {
        return Err("nested SVG image depth exceeds 8".to_owned());
    }
    let text = std::str::from_utf8(svg).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!doctype")
        || lower.contains("<!entity")
        || lower.contains("<script")
        || lower.contains("<foreignobject")
    {
        return Err("forbidden active SVG content".to_owned());
    }
    let document = roxmltree::Document::parse(text)
        .map_err(|error| format!("failed to parse SVG XML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err("input is not a top-level SVG document".to_owned());
    }

    let mut used = UsedAssets::default();
    let mut font_face_count = 0usize;
    let mut has_text = false;
    let mut text_families = BTreeSet::new();
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let name = node.tag_name().name();
        if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("foreignObject") {
            return Err("forbidden active SVG content".to_owned());
        }
        if name == "text" {
            has_text = true;
            text_families.insert(explicit_text_family(node)?);
        }
        if name == "style" {
            let css = node
                .children()
                .filter_map(|child| child.text())
                .collect::<String>();
            let fonts = validate_stylesheet(&css, catalog)?;
            font_face_count += fonts.len();
            used.fonts.extend(fonts);
        }

        let hrefs = node
            .attributes()
            .filter(|attribute| attribute.name() == "href")
            .map(|attribute| attribute.value().trim())
            .collect::<Vec<_>>();
        if name == "image" {
            if hrefs.len() != 1 {
                return Err("each SVG image must have exactly one href".to_owned());
            }
            let (media_type, bytes) = parse_data_url(hrefs[0], "image")?;
            if !is_source_image_media_type(&media_type) {
                return Err(format!("forbidden image media type: {media_type}"));
            }
            validate_media(&media_type, &bytes)?;
            let identity = AssetIdentity::new(&media_type, &bytes);
            if catalog.image(&identity).is_none() {
                return Err(format!(
                    "undeclared embedded image resource: {}",
                    identity.label()
                ));
            }
            used.images.insert(identity);
            if media_type == "image/svg+xml" {
                let nested = validate_svg_resources(&bytes, catalog, depth + 1)?;
                used.fonts.extend(nested.fonts);
                used.images.extend(nested.images);
            }
        }

        for attribute in node.attributes() {
            if attribute.name().to_ascii_lowercase().starts_with("on") {
                return Err("forbidden active SVG content".to_owned());
            }
            validate_css_resources(attribute.value())?;
            if matches!(attribute.name(), "href" | "src") {
                if name == "a" && attribute.name() == "href" {
                    continue;
                }
                if name == "image" && attribute.name() == "href" {
                    continue;
                }
                if !attribute.value().trim().starts_with('#') {
                    return Err(format!(
                        "forbidden external SVG resource: {}",
                        attribute.value()
                    ));
                }
            }
        }
        if matches!(name, "animate" | "set")
            && node.attribute("attributeName").is_some_and(|name| {
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "href" | "xlink:href" | "src"
                )
            })
        {
            return Err("animated resource attributes are forbidden".to_owned());
        }
    }
    if has_text && font_face_count == 0 {
        return Err(
            "undeclared font resource: text requires a declared embedded font face".to_owned(),
        );
    }
    for family in text_families {
        if !used.fonts.iter().any(|identity| {
            catalog
                .font(identity)
                .is_some_and(|font| font.css.properties.family == family)
        }) {
            return Err(format!(
                "undeclared font resource: text uses family {family:?} without a matching embedded face"
            ));
        }
    }
    Ok(used)
}

fn explicit_text_family(node: roxmltree::Node<'_, '_>) -> Result<String, String> {
    for ancestor in node.ancestors().filter(roxmltree::Node::is_element) {
        if let Some(family) = ancestor.attribute("font-family") {
            return one_font_family(family);
        }
        if let Some(style) = ancestor.attribute("style") {
            for declaration in style.split(';') {
                let Some((name, value)) = declaration.split_once(':') else {
                    continue;
                };
                if name.trim().eq_ignore_ascii_case("font-family") {
                    return one_font_family(value);
                }
            }
        }
    }
    Err(
        "undeclared font resource: every text node requires an explicit declared font-family"
            .to_owned(),
    )
}

fn one_font_family(value: &str) -> Result<String, String> {
    let family = value
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'));
    if family.is_empty() || family.contains(',') || family.chars().any(char::is_control) {
        return Err(
            "undeclared font resource: text font-family must name exactly one declared face"
                .to_owned(),
        );
    }
    Ok(family.to_owned())
}

fn validate_stylesheet(css: &str, catalog: &AssetCatalog) -> Result<Vec<AssetIdentity>, String> {
    let clean = strip_css_comments(css)?;
    if clean.contains('\\') {
        return Err("CSS escapes are not admitted in SVG resources".to_owned());
    }
    if clean.to_ascii_lowercase().contains("@import") {
        return Err("forbidden stylesheet resource".to_owned());
    }
    let blocks = extract_font_face_blocks(&clean)?;
    let mut masked = clean.into_bytes();
    let mut used = Vec::with_capacity(blocks.len());
    for block in blocks {
        used.push(validate_font_face(&block.body, catalog)?);
        masked[block.start..block.end].fill(b' ');
    }
    let masked = String::from_utf8(masked).expect("masking CSS with ASCII spaces preserves UTF-8");
    validate_css_resources(&masked)?;
    Ok(used)
}

struct CssBlock {
    start: usize,
    end: usize,
    body: String,
}

fn extract_font_face_blocks(css: &str) -> Result<Vec<CssBlock>, String> {
    let lower = css.to_ascii_lowercase();
    let bytes = css.as_bytes();
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = lower[cursor..].find("@font-face") {
        let start = cursor + relative;
        let mut open = start + "@font-face".len();
        while bytes.get(open).is_some_and(u8::is_ascii_whitespace) {
            open += 1;
        }
        if bytes.get(open) != Some(&b'{') {
            return Err("invalid @font-face rule".to_owned());
        }
        let mut quote = None;
        let mut end = open + 1;
        let mut closed = false;
        while end < bytes.len() {
            let byte = bytes[end];
            if let Some(expected) = quote {
                if byte == expected {
                    quote = None;
                }
            } else if matches!(byte, b'\'' | b'"') {
                quote = Some(byte);
            } else if byte == b'{' {
                return Err("nested @font-face blocks are not admitted".to_owned());
            } else if byte == b'}' {
                end += 1;
                closed = true;
                break;
            }
            end += 1;
        }
        if !closed || quote.is_some() {
            return Err("unterminated @font-face rule".to_owned());
        }
        blocks.push(CssBlock {
            start,
            end,
            body: css[open + 1..end - 1].to_owned(),
        });
        cursor = end;
    }
    Ok(blocks)
}

fn validate_font_face(body: &str, catalog: &AssetCatalog) -> Result<AssetIdentity, String> {
    let allowed = [
        "font-family",
        "font-style",
        "font-weight",
        "font-stretch",
        "unicode-range",
        "src",
    ];
    let mut declarations = BTreeMap::new();
    for declaration in split_css_declarations(body)? {
        let (name, value) = declaration
            .split_once(':')
            .ok_or_else(|| format!("invalid @font-face declaration: {declaration}"))?;
        let name = name.trim().to_ascii_lowercase();
        if !allowed.contains(&name.as_str()) {
            return Err(format!("unsupported @font-face descriptor: {name}"));
        }
        if declarations
            .insert(name.clone(), value.trim().to_owned())
            .is_some()
        {
            return Err(format!("duplicate @font-face descriptor: {name}"));
        }
    }
    let family = parse_css_family(
        declarations
            .get("font-family")
            .ok_or_else(|| "@font-face requires font-family and src descriptors".to_owned())?,
    )?;
    let source = declarations
        .get("src")
        .ok_or_else(|| "@font-face requires font-family and src descriptors".to_owned())?;
    let style_text = declarations
        .get("font-style")
        .map_or("normal", String::as_str);
    let style = DeclaredStyle::parse(&style_text.to_ascii_lowercase(), "@font-face style")?;
    let weight = parse_css_weight(
        declarations
            .get("font-weight")
            .map_or("normal", String::as_str),
    )?;
    let stretch_text = declarations
        .get("font-stretch")
        .map_or("normal", String::as_str)
        .to_ascii_lowercase();
    let stretch = DeclaredStretch::parse(&stretch_text, "@font-face stretch")?;
    let unicode_range = parse_unicode_ranges(
        declarations
            .get("unicode-range")
            .map_or("U+0-10FFFF", String::as_str),
        "@font-face",
    )?;
    let (media_type, bytes) = parse_font_source(source)?;
    validate_media(&media_type, &bytes)?;
    let identity = AssetIdentity::new(&media_type, &bytes);
    let declared = catalog
        .font(&identity)
        .ok_or_else(|| format!("undeclared embedded font resource: {}", identity.label()))?;
    let actual = CssFace {
        properties: FaceProperties {
            family,
            style,
            weight,
            stretch,
        },
        unicode_range,
    };
    if actual != declared.css {
        return Err(format!(
            "@font-face metadata does not match declaration for {}",
            identity.label()
        ));
    }
    Ok(identity)
}

fn split_css_declarations(body: &str) -> Result<Vec<String>, String> {
    let bytes = body.as_bytes();
    let mut declarations = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut parentheses = 0u32;
    for index in 0..=bytes.len() {
        let byte = bytes.get(index).copied().unwrap_or(b';');
        if let Some(expected) = quote {
            if byte == expected {
                quote = None;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        } else if byte == b'(' {
            parentheses += 1;
        } else if byte == b')' {
            parentheses = parentheses
                .checked_sub(1)
                .ok_or_else(|| "invalid CSS parentheses".to_owned())?;
        } else if byte == b';' && parentheses == 0 {
            let declaration = body[start..index].trim();
            if !declaration.is_empty() {
                declarations.push(declaration.to_owned());
            }
            start = index + 1;
        }
    }
    if quote.is_some() || parentheses != 0 {
        return Err("unterminated CSS declaration".to_owned());
    }
    Ok(declarations)
}

fn parse_font_source(value: &str) -> Result<(String, Vec<u8>), String> {
    let (url, rest) = parse_css_url(value)?;
    let lower = rest.trim().to_ascii_lowercase();
    let format = lower
        .strip_prefix("format(")
        .and_then(|value| value.strip_suffix(')'))
        .map(str::trim)
        .map(unquote);
    if format != Some("woff2") {
        return Err("@font-face src must be one WOFF2 data URL with format(woff2)".to_owned());
    }
    let (media_type, bytes) = parse_data_url(&url, "font")?;
    if media_type != "font/woff2" {
        return Err(format!("forbidden font media type: {media_type}"));
    }
    Ok((media_type, bytes))
}

fn parse_css_url(value: &str) -> Result<(String, &str), String> {
    let value = value.trim_start();
    if value
        .as_bytes()
        .get(..4)
        .is_none_or(|prefix| !prefix.eq_ignore_ascii_case(b"url("))
    {
        return Err("expected CSS url()".to_owned());
    }
    let bytes = value.as_bytes();
    let mut cursor = 4usize;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    let quote = bytes
        .get(cursor)
        .copied()
        .filter(|byte| matches!(byte, b'\'' | b'"'));
    if quote.is_some() {
        cursor += 1;
    }
    let start = cursor;
    if let Some(quote) = quote {
        while bytes.get(cursor).is_some_and(|byte| *byte != quote) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&quote) {
            return Err("unterminated quoted CSS URL".to_owned());
        }
    } else {
        while bytes.get(cursor).is_some_and(|byte| *byte != b')') {
            cursor += 1;
        }
    }
    let url = value[start..cursor].trim().to_owned();
    if quote.is_some() {
        cursor += 1;
    }
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b')') {
        return Err("invalid CSS URL".to_owned());
    }
    Ok((url, &value[cursor + 1..]))
}

fn parse_css_family(value: &str) -> Result<String, String> {
    let family = unquote(value.trim());
    if family.is_empty() || family.contains(',') || family.chars().any(char::is_control) {
        return Err("@font-face family must be one printable family name".to_owned());
    }
    Ok(family.to_owned())
}

fn parse_css_weight(value: &str) -> Result<u16, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" => Ok(700),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| format!("unsupported @font-face weight: {value}")),
    }
}

fn parse_unicode_ranges(value: &str, label: &str) -> Result<Vec<UnicodeRange>, String> {
    let mut ranges = Vec::new();
    for value in value.split(',') {
        let token = value.trim().to_ascii_uppercase();
        let digits = token
            .strip_prefix("U+")
            .ok_or_else(|| format!("{label} has invalid unicode-range: {value}"))?;
        let range = if let Some((start, end)) = digits.split_once('-') {
            UnicodeRange {
                start: parse_hex_codepoint(start, label)?,
                end: parse_hex_codepoint(end, label)?,
            }
        } else if let Some(question) = digits.find('?') {
            let (prefix, suffix) = digits.split_at(question);
            if prefix.is_empty()
                || suffix.is_empty()
                || suffix.len() > 5
                || !suffix.bytes().all(|byte| byte == b'?')
            {
                return Err(format!("{label} has invalid unicode-range: {value}"));
            }
            UnicodeRange {
                start: parse_hex_codepoint(
                    &format!("{prefix}{}", "0".repeat(suffix.len())),
                    label,
                )?,
                end: parse_hex_codepoint(&format!("{prefix}{}", "F".repeat(suffix.len())), label)?,
            }
        } else {
            let codepoint = parse_hex_codepoint(digits, label)?;
            UnicodeRange {
                start: codepoint,
                end: codepoint,
            }
        };
        if range.start > range.end || range.end > 0x10ffff {
            return Err(format!("{label} unicode-range is outside Unicode"));
        }
        ranges.push(range);
    }
    if ranges.is_empty() {
        return Err(format!("{label} unicode-range must not be empty"));
    }
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[1].start <= pair[0].end {
            return Err(format!("{label} unicode-range entries overlap"));
        }
    }
    Ok(ranges)
}

fn parse_hex_codepoint(value: &str, label: &str) -> Result<u32, String> {
    if value.is_empty() || value.len() > 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} has invalid unicode-range"));
    }
    u32::from_str_radix(value, 16).map_err(|_| format!("{label} has invalid unicode-range"))
}

fn validate_css_resources(css: &str) -> Result<(), String> {
    let clean = strip_css_comments(css)?;
    if clean.contains('\\') {
        return Err("CSS escapes are not admitted in SVG resources".to_owned());
    }
    let lower = clean.to_ascii_lowercase();
    if lower.contains("@import") {
        return Err("forbidden stylesheet resource".to_owned());
    }
    if lower.contains("@font-face") {
        return Err("@font-face is only admitted in SVG style elements".to_owned());
    }
    let mut remainder = clean.as_str();
    loop {
        let lower = remainder.to_ascii_lowercase();
        let Some(index) = lower.find("url(") else {
            return Ok(());
        };
        let (url, rest) = parse_css_url(&remainder[index..])?;
        if !url.starts_with('#') {
            return Err(format!("forbidden CSS resource: {url}"));
        }
        remainder = rest;
    }
}

fn strip_css_comments(css: &str) -> Result<String, String> {
    let mut clean = String::with_capacity(css.len());
    let mut cursor = 0usize;
    while let Some(relative) = css[cursor..].find("/*") {
        let start = cursor + relative;
        clean.push_str(&css[cursor..start]);
        let comment = &css[start + 2..];
        let end = comment
            .find("*/")
            .ok_or_else(|| "unterminated CSS comment".to_owned())?;
        cursor = start + 2 + end + 2;
    }
    clean.push_str(&css[cursor..]);
    Ok(clean)
}

fn parse_data_url(value: &str, kind: &str) -> Result<(String, Vec<u8>), String> {
    let value = value
        .strip_prefix("data:")
        .ok_or_else(|| format!("{kind} resource must be a canonical base64 data URL"))?;
    let (media_type, encoded) = value
        .split_once(";base64,")
        .ok_or_else(|| format!("{kind} resource must be a canonical base64 data URL"))?;
    if media_type.is_empty() || media_type.contains([';', ',']) {
        return Err(format!(
            "{kind} resource must be a canonical base64 data URL"
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| format!("{kind} resource has invalid base64"))?;
    if base64::engine::general_purpose::STANDARD.encode(&bytes) != encoded {
        return Err(format!("{kind} resource has non-canonical base64"));
    }
    Ok((media_type.to_ascii_lowercase(), bytes))
}

fn validate_media(media_type: &str, bytes: &[u8]) -> Result<(), String> {
    let valid = match media_type {
        "font/woff2" => bytes.starts_with(b"wOF2"),
        "font/ttf" => bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true"),
        "font/otf" => bytes.starts_with(b"OTTO"),
        "image/png" | "image/apng" => bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]),
        "image/jpeg" | "image/jfif" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "image/bmp" => bytes.starts_with(b"BM"),
        "image/x-icon" => bytes.starts_with(&[0, 0, 1, 0]),
        "image/avif" => {
            bytes.get(4..8) == Some(b"ftyp")
                && bytes.get(8..).is_some_and(|brands| {
                    brands
                        .chunks_exact(4)
                        .any(|brand| matches!(brand, b"avif" | b"avis"))
                })
        }
        "image/svg+xml" => {
            std::str::from_utf8(bytes).is_ok_and(|text| text.trim_start().starts_with("<svg"))
        }
        _ => false,
    };
    if !valid {
        return Err(format!(
            "payload bytes do not match declared media type {media_type}"
        ));
    }
    Ok(())
}

fn is_source_image_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "image/png"
            | "image/apng"
            | "image/jpeg"
            | "image/jfif"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
            | "image/x-icon"
            | "image/avif"
            | "image/svg+xml"
    )
}

fn prepare_font_database(
    catalog: &AssetCatalog,
    used: &BTreeSet<AssetIdentity>,
) -> Result<Arc<usvg::fontdb::Database>, String> {
    let mut database = usvg::fontdb::Database::new();
    set_generic_font_families(&mut database);
    for font in catalog
        .fonts
        .iter()
        .filter(|font| used.contains(&font.source))
    {
        let bytes = read_verified_descriptor(&font.sfnt.file)?;
        let source = usvg::fontdb::Source::Binary(Arc::new(bytes));
        let mut scratch = usvg::fontdb::Database::new();
        let ids = scratch.load_font_source(source);
        let mut face = ids
            .iter()
            .filter_map(|id| scratch.face(*id))
            .find(|face| face.index == font.sfnt.face_index)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "reviewed sfnt {} has no face index {}",
                    font.sfnt.file.path, font.sfnt.face_index
                )
            })?;
        if !face
            .families
            .iter()
            .any(|(family, _)| family == &font.sfnt.metadata.family)
            || face.style != font.sfnt.metadata.style.fontdb()
            || face.weight != usvg::fontdb::Weight(font.sfnt.metadata.weight)
            || face.stretch != font.sfnt.metadata.stretch.fontdb()
        {
            return Err(format!(
                "reviewed sfnt metadata does not match declaration for {}",
                font.source.label()
            ));
        }
        face.families = vec![(
            font.css.properties.family.clone(),
            usvg::fontdb::Language::English_UnitedStates,
        )];
        face.style = font.css.properties.style.fontdb();
        face.weight = usvg::fontdb::Weight(font.css.properties.weight);
        face.stretch = font.css.properties.stretch.fontdb();
        database.push_face_info(face);
    }
    Ok(Arc::new(database))
}

fn set_generic_font_families(database: &mut usvg::fontdb::Database) {
    database.set_serif_family("Liberation Sans");
    database.set_sans_serif_family("Liberation Sans");
    database.set_cursive_family("Liberation Sans");
    database.set_fantasy_family("Liberation Sans");
    database.set_monospace_family("Liberation Sans");
}

fn prepare_images(
    catalog: &AssetCatalog,
    used: &BTreeSet<AssetIdentity>,
    nested_options: &usvg::Options<'_>,
) -> Result<BTreeMap<AssetIdentity, usvg::ImageKind>, String> {
    let mut images = BTreeMap::new();
    for image in catalog
        .images
        .iter()
        .filter(|image| used.contains(&image.source))
    {
        let bytes = read_verified_descriptor(&image.canonical)?;
        let kind = match image.canonical.media_type.as_str() {
            "image/png" => usvg::ImageKind::PNG(Arc::new(bytes)),
            "image/svg+xml" => {
                validate_sanitized_nested_svg(&bytes)?;
                let tree = usvg::Tree::from_data(&bytes, nested_options).map_err(|error| {
                    format!(
                        "failed to parse canonical nested SVG {}: {error}",
                        image.canonical.path
                    )
                })?;
                usvg::ImageKind::SVG(tree)
            }
            _ => return Err("canonical image media type is not admitted".to_owned()),
        };
        images.insert(image.source.clone(), kind);
    }
    Ok(images)
}

fn validate_sanitized_nested_svg(svg: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(svg)
        .map_err(|error| format!("canonical nested SVG is not UTF-8: {error}"))?;
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!doctype")
        || lower.contains("<!entity")
        || lower.contains("<script")
        || lower.contains("<foreignobject")
    {
        return Err("canonical nested SVG contains active content".to_owned());
    }
    let document = roxmltree::Document::parse(text)
        .map_err(|error| format!("failed to parse canonical nested SVG: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err("canonical nested image is not an SVG document".to_owned());
    }
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let name = node.tag_name().name();
        if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("foreignObject") {
            return Err("canonical nested SVG contains active content".to_owned());
        }
        if name == "image" {
            return Err("canonical nested SVG must not contain image resources".to_owned());
        }
        if name == "style" {
            let css = node
                .children()
                .filter_map(|child| child.text())
                .collect::<String>();
            validate_css_resources(&css)?;
        }
        for attribute in node.attributes() {
            if attribute.name().to_ascii_lowercase().starts_with("on") {
                return Err("canonical nested SVG contains active content".to_owned());
            }
            validate_css_resources(attribute.value())?;
            if matches!(attribute.name(), "href" | "src")
                && !(name == "a" && attribute.name() == "href")
                && !attribute.value().trim().starts_with('#')
            {
                return Err(format!(
                    "canonical nested SVG contains an external resource: {}",
                    attribute.value()
                ));
            }
        }
    }
    Ok(())
}

fn read_verified_descriptor(descriptor: &FileDescriptor) -> Result<Vec<u8>, String> {
    let root = fs::canonicalize(repository_root())
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let requested = root.join(&descriptor.path);
    let canonical = fs::canonicalize(&requested)
        .map_err(|error| format!("failed to resolve descriptor {}: {error}", descriptor.path))?;
    if !canonical.starts_with(&root) {
        return Err(format!(
            "descriptor path escaped repository root: {}",
            descriptor.path
        ));
    }
    let bytes = fs::read(&canonical)
        .map_err(|error| format!("failed to read descriptor {}: {error}", descriptor.path))?;
    let actual = sha256(&bytes);
    if actual != descriptor.sha256 {
        return Err(format!(
            "descriptor SHA-256 mismatch for {}: expected {}, received {}",
            descriptor.path,
            hex(&descriptor.sha256),
            hex(&actual)
        ));
    }
    validate_media(&descriptor.media_type, &bytes)?;
    Ok(bytes)
}

fn validate_repository_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\\') || Path::new(path).is_absolute() {
        return Err("must be a repository-relative path".to_owned());
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || !Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("contains a forbidden path component".to_owned());
    }
    Ok(())
}

fn viewer_options(
    fontdb: Arc<usvg::fontdb::Database>,
    images: Arc<BTreeMap<AssetIdentity, usvg::ImageKind>>,
) -> usvg::Options<'static> {
    let resolve_data = Box::new(
        move |media_type: &str, data: Arc<Vec<u8>>, _: &usvg::Options<'_>| {
            images
                .get(&AssetIdentity::new(media_type, data.as_slice()))
                .cloned()
        },
    );
    usvg::Options {
        resources_dir: None,
        dpi: 96.0,
        font_family: "Liberation Sans".to_owned(),
        font_size: 12.0,
        languages: vec!["en-US".to_owned()],
        shape_rendering: usvg::ShapeRendering::GeometricPrecision,
        text_rendering: usvg::TextRendering::OptimizeLegibility,
        image_rendering: usvg::ImageRendering::OptimizeQuality,
        default_size: usvg::Size::from_wh(100.0, 100.0).expect("fixed default size is valid"),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data,
            resolve_string: Box::new(|_, _| None),
        },
        font_resolver: usvg::FontResolver::default(),
        fontdb,
        style_sheet: None,
    }
}

fn parse_sha256(value: &str, label: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} must be a lowercase SHA-256 digest"));
    }
    let mut hash = [0u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{label} must be a lowercase SHA-256 digest"))?;
    }
    Ok(hash)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn usage() -> String {
    "usage: excalidraw-render-svg-viewer render <input.svg> <output.rgba> [assets.json]".to_owned()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("SVG viewer crate is under tools in the repository")
        .to_owned()
}

fn straight_alpha_rgba8(premultiplied: &[u8]) -> Vec<u8> {
    let mut straight = premultiplied.to_vec();
    for pixel in straight.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 {
            pixel[..3].fill(0);
            continue;
        }
        for channel in &mut pixel[..3] {
            let value = (u32::from(*channel) * 255 + alpha / 2) / alpha;
            *channel = u8::try_from(value.min(255)).expect("unpremultiplied channel is RGBA8");
        }
    }
    straight
}
