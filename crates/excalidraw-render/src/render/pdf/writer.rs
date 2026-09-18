//! Checked, handwritten PDF serialization. The plan owns IDs and dictionary
//! contents; this module owns bytes, definitions, framing and finalization.
//! Prepared chunks have already been adapted by `nested_svg`: only their
//! recorded boundaries are checked here, never their opaque stream payloads.

use std::fmt;

use sha2::{Digest as _, Sha256};

use super::super::FormatEncodeError;
use super::pdf_size_error;

pub(super) struct Writer {
    output: Vec<u8>,
    offsets: Vec<usize>,
    failed: bool,
}

/// A scoped byte sink for one object's contents, with no access to the output
/// vector or definition table. Its inherent `write_fmt` preserves typed errors.
pub(super) struct Object<'a> {
    output: &'a mut Vec<u8>,
}

#[derive(Clone, Copy)]
pub(super) enum StreamEncoding {
    Stored,
    Flate,
}

impl Writer {
    pub(super) fn new(capacity: usize, object_count: usize) -> Result<Self, FormatEncodeError> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| output_memory())?;
        Object {
            output: &mut output,
        }
        .bytes(b"%PDF-1.7\n%\x80\x80\x80\x80\n\n")?;
        let mut offsets = Vec::new();
        offsets.try_reserve_exact(object_count).map_err(|_| {
            FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_object_offsets",
            }
        })?;
        offsets.resize(object_count, 0);
        Ok(Self {
            output,
            offsets,
            failed: false,
        })
    }

    fn ready(&self) -> Result<(), FormatEncodeError> {
        if self.failed {
            return Err(invariant("PDF writer has an incomplete object or chunk"));
        }
        Ok(())
    }

    fn define(&mut self, id: usize, offset: usize) -> Result<(), FormatEncodeError> {
        let slot = id
            .checked_sub(1)
            .and_then(|index| self.offsets.get_mut(index))
            .ok_or_else(|| invariant("PDF object ID is outside the planned layout"))?;
        if *slot != 0 {
            return Err(invariant("PDF object ID is defined more than once"));
        }
        check_offset(offset)?;
        *slot = offset;
        Ok(())
    }

    pub(super) fn object(
        &mut self,
        id: usize,
        contents: impl FnOnce(&mut Object<'_>) -> Result<(), FormatEncodeError>,
    ) -> Result<(), FormatEncodeError> {
        self.ready()?;
        self.failed = true;
        self.define(id, self.output.len())?;
        let mut object = Object {
            output: &mut self.output,
        };
        writeln!(object, "{id} 0 obj")?;
        contents(&mut object)?;
        object.bytes(b"endobj\n")?;
        self.failed = false;
        Ok(())
    }

    /// Leading entries precede Filter/Length; trailing entries follow Length
    /// (the font program's Length1 uses that established dictionary order).
    pub(super) fn stream(
        &mut self,
        id: usize,
        encoding: StreamEncoding,
        data: &[u8],
        entries: impl FnOnce(&mut Object<'_>) -> Result<(), FormatEncodeError>,
        trailing_entries: fmt::Arguments<'_>,
    ) -> Result<(), FormatEncodeError> {
        self.object(id, |object| {
            object.bytes(b"<<\n")?;
            entries(object)?;
            if matches!(encoding, StreamEncoding::Flate) {
                object.bytes(b"/Filter /FlateDecode\n")?;
            }
            write!(
                object,
                "/Length {}\n{trailing_entries}>>\nstream\n",
                data.len()
            )?;
            object.bytes(data)?;
            object.bytes(b"\nendstream\n")
        })
    }

    pub(super) fn flate_stream(
        &mut self,
        id: usize,
        entries: &str,
        data: &[u8],
    ) -> Result<(), FormatEncodeError> {
        self.stream(
            id,
            StreamEncoding::Flate,
            data,
            |object| object.bytes(entries.as_bytes()),
            format_args!(""),
        )
    }

    /// Records are in physical emission order, with chunk-relative offsets.
    /// The canonicalizer owns syntax/reference adaptation. This checks positive,
    /// unique planned IDs, ordered in-bounds offsets, matching headers, object
    /// endings and root membership without searching binary bodies for tokens.
    /// An error poisons the writer, so partial imports can never be finalized.
    pub(super) fn import_chunk(
        &mut self,
        bytes: &[u8],
        records: &[(usize, usize)],
        root: usize,
    ) -> Result<(), FormatEncodeError> {
        self.ready()?;
        self.failed = true;
        if !records.iter().any(|&(id, _)| id == root) {
            return Err(invariant("PDF chunk root is not defined by the chunk"));
        }
        let first = records
            .first()
            .map(|&(_, offset)| offset)
            .ok_or_else(pdf_size_error)?;
        if !bytes
            .get(..first)
            .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_whitespace))
        {
            return Err(invariant("PDF chunk has an invalid initial object offset"));
        }
        for (index, &(id, position)) in records.iter().enumerate() {
            let end = records
                .get(index + 1)
                .map_or(bytes.len(), |&(_, offset)| offset);
            let object = bytes
                .get(position..end)
                .filter(|object| !object.is_empty())
                .ok_or_else(|| {
                    invariant("PDF chunk object offsets are not ordered and in bounds")
                })?;
            let digits = object
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            let parsed_id = object[..digits].iter().try_fold(0_usize, |value, byte| {
                value.checked_mul(10)?.checked_add(usize::from(byte - b'0'))
            });
            if digits == 0 || parsed_id != Some(id) || !object[digits..].starts_with(b" 0 obj\n") {
                return Err(invariant(
                    "PDF chunk object header does not match its offset record",
                ));
            }
            if !object.trim_ascii_end().ends_with(b"\nendobj") {
                return Err(invariant("PDF chunk object has an invalid ending"));
            }
            let offset = self
                .output
                .len()
                .checked_add(position)
                .ok_or_else(pdf_size_error)?;
            self.define(id, offset)?;
        }
        Object {
            output: &mut self.output,
        }
        .bytes(bytes)?;
        self.failed = false;
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<Vec<u8>, FormatEncodeError> {
        self.ready()?;
        if self.offsets.is_empty() || self.offsets.contains(&0) {
            return Err(invariant("PDF planned object ID is missing a definition"));
        }
        let size = self
            .offsets
            .len()
            .checked_add(1)
            .ok_or_else(pdf_size_error)?;
        let xref_offset = self.output.len();
        check_offset(xref_offset)?;
        let mut output = Object {
            output: &mut self.output,
        };
        write!(output, "xref\n0 {size}\n0000000000 65535 f\r\n")?;
        for offset in &self.offsets {
            writeln!(output, "{offset:010} 00000 n\r")?;
        }
        // ADR-0008: hash the header, objects AND xref; emit SHA-256/16 twice
        // as uppercase hex. Stack storage avoids a separate ID allocation.
        let digest = Sha256::digest(&self.output);
        let mut identifier = [0_u8; 32];
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for (index, byte) in digest[..16].iter().copied().enumerate() {
            identifier[2 * index] = HEX[usize::from(byte >> 4)];
            identifier[2 * index + 1] = HEX[usize::from(byte & 15)];
        }
        let mut output = Object {
            output: &mut self.output,
        };
        write!(output, "trailer\n<<\n/Size {size}\n/Root 1 0 R\n/ID [<")?;
        output.bytes(&identifier)?;
        output.bytes(b"> <")?;
        output.bytes(&identifier)?;
        write!(output, ">]\n>>\nstartxref\n{xref_offset}\n%%EOF")?;
        Ok(self.output)
    }
}

