//! The one stream filter the PDF backend writes (ADR-0008): every stream the
//! renderer itself emits — page content, fallback forms, the ICC profile, the
//! embedded scene, font programs, `ToUnicode` CMaps, `CIDToGIDMap`s and image
//! planes — is one `/Filter /FlateDecode` zlib stream produced by
//! `miniz_oxide` at the fixed level and strategy ADR-0004 chose for PNG
//! `IDAT`. The XMP metadata stream is the documented exception and stays
//! stored, so a PDF/A processor can read it without a filter. The compressor is
//! pure integer code with no host, time or random input, so the same bytes
//! come out on every supported target.

use miniz_oxide::deflate::core::{compress_to_output, CompressorOxide, TDEFLFlush, TDEFLStatus};
use miniz_oxide::DataFormat;

use super::super::FormatEncodeError;

/// The zlib default, the level ADR-0004 measured for PNG `IDAT`; `svg2pdf`
/// deflates the nested-SVG chunks it returns at the same level through the
/// same crate.
const STREAM_DEFLATE_LEVEL: u8 = 6;

/// One renderer-written stream body, deflated: the RFC 1950 zlib stream a
/// `/Filter /FlateDecode` consumer inflates back to `data`.
pub(super) fn deflate_stream(
    data: &[u8],
    subject: &'static str,
) -> Result<Vec<u8>, FormatEncodeError> {
    let mut compressor = CompressorOxide::default();
    compressor.set_format_and_level(DataFormat::Zlib, STREAM_DEFLATE_LEVEL);
    let mut zlib = Vec::new();
    let mut memory_unavailable = false;
    let (status, consumed) =
        compress_to_output(&mut compressor, data, TDEFLFlush::Finish, |bytes| {
            if zlib.try_reserve(bytes.len()).is_err() {
                memory_unavailable = true;
                return false;
            }
            zlib.extend_from_slice(bytes);
            true
        });
    if memory_unavailable {
        return Err(FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject,
        });
    }
    if status != TDEFLStatus::Done || consumed != data.len() {
        return Err(FormatEncodeError::Encoding(
            "PDF deflate stream invariant failed".to_owned(),
        ));
    }
    Ok(zlib)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deflate_stream_is_a_zlib_stream_that_inflates_to_its_input() {
        let data = b"0 0 m 10 10 l S\n".repeat(64);
        let zlib = deflate_stream(&data, "test").expect("deflate succeeds");

        assert_eq!(zlib[0] & 0x0f, 8, "zlib header names deflate");
        assert!(zlib.len() < data.len(), "repetitive content deflates");
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib(&zlib).expect("stream inflates"),
            data
        );
        assert_eq!(
            zlib,
            miniz_oxide::deflate::compress_to_vec_zlib(&data, STREAM_DEFLATE_LEVEL),
            "the fixed configuration is the crate's level-6 zlib default, which svg2pdf uses too"
        );
    }

    #[test]
    fn deflate_stream_handles_an_empty_body() {
        let zlib = deflate_stream(&[], "test").expect("deflate succeeds");
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib(&zlib).expect("stream inflates"),
            Vec::<u8>::new()
        );
    }
}
