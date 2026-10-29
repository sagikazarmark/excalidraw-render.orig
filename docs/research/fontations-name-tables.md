# Fontations name-table handling (#182)

Checked 2026-09-18 against write-fonts 0.53.0 / read-fonts 0.44.0; the
decisions below are unchanged by later releases.

## Reader and policy

`render/font_subset.rs` uses the `write_fonts::read` re-export (read-fonts) for
`Name`, `NameRecord`, record layout and field reading. There is no separate
read-fonts dependency (ADR-0020).

The policy remains explicit: select the **first** platform-3, language-0x0409
record for the requested name ID, irrespective of its encoding ID. Rewriting
emits IDs 0–6, 13 and 14 in that order, omitting absent optional records;
IDs 1, 2, 3, 4 and 6 receive the deterministic generated names, while
0, 5, 13 and 14 retain their exact selected source bytes. Output records
remain platform 3 / encoding 1 / language 0x0409. The profile, semantic subset
identity, aliases and ADR-0001/ADR-0003 policy are unchanged.

Two small raw-byte adapters preserve the old malformed-input behavior:

- `Name::name_record()` returns an empty array if the declared record array
  is truncated. Instead, iterate its typed byte range and read each packed
  `NameRecord` through `FontData::read_ref_at`. A complete earlier match still
  wins; encountering a truncated record before a match returns absence.
- `Name::string_data()` substitutes empty data for an invalid storage offset,
  and `NameString` exposes decoded characters rather than the original bytes.
  Slice the original table using checked addition of the typed storage offset,
  record offset and length. An unreadable first match returns absence rather
  than trying a later duplicate. A valid empty record stays distinct from an
  out-of-bounds empty record. Table version and encoding do not add filters.

The u16 count bounds record traversal; packed record size/layout comes from
Fontations. Its parser checks the six-byte header before its infallible
header accessors are used. All raw string slicing remains fallible and bounded.
Read failure still maps to optional-record absence or the caller's existing
`Sfnt`/`Validate` error. Independent checksum, table-policy, layout-closure,
glyph/metric and naming validation remain active.

## Writer evaluation: retain raw serialization

Evaluated against **write-fonts 0.53.0**:

| Concern | Upstream behavior and decision |
| --- | --- |
| Raw legal bytes | `NameRecord.string` is a string, not bytes. `FromObjRef<NameString> for String` collects decoded characters. An unpaired UTF-16 surrogate becomes U+FFFD; an odd trailing byte disappears. These are concrete incompatibilities with preservation. The derivation regression retains `d8 00`, `dc 00` and `00 41 ff` through sfnt → subset → WOFF2 → sfnt. |
| Encoding IDs | `Encoding::new` supports Windows IDs 0, 1 and 10, not every Windows encoding. Unknown encodings iterate no characters; the writer rejects unknown platform/encoding pairs. Our reader deliberately selects raw records without that restriction. Output is still encoding 1. |
| Record ordering | `Name` validation requires sorted, unique platform/encoding/language/name-ID tuples. Our nine-ID output order satisfies this, but converting the whole source table would not implement first-match selection. |
| String pooling | `NameStringAndLenWriter` uses `TableWriter::write_offset`; that writer deduplicates equal objects. Our raw writer appends every selected string independently in record order. Even valid strings can therefore change storage offsets and sfnt/WOFF2 bytes under typed serialization. |
| Length bounds | `NameStringWriter::compute_length` sums UTF-16 byte lengths in **u16**; storage-header calculation uses `try_into().unwrap()`. Validation of encoding/sortedness is not a replacement for explicit size checks. |
| Offset bounds | Typed offsets use the generic table writer's offset resolution. Our existing writer checks every record byte length, string start offset and storage-header offset with `u16::try_from`, and uses checked total-size arithmetic plus fallible table reservation. It allows the last string to extend beyond 65535 if both its length and start offset fit, as the format permits. |

Keep the existing raw name-table writer and `FontBuilder::add_raw`,
`copy_missing_tables`, and `build`. No lossy source decoding or conditional
writer/fallback path is introduced. New derivation tests exercise individual
UTF-16 length overflow (BMP and supplementary characters) and cumulative
string-offset overflow, all returning `SvgFontSubsetStage::Sfnt`.

Primary sources (also inspected in the pinned registry source):

- [read-fonts 0.44.0 name table](https://docs.rs/read-fonts/0.44.0/src/read_fonts/tables/name.rs.html)
- [read-fonts 0.44.0 generated records](https://docs.rs/read-fonts/0.44.0/src/read_fonts/generated/generated_name.rs.html)
- [write-fonts 0.53.0 name table](https://docs.rs/write-fonts/0.53.0/src/write_fonts/tables/name.rs.html)
- [write-fonts table writer](https://docs.rs/write-fonts/0.53.0/src/write_fonts/write.rs.html)
- [FontBuilder](https://docs.rs/write-fonts/0.53.0/src/write_fonts/font_builder.rs.html)