impl Object<'_> {
    pub(super) fn bytes(&mut self, bytes: &[u8]) -> Result<(), FormatEncodeError> {
        self.output
            .len()
            .checked_add(bytes.len())
            .ok_or_else(pdf_size_error)?;
        self.output
            .try_reserve(bytes.len())
            .map_err(|_| output_memory())?;
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    pub(super) fn byte(&mut self, byte: u8) -> Result<(), FormatEncodeError> {
        self.bytes(&[byte])
    }

    pub(super) fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), FormatEncodeError> {
        struct Adapter<'a, 'b> {
            object: &'a mut Object<'b>,
            error: Option<FormatEncodeError>,
        }
        impl fmt::Write for Adapter<'_, '_> {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                match self.object.bytes(value.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        self.error = Some(error);
                        Err(fmt::Error)
                    }
                }
            }
        }
        let mut adapter = Adapter {
            object: self,
            error: None,
        };
        fmt::write(&mut adapter, args).map_err(|_| {
            adapter
                .error
                .unwrap_or_else(|| invariant("failed to encode PDF bytes"))
        })
    }
}

fn check_offset(offset: usize) -> Result<(), FormatEncodeError> {
    if offset as u128 > 9_999_999_999 {
        return Err(invariant("PDF offset exceeds classic xref capacity"));
    }
    Ok(())
}

fn output_memory() -> FormatEncodeError {
    FormatEncodeError::MemoryUnavailable {
        subject_kind: "artifact",
        subject: "pdf_output",
    }
}

