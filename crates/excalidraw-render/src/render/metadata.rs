use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::{
    CompatibilityReport, DiagnosticSeverity, ReferenceProfileIdentity, RenderRequest, SceneDocument,
};

use super::FormatEncodeError;

const SCENE_MEDIA_TYPE: &str = "application/vnd.excalidraw+json";
const WRAPPER_PREFIX: &[u8] =
    b"{\"version\":\"1\",\"encoding\":\"bstring\",\"compressed\":true,\"encoded\":\"";

#[derive(Clone)]
pub(super) struct ArtifactMetadata {
    manifest: Option<String>,
    scene: Option<EmbeddedScene>,
}

#[derive(Clone)]
struct EmbeddedScene {
    canonical: Vec<u8>,
    wrapper: Vec<u8>,
    compressed_len: usize,
}

impl ArtifactMetadata {
    pub(super) fn build(
        scene: &SceneDocument,
        profile: &ReferenceProfileIdentity,
        report: &CompatibilityReport,
        mandatory_notices: Option<&super::notices::MandatoryNotices>,
        selection: MetadataSelection,
    ) -> Result<Self, FormatEncodeError> {
        let canonical = selection
            .embedded_scene
            .then(|| scene.canonicalized_original_bytes())
            .transpose()
            .map_err(|_| memory("canonical_original_scene"))?;
        let scene_digest = canonical.as_deref().map(sha256_string);
        let scene = canonical
            .map(|canonical| {
                let compressed = stored_zlib(&canonical)?;
                let wrapper = scene_wrapper(&compressed)?;
                Ok(EmbeddedScene {
                    canonical,
                    wrapper,
                    compressed_len: compressed.len(),
                })
            })
            .transpose()?;
        let notices = mandatory_notices.map_or("[]", super::notices::MandatoryNotices::as_json);
        let has_manifest = selection.provenance
            || selection.diagnostic_summary
            || selection.embedded_scene
            || mandatory_notices.is_some();
        let manifest = has_manifest
            .then(|| manifest_json(profile, report, notices, scene_digest.as_deref(), selection))
            .transpose()?;
        Ok(Self { manifest, scene })
    }

    pub(super) fn manifest(&self) -> Option<&str> {
        self.manifest.as_deref()
    }

    pub(super) fn canonical_scene(&self) -> Option<&[u8]> {
        self.scene.as_ref().map(|scene| scene.canonical.as_slice())
    }

    pub(super) fn scene_wrapper(&self) -> Option<&[u8]> {
        self.scene.as_ref().map(|scene| scene.wrapper.as_slice())
    }

    pub(super) fn scratch_bytes(&self) -> u64 {
        let scene = self.scene.as_ref().map_or(0_u64, |scene| {
            aligned(u64::try_from(scene.canonical.len()).unwrap_or(u64::MAX)).saturating_add(
                aligned(u64::try_from(scene.compressed_len).unwrap_or(u64::MAX)),
            )
        });
        512_u64.saturating_add(scene).saturating_add(
            self.manifest.as_ref().map_or(0_u64, |manifest| {
                aligned(u64::try_from(manifest.len()).unwrap_or(u64::MAX))
            }),
        )
    }

