use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;

use base64::Engine as _;
use moxcms::{
    curve_from_gamma, Chromaticity, CicpColorPrimaries, CicpProfile, ColorPrimaries, ColorProfile,
    Layout, MatrixCoefficients, ToneReprCurve, TransferCharacteristics, TransformOptions, XyY,
};
use sha2::{Digest as _, Sha256};

use crate::scene::SourceField;
use crate::{
    CompatibilityRevision, OutputFormat, RenderError, ResourceLimits, SceneDocument,
    SupplementalAssets,
};

use super::ResolvedImageAsset;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const IDENTITY_DOMAIN: &[u8] = b"excalidraw-render:semantic-asset:v1\0";

#[derive(Clone, Copy)]
pub(super) enum Authority {
    Scene,
    Supplemental,
    Missing,
}

#[derive(Clone, Copy)]
pub(super) enum ResolveError {
    Missing,
    Malformed,
    Unsupported,
    DecodeFailed,
    AssetBytes { attempted: u64 },
    Dimensions { attempted: u64 },
    Pixels { attempted: u64 },
    AssetWork { attempted: u64 },
    Memory,
}

pub(super) enum ResolutionFailure {
    Asset(ResolveError),
    Transaction(RenderError),
}

impl From<ResolveError> for ResolutionFailure {
    fn from(error: ResolveError) -> Self {
        Self::Asset(error)
    }
}

pub(super) enum IntrinsicTransaction {
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
    pub(super) claims: Claims,
    pub(super) animated: bool,
}

struct ValidatedPng {
    identity: [u8; 32],
    bytes: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    semantics: PngSemantics,
}