fn invariant(message: &str) -> FormatEncodeError {
    FormatEncodeError::Encoding(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_encoding<T>(result: Result<T, FormatEncodeError>) {
        assert!(matches!(result, Err(FormatEncodeError::Encoding(_))));
    }

    #[test]
    fn definitions_reject_zero_out_of_range_and_duplicate_ids() {
        for id in [0, 3, usize::MAX] {
            let mut writer = Writer::new(0, 2).unwrap();
            assert_encoding(writer.object(id, |_| Ok(())));
            assert_encoding(writer.finish());
        }
        let mut writer = Writer::new(0, 2).unwrap();
        writer.object(1, |object| object.bytes(b"<<>>\n")).unwrap();
        assert_encoding(writer.object(1, |_| Ok(())));
        assert_encoding(writer.finish());
    }

    #[test]
    fn finalization_requires_every_planned_definition_including_root() {
        assert_encoding(Writer::new(0, 0).unwrap().finish());
        for defined in [1, 2] {
            let mut writer = Writer::new(0, 2).unwrap();
            writer
                .object(defined, |object| object.bytes(b"<<>>\n"))
                .unwrap();
            assert_encoding(writer.finish());
        }
    }

    #[test]
    fn incomplete_stream_preserves_the_error_and_cannot_be_finalized() {
        let mut writer = Writer::new(0, 1).unwrap();
        let result = writer.stream(
            1,
            StreamEncoding::Stored,
            b"payload",
            |object| {
                object.bytes(b"/Type /Metadata\n")?;
                Err(FormatEncodeError::MemoryUnavailable {
                    subject_kind: "metadata",
                    subject: "test_stream_entries",
                })
            },
            format_args!(""),
        );
        assert!(matches!(
            result,
            Err(FormatEncodeError::MemoryUnavailable {
                subject_kind: "metadata",
                subject: "test_stream_entries"
            })
        ));
        assert_encoding(writer.object(1, |_| Ok(())));
        assert_encoding(writer.finish());
    }

    #[test]
    fn initial_reservation_failures_keep_their_original_attribution() {
        assert!(matches!(
            Writer::new(usize::MAX, 1),
            Err(FormatEncodeError::MemoryUnavailable {
                subject_kind: "artifact",
                subject: "pdf_output"
            })
        ));
        assert!(matches!(
            Writer::new(32, usize::MAX),
            Err(FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_object_offsets"
            })
        ));
    }

    #[test]
    fn chunk_offsets_are_relocated_in_physical_order_and_binary_stays_opaque() {
        // A fake object header and endobj inside a binary body are not records.
        let payload = b"\x00\xff\n99 0 obj\nendobj\ntrailer\n";
        let first = [
            format!("3 0 obj\n<< /Length {} >>\nstream\n", payload.len()).as_bytes(),
            payload,
            b"\nendstream\nendobj\n\n",
        ]
        .concat();
        let chunk = [first.as_slice(), b"2 0 obj\n<<>>\nendobj\n\n"].concat();
        let mut writer = Writer::new(0, 3).unwrap();
        writer
            .object(1, |object| object.bytes(b"<< /Type /Catalog >>\n"))
            .unwrap();
        writer
            .import_chunk(&chunk, &[(3, 0), (2, first.len())], 3)
            .unwrap();
        let pdf = writer.finish().unwrap();
        let offsets = inspect_xref_and_id(&pdf, 3);
        assert_eq!(&pdf[offsets[2]..offsets[2] + chunk.len()], chunk);
        assert_eq!(offsets[1], offsets[2] + first.len());
        assert!(offsets[0] < offsets[2]);
    }

    #[test]
    fn chunk_import_rejects_invalid_records_and_root() {
        let first = b"2 0 obj\n<<>>\nendobj\n";
        let chunk = [first.as_slice(), b"3 0 obj\n<<>>\nendobj\n"].concat();
        for (records, root) in [
            (vec![], 2),
            (vec![(2, 0), (3, first.len())], 1), // root outside chunk
            (vec![(0, 0)], 0),
            (vec![(4, 0)], 4),
            (vec![(3, 0)], 3), // header mismatch
            (vec![(2, 1)], 2), // invalid prefix
            (vec![(2, 0), (3, first.len() + 1)], 2),
            (vec![(2, 0), (3, chunk.len() + 1)], 2),
            (vec![(2, 0), (3, 0)], 2),
            (vec![(2, first.len()), (3, 0)], 2),
            (vec![(2, usize::MAX)], 2),
        ] {
            let mut writer = Writer::new(0, 3).unwrap();
            assert_encoding(writer.import_chunk(&chunk, &records, root));
            assert_encoding(writer.finish());
        }
        let mut writer = Writer::new(0, 2).unwrap();
        assert_encoding(writer.import_chunk(b"2 0 obj\n<<>>\n", &[(2, 0)], 2));
    }

    #[test]
    fn chunk_definitions_share_the_writer_registry() {
        let object = b"2 0 obj\n<<>>\nendobj\n";
        let duplicate = [object.as_slice(), object.as_slice()].concat();
        let mut writer = Writer::new(0, 2).unwrap();
        assert_encoding(writer.import_chunk(&duplicate, &[(2, 0), (2, object.len())], 2));

        let mut writer = Writer::new(0, 2).unwrap();
        writer.object(2, |object| object.bytes(b"<<>>\n")).unwrap();
        assert_encoding(writer.import_chunk(object, &[(2, 0)], 2));

        let mut writer = Writer::new(0, 2).unwrap();
        writer.import_chunk(object, &[(2, 0)], 2).unwrap();
        assert_encoding(writer.object(2, |_| Ok(())));

        let mut writer = Writer::new(0, 1).unwrap();
        assert_encoding(writer.import_chunk(object, &[(2, 0)], 2));
    }

    #[test]
    fn stream_lengths_dictionary_order_xref_and_through_xref_identifier() {
        let stored = b"\x00\xfftrailer\nendstream\nendobj\n";
        let compressed = super::super::streams::deflate_stream(stored, "test").unwrap();
        let mut writer = Writer::new(0, 3).unwrap(); // Exercise growth beyond the estimate.
        writer
            .stream(
                3,
                StreamEncoding::Flate,
                &compressed,
                |_| Ok(()),
                format_args!("/Length1 {}\n", stored.len()),
            )
            .unwrap();
        writer
            .object(1, |object| object.bytes(b"<< /Type /Catalog >>\n"))
            .unwrap();
        writer
            .stream(
                2,
                StreamEncoding::Stored,
                stored,
                |object| object.bytes(b"/Type /Metadata\n/Subtype /XML\n"),
                format_args!(""),
            )
            .unwrap();
        let pdf = writer.finish().unwrap();
        let offsets = inspect_xref_and_id(&pdf, 3);
        let expected = [
            format!(
                "3 0 obj\n<<\n/Filter /FlateDecode\n/Length {}\n/Length1 {}\n>>\nstream\n",
                compressed.len(),
                stored.len()
            )
            .as_bytes(),
            &compressed,
            b"\nendstream\nendobj\n",
        ]
        .concat();
        assert_eq!(&pdf[offsets[2]..offsets[0]], expected);
        let expected = [
            format!(
                "2 0 obj\n<<\n/Type /Metadata\n/Subtype /XML\n/Length {}\n>>\nstream\n",
                stored.len()
            )
            .as_bytes(),
            stored,
            b"\nendstream\nendobj\n",
        ]
        .concat();
        assert_eq!(&pdf[offsets[1]..offsets[1] + expected.len()], expected);
    }

    #[test]
    fn classic_xref_offsets_have_a_checked_ten_digit_bound() {
        assert!(check_offset(usize::try_from(9_999_999_999_u64).unwrap_or(usize::MAX)).is_ok());
        if let Ok(offset) = usize::try_from(10_000_000_000_u64) {
            assert_encoding(check_offset(offset));
        }
    }

    /// Read from EOF, not the first trailer-like bytes in an opaque stream.
    fn inspect_xref_and_id(pdf: &[u8], count: usize) -> Vec<usize> {
        assert!(pdf.starts_with(b"%PDF-1.7\n%\x80\x80\x80\x80\n\n"));
        let start = pdf
            .windows(b"startxref\n".len())
            .rposition(|bytes| bytes == b"startxref\n")
            .unwrap();
        let suffix = std::str::from_utf8(&pdf[start..]).unwrap();
        let xref: usize = suffix.lines().nth(1).unwrap().parse().unwrap();
        assert!(suffix.ends_with("\n%%EOF"));
        let header = format!("xref\n0 {}\n", count + 1);
        assert!(pdf[xref..].starts_with(header.as_bytes()));
        let rows = xref + header.len();
        assert_eq!(&pdf[rows..rows + 20], b"0000000000 65535 f\r\n");
        let mut offsets = Vec::new();
        for id in 1..=count {
            let row = &pdf[rows + id * 20..rows + (id + 1) * 20];
            assert_eq!(&row[10..], b" 00000 n\r\n");
            let offset: usize = std::str::from_utf8(&row[..10]).unwrap().parse().unwrap();
            assert!(pdf[offset..].starts_with(format!("{id} 0 obj\n").as_bytes()));
            offsets.push(offset);
        }
        let trailer = rows + (count + 1) * 20;
        let id = Sha256::digest(&pdf[..trailer])[..16]
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        assert_eq!(&pdf[trailer..], format!(
            "trailer\n<<\n/Size {}\n/Root 1 0 R\n/ID [<{id}> <{id}>]\n>>\nstartxref\n{xref}\n%%EOF", count + 1
        ).as_bytes());
        offsets
    }
}
