mod pdf;
mod png;
mod svg;

use crate::{
    Artifact, CompatibilityReport, ErrorCode, FiniteValue, OutputFormat, ReferenceProfileIdentity,
    RenderError, RenderErrorCategory, RenderOutcome, RenderRequest, SceneDocument, Sizing,
};

use self::{pdf::PdfPlan, png::PngPlan, svg::SvgPlan};

pub(crate) fn render(
    profile: &'static ReferenceProfileIdentity,
    scene: &SceneDocument,
    request: &RenderRequest,
) -> Result<RenderOutcome, RenderError> {
    RenderSession::new(profile).render(scene, request)
}

fn format_finite_number(value: f64, subject: &str) -> Result<String, String> {
    let value = FiniteValue::new(value).ok_or_else(|| format!("{subject} must be finite"))?;
    serde_json::to_string(&value).map_err(|_| format!("failed to serialize {subject}"))
}

struct RenderSession {
    profile: &'static ReferenceProfileIdentity,
    report: CompatibilityReport,
}

impl RenderSession {
    fn new(profile: &'static ReferenceProfileIdentity) -> Self {
        Self {
            profile,
            report: CompatibilityReport::empty(),
        }
    }

    fn render(
        self,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Result<RenderOutcome, RenderError> {
        if !scene.has_no_element_occurrences() {
            return Err(self.internal_failure(
                request.format(),
                "nonempty scene rendering is not implemented",
                "/elements",
            ));
        }

        let resolved = ResolvedScene::empty(request);
        let (error_code, encoded) = match request.format() {
            OutputFormat::Svg => (
                ErrorCode::ENCODE_SVG_FAILED,
                SvgPlan::new(&resolved, request).encode(),
            ),
            OutputFormat::Png => (
                ErrorCode::ENCODE_PNG_FAILED,
                PngPlan::new(&resolved, request).and_then(PngPlan::encode),
            ),
            OutputFormat::Pdf => (
                ErrorCode::ENCODE_PDF_FAILED,
                PdfPlan::new(&resolved, request).and_then(PdfPlan::encode),
            ),
        };
        let bytes = match encoded {
            Ok(bytes) => bytes,
            Err(message) => {
                return Err(self.encoding_failure(request.format(), error_code, message));
            }
        };

        Ok(RenderOutcome {
            artifact: Artifact {
                format: request.format(),
                bytes,
            },
            report: self.report,
            profile: self.profile,
        })
    }

    fn encoding_failure(
        self,
        format: OutputFormat,
        code: ErrorCode,
        message: String,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::Encoding,
            code,
            message: message.into_boxed_str(),
            format,
            element_id: None,
            json_pointer: Box::from(""),
            details: None,
            report: self.report,
        }
    }

    fn internal_failure(
        self,
        format: OutputFormat,
        message: &str,
        json_pointer: &str,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::Internal,
            code: ErrorCode::RENDER_INTERNAL_FAILURE,
            message: Box::from(message),
            format,
            element_id: None,
            json_pointer: Box::from(json_pointer),
            details: None,
            report: self.report,
        }
    }
}

struct ResolvedScene {
    logical_width: f64,
    logical_height: f64,
    include_background: bool,
}

impl ResolvedScene {
    fn empty(request: &RenderRequest) -> Self {
        let logical_size = request.padding() * 2.0;
        Self {
            logical_width: logical_size,
            logical_height: logical_size,
            include_background: request.include_background(),
        }
    }

    fn raster_or_pdf_scale(&self, sizing: Sizing) -> f64 {
        let largest_dimension = self.logical_width.max(self.logical_height);
        match sizing {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(maximum) if maximum < largest_dimension => {
                maximum / largest_dimension
            }
            Sizing::MaxDimension(_) => 1.0,
        }
    }
}
