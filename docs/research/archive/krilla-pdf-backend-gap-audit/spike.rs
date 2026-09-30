//! Throwaway gap-audit spike: can Krilla produce the PDFs the renderer's PDF
//! backend qualifies? Not production code.
use base64::Engine as _;
use krilla::configure::{Archival, ConfigurationBuilder, PdfVersion};
use krilla::geom::{Point, Rect, Size};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::text::{Font, TextDirection};
use krilla::{
    annotation::{Annotation, LinkAnnotation, Target},
    action::{Action, LinkAction},
    Document, SerializeSettings,
};
use krilla_svg::{SurfaceExt, SvgSettings};
use std::sync::Arc;

fn fonts_from_svg(svg: &str) -> usvg::fontdb::Database {
    let mut db = usvg::fontdb::Database::new();
    for part in svg.split("url(data:font/woff2;base64,").skip(1) {
        let b64 = &part[..part.find(')').unwrap()];
        let woff2 = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let sfnt = wuff::decompress_woff2(&woff2).expect("woff2 decodes");
        db.load_font_data(sfnt);
    }
    for face in db.faces() {
        eprintln!("font: {:?}", face.families);
    }
    db
}

fn tree(svg: &str) -> usvg::Tree {
    let mut options = usvg::Options::default();
    options.fontdb = Arc::new(fonts_from_svg(svg));
    // The renderer's viewer resolver: PNG and nested SVG only, nested PNG kept.
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(|mime, bytes, opts| match mime {
            "image/png" => Some(usvg::ImageKind::PNG(bytes)),
            "image/svg+xml" => usvg::Tree::from_data(&bytes, opts).ok().map(usvg::ImageKind::SVG),
            _ => None,
        }),
        resolve_string: Box::new(|_, _| None),
    };
    usvg::Tree::from_str(svg, &options).expect("svg parses")
}

fn settings(archival: bool) -> SerializeSettings {
    let mut s = SerializeSettings::default();
    s.configuration = if archival {
        ConfigurationBuilder::new()
            .with_version(PdfVersion::Pdf17)
            .with_archival_validator(Archival::A2_B)
            .finish()
            .expect("configuration")
    } else {
        ConfigurationBuilder::new().with_version(PdfVersion::Pdf17).finish().unwrap()
    };
    s
}

fn svg_pdf(svg: &str, embed_text: bool, archival: bool) -> Vec<u8> {
    let tree = tree(svg);
    let (w, h) = (tree.size().width() * 0.75, tree.size().height() * 0.75);
    let mut doc = Document::new_with(settings(archival));
    let mut page = doc.start_page_with(PageSettings::new(Size::from_wh(w, h).unwrap()));
    let mut surface = page.surface();
    surface
        .draw_svg(
            &tree,
            Size::from_wh(w, h).unwrap(),
            SvgSettings { embed_text, filter_scale: 1.0 },
        )
        .expect("draw_svg");
    surface.finish();
    page.finish();
    doc.finish().unwrap_or_else(|e| panic!("finish: {e:?}"))
}

fn probe(font_path: &str, archival: bool) -> Vec<u8> {
    let data = std::fs::read(font_path).unwrap();
    eprintln!("font program bytes in: {}", data.len());
    let font = Font::new(data.into(), 0).expect("font");
    let mut doc = Document::new_with(settings(archival));
    doc.set_metadata(
        Metadata::new()
            .producer("excalidraw-render spike".into())
            .document_id("spike".into())
            .creation_date(krilla::metadata::DateTime::new(2026)),
    );
    let mut page = doc.start_page_with(PageSettings::new(Size::from_wh(200.0, 100.0).unwrap()));
    let mut surface = page.surface();
    surface.draw_text(Point::from_xy(10.0, 50.0), font, 16.0, "Hi fi", false, TextDirection::Auto);
    surface.finish();
    page.add_annotation(Annotation::new_link(
        LinkAnnotation::new(
            Rect::from_xywh(10.0, 30.0, 50.0, 25.0).unwrap(),
            Target::Action(Action::Link(LinkAction::new("https://example.com/shape".into()))),
        ),
        None,
    ));
    page.finish();
    doc.finish().unwrap_or_else(|e| panic!("finish: {e:?}"))
}

/// Line-level `/ActualText` as spec 8.6 marks text: one span per source line.
/// The last line deliberately draws "ABC" with the actual text "XYZ", which
/// shows whether an extractor reads the span rather than the glyph mapping.
fn actual_text(font_path: &str, tagging: bool) -> Vec<u8> {
    use krilla::tagging::{ContentTag, SpanTag};
    let font = Font::new(std::fs::read(font_path).unwrap().into(), 0).expect("font");
    let mut s = settings(false);
    s.enable_tagging = tagging;
    let mut doc = Document::new_with(s);
    let mut page = doc.start_page_with(PageSettings::new(Size::from_wh(300.0, 120.0).unwrap()));
    let mut surface = page.surface();
    for (i, (glyphs, actual)) in [("Hi fi office", "Hi fi office"), ("naïve café", "naïve café"), ("ABC", "XYZ")]
        .iter()
        .enumerate()
    {
        surface.start_tagged(ContentTag::Span(SpanTag::empty().with_actual_text(Some(actual))));
        surface.draw_text(
            Point::from_xy(10.0, 30.0 + 30.0 * i as f32),
            font.clone(),
            16.0,
            glyphs,
            false,
            TextDirection::Auto,
        );
        surface.end_tagged();
    }
    surface.finish();
    page.finish();
    doc.finish().unwrap_or_else(|e| panic!("finish: {e:?}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let out = match args[1].as_str() {
        "svg" => svg_pdf(&std::fs::read_to_string(&args[2]).unwrap(), !flag("--outline"), flag("--pdfa")),
        "probe" => probe(&args[2], flag("--pdfa")),
        "actualtext" => actual_text(&args[2], !flag("--no-tagging")),
        other => panic!("unknown mode {other}"),
    };
    std::fs::write(&args[3], out).unwrap();
}
