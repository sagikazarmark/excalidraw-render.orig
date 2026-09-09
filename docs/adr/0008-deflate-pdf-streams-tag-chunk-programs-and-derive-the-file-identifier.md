---
status: accepted
---

# Deflate every renderer-written PDF stream, tag embedded chunk programs, and derive the file identifier from the bytes

Spec §8.6 has promised since the PDF backend was qualified that "complete
bytes are stable through … fixed stream filters and Flate settings", and the
encoder applied no filter to anything: the page content stream, every
`FontFile2` program, the ICC profile, the XMP packet, each image's RGB plane
and gray soft mask, the embedded scene and the nested-SVG Form chunks were all
stored (`render/pdf.rs` at `a1be429:700-703, 889-918, 1103-1112`; `compress:
false` at `:3499`). A 1920×1080 raster cost about 8.3 MB per occurrence and
every text-bearing PDF carried its fonts raw: 62 KB for the Excalifont
basic-Latin chunk, 223 KB for Cascadia, 134 KB of Liberation Sans for any
label or frame name, 863 KB for one emoji. Users compare against the browser's
"print to PDF", so the size read as a defect regardless of correctness, and
the spec sentence was false. The review named this finding 3 of
`docs/research/review-2026-09-07/asset-comparability.md` and #133 asked for
the decision: adopt a fixed Flate configuration or correct the spec.