#[derive(Clone, Copy, Default)]
struct PngSemantics {
    cicp: Option<[u8; 4]>,
    animation_frames: u32,
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

struct DecodedPng {
    rgba: Arc<Vec<u8>>,
    premultiplied_rgba: Arc<Vec<u8>>,
}

#[derive(Default)]
pub(super) struct Cache {
    validated: BTreeMap<[u8; 32], Arc<ValidatedPng>>,
    decoded: BTreeMap<[u8; 32], Arc<DecodedPng>>,
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
        let identity = semantic_identity(revision, &payload.bytes);
        let (prepared_width, prepared_height) = read_png_ihdr(&payload.bytes)?;
        let pixels = u64::from(prepared_width)
            .checked_mul(u64::from(prepared_height))
            .ok_or(ResolveError::Pixels {
                attempted: u64::MAX,
            })?;
        let payload_bytes = u64::try_from(payload.bytes.len()).unwrap_or(u64::MAX);
        let animation_validation_work = png_animation_validation_work(&payload.bytes, pixels)?;
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
                return Err(ResolveError::AssetWork {
                    attempted: validation_work,
                }
                .into());
            }
            let decoder_workspace = decoder_workspace_bytes(prepared_width, prepared_height)?;
            transact(IntrinsicTransaction::Acquire {
                event: "semantic_png_source",
                identity,
                persistent_working_bytes: validation_working_bytes,
                temporary_working_bytes: decoder_workspace,
                decoded_pixels: 0,
                total_work_units: validation_work,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let validation = match validate_static_png(&payload.bytes) {
                Ok(validation) => validation,
                Err(error) => {
                    transact(IntrinsicTransaction::Abort {
                        event: "semantic_png_source",
                        identity,
                        bytes: validation_working_bytes.saturating_add(decoder_workspace),
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
            };
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "png_validation_workspace",
                identity,
                bytes: decoder_workspace,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let (width, height, semantics) = validation;
            let validated = Arc::new(ValidatedPng {
                identity,
                bytes: Arc::new(payload.bytes),
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
                return Err(ResolveError::AssetWork { attempted: work }.into());
            }
            let decoder_workspace = decoder_workspace_bytes(validated.width, validated.height)?;
            transact(IntrinsicTransaction::Acquire {
                event: "png_color_normalize",
                identity,
                persistent_working_bytes: 0,
                temporary_working_bytes: decoder_workspace,
                decoded_pixels: 0,
                total_work_units: payload_bytes,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let color_normalization = match prepare_color_normalization(
                &validated.bytes,
                validated.semantics,
                decoder_workspace,
            ) {
                Ok(normalization) => normalization,
                Err(error) => {
                    transact(IntrinsicTransaction::Abort {
                        event: "png_color_normalize",
                        identity,
                        bytes: decoder_workspace,
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
            };
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "png_color_normalize_workspace",
                identity,
                bytes: decoder_workspace,
            })
            .map_err(ResolutionFailure::Transaction)?;
            let decode_working_bytes = 256_u64
                .checked_add(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
                .ok_or(ResolveError::Memory)?;
            transact(IntrinsicTransaction::Acquire {
                event: "decoded_png_rgba",
                identity,
                persistent_working_bytes: decode_working_bytes,
                temporary_working_bytes: decoder_workspace,
                decoded_pixels: pixels,
                total_work_units: payload_bytes
                    .saturating_mul(3)
                    .saturating_add(pixels.saturating_mul(4)),
            })
            .map_err(ResolutionFailure::Transaction)?;
            let decoded = decode_static_png(
                &validated.bytes,
                pixels,
                color_normalization,
                validated.semantics.orientation,
            );
            let decoded = match decoded {
                Ok(decoded) => Arc::new(decoded),
                Err(error) => {
                    transact(IntrinsicTransaction::Abort {
                        event: "decoded_png_rgba",
                        identity,
                        bytes: decode_working_bytes.saturating_add(decoder_workspace),
                    })
                    .map_err(ResolutionFailure::Transaction)?;
                    return Err(error.into());
                }
            };
            transact(IntrinsicTransaction::ReleaseTemporary {
                event: "png_decoder_workspace",
                identity,
                bytes: decoder_workspace,
            })
            .map_err(ResolutionFailure::Transaction)?;
            self.decoded.insert(identity, Arc::clone(&decoded));
            Some(decoded)
        };

        Ok(Resolution {
            asset: Arc::new(ResolvedImageAsset {
                identity: validated.identity,
                source_png: Arc::clone(&validated.bytes),
                width: validated.width,
                height: validated.height,
                rgba: decoded.as_ref().map(|decoded| Arc::clone(&decoded.rgba)),
                premultiplied_rgba: decoded
                    .as_ref()
                    .map(|decoded| Arc::clone(&decoded.premultiplied_rgba)),
            }),
            claims: payload.claims,
            animated: validated.semantics.animation_frames > 1,
        })
    }
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

fn semantic_identity(revision: CompatibilityRevision, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(1_u64.to_le_bytes());
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
) -> Result<DecodedPng, ResolveError> {
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
            for pixel in decoded.chunks_exact(3) {
                frame_rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &value in &decoded {
                frame_rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in decoded.chunks_exact(2) {
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
    match color_normalization {
        ColorNormalization::None => {}
        ColorNormalization::LinearSrgb => {
            for pixel in rgba.chunks_exact_mut(4) {
                for channel in &mut pixel[..3] {
                    *channel = LINEAR_TO_SRGB[usize::from(*channel)];
                }
            }
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
            let source = try_copy(&rgba)?;
            transform
                .transform(&source, &mut rgba)
                .map_err(|_| ResolveError::Unsupported)?;
        }
    }
    if let Some(orientation) = orientation {
        rgba = orient_rgba(
            &rgba,
            reader.info().width,
            reader.info().height,
            orientation,
        )?;
    }
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
    let mut premultiplied = Vec::new();
    premultiplied
        .try_reserve_exact(rgba.len())
        .map_err(|_| ResolveError::Memory)?;
    premultiplied.extend_from_slice(&rgba);
    for pixel in premultiplied.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(DecodedPng {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied),
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
    semantics: PngSemantics,
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
