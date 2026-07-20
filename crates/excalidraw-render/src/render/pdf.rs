use std::io::Write;

use crate::RenderRequest;

use super::{format_finite_number, ResolvedScene};

const POINTS_PER_LOGICAL_PIXEL: f64 = 0.75;

pub(super) struct PdfPlan {
    page_width: f64,
    page_height: f64,
    include_background: bool,
}

impl PdfPlan {
    pub(super) fn new(scene: &ResolvedScene, request: &RenderRequest) -> Result<Self, String> {
        let scale = scene.raster_or_pdf_scale(request.sizing());
        let page_width = point_dimension(scene.logical_width * scale)?;
        let page_height = point_dimension(scene.logical_height * scale)?;
        Ok(Self {
            page_width,
            page_height,
            include_background: scene.include_background,
        })
    }

    pub(super) fn encode(self) -> Result<Vec<u8>, String> {
        let page_width = format_finite_number(self.page_width, "PDF page width")?;
        let page_height = format_finite_number(self.page_height, "PDF page height")?;
        let mut content = Vec::new();
        if self.include_background {
            write!(content, "1 1 1 rg\n0 0 {page_width} {page_height} re\nf\n")
                .map_err(write_error)?;
        }

        let mut pdf = Vec::new();
        pdf.try_reserve(768 + content.len())
            .map_err(|_| "failed to allocate PDF output".to_owned())?;
        pdf.extend_from_slice(b"%PDF-1.7\n%\x80\x80\x80\x80\n\n");
        let mut offsets = [0; 4];

        begin_object(&mut pdf, &mut offsets, 1)?;
        pdf.extend_from_slice(b"<<\n/Type /Catalog\n/Pages 2 0 R\n>>\nendobj\n");

        begin_object(&mut pdf, &mut offsets, 2)?;
        pdf.extend_from_slice(b"<<\n/Type /Pages\n/Kids [3 0 R]\n/Count 1\n>>\nendobj\n");

        begin_object(&mut pdf, &mut offsets, 3)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Page\n/Parent 2 0 R\n",
                "/MediaBox [0 0 {} {}]\n",
                "/CropBox [0 0 {} {}]\n",
                "/Resources << >>\n/Contents 4 0 R\n>>\nendobj\n"
            ),
            page_width, page_height, page_width, page_height,
        )
        .map_err(write_error)?;

        begin_object(&mut pdf, &mut offsets, 4)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", content.len()).map_err(write_error)?;
        pdf.extend_from_slice(&content);
        pdf.extend_from_slice(b"endstream\nendobj\n");

        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f\r\n");
        for offset in offsets {
            writeln!(pdf, "{offset:010} 00000 n\r").map_err(write_error)?;
        }
        write!(
            pdf,
            "trailer\n<<\n/Size 5\n/Root 1 0 R\n>>\nstartxref\n{xref_offset}\n%%EOF"
        )
        .map_err(write_error)?;
        Ok(pdf)
    }
}

fn begin_object(output: &mut Vec<u8>, offsets: &mut [usize; 4], id: usize) -> Result<(), String> {
    offsets[id - 1] = output.len();
    writeln!(output, "{id} 0 obj").map_err(write_error)
}

fn point_dimension(logical: f64) -> Result<f64, String> {
    let points = logical * POINTS_PER_LOGICAL_PIXEL;
    if !points.is_finite() || points <= 0.0 {
        return Err("PDF page dimensions must be finite positive point values".to_owned());
    }
    Ok(points)
}

fn write_error(_: std::io::Error) -> String {
    "failed to encode PDF bytes".to_owned()
}