The same review's finding 13 listed the metadata gaps of the font embedding
(#150 item 3): three embedded programs of one CJK scene all named
`/BaseFont /Xiaolai`, no subset tag on any program, no trailer `/ID`, no
`/Info`, and an XMP packet without `pdfaid` beside a `GTS_PDFA1` output
intent. #120 had already decided that the reviewed decodes are the only bytes
PDF may embed, so subsetting the programs was not on the table; naming and
identification were.

## Decision

Flate is adopted. Every stream the renderer itself writes is one
`/Filter /FlateDecode` zlib stream produced by the pinned `miniz_oxide 0.8.9`
at level 6 with the default strategy — the configuration ADR-0004 chose for
PNG `IDAT` and measured against the reference renderer's own export — with
`pdf/streams.rs` owning the level, the compressor call and the stream-object
writer. That is: the page content stream, the fallback Form XObject streams,
the ICC profile, the `scene.excalidraw` embedded file, every `FontFile2`
program (its `/Length1` still names the complete sfnt length), every
`ToUnicode` CMap and `CIDToGIDMap`, and each image's RGB plane and soft mask.
The nested-SVG chunks are requested from `svg2pdf` with `compress: true`,
which deflates their content streams through the same crate at the same
level — its `deflate` helper is `compress_to_vec_zlib(data, 6)`, the flags
`set_format_and_level(Zlib, 6)` produces — and its image streams were Flate
already. The chunk canonicalizer reads each stream body by its direct
`/Length`, so a binary body renumbers and canonicalizes exactly as a stored one
did.

The one stream left stored is the XMP metadata stream. PDF/A requires the
metadata stream to be readable without a filter, the file carries a
`GTS_PDFA1` output intent, and a metadata packet is small; storing it keeps
the door open without claiming anything.

Every embedded program that is an official font chunk — a Unicode-range chunk
of its family, which includes the single Liberation Sans chunk — is named with
the subset tag PDF reserves for embedded subsets: six uppercase letters and
`+` before the family's PDF name, on the Type0 `/BaseFont`, the CIDFontType2
`/BaseFont` and the descriptor `/FontName`. The letters spell the first six
nibbles of the chunk's manifested WOFF2 digest, `A` for 0 through `P` for 15,
so the tag is a pure function of the chunk: `OEECDL+Excalifont` for the
basic-Latin chunk (`e4423b…`), `AAGKCL+LiberationSans` (`006a2b…`), and the
three Xiaolai chunks a CJK scene embeds become `JGPGJG+`, `GPHIPF+` and
`DCCGEF+Xiaolai`. Whether a program is a chunk is the asset catalog's call:
`embedded_font_name` looks the program's identity up and tags only a catalog
record whose identity matches. The complete Noto Emoji face is not a subset of
anything and stays `/BaseFont /NotoEmoji`. The `ToUnicode` `/CMapName` stays
the untagged family name; it is informative, and the tag exists to keep
distinct programs distinct, which the `/BaseFont` does.

The trailer carries `/ID [<hex> <hex>]`, both entries equal: the first 16
bytes of the SHA-256 over every byte written before the `trailer` keyword —
the header, the objects and the cross-reference table — as 32 uppercase hex
digits. A first revision's two identifiers are the same by definition, and
deriving them from the bytes rather than from a clock or a random source
keeps them identical across runs and targets. No `/Info` dictionary is added
(every value it could carry is in the XMP manifest or would be a timestamp),
and the XMP gains no `pdfaid` schema: the output intent is stated, but no
PDF/A conformance is claimed without a validator having said so.

## Why this configuration and not another

Spec §10 orders remedies as adapter, pinned controlled fork, focused owned
port, and `miniz_oxide` is the first: already a direct dependency for PNG,
`forbid(unsafe_code)`, no build script, integer-only with no host, time or
random input, and with a green two-target signal in this repository through
the PNG pins and the svg2pdf image streams the PDF pins already carried. Level
6 is the level ADR-0004 measured (level 9 cost two to three times the time for
under two percent) and the level svg2pdf uses, so one configuration covers
every stream in the file whoever wrote it. A PNG predictor (`/DecodeParms`
with `/Predictor 15`) was not adopted for the image planes: it would put a
second owned filter stage in front of the compressor for a gain that on
Excalidraw content — screenshots and flat diagrams — ADR-0004 measured as a
loss, and it would give the image streams a different filter chain from every
other stream.

The streams are deflated before the output buffer is sized, so the exact
artifact size is still known before the one fallible output allocation, as
the PNG encoder knows its `IDAT`. Measurement and emission stay two `encode`
passes compared byte for byte (spec §13.2), and no ledger charge or charge
order moves (ADR-0002): the image plane rows still price the raw planes, the
1 MiB PDF stream workspace row covers the compressor state, and the deflated
streams are transients of that row's lifetime. The output-bytes charge admits
the smaller artifact, which is the one observable accounting change and is
the same kind of change ADR-0004 recorded for PNG.

## Measured

Sizes of the viewer-gated and registered-case PDFs before and after, from
the CLI at `eb42b71` and after this round:

| artifact | before | after | |
| --- | ---: | ---: | ---: |
| `rectangle-outline` | 2,101 B | 1,873 B | 89% |
| `excalifont-text` | 71,183 B | 39,226 B | 55% |
| `helvetica-text` | 145,613 B | 96,432 B | 66% |
| `cjk-propagation` (three Xiaolai chunks) | 335,400 B | 217,393 B | 65% |
| `emoji-fallback` (complete Noto Emoji) | 1,079,709 B | 679,626 B | 63% |
| `legacy-line-height` (four programs) | 489,590 B | 258,924 B | 53% |
| `image-codecs` (raster images) | 30,298 B | 9,352 B | 31% |
| `frame-clip-mask-sharing` | 283,283 B | 106,198 B | 37% |
| `project-operations` (backend qualification) | 364,464 B | 231,245 B | 63% |
| `nested-native-operations` | 24,015 B | 17,839 B | 74% |
| `isolated-fallback` | 2,651 B | 2,639 B | 100% |

Font programs deflate to 55–65% of their size (hinted TrueType outlines
compress less well than text); a raw RGB plane of a screenshot to 20–30%; a
tiny PDF gains a few bytes of filter dictionary and a 78-byte identifier,
which is why `isolated-fallback` and the two smallest frame exports are
unchanged or a few bytes larger. Nine `tests/public-scenes/` boards
measured together go from 3,199,688 to 1,497,153 bytes (47%); the largest,
`elasticsearch-spec-overview`, from 635 KB to 273 KB. Deflating adds 5–35 ms
per artifact in the unoptimized dev profile on the shared 4-core host, where
the same renders took 10–70 ms before; the fonts dominate, compressing once
per embedded program per encode pass.

What did not move: the PDFium and PDF.js rasters of every viewer-gated PDF —
`rectangle-outline`, `excalifont-text`, the isolated fallback, all seven
`frame-export` targets, both backend qualification matrices and the ten
registered text- or image-bearing cases, 44 rasters in all — are
byte-identical before and after, and `qpdf --check` is clean on every file.
The structural inspector's `operation_sha256`, which hashes every operator
with its operands after qpdf decodes the streams, is unchanged in all five
structure expectations; only `inventory_sha256` moved, and a field-by-field
diff of the reports shows exactly the two things this decision names —
`document.file_ids`, from absent to the identifier, and each chunk program's
`base_font`, from `/Excalifont` to `/OEECDL+Excalifont` and so on. The font,
image, XMP and attachment digests the inventory carries are of the decoded
bytes, so Flate alone would not have moved it, and every readable field check
(`font_count`, `image_count`, `mapping_count`, the operator minimums, the
image inventory) holds.

## What moves and what does not

Every PDF artifact byte pin moves: the six digests in `tests/fast/` (arrow,
outline shape, Excalifont text and the three backend qualification
matrices), the `expected/rectangle.pdf` and the PDF risk-set, fallback-scene
and fallback-family digests in `tests/package-consumer`, and the
`inventory_sha256` of the five structure expectations (`rectangle-outline`,
`excalifont-text`, and the three under `pdf-backend-qualification/`) with the
sha256 pins that bind them in `fast/pdf_backend_qualification.rs`,
`harness_pdf_viewers/excalifont_text.rs` and the rectangle case's
`pdf-renderer-expectations.json` (#44). The tolerance exceptions do not move
— no raster did. The PDF viewer qualification manifest (#35) moves in the
same round only for the #141 ride-along (its implementation list, the
schema's `minItems` that list must satisfy, and `xtask/src/pdf_viewers.rs`),
not for anything this decision produced: its fixtures are project-authored
controls, not renderer output.

Tests read the artifact through `tests/common/pdf_inspect.rs`, which inflates
every Flate stream in place before a test looks for an operator, a font
program or a CMap line, so what is pinned is still the artifact rather than a
viewer's reading of it; the rectangle PDF test pins the filter count, the
absence of `/Info` and the `/ID` derivation.

## Consequences

- Spec §8.6 names the stream rule, the exception, the subset tag and the
  identifier derivation; §12.2 says the embedded file's stream is Flate like
  every other while its decoded content stays the raw canonical scene.
- A `miniz_oxide` bump is a byte-affecting dependency change for PDF as it
  already is for PNG: pinned exactly, measured against the whole-corpus
  digest table, re-pinned in one round if anything moves.
- Subsetting the embedded programs stays foreclosed by #120 and ADR-0001; the
  tag says only that the program is a chunk of its family, which is what it
  is. Should a later revision embed derived subsets, the tag's derivation
  would need to bind the subset identity rather than the chunk digest.
- A PDF/A claim remains a separate decision that needs a validator; nothing
  here moves the file further from or closer to it except keeping the
  metadata stream stored.
