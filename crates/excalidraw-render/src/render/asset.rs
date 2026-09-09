use std::collections::BTreeMap;
use std::io::Cursor;
use std::num::NonZeroU64;
use std::sync::Arc;

use base64::Engine as _;
use image_webp::{DecodingError as WebPDecodingError, WebPDecoder};
use moxcms::{
    curve_from_gamma, Chromaticity, CicpColorPrimaries, CicpProfile, ColorPrimaries, ColorProfile,
    DataColorSpace, Layout, MatrixCoefficients, ToneReprCurve, TransferCharacteristics,
    TransformOptions, XyY,
};
use sha2::{Digest as _, Sha256};
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::scene::SourceField;
use crate::{
    CompatibilityRevision, OutputFormat, RenderError, ResourceLimits, SceneDocument,
    SupplementalAssets,
};

use super::resolve::ResolvedImageAsset;

mod bmp;
mod ico;
pub(super) mod svg;
#[cfg(test)]
mod tests;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIGNATURE: &[u8; 3] = b"\xff\xd8\xff";
const GIF87A_SIGNATURE: &[u8; 6] = b"GIF87a";
const GIF89A_SIGNATURE: &[u8; 6] = b"GIF89a";
const WEBP_RIFF_SIGNATURE: &[u8; 4] = b"RIFF";
const WEBP_SIGNATURE: &[u8; 4] = b"WEBP";
const BMP_SIGNATURE: &[u8; 2] = b"BM";
const ICO_SIGNATURE: &[u8; 4] = b"\0\0\x01\0";
const IDENTITY_DOMAIN: &[u8] = b"excalidraw-render:semantic-asset:v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MediaKind {
    Png,
    Jpeg,
    Gif,
    WebP,
    Bmp,
    Ico,
    Svg,
}

impl MediaKind {
    const fn tag(self) -> u64 {
        match self {
            Self::Png => 1,
            Self::Jpeg => 2,
            Self::Gif => 3,
            Self::WebP => 4,
            Self::Bmp => 5,
            Self::Ico => 6,
            Self::Svg => 8,
        }
    }

    pub(super) const fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Bmp => "image/bmp",
            Self::Ico => "image/x-icon",
            Self::Svg => "image/svg+xml",
        }
    }

    const fn source_event(self) -> &'static str {
        match self {
            Self::Png => "semantic_png_source",
            Self::Jpeg => "semantic_jpeg_source",
            Self::Gif => "semantic_gif_source",
            Self::WebP => "semantic_webp_source",
            Self::Bmp => "semantic_bmp_source",
            Self::Ico => "semantic_ico_source",
            Self::Svg => "semantic_svg_source",
        }
    }

    const fn color_event(self) -> &'static str {
        match self {
            Self::Png => "png_color_normalize",
            Self::Jpeg => "jpeg_color_normalize",
            Self::Gif => "gif_color_normalize",
            Self::WebP => "webp_color_normalize",
            Self::Bmp => "bmp_color_normalize",
            Self::Ico => "ico_color_normalize",
            Self::Svg => "svg_color_normalize",
        }
    }

    const fn decoded_event(self) -> &'static str {
        match self {
            Self::Png => "decoded_png_rgba",
            Self::Jpeg => "decoded_jpeg_rgba",
            Self::Gif => "decoded_gif_rgba",
            Self::WebP => "decoded_webp_rgba",
            Self::Bmp => "decoded_bmp_rgba",
            Self::Ico => "decoded_ico_rgba",
            Self::Svg => "decoded_svg_rgba",
        }
    }
}

