//! Reading the geometry out of a rendered SVG document, and the `font-family`
//! lists its text carries.

/// Every `<path d>` value in document order: the path vectors, which are what
/// a geometry pin holds while the rest of the document is free to move.
pub fn path_data(svg: &str) -> Vec<&str> {
    svg.split("<path d=\"")
        .skip(1)
        .map(|tail| tail.split_once('"').expect("path data terminates").0)
        .collect()
}

/// The `font-family` list SVG text carries for each bundled family (spec 8.4):
/// the effective family the document embeds first, then the canonical name,
/// the reference export's fallback chain and a generic family for a consumer
/// that ignores `@font-face`.
pub const EXCALIFONT_FAMILIES: &str = "Excalifont, Xiaolai, Segoe UI Emoji, sans-serif";
pub const LIBERATION_FAMILIES: &str =
    "ExcalidrawRender-9, Liberation Sans, Helvetica, Segoe UI Emoji, sans-serif";
pub const VIRGIL_FAMILIES: &str = "ExcalidrawRender-1, Virgil, Segoe UI Emoji, sans-serif";
pub const CASCADIA_FAMILIES: &str = "ExcalidrawRender-3, Cascadia Code, Segoe UI Emoji, monospace";
pub const LILITA_FAMILIES: &str = "ExcalidrawRender-7, Lilita One, Segoe UI Emoji, sans-serif";
pub const NUNITO_FAMILIES: &str = "Nunito, Segoe UI Emoji, sans-serif";
pub const COMIC_SHANNS_FAMILIES: &str = "Comic Shanns, Segoe UI Emoji, monospace";
pub const XIAOLAI_FAMILIES: &str = "Xiaolai, Segoe UI Emoji, sans-serif";
pub const NOTO_EMOJI_FAMILIES: &str = "Noto Emoji, Segoe UI Emoji, sans-serif";

/// The ` font-family="…"` attribute carrying `families`.
pub fn font_family_attribute(families: &str) -> String {
    format!("font-family=\"{families}\"")
}

/// The shared image definition `<defs>` holds for the asset first drawn by the
/// element at `source_index` (spec 8.4): its data URL once, as an image that
/// fills whatever viewport a referencing `<use>` sizes.
pub fn image_symbol(source_index: usize, data_url: &str) -> String {
    format!(
        "<symbol id=\"image-{source_index}\" overflow=\"visible\"><image href=\"{data_url}\" \
         preserveAspectRatio=\"none\" width=\"100%\" height=\"100%\"/></symbol>"
    )
}

/// The `<use>` an image element draws its shared definition through, sized to
/// its content rectangle, with any further attributes (a dark-mode `filter`)
/// spelled in `rest` after the height.
pub fn image_use(source_index: usize, width: &str, height: &str, rest: &str) -> String {
    format!("<use href=\"#image-{source_index}\" width=\"{width}\" height=\"{height}\"{rest}/>")
}