    pub(super) fn generation_work(&self) -> u64 {
        self.scene
            .as_ref()
            .map_or(0_u64, |scene| {
                u64::try_from(scene.canonical.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(u64::try_from(scene.compressed_len).unwrap_or(u64::MAX))
                    .saturating_add(u64::try_from(scene.wrapper.len()).unwrap_or(u64::MAX))
            })
            .saturating_add(self.manifest.as_ref().map_or(0_u64, |manifest| {
                u64::try_from(manifest.len()).unwrap_or(u64::MAX)
            }))
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct MetadataSelection {
    pub(super) provenance: bool,
    pub(super) diagnostic_summary: bool,
    pub(super) embedded_scene: bool,
}

impl MetadataSelection {
    pub(super) fn requested(request: &RenderRequest) -> Self {
        Self {
            provenance: request.metadata_options().provenance(),
            diagnostic_summary: request.metadata_options().diagnostic_summary(),
            embedded_scene: request.metadata_options().embedded_scene(),
        }
    }
}

fn manifest_json(
    profile: &ReferenceProfileIdentity,
    report: &CompatibilityReport,
    notices: &str,
    scene_digest: Option<&str>,
    selection: MetadataSelection,
) -> Result<String, FormatEncodeError> {
    let diagnostic_capacity = report
        .diagnostics()
        .iter()
        .try_fold(0_usize, |total, diagnostic| {
            total
                .checked_add(diagnostic.code().as_str().len())
                .and_then(|total| total.checked_add(64))
        })
        .ok_or_else(metadata_size_error)?;
    let capacity = 1_024_usize
        .checked_add(notices.len())
        .and_then(|capacity| capacity.checked_add(diagnostic_capacity))
        .ok_or_else(metadata_size_error)?;
    let mut manifest = String::new();
    manifest
        .try_reserve_exact(capacity)
        .map_err(|_| memory("artifact_manifest"))?;
    manifest.push_str("{\"schema_version\":1,\"provenance\":");
    if selection.provenance {
        manifest.push_str("{\"renderer_version\":\"");
        manifest.push_str(env!("CARGO_PKG_VERSION"));
        manifest.push_str("\",\"compatibility_revision\":");
        write!(manifest, "{}", profile.compatibility_revision().get())
            .map_err(|_| metadata_size_error())?;
        manifest.push_str(",\"excalidraw_release\":\"");
        manifest.push_str(profile.excalidraw_release());
        manifest.push_str("\",\"reference_profile_digest\":\"");
        manifest.push_str(profile.profile_digest());
        manifest.push_str("\"}");
    } else {
        manifest.push_str("null");
    }
    manifest.push_str(",\"diagnostic_summary\":");
    if selection.diagnostic_summary {
        push_diagnostic_summary(&mut manifest, report)?;
    } else {
        manifest.push_str("null");
    }
    manifest.push_str(",\"notices\":");
    manifest.push_str(notices);
    manifest.push_str(",\"embedded_scene\":");
    if let Some(digest) = scene_digest {
        manifest.push_str("{\"name\":\"scene.excalidraw\",\"media_type\":\"");
        manifest.push_str(SCENE_MEDIA_TYPE);
        manifest.push_str("\",\"sha256\":\"");
        manifest.push_str(digest);
        manifest.push_str("\"}");
    } else {
        manifest.push_str("null");
    }
    manifest.push('}');
    Ok(manifest)
}

fn push_diagnostic_summary(
    output: &mut String,
    report: &CompatibilityReport,
) -> Result<(), FormatEncodeError> {
    let mut info = 0_u64;
    let mut warning = 0_u64;
    let mut by_code = Vec::new();
    by_code
        .try_reserve_exact(report.len())
        .map_err(|_| memory("diagnostic_summary_counts"))?;
    for diagnostic in report.diagnostics() {
        match diagnostic.severity() {
            DiagnosticSeverity::Info => info += 1,
            DiagnosticSeverity::Warning => warning += 1,
        }
        if let Some((_, count)) = by_code
            .iter_mut()
            .find(|(code, _)| *code == diagnostic.code().as_str())
        {
            *count += 1;
        } else {
            by_code.push((diagnostic.code().as_str(), 1_u64));
        }
    }
    by_code.sort_unstable_by_key(|(code, _)| *code);
    output.push_str("{\"snapshot\":\"pre_metadata\",\"total\":");
    write!(output, "{}", report.len()).map_err(|_| metadata_size_error())?;
    output.push_str(",\"by_severity\":{\"info\":");
    write!(output, "{info}").map_err(|_| metadata_size_error())?;
    output.push_str(",\"warning\":");
    write!(output, "{warning}").map_err(|_| metadata_size_error())?;
    output.push_str("},\"by_code\":[");
    let mut separator = "";
    for (code, count) in by_code {
        output.push_str(separator);
        output.push_str("{\"code\":\"");
        output.push_str(code);
        output.push_str("\",\"count\":");
        write!(output, "{count}").map_err(|_| metadata_size_error())?;
        output.push('}');
        separator = ",";
    }
    output.push_str("]}");
    Ok(())
}

fn scene_wrapper(compressed: &[u8]) -> Result<Vec<u8>, FormatEncodeError> {
    let capacity = WRAPPER_PREFIX
        .len()
        .checked_add(compressed.len().saturating_mul(6))
        .and_then(|length| length.checked_add(2))
        .ok_or_else(metadata_size_error)?;
    let mut wrapper = Vec::new();
    wrapper
        .try_reserve_exact(capacity)
        .map_err(|_| memory("embedded_scene_wrapper"))?;
    wrapper.extend_from_slice(WRAPPER_PREFIX);
    for byte in compressed {
        match byte {
            b'"' => wrapper.extend_from_slice(b"\\\""),
            b'\\' => wrapper.extend_from_slice(b"\\\\"),
            0x08 => wrapper.extend_from_slice(b"\\b"),
            0x09 => wrapper.extend_from_slice(b"\\t"),
            0x0a => wrapper.extend_from_slice(b"\\n"),
            0x0c => wrapper.extend_from_slice(b"\\f"),
            0x0d => wrapper.extend_from_slice(b"\\r"),
            0x00..=0x1f => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                wrapper.extend_from_slice(b"\\u00");
                wrapper.push(HEX[usize::from(byte >> 4)]);
                wrapper.push(HEX[usize::from(byte & 0x0f)]);
            }
            byte => wrapper.push(*byte),
        }
    }
    wrapper.extend_from_slice(b"\"}");
    Ok(wrapper)
}

fn stored_zlib(source: &[u8]) -> Result<Vec<u8>, FormatEncodeError> {
    let block_count = source.len().max(1).div_ceil(usize::from(u16::MAX));
    let capacity = 2_usize
        .checked_add(block_count.checked_mul(5).ok_or_else(metadata_size_error)?)
        .and_then(|length| length.checked_add(source.len()))
        .and_then(|length| length.checked_add(4))
        .ok_or_else(metadata_size_error)?;
    let mut zlib = Vec::new();
    zlib.try_reserve_exact(capacity)
        .map_err(|_| memory("embedded_scene_compression"))?;
    zlib.extend_from_slice(&[0x78, 0x01]);
    let mut offset = 0_usize;
    loop {
        let length = (source.len() - offset).min(usize::from(u16::MAX));
        let final_block = offset + length == source.len();
        zlib.push(u8::from(final_block));
        let length = u16::try_from(length).map_err(|_| metadata_size_error())?;
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(&source[offset..offset + usize::from(length)]);
        offset += usize::from(length);
        if final_block {
            break;
        }
    }
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in source {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    Ok(zlib)
}

fn sha256_string(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn memory(subject: &'static str) -> FormatEncodeError {
    FormatEncodeError::MemoryUnavailable {
        subject_kind: "metadata",
        subject,
    }
}

fn metadata_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("artifact metadata size exceeds the supported range".to_owned())
}

fn aligned(bytes: u64) -> u64 {
    bytes.saturating_add(63) / 64 * 64
}

#[cfg(test)]
mod tests {
    use super::stored_zlib;

    #[test]
    fn stored_zlib_uses_exact_65535_byte_block_boundaries() {
        let Ok(one_block) = stored_zlib(&vec![b'a'; 65_535]) else {
            panic!("one block encodes");
        };
        assert_eq!(&one_block[..7], &[0x78, 0x01, 0x01, 0xff, 0xff, 0x00, 0x00]);
        assert_eq!(one_block.len(), 2 + 5 + 65_535 + 4);

        let Ok(two_blocks) = stored_zlib(&vec![b'a'; 65_536]) else {
            panic!("two blocks encode");
        };
        assert_eq!(
            &two_blocks[..7],
            &[0x78, 0x01, 0x00, 0xff, 0xff, 0x00, 0x00]
        );
        assert_eq!(
            &two_blocks[2 + 5 + 65_535..2 + 5 + 65_535 + 5],
            &[0x01, 0x01, 0x00, 0xfe, 0xff]
        );
        assert_eq!(two_blocks.len(), 2 + 10 + 65_536 + 4);
    }
}