pub(super) fn is_persistent_event(event: &str) -> bool {
    matches!(
        event,
        "semantic_png_source"
            | "decoded_png_rgba"
            | "semantic_jpeg_source"
            | "decoded_jpeg_rgba"
            | "semantic_gif_source"
            | "decoded_gif_rgba"
            | "semantic_webp_source"
            | "decoded_webp_rgba"
            | "semantic_bmp_source"
            | "decoded_bmp_rgba"
            | "semantic_ico_source"
            | "decoded_ico_rgba"
            | "semantic_svg_source"
            | "decoded_svg_rgba"
    )
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Authority {
    Scene,
    Supplemental,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ResolveError {
    Missing,
    Malformed,
    Unsupported,
    DecodeFailed,
    JpegDecodeFailed,
    GifDecodeFailed,
    WebPDecodeFailed,
    BmpDecodeFailed,
    IcoDecodeFailed,
    SvgDecodeFailed,
    AssetBytes { attempted: u64 },
    Dimensions { attempted: u64 },
    Pixels { attempted: u64 },
    AssetWork { attempted: u64 },
    SvgComplexity { attempted: u64 },
    SvgDepth { attempted: u64 },
    Memory,
}

#[derive(Debug)]
pub(super) enum ResolutionFailure {
    Asset(ResolveError),
    Transaction(RenderError),
    Memory,
}

impl From<ResolveError> for ResolutionFailure {
    fn from(error: ResolveError) -> Self {
        Self::Asset(error)
    }
}

#[derive(Debug)]
pub(super) enum IntrinsicTransaction {
    PreflightNested {
        asset_entries: u64,
        asset_bytes: u64,
        decoded_pixels: u64,
        total_work_units: u64,
    },
    DiscoverNestedOccurrence {
        asset_bytes: u64,
    },
    DiscoverNestedPixels {
        decoded_pixels: u64,
    },
    RejectLocalWork {
        event: &'static str,
        identity: [u8; 32],
        total_work_units: u64,
    },
    Acquire {
        event: &'static str,
        identity: [u8; 32],
        persistent_working_bytes: u64,
        temporary_working_bytes: u64,
        decoded_pixels: u64,
        total_work_units: u64,
    },
    ReleaseTemporary {
        event: &'static str,
        identity: [u8; 32],
        bytes: u64,
    },
    Abort {
        event: &'static str,
        identity: [u8; 32],
        bytes: u64,
    },
}

pub(super) struct Claims {
    pub(super) mime_type: Option<String>,
    pub(super) data_url_media_type: Option<String>,
}

pub(super) struct Resolution {
    pub(super) asset: Arc<ResolvedImageAsset>,
    pub(super) svg_tree: Option<Arc<usvg::Tree>>,
    pub(super) claims: Claims,
    pub(super) animated: bool,
    pub(super) media_kind: MediaKind,
    pub(super) svg_removals: Vec<svg::Removal>,
    pub(super) svg_failures: Vec<svg::Failure>,
}

struct ValidatedRaster {
    identity: [u8; 32],
    bytes: Arc<Vec<u8>>,
    media_kind: MediaKind,
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    semantics: RasterSemantics,
}

enum RasterSemantics {
    Png(PngSemantics),
    Jpeg(JpegSemantics),
    Gif(GifSemantics),
    WebP(WebPSemantics),
    Bmp(bmp::Info),
    Ico(IcoSemantics),
}

impl RasterSemantics {
    const fn orientation(&self) -> Option<u8> {
        match self {
            Self::Png(semantics) => semantics.orientation,
            Self::Jpeg(semantics) => semantics.orientation,
            Self::Gif(_) => None,
            Self::WebP(semantics) => semantics.orientation,
            Self::Bmp(_) => None,
            Self::Ico(_) => None,
        }
    }

    const fn animated(&self) -> bool {
        match self {
            Self::Png(semantics) => semantics.animation_frames > 1,
            Self::Gif(semantics) => semantics.animation_frames > 1,
            Self::Jpeg(_) => false,
            Self::WebP(semantics) => semantics.animation_frames > 1,
            Self::Bmp(_) => false,
            Self::Ico(_) => false,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct PngSemantics {
    cicp: Option<[u8; 4]>,
    animation_frames: u32,
    orientation: Option<u8>,
}

#[derive(Clone, Copy, Default)]
struct JpegSemantics {
    orientation: Option<u8>,
}

#[derive(Clone, Copy)]
struct GifSemantics {
    animation_frames: u32,
}

#[derive(Clone, Copy)]
struct WebPSemantics {
    animation_frames: u32,
    orientation: Option<u8>,
}

#[derive(Clone)]
struct IcoSemantics {
    offset: usize,
    length: usize,
    payload: IcoPayloadSemantics,
}

#[derive(Clone)]
enum IcoPayloadSemantics {
    Png(PngSemantics),
    Dib(ico::DibInfo),
}

struct GifMetadata {
    width: u32,
    height: u32,
    animation_frames: u32,
    additional_frame_pixels: u64,
}

struct JpegMetadata {
    width: u32,
    height: u32,
    orientation: Option<u8>,
}

enum ColorNormalization {
    None,
    LinearSrgb,
    Profile(Box<ColorProfile>),
}

const LINEAR_TO_SRGB: [u8; 256] = [
    0, 13, 22, 28, 34, 38, 42, 46, 50, 53, 56, 59, 61, 64, 66, 69, 71, 73, 75, 77, 79, 81, 83, 85,
    86, 88, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104, 105, 106, 108, 109, 110, 112, 113, 114, 115,
    117, 118, 119, 120, 121, 122, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
    137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 148, 149, 150, 151, 152, 153, 154,
    155, 155, 156, 157, 158, 159, 159, 160, 161, 162, 163, 163, 164, 165, 166, 167, 167, 168, 169,
    170, 170, 171, 172, 173, 173, 174, 175, 175, 176, 177, 178, 178, 179, 180, 180, 181, 182, 182,
    183, 184, 185, 185, 186, 187, 187, 188, 189, 189, 190, 190, 191, 192, 192, 193, 194, 194, 195,
    196, 196, 197, 197, 198, 199, 199, 200, 200, 201, 202, 202, 203, 203, 204, 205, 205, 206, 206,
    207, 208, 208, 209, 209, 210, 210, 211, 212, 212, 213, 213, 214, 214, 215, 215, 216, 216, 217,
    218, 218, 219, 219, 220, 220, 221, 221, 222, 222, 223, 223, 224, 224, 225, 226, 226, 227, 227,
    228, 228, 229, 229, 230, 230, 231, 231, 232, 232, 233, 233, 234, 234, 235, 235, 236, 236, 237,
    237, 238, 238, 238, 239, 239, 240, 240, 241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246,
    246, 246, 247, 247, 248, 248, 249, 249, 250, 250, 251, 251, 251, 252, 252, 253, 253, 254, 254,
    255, 255,
];

struct DecodedRaster {
    rgba: Arc<Vec<u8>>,
    premultiplied_rgba: Arc<Vec<u8>>,
}

#[derive(Default)]
pub(super) struct Cache {
    validated: BTreeMap<[u8; 32], Arc<ValidatedRaster>>,
    decoded: BTreeMap<[u8; 32], Arc<DecodedRaster>>,
    svg: BTreeMap<[u8; 32], Arc<svg::SanitizedSvg>>,
    normalized_nested_rasters: BTreeMap<[u8; 32], Arc<NormalizedNestedRaster>>,
    svg_discovery_owners: std::collections::BTreeSet<String>,
    svg_diagnostic_owners: std::collections::BTreeSet<String>,
}

impl Cache {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve<F>(
        &mut self,
        scene: &SceneDocument,
        supplemental: &SupplementalAssets,
        revision: CompatibilityRevision,
        format: OutputFormat,
        limits: &ResourceLimits,
        file_id: &str,
        authority: Authority,
        mut transact: F,
    ) -> Result<Resolution, ResolutionFailure>
    where
        F: FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
    {
        let payload = payload(
            scene,
            supplemental,
            file_id,
            authority,
            limits.max_asset_bytes(),
        )?;
        if svg::looks_like_svg(&payload.bytes) {
            return self.resolve_svg(
                revision,
                format,
                limits,
                file_id,
                authority,
                payload,
                &mut transact,
            );
        }
        let media_kind = sniff_media_kind(&payload.bytes)?;
        let identity = semantic_identity(revision, media_kind, &payload.bytes);
        let (source_width, source_height) = match media_kind {
            MediaKind::Png => read_png_ihdr(&payload.bytes)?,
            MediaKind::Jpeg => read_jpeg_dimensions(&payload.bytes)?,
            MediaKind::Gif => read_gif_dimensions(&payload.bytes)?,
            MediaKind::WebP => read_webp_dimensions(&payload.bytes)?,
            MediaKind::Bmp => read_bmp_dimensions(&payload.bytes)?,
            MediaKind::Ico => read_ico_dimensions(&payload.bytes)?,
            MediaKind::Svg => unreachable!("SVG assets use the sanitizer path"),
        };
        let pixels = u64::from(source_width)
            .checked_mul(u64::from(source_height))
            .ok_or(ResolveError::Pixels {
                attempted: u64::MAX,
            })?;
        let payload_bytes = u64::try_from(payload.bytes.len()).unwrap_or(u64::MAX);
        let animation_validation_work =
            match media_kind {
                MediaKind::Png => png_animation_validation_work(&payload.bytes, pixels)?,
                MediaKind::Jpeg => 0,
                MediaKind::Gif => gif_animation_validation_work(&payload.bytes, pixels)?,
                MediaKind::WebP => webp_animation_validation_work(&payload.bytes)?,
                MediaKind::Bmp => bmp::validation_work(&payload.bytes)
                    .map_err(|_| ResolveError::BmpDecodeFailed)?,
                MediaKind::Ico => ico::validation_work(&payload.bytes)
                    .map_err(|_| ResolveError::IcoDecodeFailed)?,
                MediaKind::Svg => unreachable!("SVG assets use the sanitizer path"),
            };
        let validation_working_bytes = if self.validated.contains_key(&identity) {
            0
        } else {
            512_u64.saturating_add(logical_aligned_bytes(
                u64::try_from(payload.bytes.len()).unwrap_or(u64::MAX),
            ))
        };
        let validated = if let Some(validated) = self.validated.get(&identity) {
            Arc::clone(validated)
        } else {
            let validation_work = payload_bytes.checked_add(animation_validation_work).ok_or(
                ResolveError::AssetWork {
                    attempted: u64::MAX,
                },
            )?;
            if validation_work > limits.max_asset_work_units() {
                transact(IntrinsicTransaction::RejectLocalWork {
                    event: media_kind.source_event(),
                    identity,
                    total_work_units: validation_work,
                })
                .map_err(ResolutionFailure::Transaction)?;
                return Err(ResolveError::AssetWork {
                    attempted: validation_work,
                }
                .into());
            }
            let decoder_workspace = decoder_workspace_bytes(source_width, source_height)?
                .checked_add(if media_kind == MediaKind::Ico {
                    ico::directory_workspace(&payload.bytes)
                        .map_err(|_| ResolveError::IcoDecodeFailed)?
                } else {
                    0
                })
                .ok_or(ResolveError::Memory)?;
            transact(IntrinsicTransaction::Acquire {
                event: media_kind.source_event(),
                identity,
                persistent_working_bytes: validation_working_bytes,
                temporary_working_bytes: decoder_workspace,
                decoded_pixels: 0,
                total_work_units: validation_work,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let validation = match media_kind {
                MediaKind::Png => {
                    validate_static_png(&payload.bytes).map(|(width, height, semantics)| {
                        (width, height, RasterSemantics::Png(semantics))
                    })
                }
                MediaKind::Jpeg => {
                    validate_static_jpeg(&payload.bytes).map(|(width, height, semantics)| {
                        (width, height, RasterSemantics::Jpeg(semantics))
                    })
                }
                MediaKind::Gif => validate_gif(&payload.bytes).map(|(width, height, semantics)| {
                    (width, height, RasterSemantics::Gif(semantics))
                }),
                MediaKind::WebP => {
                    validate_webp(&payload.bytes).map(|(width, height, semantics)| {
                        (width, height, RasterSemantics::WebP(semantics))
                    })
                }
                MediaKind::Bmp => validate_bmp(&payload.bytes).map(|(width, height, semantics)| {
                    (width, height, RasterSemantics::Bmp(semantics))
                }),
                MediaKind::Ico => validate_ico(&payload.bytes).map(|(width, height, semantics)| {
                    (width, height, RasterSemantics::Ico(semantics))
                }),
                MediaKind::Svg => unreachable!("SVG assets use the sanitizer path"),
            };
            let validation = match validation {
                Ok(validation) => validation,
                Err(error) => {
                    transact(IntrinsicTransaction::Abort {
                        event: media_kind.source_event(),
                        identity,
                        bytes: validation_working_bytes.saturating_add(decoder_workspace),
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
            };
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "raster_validation_workspace",
                identity,
                bytes: decoder_workspace,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let (width, height, semantics) = validation;
            let validated = Arc::new(ValidatedRaster {
                identity,
                bytes: Arc::new(payload.bytes),
                media_kind,
                source_width,
                source_height,
                width,
                height,
                semantics,
            });
            self.validated.insert(identity, Arc::clone(&validated));
            validated
        };

        let decoded = if format == OutputFormat::Svg {
            None
        } else if let Some(decoded) = self.decoded.get(&identity) {
            Some(Arc::clone(decoded))
        } else {
            let largest_dimension = u64::from(validated.width.max(validated.height));
            if largest_dimension > u64::from(limits.max_raster_dimension()) {
                return Err(ResolveError::Dimensions {
                    attempted: largest_dimension,
                }
                .into());
            }
            if pixels > limits.max_decoded_pixels_per_asset() {
                return Err(ResolveError::Pixels { attempted: pixels }.into());
            }
            let work = payload_bytes
                .checked_mul(5)
                .and_then(|work| {
                    pixels
                        .checked_mul(4)
                        .and_then(|pixel_work| work.checked_add(pixel_work))
                })
                .ok_or(ResolveError::AssetWork {
                    attempted: u64::MAX,
                })?;
            if work > limits.max_asset_work_units() {
                transact(IntrinsicTransaction::RejectLocalWork {
                    event: validated.media_kind.decoded_event(),
                    identity,
                    total_work_units: work,
                })
                .map_err(ResolutionFailure::Transaction)?;
                return Err(ResolveError::AssetWork { attempted: work }.into());
            }
            let decoder_workspace = decoder_workspace_bytes(validated.width, validated.height)?;
            transact(IntrinsicTransaction::Acquire {
                event: validated.media_kind.color_event(),
                identity,
                persistent_working_bytes: 0,
                temporary_working_bytes: decoder_workspace,
                decoded_pixels: 0,
                total_work_units: payload_bytes,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let color_normalization = match prepare_color_normalization(
                &validated.bytes,
                &validated.semantics,
                decoder_workspace,
            ) {
                Ok(normalization) => normalization,
                Err(error) => {
                    transact(IntrinsicTransaction::Abort {
                        event: validated.media_kind.color_event(),
                        identity,
                        bytes: decoder_workspace,
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
            };
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "raster_color_normalize_workspace",
                identity,
                bytes: decoder_workspace,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let decode_working_bytes = 256_u64
                .checked_add(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
                .ok_or(ResolveError::Memory)?;
            transact(IntrinsicTransaction::Acquire {
                event: validated.media_kind.decoded_event(),
                identity,
                persistent_working_bytes: decode_working_bytes,
                temporary_working_bytes: decoder_workspace,
                decoded_pixels: pixels,
                total_work_units: payload_bytes
                    .saturating_mul(3)
                    .saturating_add(pixels.saturating_mul(4)),
            })
            .map_err(ResolutionFailure::Transaction)?;
            let decoded = match validated.media_kind {
                MediaKind::Png => decode_static_png(
                    &validated.bytes,
                    pixels,
                    color_normalization,
                    validated.semantics.orientation(),
                ),
                MediaKind::Jpeg => decode_static_jpeg(
                    &validated.bytes,
                    pixels,
                    color_normalization,
                    validated.semantics.orientation(),
                ),
                MediaKind::Gif => decode_gif_default_frame(
                    &validated.bytes,
                    validated.width,
                    validated.height,
                    pixels,
                ),
                MediaKind::WebP => decode_webp_default_frame(
                    &validated.bytes,
                    validated.source_width,
                    validated.source_height,
                    pixels,
                    color_normalization,
                    validated.semantics.orientation(),
                ),
                MediaKind::Bmp => decode_bmp(&validated.bytes, &validated.semantics),
                MediaKind::Ico => decode_ico(
                    &validated.bytes,
                    pixels,
                    color_normalization,
                    &validated.semantics,
                ),
                MediaKind::Svg => unreachable!("SVG assets use the sanitizer path"),
            };
            let decoded = match decoded {
                Ok(decoded) => Arc::new(decoded),
                Err(error) => {
                    transact(IntrinsicTransaction::Abort {
                        event: validated.media_kind.decoded_event(),
                        identity,
                        bytes: decode_working_bytes.saturating_add(decoder_workspace),
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
            };
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "raster_decoder_workspace",
                identity,
                bytes: decoder_workspace,
            })
            .map_err(ResolutionFailure::Transaction)?;
            self.decoded.insert(identity, Arc::clone(&decoded));
            Some(decoded)
        };

        let (width, height) = if format == OutputFormat::Svg {
            (validated.source_width, validated.source_height)
        } else {
            (validated.width, validated.height)
        };
        Ok(Resolution {
            asset: Arc::new(ResolvedImageAsset {
                identity: validated.identity,
                source_bytes: Arc::clone(&validated.bytes),
                media_type: validated.media_kind.media_type(),
                width,
                height,
                rgba: decoded.as_ref().map(|decoded| Arc::clone(&decoded.rgba)),
                premultiplied_rgba: decoded
                    .as_ref()
                    .map(|decoded| Arc::clone(&decoded.premultiplied_rgba)),
                sanitized_svg: None,
                svg_effects: Vec::new(),
                svg_fallback_pointer: None,
            }),
            svg_tree: None,
            claims: payload.claims,
            animated: validated.semantics.animated(),
            media_kind: validated.media_kind,
            svg_removals: Vec::new(),
            svg_failures: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_svg<F>(
        &mut self,
        revision: CompatibilityRevision,
        format: OutputFormat,
        limits: &ResourceLimits,
        file_id: &str,
        authority: Authority,
        payload: Payload,
        transact: &mut F,
    ) -> Result<Resolution, ResolutionFailure>
    where
        F: FnMut(IntrinsicTransaction) -> Result<(), RenderError>,
    {
        let source_identity = semantic_identity(revision, MediaKind::Svg, &payload.bytes);
        let mut pending_nested_rasters = BTreeMap::new();
        let (sanitized, discovered_live) = if let Some(svg) = self.svg.get(&source_identity) {
            (Arc::clone(svg), false)
        } else {
            let source_len = u64::try_from(payload.bytes.len()).unwrap_or(u64::MAX);
            if source_len > limits.max_asset_work_units() {
                transact(IntrinsicTransaction::RejectLocalWork {
                    event: MediaKind::Svg.source_event(),
                    identity: source_identity,
                    total_work_units: source_len,
                })
                .map_err(ResolutionFailure::Transaction)?;
                return Err(ResolveError::AssetWork {
                    attempted: source_len,
                }
                .into());
            }
            let temporary = 1_048_576;
            transact(IntrinsicTransaction::Acquire {
                event: MediaKind::Svg.source_event(),
                identity: source_identity,
                persistent_working_bytes: 0,
                temporary_working_bytes: temporary,
                decoded_pixels: 0,
                total_work_units: source_len,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let sanitized = match svg::sanitize(
                &payload.bytes,
                limits,
                revision,
                format,
                &self.normalized_nested_rasters,
                &mut pending_nested_rasters,
                transact,
            ) {
                Ok(svg) => Arc::new(svg),
                Err(ResolutionFailure::Asset(error)) => {
                    transact(IntrinsicTransaction::Abort {
                        event: MediaKind::Svg.source_event(),
                        identity: source_identity,
                        bytes: temporary,
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
                Err(ResolutionFailure::Transaction(error)) => {
                    return Err(ResolutionFailure::Transaction(error));
                }
                Err(ResolutionFailure::Memory) => return Err(ResolutionFailure::Memory),
            };
            if sanitized.work_units > limits.max_asset_work_units() {
                transact(IntrinsicTransaction::RejectLocalWork {
                    event: "derived_sanitized_svg",
                    identity: source_identity,
                    total_work_units: sanitized.work_units.saturating_sub(source_len),
                })
                .map_err(ResolutionFailure::Transaction)?;
                transact(IntrinsicTransaction::Abort {
                    event: MediaKind::Svg.source_event(),
                    identity: source_identity,
                    bytes: temporary,
                })
                .map_err(ResolutionFailure::Transaction)?;
                return Err(ResolveError::AssetWork {
                    attempted: sanitized.work_units,
                }
                .into());
            }
            transact(IntrinsicTransaction::Acquire {
                event: "derived_sanitized_svg",
                identity: source_identity,
                persistent_working_bytes: sanitized.working_bytes,
                temporary_working_bytes: 0,
                decoded_pixels: 0,
                total_work_units: sanitized.work_units.saturating_sub(source_len),
            })
            .map_err(ResolutionFailure::Transaction)?;
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "svg_sanitizer_workspace",
                identity: source_identity,
                bytes: temporary,
            })
            .map_err(ResolutionFailure::Transaction)?;
            for admission in &sanitized.nested_admissions {
                let transaction = match admission {
                    svg::NestedAdmission::Occurrence { asset_bytes } => {
                        IntrinsicTransaction::DiscoverNestedOccurrence {
                            asset_bytes: *asset_bytes,
                        }
                    }
                    svg::NestedAdmission::DecodedPixels { decoded_pixels } => {
                        IntrinsicTransaction::DiscoverNestedPixels {
                            decoded_pixels: *decoded_pixels,
                        }
                    }
                    svg::NestedAdmission::Work {
                        identity,
                        total_work_units,
                    } => IntrinsicTransaction::Acquire {
                        event: "nested_asset_intrinsic_work",
                        identity: *identity,
                        persistent_working_bytes: 0,
                        temporary_working_bytes: 0,
                        decoded_pixels: 0,
                        total_work_units: *total_work_units,
                    },
                };
                transact(transaction).map_err(ResolutionFailure::Transaction)?;
            }
            self.normalized_nested_rasters
                .append(&mut pending_nested_rasters);
            self.svg.insert(source_identity, Arc::clone(&sanitized));
            (sanitized, true)
        };
        if self.svg_discovery_owners.insert(file_id.to_owned()) && !discovered_live {
            for admission in &sanitized.nested_admissions {
                if let svg::NestedAdmission::Occurrence { asset_bytes } = admission {
                    transact(IntrinsicTransaction::DiscoverNestedOccurrence {
                        asset_bytes: *asset_bytes,
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                }
            }
        }
        let identity = semantic_identity(revision, MediaKind::Svg, &sanitized.bytes);
        let report_svg_diagnostics = self.svg_diagnostic_owners.insert(file_id.to_owned());
        Ok(Resolution {
            asset: Arc::new(ResolvedImageAsset {
                identity,
                source_bytes: Arc::new(sanitized.bytes.clone()),
                media_type: MediaKind::Svg.media_type(),
                width: sanitized.width,
                height: sanitized.height,
                rgba: None,
                premultiplied_rgba: None,
                sanitized_svg: Some(Arc::new(sanitized.bytes.clone())),
                svg_effects: sanitized.effects.clone(),
                svg_fallback_pointer: match authority {
                    Authority::Scene => Some(format!(
                        "/files/{}/dataURL",
                        file_id.replace('~', "~0").replace('/', "~1")
                    )),
                    Authority::Supplemental | Authority::Missing => Some(String::new()),
                },
            }),
            svg_tree: Some(Arc::clone(&sanitized.tree)),
            claims: payload.claims,
            animated: false,
            media_kind: MediaKind::Svg,
            svg_removals: if report_svg_diagnostics {
                sanitized.removals.clone()
            } else {
                Vec::new()
            },
            svg_failures: if report_svg_diagnostics {
                sanitized.failures.clone()
            } else {
                Vec::new()
            },
        })
    }
}

struct NormalizedNestedRaster {
    source_media_kind: MediaKind,
    normalized_png: Vec<u8>,
}

struct ValidatedNestedRaster {
    media_kind: MediaKind,
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    pixels: u64,
    validation_work: u64,
    semantics: RasterSemantics,
}

fn normalize_nested_raster(
    bytes: &[u8],
    validated: ValidatedNestedRaster,
) -> Result<NormalizedNestedRaster, ResolveError> {
    let color_normalization = prepare_color_normalization(
        bytes,
        &validated.semantics,
        decoder_workspace_bytes(validated.width, validated.height)?,
    )?;
    let decoded = match validated.media_kind {
        MediaKind::Png => decode_static_png(
            bytes,
            validated.pixels,
            color_normalization,
            validated.semantics.orientation(),
        ),
        MediaKind::Jpeg => decode_static_jpeg(
            bytes,
            validated.pixels,
            color_normalization,
            validated.semantics.orientation(),
        ),
        MediaKind::Gif => {
            decode_gif_default_frame(bytes, validated.width, validated.height, validated.pixels)
        }
        MediaKind::WebP => decode_webp_default_frame(
            bytes,
            validated.source_width,
            validated.source_height,
            validated.pixels,
            color_normalization,
            validated.semantics.orientation(),
        ),
        MediaKind::Bmp => decode_bmp(bytes, &validated.semantics),
        MediaKind::Ico => decode_ico(
            bytes,
            validated.pixels,
            color_normalization,
            &validated.semantics,
        ),
        MediaKind::Svg => unreachable!("nested SVG uses recursive sanitization"),
    }?;
    let normalized_png = encode_normalized_png(validated.width, validated.height, &decoded.rgba)?;
    Ok(NormalizedNestedRaster {
        source_media_kind: validated.media_kind,
        normalized_png,
    })
}

fn validate_nested_raster(
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<(MediaKind, u64, u64), ResolveError> {
    let validated = validate_nested_raster_semantics(bytes, limits)?;
    Ok((
        validated.media_kind,
        validated.pixels,
        validated.validation_work,
    ))
}

fn validate_nested_raster_semantics(
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<ValidatedNestedRaster, ResolveError> {
    let media_kind = sniff_media_kind(bytes)?;
    let (source_width, source_height) = match media_kind {
        MediaKind::Png => read_png_ihdr(bytes)?,
        MediaKind::Jpeg => read_jpeg_dimensions(bytes)?,
        MediaKind::Gif => read_gif_dimensions(bytes)?,
        MediaKind::WebP => read_webp_dimensions(bytes)?,
        MediaKind::Bmp => read_bmp_dimensions(bytes)?,
        MediaKind::Ico => read_ico_dimensions(bytes)?,
        MediaKind::Svg => unreachable!("nested SVG uses recursive sanitization"),
    };
    let source_pixels = u64::from(source_width)
        .checked_mul(u64::from(source_height))
        .ok_or(ResolveError::Memory)?;
    let animation_work = match media_kind {
        MediaKind::Png => png_animation_validation_work(bytes, source_pixels)?,
        MediaKind::Jpeg => 0,
        MediaKind::Gif => gif_animation_validation_work(bytes, source_pixels)?,
        MediaKind::WebP => webp_animation_validation_work(bytes)?,
        MediaKind::Bmp => bmp::validation_work(bytes).map_err(|_| ResolveError::BmpDecodeFailed)?,
        MediaKind::Ico => ico::validation_work(bytes).map_err(|_| ResolveError::IcoDecodeFailed)?,
        MediaKind::Svg => unreachable!("nested SVG uses recursive sanitization"),
    };
    let work = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .checked_add(animation_work)
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })?;
    if work > limits.max_asset_work_units() {
        return Err(ResolveError::AssetWork { attempted: work });
    }
    let (width, height, semantics) = match media_kind {
        MediaKind::Png => validate_static_png(bytes)
            .map(|(width, height, semantics)| (width, height, RasterSemantics::Png(semantics)))?,
        MediaKind::Jpeg => validate_static_jpeg(bytes)
            .map(|(width, height, semantics)| (width, height, RasterSemantics::Jpeg(semantics)))?,
        MediaKind::Gif => validate_gif(bytes)
            .map(|(width, height, semantics)| (width, height, RasterSemantics::Gif(semantics)))?,
        MediaKind::WebP => validate_webp(bytes)
            .map(|(width, height, semantics)| (width, height, RasterSemantics::WebP(semantics)))?,
        MediaKind::Bmp => validate_bmp(bytes)
            .map(|(width, height, semantics)| (width, height, RasterSemantics::Bmp(semantics)))?,
        MediaKind::Ico => validate_ico(bytes)
            .map(|(width, height, semantics)| (width, height, RasterSemantics::Ico(semantics)))?,
        MediaKind::Svg => unreachable!("nested SVG uses recursive sanitization"),
    };
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(ResolveError::Memory)?;
    Ok(ValidatedNestedRaster {
        media_kind,
        source_width,
        source_height,
        width,
        height,
        pixels,
        validation_work: work,
        semantics,
    })
}

fn encode_normalized_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, ResolveError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Best);
        encoder.set_filter(png::FilterType::NoFilter);
        let mut writer = encoder
            .write_header()
            .map_err(|_| ResolveError::DecodeFailed)?;
        writer
            .write_image_data(rgba)
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    Ok(bytes)
}

fn read_png_ihdr(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let chunk = bytes
        .get(PNG_SIGNATURE.len()..PNG_SIGNATURE.len() + 25)
        .ok_or(ResolveError::DecodeFailed)?;
    if chunk[..4] != 13_u32.to_be_bytes() || &chunk[4..8] != b"IHDR" {
        return Err(ResolveError::DecodeFailed);
    }
    let width = u32::from_be_bytes(
        chunk[8..12]
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?,
    );
    let height = u32::from_be_bytes(
        chunk[12..16]
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?,
    );
    if width == 0 || height == 0 {
        return Err(ResolveError::DecodeFailed);
    }
    Ok((width, height))
}

fn sniff_media_kind(bytes: &[u8]) -> Result<MediaKind, ResolveError> {
    if bytes.starts_with(PNG_SIGNATURE) {
        Ok(MediaKind::Png)
    } else if bytes.starts_with(JPEG_SIGNATURE) {
        Ok(MediaKind::Jpeg)
    } else if bytes.starts_with(GIF87A_SIGNATURE) || bytes.starts_with(GIF89A_SIGNATURE) {
        Ok(MediaKind::Gif)
    } else if bytes.starts_with(WEBP_RIFF_SIGNATURE) && bytes.get(8..12) == Some(WEBP_SIGNATURE) {
        Ok(MediaKind::WebP)
    } else if bytes.starts_with(BMP_SIGNATURE) {
        Ok(MediaKind::Bmp)
    } else if bytes.starts_with(ICO_SIGNATURE) {
        Ok(MediaKind::Ico)
    } else {
        Err(ResolveError::Unsupported)
    }
}

fn read_bmp_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    bmp::dimensions(bytes).map_err(|_| ResolveError::BmpDecodeFailed)
}

fn validate_bmp(bytes: &[u8]) -> Result<(u32, u32, bmp::Info), ResolveError> {
    let info = bmp::validate(bytes).map_err(|_| ResolveError::BmpDecodeFailed)?;
    Ok((info.width, info.height, info))
}

fn decode_bmp(bytes: &[u8], semantics: &RasterSemantics) -> Result<DecodedRaster, ResolveError> {
    let RasterSemantics::Bmp(info) = semantics else {
        return Err(ResolveError::BmpDecodeFailed);
    };
    finish_rgba(bmp::decode(bytes, info).map_err(|_| ResolveError::BmpDecodeFailed)?)
}

fn read_ico_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    let selected = selected_ico_entry(bytes)?;
    let payload = bytes
        .get(selected.offset..selected.offset + selected.length)
        .ok_or(ResolveError::IcoDecodeFailed)?;
    if payload.starts_with(PNG_SIGNATURE) {
        let dimensions = read_png_ihdr(payload).map_err(|_| ResolveError::IcoDecodeFailed)?;
        if dimensions != (selected.width, selected.height) {
            return Err(ResolveError::IcoDecodeFailed);
        }
    } else {
        ico::dib_dimensions(payload, selected).map_err(|_| ResolveError::IcoDecodeFailed)?;
    }
    Ok((selected.width, selected.height))
}

fn validate_ico(bytes: &[u8]) -> Result<(u32, u32, IcoSemantics), ResolveError> {
    let entries = ico::entries(bytes).map_err(|_| ResolveError::IcoDecodeFailed)?;
    let selected = selected_ico_entry_from(&entries)?;
    let mut selected_payload = None;
    for entry in entries {
        let payload = bytes
            .get(entry.offset..entry.offset + entry.length)
            .ok_or(ResolveError::IcoDecodeFailed)?;
        let semantics = if payload.starts_with(PNG_SIGNATURE) {
            let (width, height, semantics) =
                validate_static_png(payload).map_err(|_| ResolveError::IcoDecodeFailed)?;
            if width != entry.width || height != entry.height {
                return Err(ResolveError::IcoDecodeFailed);
            }
            IcoPayloadSemantics::Png(semantics)
        } else {
            IcoPayloadSemantics::Dib(
                ico::validate_dib(payload, entry).map_err(|_| ResolveError::IcoDecodeFailed)?,
            )
        };
        if entry.index == selected.index {
            selected_payload = Some(semantics);
        }
    }
    Ok((
        selected.width,
        selected.height,
        IcoSemantics {
            offset: selected.offset,
            length: selected.length,
            payload: selected_payload.ok_or(ResolveError::IcoDecodeFailed)?,
        },
    ))
}

fn selected_ico_entry(bytes: &[u8]) -> Result<ico::Entry, ResolveError> {
    ico::selected_entry(bytes).map_err(|_| ResolveError::IcoDecodeFailed)
}

fn selected_ico_entry_from(entries: &[ico::Entry]) -> Result<ico::Entry, ResolveError> {
    entries
        .iter()
        .max_by_key(|entry| {
            (
                u64::from(entry.width) * u64::from(entry.height),
                entry.depth,
                std::cmp::Reverse(entry.index),
            )
        })
        .copied()
        .ok_or(ResolveError::IcoDecodeFailed)
}

fn decode_ico(
    bytes: &[u8],
    pixels: u64,
    color_normalization: ColorNormalization,
    semantics: &RasterSemantics,
) -> Result<DecodedRaster, ResolveError> {
    let RasterSemantics::Ico(semantics) = semantics else {
        return Err(ResolveError::IcoDecodeFailed);
    };
    let payload = bytes
        .get(semantics.offset..semantics.offset + semantics.length)
        .ok_or(ResolveError::IcoDecodeFailed)?;
    match &semantics.payload {
        IcoPayloadSemantics::Png(png) => {
            decode_static_png(payload, pixels, color_normalization, png.orientation)
                .map_err(|_| ResolveError::IcoDecodeFailed)
        }
        IcoPayloadSemantics::Dib(info) => {
            finish_rgba(ico::decode_dib(payload, info).map_err(|_| ResolveError::IcoDecodeFailed)?)
        }
    }
}

fn webp_decoder(bytes: &[u8]) -> Result<WebPDecoder<Cursor<&[u8]>>, ResolveError> {
    let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(map_webp_error)?;
    let (width, height) = decoder.dimensions();
    decoder.set_memory_limit(
        usize::try_from(decoder_workspace_bytes(width, height)?)
            .map_err(|_| ResolveError::Memory)?,
    );
    Ok(decoder)
}

fn read_webp_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(WEBP_RIFF_SIGNATURE) || bytes.get(8..12) != Some(WEBP_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let decoder = webp_decoder(bytes)?;
    let dimensions = decoder.dimensions();
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return Err(ResolveError::WebPDecodeFailed);
    }
    Ok(dimensions)
}

fn webp_animation_validation_work(bytes: &[u8]) -> Result<u64, ResolveError> {
    let decoder = webp_decoder(bytes)?;
    let (frames, additional_frame_pixels, _) = scan_webp_animation(bytes, decoder.dimensions())?;
    if frames != decoder.num_frames() {
        return Err(ResolveError::WebPDecodeFailed);
    }
    additional_frame_pixels
        .checked_mul(4)
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })
}

fn scan_webp_animation(bytes: &[u8], canvas: (u32, u32)) -> Result<(u32, u64, bool), ResolveError> {
    let declared_size = usize::try_from(u32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or(ResolveError::WebPDecodeFailed)?
            .try_into()
            .map_err(|_| ResolveError::WebPDecodeFailed)?,
    ))
    .map_err(|_| ResolveError::WebPDecodeFailed)?;
    if declared_size.checked_add(8) != Some(bytes.len()) {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let mut offset = 12_usize;
    let mut frames = 0_u32;
    let mut additional_frame_pixels = 0_u64;
    let mut animation_has_alpha = false;
    while offset < bytes.len() {
        let (kind, data, next) = webp_chunk_at(bytes, offset)?;
        if kind == *b"ANMF" {
            let frame_header = data.get(..16).ok_or(ResolveError::WebPDecodeFailed)?;
            if frame_header[15] & !0x03 != 0 {
                return Err(ResolveError::WebPDecodeFailed);
            }
            let x = read_webp_u24(&frame_header[..3])?.saturating_mul(2);
            let y = read_webp_u24(&frame_header[3..6])?.saturating_mul(2);
            let width = u64::from(read_webp_u24(&frame_header[6..9])?).saturating_add(1);
            let height = u64::from(read_webp_u24(&frame_header[9..12])?).saturating_add(1);
            if u64::from(x).saturating_add(width) > u64::from(canvas.0)
                || u64::from(y).saturating_add(height) > u64::from(canvas.1)
            {
                return Err(ResolveError::WebPDecodeFailed);
            }
            animation_has_alpha |=
                validate_webp_frame_chunks(data.get(16..).ok_or(ResolveError::WebPDecodeFailed)?)?;
            frames = frames
                .checked_add(1)
                .ok_or(ResolveError::WebPDecodeFailed)?;
            if frames > 1 {
                additional_frame_pixels = additional_frame_pixels
                    .checked_add(width.checked_mul(height).ok_or(ResolveError::AssetWork {
                        attempted: u64::MAX,
                    })?)
                    .ok_or(ResolveError::AssetWork {
                        attempted: u64::MAX,
                    })?;
            }
        }
        offset = next;
    }
    if offset != bytes.len() {
        return Err(ResolveError::WebPDecodeFailed);
    }
    Ok((frames, additional_frame_pixels, animation_has_alpha))
}

fn webp_chunk_at(bytes: &[u8], offset: usize) -> Result<([u8; 4], &[u8], usize), ResolveError> {
    let header = bytes
        .get(offset..offset.saturating_add(8))
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let kind = header[..4]
        .try_into()
        .map_err(|_| ResolveError::WebPDecodeFailed)?;
    let length = usize::try_from(u32::from_le_bytes(
        header[4..8]
            .try_into()
            .map_err(|_| ResolveError::WebPDecodeFailed)?,
    ))
    .map_err(|_| ResolveError::WebPDecodeFailed)?;
    let data_start = offset
        .checked_add(8)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let data_end = data_start
        .checked_add(length)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let data = bytes
        .get(data_start..data_end)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let next = data_end
        .checked_add(length & 1)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    bytes
        .get(data_end..next)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    Ok((kind, data, next))
}

fn validate_webp_frame_chunks(chunks: &[u8]) -> Result<bool, ResolveError> {
    let (first, first_data, next) = webp_chunk_at(chunks, 0)?;
    match first {
        kind if kind == *b"VP8 " => {
            if next != chunks.len() {
                return Err(ResolveError::WebPDecodeFailed);
            }
            Ok(false)
        }
        kind if kind == *b"VP8L" => {
            if next != chunks.len() {
                return Err(ResolveError::WebPDecodeFailed);
            }
            webp_lossless_has_alpha(first_data)
        }
        kind if kind == *b"ALPH" => {
            let (second, _, end) = webp_chunk_at(chunks, next)?;
            if second != *b"VP8 " || end != chunks.len() {
                return Err(ResolveError::WebPDecodeFailed);
            }
            Ok(true)
        }
        _ => Err(ResolveError::WebPDecodeFailed),
    }
}

fn webp_lossless_has_alpha(data: &[u8]) -> Result<bool, ResolveError> {
    if data.first() != Some(&0x2f) {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let header = u32::from_le_bytes(
        data.get(1..5)
            .ok_or(ResolveError::WebPDecodeFailed)?
            .try_into()
            .map_err(|_| ResolveError::WebPDecodeFailed)?,
    );
    Ok(header & (1 << 28) != 0)
}

fn read_webp_u24(bytes: &[u8]) -> Result<u32, ResolveError> {
    let bytes: [u8; 3] = bytes
        .try_into()
        .map_err(|_| ResolveError::WebPDecodeFailed)?;
    Ok(u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16))
}

fn validate_webp(bytes: &[u8]) -> Result<(u32, u32, WebPSemantics), ResolveError> {
    let mut decoder = webp_decoder(bytes)?;
    let (width, height) = decoder.dimensions();
    let animation_frames = decoder.num_frames().max(1);
    let orientation = decoder
        .exif_metadata()
        .map_err(map_webp_error)?
        .as_deref()
        .and_then(webp_exif_orientation);
    if decoder.is_animated() {
        let (_, _, frames_have_alpha) = scan_webp_animation(bytes, (width, height))?;
        if decoder.has_alpha() != frames_have_alpha {
            return Err(ResolveError::WebPDecodeFailed);
        }
        validate_webp_animation_frames(bytes)?;
    } else {
        let output_size = decoder.output_buffer_size().ok_or(ResolveError::Memory)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(output_size)
            .map_err(|_| ResolveError::Memory)?;
        output.resize(output_size, 0);
        decoder.read_image(&mut output).map_err(map_webp_error)?;
    }
    let (width, height) = oriented_dimensions(width, height, orientation.unwrap_or(1));
    Ok((
        width,
        height,
        WebPSemantics {
            animation_frames,
            orientation,
        },
    ))
}

fn validate_webp_animation_frames(bytes: &[u8]) -> Result<(), ResolveError> {
    let mut offset = 12_usize;
    while offset < bytes.len() {
        let (kind, data, next) = webp_chunk_at(bytes, offset)?;
        if kind == *b"ANMF" {
            validate_webp_animation_frame(data)?;
        }
        offset = next;
    }
    Ok(())
}

fn validate_webp_animation_frame(frame: &[u8]) -> Result<(), ResolveError> {
    let frame_header = frame.get(..16).ok_or(ResolveError::WebPDecodeFailed)?;
    if frame_header[15] & !0x03 != 0 {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let width = read_webp_u24(&frame_header[6..9])?.saturating_add(1);
    let height = read_webp_u24(&frame_header[9..12])?.saturating_add(1);
    let chunks = frame.get(16..).ok_or(ResolveError::WebPDecodeFailed)?;
    let has_alpha = validate_webp_frame_chunks(chunks)?;
    let mut body = Vec::new();
    if has_alpha && chunks.starts_with(b"ALPH") {
        append_webp_chunk(&mut body, b"VP8X", &[0x10, 0, 0, 0], width, height)?;
    }
    body.try_reserve_exact(chunks.len())
        .map_err(|_| ResolveError::Memory)?;
    body.extend_from_slice(chunks);
    let riff_size = body
        .len()
        .checked_add(4)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(ResolveError::Memory)?;
    let mut standalone = Vec::new();
    standalone
        .try_reserve_exact(body.len().saturating_add(12))
        .map_err(|_| ResolveError::Memory)?;
    standalone.extend_from_slice(b"RIFF");
    standalone.extend_from_slice(&riff_size.to_le_bytes());
    standalone.extend_from_slice(b"WEBP");
    standalone.extend_from_slice(&body);
    let mut decoder = webp_decoder(&standalone)?;
    if decoder.dimensions() != (width, height) || decoder.is_animated() {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let output_size = decoder.output_buffer_size().ok_or(ResolveError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_size)
        .map_err(|_| ResolveError::Memory)?;
    output.resize(output_size, 0);
    decoder.read_image(&mut output).map_err(map_webp_error)
}

fn append_webp_chunk(
    output: &mut Vec<u8>,
    kind: &[u8; 4],
    flags: &[u8; 4],
    width: u32,
    height: u32,
) -> Result<(), ResolveError> {
    output
        .try_reserve_exact(18)
        .map_err(|_| ResolveError::Memory)?;
    output.extend_from_slice(kind);
    output.extend_from_slice(&10_u32.to_le_bytes());
    output.extend_from_slice(flags);
    output.extend_from_slice(&webp_u24(width.saturating_sub(1)));
    output.extend_from_slice(&webp_u24(height.saturating_sub(1)));
    Ok(())
}

fn webp_u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

fn webp_exif_orientation(exif: &[u8]) -> Option<u8> {
    exif_orientation(exif.strip_prefix(b"Exif\0\0").unwrap_or(exif))
}

fn decode_webp_default_frame(
    bytes: &[u8],
    source_width: u32,
    source_height: u32,
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let mut decoder = webp_decoder(bytes)?;
    let output_size = decoder.output_buffer_size().ok_or(ResolveError::Memory)?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(output_size)
        .map_err(|_| ResolveError::Memory)?;
    decoded.resize(output_size, 0);
    decoder.read_image(&mut decoded).map_err(map_webp_error)?;

    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    if decoder.has_alpha() {
        if decoded.len() != rgba_size {
            return Err(ResolveError::WebPDecodeFailed);
        }
        rgba.extend_from_slice(&decoded);
    } else {
        for pixel in decoded.as_chunks::<3>().0 {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
        if rgba.len() != rgba_size {
            return Err(ResolveError::WebPDecodeFailed);
        }
    }
    normalize_rgba(&mut rgba, color_normalization)?;
    if let Some(orientation) = orientation {
        rgba = orient_rgba(&rgba, source_width, source_height, orientation)?;
    }
    finish_rgba(rgba)
}

fn map_webp_error(error: WebPDecodingError) -> ResolveError {
    if matches!(error, WebPDecodingError::MemoryLimitExceeded) {
        ResolveError::Memory
    } else {
        ResolveError::WebPDecodeFailed
    }
}

fn read_gif_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(GIF87A_SIGNATURE) && !bytes.starts_with(GIF89A_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let descriptor = bytes.get(6..13).ok_or(ResolveError::GifDecodeFailed)?;
    let width = u32::from(u16::from_le_bytes([descriptor[0], descriptor[1]]));
    let height = u32::from(u16::from_le_bytes([descriptor[2], descriptor[3]]));
    if width == 0 || height == 0 {
        return Err(ResolveError::GifDecodeFailed);
    }
    Ok((width, height))
}

fn gif_animation_validation_work(bytes: &[u8], _pixels: u64) -> Result<u64, ResolveError> {
    scan_gif(bytes)?
        .additional_frame_pixels
        .checked_mul(4)
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })
}

fn validate_gif(bytes: &[u8]) -> Result<(u32, u32, GifSemantics), ResolveError> {
    let metadata = scan_gif(bytes)?;
    let mut options = gif_decoder_options(gif_validation_memory_limit(
        metadata.width,
        metadata.height,
    )?);
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options
        .read_info(Cursor::new(bytes))
        .map_err(map_gif_error)?;
    let mut decoded_frames = 0_u32;
    while let Some(frame) = decoder.next_frame_info().map_err(map_gif_error)? {
        let expected_pixels = u64::from(frame.width)
            .checked_mul(u64::from(frame.height))
            .ok_or(ResolveError::GifDecodeFailed)?;
        let transparent = frame.transparent;
        let palette_entries = decoder.palette().map_err(map_gif_error)?.len() / 3;
        if transparent.is_some_and(|index| usize::from(index) >= palette_entries) {
            return Err(ResolveError::GifDecodeFailed);
        }
        let mut decoded_pixels = 0_u64;
        let mut index = [0_u8; 1];
        while decoder.fill_buffer(&mut index).map_err(map_gif_error)? {
            decoded_pixels = decoded_pixels
                .checked_add(1)
                .ok_or(ResolveError::GifDecodeFailed)?;
            if decoded_pixels > expected_pixels || usize::from(index[0]) >= palette_entries {
                return Err(ResolveError::GifDecodeFailed);
            }
        }
        if decoded_pixels != expected_pixels {
            return Err(ResolveError::GifDecodeFailed);
        }
        decoded_frames = decoded_frames
            .checked_add(1)
            .ok_or(ResolveError::GifDecodeFailed)?;
    }
    if decoded_frames != metadata.animation_frames {
        return Err(ResolveError::GifDecodeFailed);
    }
    Ok((
        metadata.width,
        metadata.height,
        GifSemantics {
            animation_frames: metadata.animation_frames,
        },
    ))
}

fn decode_gif_default_frame(
    bytes: &[u8],
    width: u32,
    height: u32,
    pixels: u64,
) -> Result<DecodedRaster, ResolveError> {
    let mut options = gif_decoder_options(gif_projection_memory_limit(width, height)?);
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options
        .read_info(Cursor::new(bytes))
        .map_err(map_gif_error)?;
    let frame = decoder
        .next_frame_info()
        .map_err(map_gif_error)?
        .ok_or(ResolveError::GifDecodeFailed)?;
    let frame_width = frame.width;
    let frame_height = frame.height;
    let frame_left = frame.left;
    let frame_top = frame.top;
    let transparent = frame.transparent;
    let palette = decoder.palette().map_err(map_gif_error)?;
    let mut palette_copy = [0_u8; 256 * 3];
    let palette_copy = palette_copy
        .get_mut(..palette.len())
        .ok_or(ResolveError::GifDecodeFailed)?;
    palette_copy.copy_from_slice(palette);
    let palette_entries = palette_copy.len() / 3;
    if transparent.is_some_and(|index| usize::from(index) >= palette_entries) {
        return Err(ResolveError::GifDecodeFailed);
    }
    let frame_pixels = u64::from(frame_width)
        .checked_mul(u64::from(frame_height))
        .ok_or(ResolveError::Memory)?;
    let frame_size = usize::try_from(frame_pixels).map_err(|_| ResolveError::Memory)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(frame_size)
        .map_err(|_| ResolveError::Memory)?;
    indices.resize(frame_size, 0);
    decoder
        .read_into_buffer(&mut indices)
        .map_err(map_gif_error)?;
    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    rgba.resize(rgba_size, 0);
    let frame_width = usize::from(frame_width);
    let canvas_width = usize::try_from(width).map_err(|_| ResolveError::Memory)?;
    for y in 0..usize::from(frame_height) {
        for x in 0..frame_width {
            let index = *indices
                .get(y.saturating_mul(frame_width).saturating_add(x))
                .ok_or(ResolveError::GifDecodeFailed)?;
            if usize::from(index) >= palette_entries {
                return Err(ResolveError::GifDecodeFailed);
            }
            let destination = y
                .checked_add(usize::from(frame_top))
                .and_then(|value| value.checked_mul(canvas_width))
                .and_then(|value| value.checked_add(usize::from(frame_left)))
                .and_then(|value| value.checked_add(x))
                .and_then(|value| value.checked_mul(4))
                .ok_or(ResolveError::Memory)?;
            let pixel = rgba
                .get_mut(destination..destination.saturating_add(4))
                .ok_or(ResolveError::GifDecodeFailed)?;
            if transparent == Some(index) {
                pixel.fill(0);
            } else {
                let palette_offset = usize::from(index).saturating_mul(3);
                pixel[..3].copy_from_slice(
                    palette_copy
                        .get(palette_offset..palette_offset.saturating_add(3))
                        .ok_or(ResolveError::GifDecodeFailed)?,
                );
                pixel[3] = 255;
            }
        }
    }
    for pixel in rgba.as_chunks_mut::<4>().0 {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
    let mut premultiplied_rgba = try_copy(&rgba)?;
    for pixel in premultiplied_rgba.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}

fn gif_decoder_options(memory_limit: gif::MemoryLimit) -> gif::DecodeOptions {
    let mut options = gif::DecodeOptions::new();
    options.check_frame_consistency(true);
    options.check_lzw_end_code(true);
    options.set_memory_limit(memory_limit);
    options
}

fn gif_validation_memory_limit(width: u32, height: u32) -> Result<gif::MemoryLimit, ResolveError> {
    NonZeroU64::new(decoder_workspace_bytes(width, height)?)
        .map(gif::MemoryLimit::Bytes)
        .ok_or(ResolveError::Memory)
}

fn gif_projection_memory_limit(width: u32, height: u32) -> Result<gif::MemoryLimit, ResolveError> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| bytes.checked_add(1_048_576))
        .and_then(NonZeroU64::new)
        .ok_or(ResolveError::Memory)?;
    Ok(gif::MemoryLimit::Bytes(bytes))
}

fn map_gif_error(error: gif::DecodingError) -> ResolveError {
    match error {
        gif::DecodingError::OutOfMemory | gif::DecodingError::MemoryLimit => ResolveError::Memory,
        _ => ResolveError::GifDecodeFailed,
    }
}

fn scan_gif(bytes: &[u8]) -> Result<GifMetadata, ResolveError> {
    let (width, height) = read_gif_dimensions(bytes)?;
    let is_gif89a = bytes.starts_with(GIF89A_SIGNATURE);
    let descriptor = bytes.get(6..13).ok_or(ResolveError::GifDecodeFailed)?;
    let mut offset = 13_usize;
    if descriptor[4] & 0x80 != 0 {
        let entries = gif_color_table_entries(descriptor[4])?;
        if usize::from(descriptor[5]) >= entries {
            return Err(ResolveError::GifDecodeFailed);
        }
        offset = skip_gif_color_table(bytes, offset, descriptor[4])?;
    }
    let mut frames = 0_u32;
    let mut additional_frame_pixels = 0_u64;
    loop {
        let block = *bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?;
        offset = offset.checked_add(1).ok_or(ResolveError::GifDecodeFailed)?;
        match block {
            0x2c => {
                let image = bytes
                    .get(offset..offset.saturating_add(9))
                    .ok_or(ResolveError::GifDecodeFailed)?;
                let frame_width = u32::from(u16::from_le_bytes([image[4], image[5]]));
                let frame_height = u32::from(u16::from_le_bytes([image[6], image[7]]));
                if frame_width == 0 || frame_height == 0 {
                    return Err(ResolveError::GifDecodeFailed);
                }
                if image[8] & 0x18 != 0 {
                    return Err(ResolveError::GifDecodeFailed);
                }
                offset = offset.checked_add(9).ok_or(ResolveError::GifDecodeFailed)?;
                if image[8] & 0x80 != 0 {
                    offset = skip_gif_color_table(bytes, offset, image[8])?;
                }
                let code_size = *bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?;
                if !(2..=8).contains(&code_size) {
                    return Err(ResolveError::GifDecodeFailed);
                }
                offset = skip_gif_sub_blocks(bytes, offset.saturating_add(1))?;
                frames = frames.checked_add(1).ok_or(ResolveError::GifDecodeFailed)?;
                if frames > 1 {
                    additional_frame_pixels = additional_frame_pixels
                        .checked_add(
                            u64::from(frame_width)
                                .checked_mul(u64::from(frame_height))
                                .ok_or(ResolveError::AssetWork {
                                    attempted: u64::MAX,
                                })?,
                        )
                        .ok_or(ResolveError::AssetWork {
                            attempted: u64::MAX,
                        })?;
                }
            }
            0x21 => {
                if !is_gif89a {
                    return Err(ResolveError::GifDecodeFailed);
                }
                let label = *bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?;
                offset = offset.checked_add(1).ok_or(ResolveError::GifDecodeFailed)?;
                if label == 0xf9 {
                    let control = bytes
                        .get(offset..offset.saturating_add(6))
                        .ok_or(ResolveError::GifDecodeFailed)?;
                    let disposal = (control[1] >> 2) & 0x07;
                    if control[0] != 4 || control[5] != 0 || control[1] & 0xe0 != 0 || disposal > 3
                    {
                        return Err(ResolveError::GifDecodeFailed);
                    }
                    offset = offset.checked_add(6).ok_or(ResolveError::GifDecodeFailed)?;
                } else {
                    let required_block_size = match label {
                        0x01 => Some(12),
                        0xff => Some(11),
                        0xfe => None,
                        _ => return Err(ResolveError::GifDecodeFailed),
                    };
                    if required_block_size
                        .is_some_and(|required| bytes.get(offset).copied() != Some(required))
                    {
                        return Err(ResolveError::GifDecodeFailed);
                    }
                    offset = skip_gif_sub_blocks(bytes, offset)?;
                }
            }
            0x3b if frames > 0 && offset == bytes.len() => {
                return Ok(GifMetadata {
                    width,
                    height,
                    animation_frames: frames,
                    additional_frame_pixels,
                });
            }
            _ => return Err(ResolveError::GifDecodeFailed),
        }
    }
}

fn skip_gif_color_table(bytes: &[u8], offset: usize, packed: u8) -> Result<usize, ResolveError> {
    let entries = gif_color_table_entries(packed)?;
    let end = offset
        .checked_add(entries.saturating_mul(3))
        .ok_or(ResolveError::GifDecodeFailed)?;
    bytes
        .get(offset..end)
        .ok_or(ResolveError::GifDecodeFailed)?;
    Ok(end)
}

fn gif_color_table_entries(packed: u8) -> Result<usize, ResolveError> {
    1_usize
        .checked_shl(u32::from((packed & 0x07) + 1))
        .ok_or(ResolveError::GifDecodeFailed)
}

fn skip_gif_sub_blocks(bytes: &[u8], mut offset: usize) -> Result<usize, ResolveError> {
    loop {
        let length = usize::from(*bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?);
        offset = offset
            .checked_add(1)
            .and_then(|offset| offset.checked_add(length))
            .ok_or(ResolveError::GifDecodeFailed)?;
        if length == 0 {
            return Ok(offset);
        }
        bytes
            .get(offset.saturating_sub(length)..offset)
            .ok_or(ResolveError::GifDecodeFailed)?;
    }
}

fn jpeg_decoder(bytes: &[u8]) -> JpegDecoder<ZCursor<&[u8]>> {
    let options = DecoderOptions::default()
        .set_max_width(usize::from(u16::MAX))
        .set_max_height(usize::from(u16::MAX))
        .set_use_unsafe(false)
        .set_strict_mode(true)
        .jpeg_set_out_colorspace(ColorSpace::RGB);
    JpegDecoder::new_with_options(ZCursor::new(bytes), options)
}

fn read_jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(JPEG_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let metadata = read_jpeg_metadata(bytes)?;
    Ok((metadata.width, metadata.height))
}

fn read_jpeg_metadata(bytes: &[u8]) -> Result<JpegMetadata, ResolveError> {
    let mut decoder = jpeg_decoder(bytes);
    decoder
        .decode_headers()
        .map_err(|_| ResolveError::JpegDecodeFailed)?;
    let info = decoder.info().ok_or(ResolveError::JpegDecodeFailed)?;
    if info.width == 0 || info.height == 0 {
        return Err(ResolveError::JpegDecodeFailed);
    }
    Ok(JpegMetadata {
        width: u32::from(info.width),
        height: u32::from(info.height),
        orientation: decoder.exif().and_then(|data| exif_orientation(data)),
    })
}

fn validate_static_jpeg(bytes: &[u8]) -> Result<(u32, u32, JpegSemantics), ResolveError> {
    if !bytes.starts_with(JPEG_SIGNATURE) || validate_jpeg_markers(bytes).is_err() {
        return Err(if bytes.starts_with(JPEG_SIGNATURE) {
            ResolveError::JpegDecodeFailed
        } else {
            ResolveError::Unsupported
        });
    }
    let metadata = read_jpeg_metadata(bytes)?;
    let orientation = metadata.orientation;
    let (width, height) =
        oriented_dimensions(metadata.width, metadata.height, orientation.unwrap_or(1));
    Ok((width, height, JpegSemantics { orientation }))
}

fn validate_jpeg_markers(bytes: &[u8]) -> Result<(), ResolveError> {
    let mut image_start = 0_usize;
    let mut image_count = 0_u8;
    let mut primary_has_mpf = false;
    let mut primary_has_gain_map_metadata = false;
    while image_start < bytes.len() {
        let mut icc_parts = [false; 256];
        let mut icc_part_count = None;
        if bytes.get(image_start..image_start.saturating_add(2)) != Some(b"\xff\xd8") {
            return Err(ResolveError::JpegDecodeFailed);
        }
        let mut offset = image_start + 2;
        let mut saw_frame = false;
        let mut saw_scan = false;
        loop {
            let marker_start = offset;
            if bytes.get(offset) != Some(&0xff) {
                return Err(ResolveError::JpegDecodeFailed);
            }
            while bytes.get(offset) == Some(&0xff) {
                offset = offset
                    .checked_add(1)
                    .ok_or(ResolveError::JpegDecodeFailed)?;
            }
            let marker = *bytes.get(offset).ok_or(ResolveError::JpegDecodeFailed)?;
            offset = offset
                .checked_add(1)
                .ok_or(ResolveError::JpegDecodeFailed)?;
            match marker {
                0xd9 if saw_frame && saw_scan => {
                    if let Some(part_count) = icc_part_count {
                        if !(1..=part_count).all(|part| icc_parts[usize::from(part)]) {
                            return Err(ResolveError::JpegDecodeFailed);
                        }
                    }
                    image_start = offset;
                    break;
                }
                0xd8 | 0x00 | 0x01 | 0xd0..=0xd8 => {
                    return Err(ResolveError::JpegDecodeFailed);
                }
                _ => {
                    let length_end = offset
                        .checked_add(2)
                        .ok_or(ResolveError::JpegDecodeFailed)?;
                    let length = usize::from(u16::from_be_bytes(
                        bytes
                            .get(offset..length_end)
                            .ok_or(ResolveError::JpegDecodeFailed)?
                            .try_into()
                            .map_err(|_| ResolveError::JpegDecodeFailed)?,
                    ));
                    if length < 2 {
                        return Err(ResolveError::JpegDecodeFailed);
                    }
                    let segment_end = offset
                        .checked_add(length)
                        .ok_or(ResolveError::JpegDecodeFailed)?;
                    let segment = bytes
                        .get(length_end..segment_end)
                        .ok_or(ResolveError::JpegDecodeFailed)?;
                    if image_count == 0 && marker == 0xe2 && segment.starts_with(b"MPF\0") {
                        primary_has_mpf = true;
                    }
                    if marker == 0xe2 && segment.starts_with(b"ICC_PROFILE\0") {
                        let sequence = *segment.get(12).ok_or(ResolveError::JpegDecodeFailed)?;
                        let part_count = *segment.get(13).ok_or(ResolveError::JpegDecodeFailed)?;
                        if sequence == 0
                            || part_count == 0
                            || sequence > part_count
                            || icc_part_count.is_some_and(|expected| expected != part_count)
                            || icc_parts[usize::from(sequence)]
                        {
                            return Err(ResolveError::JpegDecodeFailed);
                        }
                        icc_part_count = Some(part_count);
                        icc_parts[usize::from(sequence)] = true;
                    }
                    if image_count == 0
                        && marker == 0xe1
                        && segment
                            .windows(b"hdrgm:Version".len())
                            .any(|window| window == b"hdrgm:Version")
                    {
                        primary_has_gain_map_metadata = true;
                    }
                    if matches!(marker, 0xc0..=0xc2) {
                        if segment.len() < 6 || segment[0] != 8 {
                            return Err(ResolveError::JpegDecodeFailed);
                        }
                        saw_frame = true;
                    } else if matches!(marker, 0xc3..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc)
                    {
                        return Err(ResolveError::JpegDecodeFailed);
                    }
                    offset = segment_end;
                    if marker != 0xda {
                        continue;
                    }
                    if !saw_frame || segment.is_empty() {
                        return Err(ResolveError::JpegDecodeFailed);
                    }
                    saw_scan = true;
                    loop {
                        let byte = *bytes.get(offset).ok_or(ResolveError::JpegDecodeFailed)?;
                        offset = offset
                            .checked_add(1)
                            .ok_or(ResolveError::JpegDecodeFailed)?;
                        if byte != 0xff {
                            continue;
                        }
                        while bytes.get(offset) == Some(&0xff) {
                            offset = offset
                                .checked_add(1)
                                .ok_or(ResolveError::JpegDecodeFailed)?;
                        }
                        match bytes.get(offset).copied() {
                            Some(0x00 | 0xd0..=0xd7) => offset += 1,
                            Some(_) => {
                                offset = offset
                                    .checked_sub(1)
                                    .ok_or(ResolveError::JpegDecodeFailed)?;
                                break;
                            }
                            None => return Err(ResolveError::JpegDecodeFailed),
                        }
                    }
                }
            }
            if offset <= marker_start {
                return Err(ResolveError::JpegDecodeFailed);
            }
        }
        image_count = image_count
            .checked_add(1)
            .ok_or(ResolveError::JpegDecodeFailed)?;
        if image_start < bytes.len()
            && (image_count != 1 || !primary_has_mpf || !primary_has_gain_map_metadata)
        {
            return Err(ResolveError::JpegDecodeFailed);
        }
    }
    if image_count > 2 {
        return Err(ResolveError::JpegDecodeFailed);
    }
    Ok(())
}

fn decoder_workspace_bytes(width: u32, height: u32) -> Result<u64, ResolveError> {
    1_048_576_u64
        .checked_add(32_u64.saturating_mul(u64::from(width.max(height))))
        .ok_or(ResolveError::Memory)
}

fn png_animation_validation_work(bytes: &[u8], pixels: u64) -> Result<u64, ResolveError> {
    let mut offset = PNG_SIGNATURE.len();
    let mut animation_frames = 0_u32;
    let mut first_frame_control_precedes_idat = false;
    while offset < bytes.len() {
        let header_end = offset.checked_add(8).ok_or(ResolveError::DecodeFailed)?;
        let header = bytes
            .get(offset..header_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let length = usize::try_from(u32::from_be_bytes(
            header[..4]
                .try_into()
                .map_err(|_| ResolveError::DecodeFailed)?,
        ))
        .map_err(|_| ResolveError::DecodeFailed)?;
        let chunk_type = &header[4..8];
        let data_end = header_end
            .checked_add(length)
            .ok_or(ResolveError::DecodeFailed)?;
        let crc_end = data_end.checked_add(4).ok_or(ResolveError::DecodeFailed)?;
        let data = bytes
            .get(header_end..data_end)
            .ok_or(ResolveError::DecodeFailed)?;
        match chunk_type {
            b"acTL" if length == 8 => {
                animation_frames = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
            }
            b"fcTL" if animation_frames != 0 => first_frame_control_precedes_idat = true,
            b"IDAT" => break,
            _ => {}
        }
        offset = crc_end;
    }
    if animation_frames == 0 {
        return Ok(0);
    }
    let decoded_frames =
        animation_frames.saturating_add(u32::from(!first_frame_control_precedes_idat));
    u64::from(decoded_frames.saturating_sub(1))
        .checked_mul(pixels)
        .and_then(|work| work.checked_mul(4))
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })
}

struct Payload {
    bytes: Vec<u8>,
    claims: Claims,
}

fn payload(
    scene: &SceneDocument,
    supplemental: &SupplementalAssets,
    file_id: &str,
    authority: Authority,
    max_asset_bytes: u64,
) -> Result<Payload, ResolveError> {
    match authority {
        Authority::Scene => scene_payload(scene, file_id, max_asset_bytes),
        Authority::Supplemental => {
            let asset = supplemental.get(file_id).ok_or(ResolveError::Missing)?;
            let attempted = u64::try_from(asset.bytes().len()).unwrap_or(u64::MAX);
            if attempted > max_asset_bytes {
                return Err(ResolveError::AssetBytes { attempted });
            }
            Ok(Payload {
                bytes: try_copy(asset.bytes())?,
                claims: Claims {
                    mime_type: asset.advisory_media_type().map(try_owned).transpose()?,
                    data_url_media_type: None,
                },
            })
        }
        Authority::Missing => Err(ResolveError::Missing),
    }
}

pub(super) fn data_url_decoded_len(data_url: &str) -> u64 {
    let Some((_, encoded)) = split_data_url(data_url) else {
        return 0;
    };
    decoded_prefix(encoded).0
}

fn scene_payload(
    scene: &SceneDocument,
    file_id: &str,
    max_asset_bytes: u64,
) -> Result<Payload, ResolveError> {
    let record = (0..scene.file_record_count())
        .filter_map(|index| scene.file_record_at(index))
        .find(|record| record.key() == file_id)
        .ok_or(ResolveError::Missing)?;
    let SourceField::Value(data_url) = record.data_url() else {
        return Err(ResolveError::Malformed);
    };
    let (media_type, encoded) = split_data_url(data_url).ok_or(ResolveError::Malformed)?;
    let (attempted, valid) = decoded_prefix(encoded);
    if attempted > max_asset_bytes {
        return Err(ResolveError::AssetBytes { attempted });
    }
    if !valid {
        return Err(ResolveError::Malformed);
    }
    let estimate = usize::try_from(attempted).map_err(|_| ResolveError::Memory)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(estimate)
        .map_err(|_| ResolveError::Memory)?;
    bytes.resize(estimate, 0);
    let length = base64::engine::general_purpose::STANDARD
        .decode_slice(encoded, &mut bytes)
        .map_err(|_| ResolveError::Malformed)?;
    bytes.truncate(length);
    Ok(Payload {
        bytes,
        claims: Claims {
            mime_type: match record.mime_type() {
                SourceField::Value(value) => Some(try_owned(value)?),
                SourceField::Missing | SourceField::Invalid => None,
            },
            data_url_media_type: Some(try_owned(media_type)?),
        },
    })
}

fn split_data_url(data_url: &str) -> Option<(&str, &str)> {
    data_url.strip_prefix("data:")?.split_once(";base64,")
}

fn decoded_prefix(encoded: &str) -> (u64, bool) {
    let mut decoded = 0_u64;
    let quanta = encoded.as_bytes().chunks(4);
    let quantum_count = quanta.len();
    for (index, quantum) in quanta.enumerate() {
        if quantum.len() != 4 {
            return (decoded, false);
        }
        if base64_value(quantum[0]).is_none() {
            return (decoded, false);
        }
        let Some(second) = base64_value(quantum[1]) else {
            return (decoded, false);
        };
        let (bytes, terminal) = match (quantum[2], quantum[3]) {
            (b'=', b'=') if second & 0x0f == 0 => (1, true),
            (third, b'=') if base64_value(third).is_some_and(|value| value & 0x03 == 0) => {
                (2, true)
            }
            (third, fourth) if base64_value(third).is_some() && base64_value(fourth).is_some() => {
                (3, false)
            }
            _ => return (decoded, false),
        };
        decoded = decoded.saturating_add(bytes);
        if terminal {
            return (decoded, index + 1 == quantum_count);
        }
    }
    (decoded, true)
}

const fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn semantic_identity(
    revision: CompatibilityRevision,
    media_kind: MediaKind,
    bytes: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(media_kind.tag().to_le_bytes());
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn validate_static_png(bytes: &[u8]) -> Result<(u32, u32, PngSemantics), ResolveError> {
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let mut offset = PNG_SIGNATURE.len();
    let mut dimensions = None;
    let mut color_type = None;
    let mut chunk_index = 0_u64;
    let mut saw_plte = false;
    let mut saw_idat = false;
    let mut idat_ended = false;
    let mut saw_iend = false;
    let mut saw_exif = false;
    let mut semantics = PngSemantics::default();
    while offset < bytes.len() {
        let header_end = offset.checked_add(8).ok_or(ResolveError::DecodeFailed)?;
        let header = bytes
            .get(offset..header_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let length = usize::try_from(u32::from_be_bytes(
            header[..4]
                .try_into()
                .map_err(|_| ResolveError::DecodeFailed)?,
        ))
        .map_err(|_| ResolveError::DecodeFailed)?;
        let chunk_type: [u8; 4] = header[4..8]
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?;
        if !chunk_type.iter().all(u8::is_ascii_alphabetic) || chunk_type[2].is_ascii_lowercase() {
            return Err(ResolveError::DecodeFailed);
        }
        let data_end = header_end
            .checked_add(length)
            .ok_or(ResolveError::DecodeFailed)?;
        let crc_end = data_end.checked_add(4).ok_or(ResolveError::DecodeFailed)?;
        let data = bytes
            .get(header_end..data_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let expected_crc = u32::from_be_bytes(
            bytes
                .get(data_end..crc_end)
                .ok_or(ResolveError::DecodeFailed)?
                .try_into()
                .map_err(|_| ResolveError::DecodeFailed)?,
        );
        let mut crc = crc32fast::Hasher::new();
        crc.update(&chunk_type);
        crc.update(data);
        if crc.finalize() != expected_crc {
            return Err(ResolveError::DecodeFailed);
        }
        idat_ended |= saw_idat && &chunk_type != b"IDAT";
        match &chunk_type {
            b"IHDR" if chunk_index == 0 && length == 13 => {
                let width = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
                let height = u32::from_be_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
                let bit_depth = data[8];
                let png_color_type = data[9];
                let valid_depth = matches!(
                    (png_color_type, bit_depth),
                    (0, 1 | 2 | 4 | 8 | 16) | (2, 8 | 16) | (3, 1 | 2 | 4 | 8) | (4 | 6, 8 | 16)
                );
                if width == 0
                    || height == 0
                    || !valid_depth
                    || data[10] != 0
                    || data[11] != 0
                    || data[12] > 1
                {
                    return Err(ResolveError::DecodeFailed);
                }
                dimensions = Some((width, height));
                color_type = Some(png_color_type);
            }
            b"acTL" if length == 8 && !saw_idat && semantics.animation_frames == 0 => {
                semantics.animation_frames = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
                if semantics.animation_frames == 0 {
                    return Err(ResolveError::DecodeFailed);
                }
            }
            b"acTL" => return Err(ResolveError::DecodeFailed),
            b"cICP" if length == 4 && !saw_plte && !saw_idat && semantics.cicp.is_none() => {
                semantics.cicp = Some(data.try_into().map_err(|_| ResolveError::DecodeFailed)?);
            }
            b"cICP" => return Err(ResolveError::DecodeFailed),
            b"eXIf" if !saw_exif => {
                saw_exif = true;
                semantics.orientation = exif_orientation(data);
            }
            b"eXIf" => return Err(ResolveError::DecodeFailed),
            b"IHDR" => return Err(ResolveError::DecodeFailed),
            b"PLTE"
                if !saw_plte
                    && !saw_idat
                    && matches!(color_type, Some(2 | 3 | 6))
                    && length > 0
                    && length <= 768
                    && length % 3 == 0 =>
            {
                saw_plte = true;
            }
            b"PLTE" => return Err(ResolveError::DecodeFailed),
            b"IDAT" if dimensions.is_some() && !idat_ended => saw_idat = true,
            b"IDAT" => return Err(ResolveError::DecodeFailed),
            b"IEND" if length == 0 && saw_idat && (!matches!(color_type, Some(3)) || saw_plte) => {
                saw_iend = true;
                offset = crc_end;
                break;
            }
            b"IEND" => return Err(ResolveError::DecodeFailed),
            _ if chunk_type[0].is_ascii_uppercase() => {
                return Err(ResolveError::DecodeFailed);
            }
            _ => {}
        }
        chunk_index = chunk_index
            .checked_add(1)
            .ok_or(ResolveError::DecodeFailed)?;
        offset = crc_end;
    }
    if !saw_iend || offset != bytes.len() {
        return Err(ResolveError::DecodeFailed);
    }
    let (source_width, source_height) = dimensions.ok_or(ResolveError::DecodeFailed)?;
    let (width, height) = oriented_dimensions(
        source_width,
        source_height,
        semantics.orientation.unwrap_or(1),
    );
    validate_png_stream(bytes, source_width, source_height)?;
    Ok((width, height, semantics))
}

fn validate_png_stream(bytes: &[u8], width: u32, height: u32) -> Result<(), ResolveError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let logical_bytes = 1_048_576_u64
        .checked_add(32_u64.saturating_mul(u64::from(width.max(height))))
        .ok_or(ResolveError::Memory)?;
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(logical_bytes).map_err(|_| ResolveError::Memory)?,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    let default_image_is_separate =
        reader.info().frame_control.is_none() && reader.info().animation_control.is_some();
    let frame_count = reader.info().animation_control.map_or(1, |animation| {
        animation
            .num_frames
            .saturating_add(u32::from(default_image_is_separate))
    });
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(reader.output_buffer_size())
        .map_err(|_| ResolveError::Memory)?;
    frame.resize(reader.output_buffer_size(), 0);
    for _ in 0..frame_count {
        reader
            .next_frame(&mut frame)
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    reader.finish().map_err(|_| ResolveError::DecodeFailed)
}

fn decode_static_png(
    bytes: &[u8],
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let logical_bytes = pixels
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(1_048_576))
        .ok_or(ResolveError::Memory)?;
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(logical_bytes).map_err(|_| ResolveError::Memory)?,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    if reader.info().animation_control.is_some() && reader.info().frame_control.is_none() {
        reader
            .next_frame_info()
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    let output_size = reader.output_buffer_size();
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(output_size)
        .map_err(|_| ResolveError::Memory)?;
    decoded.resize(output_size, 0);
    let output = reader
        .next_frame(&mut decoded)
        .map_err(|_| ResolveError::DecodeFailed)?;
    decoded.truncate(output.buffer_size());
    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let frame_pixels = u64::from(output.width)
        .checked_mul(u64::from(output.height))
        .ok_or(ResolveError::Memory)?;
    let frame_rgba_size = usize::try_from(frame_pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut frame_rgba = Vec::new();
    frame_rgba
        .try_reserve_exact(frame_rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    match output.color_type {
        png::ColorType::Rgba => frame_rgba.extend_from_slice(&decoded),
        png::ColorType::Rgb => {
            for pixel in decoded.as_chunks::<3>().0 {
                frame_rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &value in &decoded {
                frame_rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in decoded.as_chunks::<2>().0 {
                frame_rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Indexed => return Err(ResolveError::DecodeFailed),
    }
    if frame_rgba.len() != frame_rgba_size {
        return Err(ResolveError::DecodeFailed);
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    rgba.resize(rgba_size, 0);
    let frame_control = reader.info().frame_control;
    let (x_offset, y_offset) =
        frame_control.map_or((0, 0), |frame| (frame.x_offset, frame.y_offset));
    let canvas_width = usize::try_from(reader.info().width).map_err(|_| ResolveError::Memory)?;
    let frame_width = usize::try_from(output.width).map_err(|_| ResolveError::Memory)?;
    let x_offset = usize::try_from(x_offset).map_err(|_| ResolveError::Memory)?;
    let y_offset = usize::try_from(y_offset).map_err(|_| ResolveError::Memory)?;
    for y in 0..usize::try_from(output.height).map_err(|_| ResolveError::Memory)? {
        let source_start = y
            .checked_mul(frame_width)
            .and_then(|value| value.checked_mul(4))
            .ok_or(ResolveError::Memory)?;
        let source_end = source_start
            .checked_add(frame_width.saturating_mul(4))
            .ok_or(ResolveError::Memory)?;
        let destination_start = y
            .checked_add(y_offset)
            .and_then(|value| value.checked_mul(canvas_width))
            .and_then(|value| value.checked_add(x_offset))
            .and_then(|value| value.checked_mul(4))
            .ok_or(ResolveError::Memory)?;
        let destination_end = destination_start
            .checked_add(frame_width.saturating_mul(4))
            .ok_or(ResolveError::Memory)?;
        let source = frame_rgba
            .get(source_start..source_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let destination = rgba
            .get_mut(destination_start..destination_end)
            .ok_or(ResolveError::DecodeFailed)?;
        destination.copy_from_slice(source);
    }
    normalize_rgba(&mut rgba, color_normalization)?;
    if let Some(orientation) = orientation {
        rgba = orient_rgba(
            &rgba,
            reader.info().width,
            reader.info().height,
            orientation,
        )?;
    }
    finish_rgba(rgba)
}

fn normalize_rgba(
    rgba: &mut [u8],
    color_normalization: ColorNormalization,
) -> Result<(), ResolveError> {
    match color_normalization {
        ColorNormalization::None => Ok(()),
        ColorNormalization::LinearSrgb => {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                for channel in &mut pixel[..3] {
                    *channel = LINEAR_TO_SRGB[usize::from(*channel)];
                }
            }
            Ok(())
        }
        ColorNormalization::Profile(profile) => {
            let srgb = ColorProfile::new_srgb();
            let transform = profile
                .create_transform_8bit(
                    Layout::Rgba,
                    &srgb,
                    Layout::Rgba,
                    TransformOptions::default(),
                )
                .map_err(|_| ResolveError::Unsupported)?;
            let source = try_copy(rgba)?;
            transform
                .transform(&source, rgba)
                .map_err(|_| ResolveError::Unsupported)
        }
    }
}

fn finish_rgba(mut rgba: Vec<u8>) -> Result<DecodedRaster, ResolveError> {
    for pixel in rgba.as_chunks_mut::<4>().0 {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
    let mut premultiplied_rgba = try_copy(&rgba)?;
    for pixel in premultiplied_rgba.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}

fn decode_static_jpeg(
    bytes: &[u8],
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let source_color_space = match &color_normalization {
        ColorNormalization::Profile(profile) => match profile.color_space {
            DataColorSpace::Rgb => ColorSpace::RGB,
            DataColorSpace::Gray => ColorSpace::Luma,
            _ => return Err(ResolveError::Unsupported),
        },
        ColorNormalization::None | ColorNormalization::LinearSrgb => ColorSpace::RGB,
    };
    let mut decoder = jpeg_decoder(bytes);
    decoder.set_options((*decoder.options()).jpeg_set_out_colorspace(source_color_space));
    let mut rgb = decoder
        .decode()
        .map_err(|_| ResolveError::JpegDecodeFailed)?;
    let info = decoder.info().ok_or(ResolveError::JpegDecodeFailed)?;
    let components =
        u64::try_from(source_color_space.num_components()).map_err(|_| ResolveError::Memory)?;
    let expected_rgb = usize::try_from(pixels.checked_mul(components).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    if rgb.len() != expected_rgb {
        return Err(ResolveError::JpegDecodeFailed);
    }
    if let ColorNormalization::Profile(profile) = color_normalization {
        let srgb = ColorProfile::new_srgb();
        let (source_layout, source_is_gray) = match profile.color_space {
            DataColorSpace::Rgb => (Layout::Rgb, false),
            DataColorSpace::Gray => (Layout::Gray, true),
            _ => return Err(ResolveError::Unsupported),
        };
        let transform = profile
            .create_transform_8bit(
                source_layout,
                &srgb,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .map_err(|_| ResolveError::Unsupported)?;
        let source = try_copy(&rgb)?;
        if source_is_gray {
            let rgb_size = usize::try_from(pixels.checked_mul(3).ok_or(ResolveError::Memory)?)
                .map_err(|_| ResolveError::Memory)?;
            rgb.try_reserve_exact(rgb_size.saturating_sub(rgb.len()))
                .map_err(|_| ResolveError::Memory)?;
            rgb.resize(rgb_size, 0);
        }
        transform
            .transform(&source, &mut rgb)
            .map_err(|_| ResolveError::Unsupported)?;
    } else if matches!(color_normalization, ColorNormalization::LinearSrgb) {
        for channel in &mut rgb {
            *channel = LINEAR_TO_SRGB[usize::from(*channel)];
        }
    }

    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    for pixel in rgb.as_chunks::<3>().0 {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    if let Some(orientation) = orientation {
        rgba = orient_rgba(
            &rgba,
            u32::from(info.width),
            u32::from(info.height),
            orientation,
        )?;
    }
    let premultiplied_rgba = try_copy(&rgba)?;
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}

fn exif_orientation(exif: &[u8]) -> Option<u8> {
    let little_endian = match exif.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let read_u16 = |bytes: &[u8]| {
        let bytes: [u8; 2] = bytes.try_into().ok()?;
        Some(if little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    };
    let read_u32 = |bytes: &[u8]| {
        let bytes: [u8; 4] = bytes.try_into().ok()?;
        Some(if little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };
    if read_u16(exif.get(2..4)?)? != 42 {
        return None;
    }
    let ifd_offset = usize::try_from(read_u32(exif.get(4..8)?)?).ok()?;
    let entry_count = usize::from(read_u16(exif.get(ifd_offset..ifd_offset.checked_add(2)?)?)?);
    let entries_start = ifd_offset.checked_add(2)?;
    for index in 0..entry_count {
        let start = entries_start.checked_add(index.checked_mul(12)?)?;
        let entry = exif.get(start..start.checked_add(12)?)?;
        if read_u16(&entry[..2])? != 0x0112 {
            continue;
        }
        if read_u16(&entry[2..4])? != 3 || read_u32(&entry[4..8])? != 1 {
            return None;
        }
        let orientation = read_u16(&entry[8..10])?;
        return u8::try_from(orientation)
            .ok()
            .filter(|value| (1..=8).contains(value));
    }
    None
}

fn oriented_dimensions(width: u32, height: u32, orientation: u8) -> (u32, u32) {
    if matches!(orientation, 5..=8) {
        (height, width)
    } else {
        (width, height)
    }
}

fn orient_rgba(
    source: &[u8],
    width: u32,
    height: u32,
    orientation: u8,
) -> Result<Vec<u8>, ResolveError> {
    if orientation == 1 {
        return try_copy(source);
    }
    let (output_width, _) = oriented_dimensions(width, height, orientation);
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| ResolveError::Memory)?;
    output.resize(source.len(), 0);
    let width = usize::try_from(width).map_err(|_| ResolveError::Memory)?;
    let height = usize::try_from(height).map_err(|_| ResolveError::Memory)?;
    let output_width = usize::try_from(output_width).map_err(|_| ResolveError::Memory)?;
    for y in 0..height {
        for x in 0..width {
            let (destination_x, destination_y) = match orientation {
                2 => (width - 1 - x, y),
                3 => (width - 1 - x, height - 1 - y),
                4 => (x, height - 1 - y),
                5 => (y, x),
                6 => (height - 1 - y, x),
                7 => (height - 1 - y, width - 1 - x),
                8 => (y, width - 1 - x),
                _ => (x, y),
            };
            let source_start = (y * width + x) * 4;
            let destination_start = (destination_y * output_width + destination_x) * 4;
            output[destination_start..destination_start + 4]
                .copy_from_slice(&source[source_start..source_start + 4]);
        }
    }
    Ok(output)
}

fn prepare_color_normalization(
    bytes: &[u8],
    semantics: &RasterSemantics,
    workspace_bytes: u64,
) -> Result<ColorNormalization, ResolveError> {
    match semantics {
        RasterSemantics::Png(semantics) => {
            prepare_png_color_normalization(bytes, semantics, workspace_bytes)
        }
        RasterSemantics::Jpeg(_) => {
            let mut decoder = jpeg_decoder(bytes);
            decoder
                .decode_headers()
                .map_err(|_| ResolveError::JpegDecodeFailed)?;
            decoder
                .icc_profile()
                .map_or(Ok(ColorNormalization::None), |profile| {
                    ColorProfile::new_from_slice(&profile)
                        .map(Box::new)
                        .map(ColorNormalization::Profile)
                        .map_err(|_| ResolveError::Unsupported)
                })
        }
        RasterSemantics::Gif(_) => Ok(ColorNormalization::None),
        RasterSemantics::WebP(_) => {
            let mut decoder = webp_decoder(bytes)?;
            decoder.icc_profile().map_err(map_webp_error)?.map_or(
                Ok(ColorNormalization::None),
                |profile| {
                    ColorProfile::new_from_slice(&profile)
                        .map(Box::new)
                        .map(ColorNormalization::Profile)
                        .map_err(|_| ResolveError::Unsupported)
                },
            )
        }
        RasterSemantics::Bmp(_) => Ok(ColorNormalization::None),
        RasterSemantics::Ico(semantics) => match &semantics.payload {
            IcoPayloadSemantics::Png(png) => {
                let payload = bytes
                    .get(semantics.offset..semantics.offset + semantics.length)
                    .ok_or(ResolveError::IcoDecodeFailed)?;
                prepare_png_color_normalization(payload, png, workspace_bytes)
                    .map_err(|_| ResolveError::IcoDecodeFailed)
            }
            IcoPayloadSemantics::Dib(_) => Ok(ColorNormalization::None),
        },
    }
}

fn prepare_png_color_normalization(
    bytes: &[u8],
    semantics: &PngSemantics,
    workspace_bytes: u64,
) -> Result<ColorNormalization, ResolveError> {
    if let Some(cicp) = semantics.cicp {
        return cicp_color_profile(cicp)
            .map(Box::new)
            .map(ColorNormalization::Profile);
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(workspace_bytes).map_err(|_| ResolveError::Memory)?,
    });
    let reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    png_color_profile(reader.info())
}

fn cicp_color_profile(cicp: [u8; 4]) -> Result<ColorProfile, ResolveError> {
    let color_primaries =
        CicpColorPrimaries::try_from(cicp[0]).map_err(|_| ResolveError::Unsupported)?;
    let transfer_characteristics =
        TransferCharacteristics::try_from(cicp[1]).map_err(|_| ResolveError::Unsupported)?;
    let matrix_coefficients =
        MatrixCoefficients::try_from(cicp[2]).map_err(|_| ResolveError::Unsupported)?;
    if !matches!(matrix_coefficients, MatrixCoefficients::Identity)
        || cicp[3] > 1
        || matches!(
            color_primaries,
            CicpColorPrimaries::Reserved | CicpColorPrimaries::Unspecified
        )
        || matches!(
            transfer_characteristics,
            TransferCharacteristics::Reserved | TransferCharacteristics::Unspecified
        )
    {
        return Err(ResolveError::Unsupported);
    }
    Ok(ColorProfile::new_from_cicp(CicpProfile {
        color_primaries,
        transfer_characteristics,
        matrix_coefficients,
        full_range: cicp[3] == 1,
    }))
}

fn png_color_profile(info: &png::Info<'_>) -> Result<ColorNormalization, ResolveError> {
    if let Some(profile) = info.icc_profile.as_deref() {
        return ColorProfile::new_from_slice(profile)
            .map(Box::new)
            .map(ColorNormalization::Profile)
            .map_err(|_| ResolveError::Unsupported);
    }
    if info.srgb.is_some() {
        return Ok(ColorNormalization::None);
    }
    if info.source_gamma.is_none() && info.source_chromaticities.is_none() {
        return Ok(ColorNormalization::None);
    }
    if info.source_chromaticities.is_none()
        && info
            .source_gamma
            .is_some_and(|gamma| gamma.into_scaled() == 100_000)
    {
        return Ok(ColorNormalization::LinearSrgb);
    }

    let mut profile = ColorProfile::new_srgb();
    profile.cicp = None;
    if let Some(chromaticities) = info.source_chromaticities {
        let value = |scaled: png::ScaledFloat| f64::from(scaled.into_scaled()) / 100_000.0;
        profile.update_rgb_colorimetry(
            XyY::new(
                value(chromaticities.white.0),
                value(chromaticities.white.1),
                1.0,
            ),
            ColorPrimaries {
                red: Chromaticity {
                    x: value(chromaticities.red.0) as f32,
                    y: value(chromaticities.red.1) as f32,
                },
                green: Chromaticity {
                    x: value(chromaticities.green.0) as f32,
                    y: value(chromaticities.green.1) as f32,
                },
                blue: Chromaticity {
                    x: value(chromaticities.blue.0) as f32,
                    y: value(chromaticities.blue.1) as f32,
                },
            },
        );
    }
    if let Some(gamma) = info.source_gamma {
        let gamma = gamma.into_value();
        if gamma <= 0.0 {
            return Err(ResolveError::Unsupported);
        }
        let curve: ToneReprCurve = curve_from_gamma(1.0 / gamma);
        profile.red_trc = Some(curve.clone());
        profile.green_trc = Some(curve.clone());
        profile.blue_trc = Some(curve);
    }
    Ok(ColorNormalization::Profile(Box::new(profile)))
}

fn try_copy(bytes: &[u8]) -> Result<Vec<u8>, ResolveError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| ResolveError::Memory)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

fn try_owned(value: &str) -> Result<String, ResolveError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| ResolveError::Memory)?;
    owned.push_str(value);
    Ok(owned)
}

fn logical_aligned_bytes(bytes: u64) -> u64 {
    bytes.saturating_add(63) / 64 * 64
}
