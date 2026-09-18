# Evaluating pdf-writer 0.12.0 for the direct PDF backend

Research, integration design and bounded proof of concept for [#186][issue186],
2026-09-18. Audited production source snapshot:
`931ee812afb1a547cd2d307ea4ac34034ecc4be7`. The independent experiment and its
measured results are in [§9](#9-experimental-results).

## 1. Recommendation

**Retain the production outer writer for now; pursue bounded adoption of
pdf-writer's object serialization after upstream capability and qualification
gates.** Its existing APIs can represent nearly all the renderer's dictionaries,
arrays, references, and streams. Exact scene `f64` serialization is possible
through the public `Primitive` interface without changing upstream. However,
0.12.0 does not provide fallible construction, the required through-xref file-ID
finalization hook, or structured editing of an already-produced chunk. These are
substantial integration boundaries, not missing PDF drawing primitives.
([Writer source][pw-object], [chunk source][pw-chunk], [finalization][pw-lib].)

The promising end state is:

```text
resolved scene → existing PdfPlan / resource and closure plans
               → checked semantic IDs + exact-number adapter
               → qualified typed object/content serialization
               → owned Flate policy + through-xref ID finalization

sanitized nested SVG → existing occurrence-specific closure plan
                     → admitted raster replacements + native SVG conversion
                     → structured, ordered chunk integration
```

Retain scene lowering, typography, colors, links, image normalization, metadata,
and the semantic ledger. Do not replace the direct scene projection with
whole-scene SVG conversion. [Spec §8.6][spec-pdf] explicitly requires direct
semantic PDF, full-precision geometry, selectable ordinary text, and complete
reviewed font programs. The issue records why the previously inspected Krilla
route conflicts with these constraints; this research does not independently
qualify Krilla or another backend. [#186][issue186]

## 2. Evidence and scope

The audit read [CONTEXT.md][context], [ADR-0008][adr8], [ADR-0009][adr9], relevant
renderer-spec sections (§8.2, §8.6, §13.2–13.3), and the bodies/comments of
[#186][issue186], [#154][issue154], and [#178][issue178]. Dependency versions are
exactly pinned in [Cargo.toml][cargo]: `pdf-writer = 0.12.0`, `svg2pdf = 0.13.0`,
`usvg = 0.45.1`, `miniz_oxide = 0.8.9`.

Current renderer code inspected:

| Local source | Responsibility observed |
| --- | --- |
| [`render/pdf.rs`][local-pdf] | Plan, resource census, object layout, output capacity, dictionaries, links, ICC/XMP, image planes, xref and trailer |
| [`pdf/content.rs`][local-content] | Paint-order projection, full-precision transforms/clips, text positioning and ActualText, fallback Form content |
| [`pdf/fonts.rs`][local-fonts] | Complete font embedding, six-object layout per face, first-use CID assignment, ToUnicode and CIDToGIDMap |
| [`pdf/streams.rs`][local-streams] | Fallible level-6 zlib compression and handwritten stream framing |
| [`pdf/nested_svg.rs`][local-nested] | Closure collection, svg2pdf conversion, renumbering, byte-level canonicalization, wrapper compaction and chunk offsets |
| [`pdf/raster.rs`][local-raster] | Occurrence expansion, single-plan raster replacement, input/output surfaces and SVG reparse |
| [`pdf/raster_svg.rs`][local-raster-svg] | usvg serialization repair, round-trip normalized numbers, anonymous feImage inputs, nested-image bindings |
| [`pdf/raster_preflight.rs`][local-preflight] | Fallback policy, surface admission, ledger transactions and occurrence diagnostics |
| [`render/number_format.rs`][local-numbers], [`render/encode.rs`][local-encode] | Number policy and measurement/emission integration |

Upstream claims below were checked against actual cached crate sources under
`/home/laborant/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`, not inferred
from current/latest documentation. The version-pinned official source links in
the references identify those same release files. Particularly important local
inspection entry points are [pdf-writer's finalizer][pw-lib],
[svg2pdf's resource writer][svg-resources], and [usvg's writer][usvg-writer].
No proposed method in §7 exists in the audited releases.

## 3. Responsibility-by-responsibility mapping

**API** means present in 0.12.0; **adapter** means a small renderer-owned policy
layer over a present API; **gap** means a capability requiring upstream work.
An API mapping establishes expressibility, not production allocation safety.
Every proposed use remains subject to §4's fallibility gate.

| Responsibility / current owner | Confirmed API, adapter, or specific gap | Custom code that could disappear; code that remains |
| --- | --- | --- |
| Indirect object framing (`begin_object`, `endobj` strings) | **API:** `Chunk::indirect`, `Obj::dict`, typed writers. They record offsets and delimit objects. [pw-chunk][pw-chunk] | Object framing and most explicit offset writes disappear once the entire object sink is qualified. Semantic object identity remains owned. |
| PDF names, byte/text strings, arrays, dictionary syntax | **API:** `Name`, `Str`, `TextStr`, `Dict::pair/insert`, `Array::item`. Typed writers expose lower-level dictionaries. **Adapter:** fixed entry order, one write per key, chosen string spelling. [pw-object][pw-object] | Manual escaping and separator/bracket assembly can disappear. URI resolution/safety, manifest XML escaping and number policy remain. `Dict` appends; it neither sorts nor replaces duplicate keys. |
| Catalog, Pages, Page, resources | **API:** `Pdf::catalog`, `Chunk::pages/page`, page resources and reference arrays. **Adapter:** `MediaBox`/`CropBox` via raw arrays of exact numbers, bypassing `Rect`'s f32. [pw-structure][pw-structure] | Catalog/page/resource dictionary assembly disappears; one-page policy, bounds, semantic resource names and ordering remain. Calling `Pdf::catalog` registers `/Root`; writing a generic catalog object alone does not. |
| ICC profile and output intent | **API:** `icc_profile`, `ColorSpace::icc_based`, `indirect(id).start::<writers::OutputIntent>()` or a generic dictionary. [pw-color][pw-color] | Stream and OutputIntent syntax disappears. The project's generated ICC bytes, `/CS1`, intended color semantics and no-PDF/A-claim policy remain. |
| Opacity and fallback Forms | **API:** `ext_graphics`, `form_xobject`, transparency group/resources. **Adapter:** exact `CA`, `ca`, BBox and matrices via generic entries. [pw-content][pw-content], [pw-xobject][pw-xobject] | ExtGState/Form dictionary framing disappears. Isolation, compositing scope, form selection and fallback content remain. |
| Content operators and full f64 geometry | **API + adapter:** `Content::op`, `Operation::operand/obj`, custom validated `Primitive`; see §4.1. [pw-content][pw-content] | Operand separators and operator text assembly can disappear. All geometric reductions, quadratic-to-cubic lowering, transformations and paint order remain in `content.rs`. No conversion of scene values through f32 helpers. |
| URI annotations | **API:** `annotation`, link subtype/action/URI; **adapter:** exact `/Rect` and `/QuadPoints` arrays. [pw-annotations][pw-annotations], [pw-actions][pw-actions] | Annotation dictionary and PDF URI escaping disappear. Clipped polygon generation, triangulation, relative-origin resolution, safe-action restriction and topmost-first order remain. |
| RGB images and soft masks | **API:** `image_xobject`, dimensions, color space, bits/component, `s_mask`, filter. [pw-xobject][pw-xobject] | Two-image-object syntax disappears. Checked dimensions, identity sharing, canonical straight-alpha planes, invisible RGB and dark-mode pixel processing remain. |
| XMP, scene attachment, names/AF | **API:** `metadata`, `embedded_file`, `file_spec`, catalog name tree; generic `/AF` array and `AssociationKind::Data`. Use `embedded_file_with_unicode` once for both EF references. [pw-files][pw-files], [pw-structure][pw-structure] | Metadata stream/Filespec/name-tree syntax disappears. Manifest generation, notices, canonical scene bytes, metadata failure stages and atomic omission remain. Pass unescaped MIME bytes to `Name`; its implementation escapes `/` and `#` itself. |
| Font object dictionaries | **API:** `type0_font`, `cid_font`, `font_descriptor`, generic font-program stream; raw entries cover exact integer metrics and widths. [pw-font][pw-font] | Most of `write_font_objects`' dictionary text disappears. Reviewed-program selection, CID allocation, widths, naming, maps and legal policy remain (§5). |
| ToUnicode body | **API:** `UnicodeCmap<u16>::pair_with_multiple` and `.finish`; **adapter/qualification:** map semantic CIDs, decode UTF-16 sequences strictly, explicitly preserve empty-map replacement. [pw-font][pw-font] | `to_unicode_cmap` is a *later* removal candidate, not a mechanical first slice: upstream emits different prologue, `/CMapType 0` versus current `2`, WMode and trailing comments. Keep the current body initially and use `stream` for its framing. |
| CIDToGIDMap and complete FontFile2 payload | **API:** `stream`, `/Length1` generic entry, `FlateDecode`. [pw-chunk][pw-chunk] | Stream delimiters and `/Length` assembly disappear. Big-endian map construction and complete sfnt payload stay; pdf-writer does not derive or subset fonts. |
| Stream lengths/filter framing | **API + adapter:** `stream(id, compressed_bytes).filter(Filter::FlateDecode)` computes direct `/Length`. Precheck `len <= i32::MAX` to avoid its panic. [pw-object][pw-object] | `write_flate_stream_object`'s framing can disappear. `deflate_stream` remains: pdf-writer performs no compression. XMP remains stored. |
| Object ID allocation and reference validity | **Adapter:** checked semantic ID registry; `Ref`, `refs`, `renumber` provide representation, not validity. **Gap:** fallible checked import/finalization for library-generated graphs. [pw-chunk][pw-chunk], [pw-lib][pw-lib] | Repeated `first_*`/stride arithmetic may become named handles, but pdf-writer does not eliminate the registry or dependency checks. |
| Object and dictionary order | **API:** emission follows call order; `refs` follows physical object order; xref offsets are sorted by ID at finish. **Adapter/gap:** canonical input order, structured nested-resource ordering. [pw-object][pw-object], [svg-resources][svg-resources] | Owned outer dictionary sorting is unnecessary if calls are ordered. Nested textual sorting cannot disappear merely by calling `Pdf::extend`. |
| Header, offsets, classic xref, trailer | **API:** `Pdf` emits PDF 1.7 and classic xref/trailer. **Gap:** fallible finalization plus post-xref/pre-trailer hook and forced hex ID. [pw-lib][pw-lib] | Outer header/xref/offset/trailer code can disappear only after these gaps close; §4.3 explains current-version probe workarounds. |
| Nested-SVG object renumber/import | **API:** `Chunk::renumber/renumber_into/extend`. **Gap:** structured object/stream access and validated import. [pw-chunk][pw-chunk], [pw-renumber][pw-renumber] | Simple immutable imports can avoid owned append/offset logic, but current canonicalized/adapted bytes cannot be placed back into a Chunk through a public mutation API. |
| Adapted nested stream compaction/compression | **Gap:** pre-serialization Form/pattern content callback or structured chunk transform. [local adapter][local-nested], [svg-context][svg-context] | Boundary scans, `/Length` parsing/patching and byte-line wrapper matching become removable after an ordered, structured replacement is qualified (§6). |
| Raster-closure plan, provenance and ledger | **Retain project responsibility.** pdf-writer has no scene/effect semantics. [local-preflight][local-preflight], [adr9][adr9] | No closure selection, grids, resource transactions or diagnostics should disappear as a consequence of changing the PDF serializer. |

## 4. Hard integration boundaries

### 4.1 Exact f64 through Primitive: available now

The built-in real primitive is `f32`; `Rect`, transforms, text matrices, font
sizes and geometry helpers also use f32. The public, unsealed trait is simply
`fn write(self, buf: &mut Vec<u8>)`. `Dict::pair`, `Array::item`, and
`Operation::operand` accept any `Primitive`. Thus a small prevalidated token
adapter can preserve the current formatter's bytes. [Primitive API][primitive-api],
[object source][pw-object], [content source][pw-content]

Illustrative adapter shape, **not a fallible production sink**:

```rust
// Private constructor: only tokens produced/validated by format_pdf_number.
#[derive(Clone, Copy)]
struct ExactNumber<'a>(&'a str);

impl pdf_writer::Primitive for ExactNumber<'_> {
    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.0.as_bytes());
    }
}

// Given a token from format_pdf_number(value, subject):
// content.op("m").operand(ExactNumber(&x)).operand(ExactNumber(&y));
// page.insert(Name(b"MediaBox")).array().items(exact_bounds);
```

Validate before opening an object/operator: finite input, one syntactically valid
PDF decimal token, no exponent, canonical zero, round-trip to the intended f64.
Keep [the existing formatter][local-numbers] and the distinct Rough/freedraw
reduction policies. Never take arbitrary user text as a primitive token. This
avoids a *new* numeric upstream requirement, but cannot report allocation failure
from `Primitive::write`. Likewise use lower-level entries rather than first
calling an f32 helper and then appending a second key. `Content::finish` removes
a trailing newline; dictionary indentation and string spelling also differ, so
exact operands do not imply byte identity with the current artifact.
([Spec §8.6][spec-pdf], [pw-content][pw-content], [pw-object][pw-object].)

### 4.2 Fallibility and graph validity: capacity is not admission

`Pdf::with_capacity` delegates to `Chunk::with_capacity`, which uses
`Vec::with_capacity` for bytes and a separate initially empty offsets vector.
`Content`, `UnicodeCmap`, `extend`, renumbering, object insertion, and writer
destructors also grow vectors without typed allocation errors. Neither a large
capacity hint nor constructing a pre-reserved external Vec gives the current
API a bounded, fallible sink. `Primitive`'s Vec is borrowed only while writing
one primitive; exploiting that access cannot make all other writes fallible.
([pw-chunk][pw-chunk], [pw-content][pw-content], [pw-font][pw-font], [pw-object][pw-object].)

The current renderer explicitly uses `try_reserve` for many plan/output/map
buffers and the compressor callback. **It is not evidence of a completely
fallible existing implementation:** `encode`'s capacity is a checked estimate
with fixed allowances, `begin_object` indexes an offset slice, and nested
canonicalization uses `Vec::with_capacity` and other infallible allocations.
Growing String writes and existing upstream calls also remain audit surfaces.
Do not claim that a swap either fixes those paths automatically or inherits a
complete allocation proof. The governing requirement remains all
untrusted-dependent allocations fallible, with physical refusal fatal
`resource.memory-unavailable`, no partial artifact, and no `catch_unwind`
boundary. [Current encode/helpers][local-pdf], [nested adapter][local-nested],
[compression][local-streams], [spec §13.3][spec-safety]

For a checked adapter, allocate named handles in the existing semantic order
before emission and track **allocated, defined, and referenced** IDs separately:

- Check positive IDs and checked arithmetic before `Ref::new`; reserve room for
  `max_id + 1` in the finalizer's i32 `/Size`. `Ref::next/bump` is not a checked
  allocator. Check stream and array/count conversions; bound classic xref
  offsets to its ten-digit representation. These should normally be unreachable
  under admitted limits, but need an explicit proof or error, not a panic.
- Reject duplicate definitions and missing referenced objects; verify catalog,
  page tree, resources and imported root roles. `Pdf::finish` panics on duplicate
  definitions, writes gaps as free entries, and does not validate the referenced
  graph or even require a catalog to be present.
- For imported chunks, `refs()` enumerates definitions, not all outgoing
  references. `renumber` invokes the mapping for references outside the chunk as
  well. Build an explicit mapping from definitions plus permitted external
  references, then verify closure/root membership. A memoizing allocator that
  silently allocates every encountered reference can create dangling objects.

These findings follow directly from [Ref/stream implementation][pw-object],
[finish][pw-lib], and [renumber API/source][pw-chunk]. Structural validation with
qpdf is qualification evidence; it is not a production substitute for checked
construction. Mapping library invariant failures should yield a typed encoding
failure, not a compatibility placeholder or invented resource charge.

### 4.3 ADR-0008 file ID: two gaps, not just a missing setter

The current [finalization block][local-finalization] writes classic xref, hashes
all bytes so far, and writes two identical uppercase hex strings containing the
first 16 bytes of SHA-256. The preimage includes **header + objects + xref** and
ends immediately before `trailer`. [ADR-0008][adr8]

`Pdf::as_bytes()` before `finish()` has no xref. `finish(self)` consumes the writer
and emits xref and trailer together. `set_file_id` accepts a value known before
that operation; it supplies neither a callback nor the hash boundary. Moreover,
it emits IDs using `Str`: an all-ASCII digest is written as a literal string,
not forced uppercase hexadecimal. Passing ASCII hex digits as the ID changes
the actual ID bytes and still does not solve this. [Pdf API][pdf-api],
[finish source][pw-lib], [Str source][pw-object]

For the independent current-version **probe**, a bounded post-finish adapter can
locate the real final trailer from the end-of-file/startxref structure, hash the
prefix including xref, and reconstruct the trailer with forced hex IDs. It must
not search for the first `trailer` byte sequence: a binary stream or literal can
contain it. Since object offsets precede the trailer, changing only that suffix
does not move them. The bounded five-object experiment in §9 demonstrates this;
it is not a recommended production parser. It retains handwritten finalization
and would need checked parsing/allocation in production. A two-build/set-ID approach additionally depends on
identical prefixes and still lacks forced-hex output. Prefer §7's explicit
finalization boundary; retain the outer finalizer until then.

### 4.4 Determinism and compression

pdf-writer streams caller-supplied values in call order; it does not canonicalize
dictionary entries. Preserve the renderer's semantic object order, first-use
opacity/CID order, asset-identity-sorted image/SVG resources, stable occurrence
ties, and reversed link-annotation emission. Choose an explicit dictionary
order. Renumbering must use a stable map independent of randomized dictionary
traversal; sorting dictionary text *after* allocating IDs is not in general a
proof that IDs were stable. [Current resource census][local-pdf],
[font mappings][local-fonts], [pw-object][pw-object], [pw-chunk][pw-chunk]

This is an actual nested-resource issue: svg2pdf 0.13.0's
`ResourceContainer::finish` iterates `HashMap::values()` inside each fixed
resource category without sorting. The current canonicalizer sorts runs of
same-indent PDF dictionary lines, preserving stream bytes by direct length.
`Chunk::extend` would preserve the unsorted serialization if this adapter were
removed. Keep it until upstream ordering or a structured canonicalizing seam
exists. [svg-resources][svg-resources], [local canonicalizer][local-canonicalizer]

Keep [the owned compressor][local-streams]: miniz_oxide 0.8.9, zlib, level 6,
default strategy, checked output growth. Supply compressed bytes to pdf-writer
and set `FlateDecode`; its stream writer handles length/framing only. Preserve
`Length1` as the *uncompressed complete sfnt* length and leave XMP unfiltered.
No predictor, new filter chain, automatic recompression or compression-level
choice is part of this adoption. [ADR-0008][adr8], [pw-chunk][pw-chunk]

For nested SVG, ordinary chunks use `compress: true`; adapted chunks use
`compress: false`, compact eligible wrappers, then deflate previously stored
non-XMP streams through the owned compressor. Already filtered bodies remain
opaque. svg2pdf's content helper uses level 6, and its PNG image planes are
compressed regardless of `compress`. It also has a JPEG/DCT branch: the renderer
must continue enforcing its sanitized, normalized image inputs rather than
treating arbitrary svg2pdf input as satisfying the renderer's Flate-only policy.
[Local conversion/canonicalization][local-nested], [ADR-0009][adr9],
[svg helper][svg-helper], [svg image encoder][svg-image]

## 5. Complete reviewed programs versus derived subsets

These are different font contracts, not an optimization setting:

- **Direct PDF:** only selected complete reviewed sfnt decodes of official
  chunks, admitted by recorded length/digest; complete manifested Noto Emoji
  bytes when selected. Preserve program bytes and name tables before Flate.
  Chunk programs, including Liberation Sans, carry the six-letter PDF-level
  WOFF2-digest tag; Noto Emoji is untagged. The tag identifies the preexisting
  official chunk, not a request-derived subset. [Spec §8.6][spec-pdf],
  [ADR-0008][adr8], [embedded_font_name/write_font_objects][local-fonts]
- **Direct SVG:** derived subsets follow the subset profile, semantic identity,
  Reserved Font Name aliases and derived notices. This policy, including
  subsetting the Noto Emoji fallback for SVG, does not authorize derived programs
  in PDF. [CONTEXT: reviewed decode/derived font subset][context],
  [spec §8.3][spec-text]
- **Nested-SVG text in PDF:** outlined under `embed_text: false`, as the nested
  contract requires. Do not enable svg2pdf's text embedding/subsetting path
  while adopting its writer. Ordinary scene text continues to use the renderer's
  shaping, stored source lines, semantic CID mapping, explicit positions,
  complete ToUnicode and line-level ActualText. [Local svg_chunks][local-nested],
  [svg options/context][svg-lib], [local content][local-content]

pdf-writer is suitable at this boundary precisely because its font writers take
already-decided dictionaries and stream bytes. No automatic font subsetting is
required. Preserve `PdfGlyphMapping`'s `(glyph_id, unicode)` identity and first-use
codes (a glyph can need distinct codes for distinct Unicode sequences), CID 0
handling, map widths and source-line terminators. Its `UnicodeCmap` convenience
builder takes Unicode scalars rather than the renderer's UTF-16 units; a later
adapter must reject malformed surrogate sequences rather than silently repair
them, and explicitly use U+FFFD for the currently empty mapping case.
[Local mappings/CMap][local-fonts], [upstream UnicodeCmap][pw-font]

`TextStr` chooses ASCII or UTF-16BE, while current ActualText always uses a BOM
and UTF-16BE hex. Both may carry the same string, but replacement must qualify
decoded text and exact source-line concatenation, not assume identical bytes.
Keep a small forced-hex primitive if preserving that spelling is desired.
Any PDF program derivation/subsetting or name-table rewrite requires a separate
font-policy decision; it is not a follow-up serialization cleanup.
[pw-object][pw-object], [local-content][local-content], [spec-pdf][spec-pdf]

## 6. Structured adapted nested-SVG integration

### Current seam and why importing a Chunk is insufficient

The existing chain is occurrence-sensitive:

1. `build_svg_resources` prepares effect-bearing SVG per scene source occurrence;
   plain native SVG can share by asset identity.
2. `PreparedSvg::new` expands nested SVG images and per-consumer patterns/masks,
   repairs normalized serialization, and separates filter groups from native
   ancestors. `collect_raster_closures` records causes, input/output grids,
   consumer transforms and merged dependencies.
3. `preflight_pdf_raster_fallbacks` checks unisolatable/Forbid policy before
   rendering, admits input and distinct output surfaces, and emits one diagnostic
   per retained occurrence in canonical order.
4. `PreparedSvg::lower` renders the input grid, copies the selected output grid
   without resampling, substitutes a placed PNG and verifies that no unplanned
   filter survives, including resource subroots.
5. `svg2pdf::to_chunk` receives only native content and replacement images.
   Renumbering, dictionary canonicalization, Form/tiling-pattern wrapper
   compaction, compression and object-offset capture finish the integration.

Each step is visible in [pdf.rs][local-pdf], [nested_svg.rs][local-nested],
[raster.rs][local-raster], [raster_svg.rs][local-raster-svg] and
[raster_preflight.rs][local-preflight]. It is the single-plan ownership
[ADR-0009][adr9] requires. svg2pdf's own filter renderer instead chooses a local
layer bounding box and `raster_scale`, allocates a pixmap and renders on its own;
its `Result` is not an admission interface. Passing filtered original trees back
to that code would recreate the two-authority defect. [svg filter source][svg-filter]

`Chunk` stores serialized bytes and private offsets. Its public views are
`as_bytes` and definition IDs; `renumber` scans serialization and copies stream
payloads unchanged. There is no public object iterator carrying typed
dictionaries, stream ranges/filter information, operand sequences, or a stream
replacement operation that recomputes offsets. Editing `as_bytes().to_vec()`
does not edit the Chunk; `extend` imports the original. [pw-chunk][pw-chunk],
[pw-renumber][pw-renumber]

### Proposed structured seam, preserving the plan

First target **post-conversion serialization**, keeping the existing closure
plan and usvg adapter:

- Have the converter emit ordered object/stream records, or invoke a
  pre-serialization stream callback (proposals U3/U4 below). Records include
  definition ID, explicit references, stream kind, ordered dictionary entries,
  encoding/filter state, and either opaque bytes or content operations.
- Carry stable import mapping and the root Form reference explicitly. Preserve
  raw bytes for already compressed images/ICC resources; do not parse arbitrary
  binary streams as content.
- Represent the qualified identity image wrapper as graphics-state/transform/
  XObject operations. Remove only the exact redundant scope proven by today's
  `compact_image_wrappers`, in Forms and tiling patterns. Retain enclosing clip,
  mask, opacity, blend and isolation scopes. A failed optimization match leaves
  the content intact; a malformed operation structure is an encoding error.
- Canonicalize dictionary entries structurally, then serialize once with the
  chosen stream filter and newly computed direct lengths/offsets. Preserve
  established semantic IDs or explicitly qualify any new mapping. This is the
  path to deleting the current byte-boundary and dictionary-line scanners.

This seam does **not** remove the pre-conversion SVG adapter. usvg's normalized
`Group` fields/children are crate-private, exposed through read-only accessors.
Its writer rounds coordinates/transforms using a table ending at precision 12,
deduplicates feImage child definitions by child ID (including empty IDs), and
serializes nested SVG images through data URLs. These are specific reasons for
the current repairs, not defects a PDF writer solves. The numeric repair restores
the normalized tree's **f32** values; it does not recover pre-usvg f64 precision.
[usvg tree][usvg-tree], [usvg writer][usvg-writer], [local repair][local-raster-svg]

A later converter-level occurrence replacement API could bypass the XML bridge:
the renderer supplies stable occurrence handles and *already planned* image
replacements with exact output dimensions/placement, while conversion retains
native ancestors and rejects every unreplaced filter. Handles must distinguish
pattern/mask uses at different transforms, nested assets, and feImage dependencies;
an SVG source ID alone is insufficient. Only after equivalent provenance,
expansion, normalized numeric values and image-wrapper structure are qualified
could `PreparedSvg`'s serialization/reparse work and `raster_svg.rs` disappear.
This is a separate upstream design, not an available hook or a reason to start a
private fork. [ADR-0009][adr9], [local occurrence expansion][local-raster]

### Semantic ledger invariants

Keep the ledger driven by the admitted plan, never by the new writer's allocation,
callback, compression, object or cache counts. Preserve input/output surface
dimensions, both-surface charges when grids differ, image-plane records, warning
multiplicity/order, Forbid/unisolatable outcomes and first limit crossing. A
shared byte stream does not share occurrence diagnostics or fallback admission.
Keep the two semantic measurement/emission passes and their equality check.
[Spec §13.2][spec-accounting], [preflight][local-preflight], [encode_pdf][local-encode]

Serialized lengths may change because of whitespace, string spelling or CMap
layout. Therefore exact output-byte/work charges and boundary outcomes may change;
do not claim the *entire* trace remains identical when artifact length changes.
Report those deltas explicitly, retain all unchanged semantic transactions, and
obtain the applicable revision/baseline decision before production adoption.
Actual allocation refusal remains a separate fatal physical failure after the
same committed semantic prefix. [Spec §13.2–13.3][spec-accounting], [ADR-0008][adr8]

## 7. Concrete upstream extension proposals

The following names/signatures are **proposals**, not implemented APIs, accepted
upstream plans, or permission to fork. Each can be discussed independently.

### U1 — Fallible serialization all the way to the buffer

**Evidence:** private Vec storage in Chunk/Content, infallible insertion and
renumber/extend, allocating Drop implementations, and Vec-bound `Primitive`.
[pw-chunk][pw-chunk], [pw-content][pw-content], [pw-object][pw-object]

Propose a fallible writer family over a checked sink, with operations equivalent
to `try_write`, checked position, `try_indirect`, `try_stream`, `try_extend`,
`try_renumber`, and `try_finish`. Support a counting/hash sink and a bounded
in-memory sink, as well as fallible reservation for offsets and temporary tables.
A new `TryPrimitive`/validated-token write path must target this sink; merely
wrapping legacy `Primitive::write(&mut Vec)` would leave the same allocation gap.

Failure semantics: checked overflow/representational bounds, sink exhaustion,
allocation refusal and callback errors are distinguishable typed errors. A
sticky-error builder may preserve fluent calls, but finalization must return
the first error and no successful artifact. Drop must neither panic nor attempt
fresh fallible work whose error is lost; explicit close or latched error is
needed. A generic external sink may contain an unusable prefix on error, so the
renderer uses private staging and publishes only after success. Fault injection
must cover every buffer, offsets growth, destructors, imports and finalization,
not just the final output Vec.

An alternative upstream design could accept caller-owned byte/offset buffers
with hard no-growth bounds and return `CapacityExceeded`; it still needs checked
every-write enforcement and a nonallocating measurement path. A
`try_with_capacity` constructor alone is insufficient.

### U2 — Checked two-stage finalization and forced hexadecimal file IDs

**Evidence:** `finish`'s xref/trailer boundary is private; `set_file_id` uses `Str`.
[pw-lib][pw-lib], [pw-object][pw-object]

Propose `try_finish_xref() -> Result<FinalizationState, Error>` followed by a
consuming `try_finish_trailer(...)`, or one `try_finish_with_id` callback invoked
**after xref and before any trailer byte**. The state exposes the exact prefix
or a streaming observation of it, its length and xref offset. It must be sealed
against adding/reordering objects after the hash. The callback returns raw ID
bytes and the writer offers a forced-uppercase-hex encoding (e.g. `HexStr` or an
explicit ID encoding parameter). This covers all-ASCII digest bytes too.

Failure semantics: validate duplicate definitions, required root, maximum ID,
xref offset and all size conversions before publishing success; report callback
or sink failure without a completed artifact. Referenced-object validation must
be documented separately if the low-level writer permits raw primitives. Test
empty/sparse graphs, duplicate and dangling refs, trailer-like bytes inside
streams, maximum representable boundaries and digest strings containing only
ASCII bytes. The renderer selects classic xref and SHA-256/16; upstream need not
hardcode this project's hash algorithm.

### U3 — Structured chunk recording/transform and checked import

**Evidence:** serialized-only Chunk API and renumber scanner; current owned
canonicalizer reconstructs lengths/offsets from bytes. [pw-chunk][pw-chunk],
[pw-renumber][pw-renumber], [local-canonicalizer][local-canonicalizer]

Propose an opt-in recording mode or visitor over objects *as they are written*:
ordered dictionary/array values with explicit Ref operands, stream kind,
payload and filter state, and object identity. Allow a fallible transformation
to return replacement entries/payload and let the writer recompute lengths and
offsets. Add a checked import operation with an explicit old→new map and root
handle, definitions and outgoing-reference census. Byte spans alone are useful
for robust framing but do not solve structured dictionary ordering or content
compaction; the accepted scope must say which level it supplies.

Failure semantics: reject duplicate IDs/keys, missing mappings, undeclared
external references, invalid lengths and unsupported opaque transformations;
never silently skip an object. Preserve compressed bodies byte for byte unless
the caller explicitly supplies a valid replacement/filter pair. Either exclude
arbitrary raw primitives from validated mode or mark their reference inventory
unknown and reject strict validation; do not promise graph validity for bytes
the writer cannot interpret. Transform failure leaves the source chunk usable
and no import published. Fallible allocation requirements include record storage.

### U4 — Deterministic svg2pdf resources and pre-compression content seam

**Evidence:** randomized resource dictionary iteration;
`Context::finish_content` returns opaque compressed bytes, and `to_chunk` exposes
only `(Chunk, Ref)`. [svg-resources][svg-resources], [svg-context][svg-context],
[svg-lib][svg-lib]

Propose a documented stable resource/object order (fixed category order, then a
defined resource-name/identity order) and a fallible stream callback before
compression. Include a role such as Form or tiling-pattern content, structured
operators or a well-defined restricted content representation, and explicit
encoded versus decoded state. The callback can compact only eligible wrappers
and return ADR-0008-compressed bytes with their filter declaration; image/ICC
streams also need an explicit opaque path. Stable dictionary order alone is a
small useful upstream change even before the larger callback design.

Failure semantics: propagate callback/allocation failures through conversion;
never silently invoke automatic rasterization, change compression or omit
content. Retain a strict mode that errors if any filter reaches native encoding.
Changing resource ordering is a documented byte-affecting release change.
Repeated fresh-process conversions must agree; one process repeat is inadequate.

### U5 — Optional normalized-tree/occurrence replacement support

**Evidence:** immutable usvg tree, decimal/feImage serialization limitations,
and svg2pdf's independent filter rasterizer. [usvg-tree][usvg-tree],
[usvg-writer][usvg-writer], [svg-filter][svg-filter], [ADR-0009][adr9]

Propose either a checked usvg transformation builder or a svg2pdf conversion
overlay accepting pre-resolved occurrence replacements. It must preserve or
recompute tree bounds consistently, expose stable resource-use identities,
retain native ancestor operations and accept explicit image dimensions and
placement without choosing its own raster grid. A smaller usvg writer proposal
is a true normalized-value round-trip mode plus unique anonymous feImage
definition IDs and reusable nested-image bindings; that can reduce repairs
without changing the converter's model.

Failure semantics: reject unknown/duplicate occurrence handles, cyclic expansion,
nonfinite/singular placement, dimension mismatches and any unplanned effect.
No subtree may vanish because it lacked an ID. Failure to match a planned
replacement is an error, never permission to use automatic fallback. Retain the
current adapter unless this seam proves every ADR-0009 qualification case.

## 8. Qualification scope, related decisions and follow-up slices

### Qualification required before production adoption

1. **Numeric/structural slice:** exact tokens in content and page/annotation
   dictionaries, signed zero, tiny/subnormal and large finite values, values
   differing after f32 conversion, and the distinct Rough/freedraw policies.
   Verify no exponent/second quantization. Validate xref, stream lengths,
   reference closure, root/page resources and raw-to-decoded stream equality.
2. **File/metadata slice:** exact ADR-0008 through-xref digest, forced hex for
   ASCII-only IDs, stored XMP, no Info/timestamps/pdfaid, ICC/output intent,
   canonical scene attachment, notices and metadata failure atomicity.
3. **Text/font slice:** decoded FontFile2 bytes/length/digest and name tables;
   chunk tags versus Noto Emoji; CID widths/maps, ToUnicode including
   multi-scalar/non-BMP text, logical ActualText/source-line terminators, bidi,
   fallback fonts and viewer selection/extraction. Any UnicodeCmap body migration
   is qualified separately.
4. **Nested/adapted slice:** existing backend matrices plus cropped/scaled/rotated
   effects, distinct input/output grids, subpixel placements, native ancestors,
   pattern/mask multiple uses, dependent/anonymous feImage, nested SVG images,
   numeric round trips and Form/tiling-pattern wrapper compaction. Compare
   diagnostics, exact embedded pixel dimensions, decoded pixels and semantic
   transactions, not just pictures.
5. **Safety/resource slice:** injected allocator/sink refusal at all growth
   points, duplicate/dangling/imported refs, size arithmetic, N−1/N/N+1 output and
   working/work limits, Forbid/unisolatable failures, metadata omission and no
   partial artifacts. Preserve first-crossing semantics except explicitly
   reviewed exact-output-length deltas.
6. **Integration gates:** pinned qpdf structural checks and both independent
   PDFium/PDF.js 96-DPI viewers; existing backend qualification, registered cases,
   text/image/frame exports, resource traces and packaged native/WASM replay in
   fresh processes and forward/reverse order. Review all moved artifact and
   inventory digests; keep official SVG plus recorded divergences/exceptions as
   visual authority. Current renderer output is a useful differential control,
   not replacement reference evidence.

These gates derive from [spec §8.6][spec-pdf], [§13.2–13.3][spec-accounting],
[ADR-0008][adr8], [ADR-0009][adr9], the [PDF harness contract][pdf-harness] and
[existing backend qualification tests][pdf-qualification]. A tiny valid/viewable
PDF is enough to establish an API experiment, not enough to qualify this scope.

### #154: separate dark-mode policy decision

[#154][issue154] records that nested-SVG vector paints retain light-mode colors
under the current svg2pdf path. It asks for a choice among converter paint
lowering (with ownership implications), whole-image raster fallback, or an
explicit diagnosed compatibility exception. More structured paint/content access
could make the first route easier, but adopting pdf-writer decides none of these.
Whole-image fallback would change closure eligibility, Forbid behavior and
diagnostics; an exception would change the public compatibility contract. Keep
the revision-1 limitation explicit and preserve its tests while evaluating
serialization. #154 does not block this research or force a fork.

### #178: cross-target qualification context, not a writer defect

[#178][issue178] and its maintainer comment report 164/168 packaged artifact/report
keys agreeing, with four PNGs differing by one channel value in 1–2 pixels.
The reported controlled probe isolates host-dependent `acos`, `cos` and `powf`
in tiny-skia-path's cubic-root math; it also corrects the historical assumption
that disabling direct tiny-skia defaults removes transitive SIMD features. No
production portable-math fix or approved native-baseline update is established
by that ticket's evidence.

This is not evidence that pdf-writer's serializer is nondeterministic, nor that
these particular four PNG defects reproduce in PDF. It is evidence that a
native-only serializer/PoC run cannot certify the rendering stack. PDF closures
use resvg/tiny-skia, so include their decoded pixels and PDF bytes in cross-target
qualification, distinguish serialization differences from raster differences,
and coordinate any dependency/evidence rebinding with #178. Do not loosen visual
tolerances or rebaseline its four keys as a side effect of this work.

### Small follow-up slices

| Slice | Deliverable and exit criterion |
| --- | --- |
| A. Independent current-API probe — completed here | Exact number adapter, compressed content plus stored XMP, paired structural/viewer evidence and explicit file-ID workaround (§9). No production replacement. |
| B. Upstream feasibility | Discuss U1/U2 with source-backed minimal cases; establish whether maintainers support the required fallible sink and finalization boundary. Retention remains the outcome if not. No automatic private fork. |
| C. Checked semantic object handles | Isolate ID/range/reference validation around the existing plan; preserve emitted order and qualify graph/error boundaries before changing writer ownership. |
| D. First production serialization candidate | After U1/U2 are available and qualified, migrate page/catalog, one compressed stream and stored XMP behind the backend seam. Review exact-output/resource deltas; remove only the replaced framing. |
| E. Fonts/images/annotations | Move one resource family at a time, keeping payloads and policy. Retain CMap body generation initially; qualify all precision/string/metadata consequences. |
| F. Ordered nested chunk integration | First qualify deterministic resources; then U3/U4 to replace boundary scans, length rewriting and wrapper byte matching. Keep ADR-0009's pre-conversion adapter. |
| G. Optional XML-bridge removal | Only if U5 is practical; prove occurrence/provenance/grid equivalence before removing normalized SVG repair/reparse code. |

Any weakening of full f64 geometry, complete reviewed fonts, through-xref ID,
single-plan closures, fallible allocation or semantic accounting is a **spec/ADR
decision**, not an implementation shortcut. Serializer spelling changes need
byte-pin and exact-size outcome review under the repository's release/revision
rules. Also reconcile the historical fork-first wording at the end of §8.6 with
the actual direct-scene implementation and accepted ADR-0009 adapter path through
an explicit documentation decision; do not infer that a fork already exists or
is authorized by this research. [Spec §8.6][spec-pdf], [release policy][spec-release]

## 9. Experimental results

### Reproduce

- [`pdf_writer_probe.rs`][probe-rust] exercises **unmodified pdf-writer 0.12.0**
  using the existing dependency lock. It emits five objects: catalog, page tree,
  page, level-6 Flate content and stored XMP. It uses typed object writers and
  generic dictionary/operation operands for exact numbers.
- [`probe-pdf-writer.py`][probe-python] builds an independently handwritten
  five-object control, validates both through qpdf and raw byte/stream checks,
  and runs the pinned PDFium and PDF.js adapters. Generated PDFs, qpdf JSON,
  RGBA buffers and `report.json` go under ignored `target/pdf-writer-probe/`.
  The script fails on any assertion, parser warning, viewer failure or mismatch.

With Rust **1.97.1** selected on `PATH`, Python **3.12.3** and Docker available:

```sh
bash tools/pdf-harness/install.sh
cargo check --locked --offline -p excalidraw-render --example pdf_writer_probe
python3 tools/pdf-harness/probe-pdf-writer.py
```

The installer pins qpdf **12.3.2**, pypdfium2 **5.12.1** / PDFium
**152.0.7947.0**, and PDF.js through its checked npm lock and container image.
The control compressor is Python's zlib **1.3**, level 6; the candidate uses
miniz_oxide **0.8.9**, level 6. These need not yield identical compressed bytes:
their independently inflated content must match exactly. The inspected
`Cargo.lock` SHA-256 was
`f28bef0fc9065ef197c5c0f0bbae7bafcaa9cf1c5b13fb5652f459a7e9f9ba5a`.

### What passed

| Evidence | Observed result |
| --- | --- |
| Exact dictionary numbers | Raw `/MediaBox [0 0 144.00000000000003 108.00000000000001]` retained exactly; no f32 round trip. |
| Exact content numbers | Independently specified content matched byte for byte after decoding, including `0.30000000000000004`, `-0.0000000000000001`, `12.123456789012345`, `18.000000000000004` and `96.00000000000001`. |
| Primitive boundary | The executable also checks canonical negative zero, smallest subnormal, smallest normal, large finite values and f64 max in isolated content serialization. Rejects nonfinite inputs, exponent syntax, mismatching values and injected PDF tokens. Extreme numbers are serialization checks, not viewer geometry qualification. |
| Streams/metadata | Direct encoded lengths and stream delimiters checked; content decoded by both qpdf and Python zlib; XMP stored without a filter, independently XML-parsed, with expected title and no `pdfaid`. No `/Info`. |
| Object/reference structure | qpdf `--check` exit 0 with no warnings for both PDFs; exactly five defined objects; root/pages/parent/content/metadata references and empty page resources checked. Every classic xref entry points at its object's header. |
| ADR-0008 identifier | Both entries are identical uppercase hex of SHA-256(prefix)[0..16]. Prefix includes xref. Candidate postprocessing is restricted to the known final trailer suffix, reached through final `startxref` and the six xref rows. XMP deliberately contains `trailer` followed by a newline to guard against first-substring scanning. |
| Native repeatability | Two candidate builds in one process and two fresh executable processes agree. Each viewer's candidate render agrees in two fresh viewer processes. |
| PDFium differential | **0 changed pixels** out of 27,648, 192×144 RGBA at 96 DPI. |
| PDF.js differential | **0 changed pixels** out of 27,648, 192×144 RGBA at 96 DPI. |
| Artifact sizes | Independently serialized control **1,287 B**; pdf-writer candidate with retained trailer adapter **1,324 B**. This is a 37-byte example difference, not an estimate for scene PDFs. |

The control is an authored vector slice with a colored rectangle, exact
translation, fractional page dimensions, a stroked cubic and metadata. It is
**serializer differential evidence**, not official Excalidraw reference evidence
or a current-renderer snapshot. Comparisons are candidate-versus-control *within
each viewer*: PDFium and PDF.js antialiasing differ from one another. The script
also checks that rendered buffers are nontrivial, rather than passing on two
blank rasters.

Recorded SHA-256 evidence:

| Artifact | SHA-256 |
| --- | --- |
| Candidate PDF, after explicit trailer adaptation | `260dac2cfa4623ce180076e18542abd99d8d90f5d821ce029fc985818d6aee95` |
| Independently serialized control PDF | `e1fa88319b9151384a6aa805fa34e2b3216e212683024fc1462e2b127a247f34` |
| Decoded content, both PDFs | `e344610a1d37453315f2b919fb5af2c23e5aeba897cbbebeb249e0e6583df17e` |
| Stored XMP, both PDFs | `77061dedb67d93350c22147ed8548addbe96651354a0bc9090a6df96c626f4c3` |
| PDFium RGBA, both PDFs | `efdd9abb69c1149293bdf605c29b4dff83193dcc84aeeef420e1638983303dc3` |
| PDF.js RGBA, both PDFs | `5c1648f9a2f345c456a5847a951963b0ea1aea18442ab6e43028a2b0944269e0` |

### What this establishes, and what remains unqualified

Exact numbers, typed object syntax, stream framing and stored metadata work with
the released dependency and both representative viewers. Production adoption is
still **conditional**, as §1 recommends. The experiment deliberately retains
handwritten trailer finalization and uses infallibly growing buffers. It is not
a fallible sink demonstration. No allocation-failure injection, timing benchmark,
WASM/browser artifact replay, dynamic/imported graph validation, font/CMap
migration or nested-SVG adaptation was performed. Font policy and nested chunk
interfaces are assessed from source in §§5–6, with their separate qualification
requirements in §8. There is no production backend, dependency, spec, ADR or
approved evidence-pin change in this experiment.

Repository checks also passed with Rust 1.97.1:

```sh
cargo fmt --all -- --check
cargo clippy --locked --offline -p excalidraw-render --example pdf_writer_probe -- -D warnings
cargo test --workspace --locked --offline
```

The workspace suite had 1,117 passing tests and 18 explicitly ignored tests;
the probe's qpdf/PDFium/PDF.js runs above were executed separately. Independent
standards and issue-coverage reviews reported no findings.

## 10. Unresolved questions

- Will upstream support a complete fallible writer family, including primitives,
  offsets, imports and finalization, or only narrower capacity APIs?
- Is the acceptable integration a two-stage finalizer or a hash callback, and
  can it guarantee forced-hex IDs without postprocessing?
- Can svg2pdf expose ordered pre-compression stream records without requiring
  pdf-writer to retain a full document object model? Which layer should own the
  optional structured representation and strict reference inventory?
- What byte/output-limit changes are acceptable for the candidate release, and
  what approval/revision consequences follow? No current-artifact byte-identity
  claim has been demonstrated.
- Would a separate font/CMap probe justify replacing the current ToUnicode
  builder, given the observed CMap spelling/type differences? The metadata-only
  slice here makes no claim about that migration.
- Can the XML bridge be reduced by small usvg writer fixes, or is a broader
  occurrence replacement API worth its qualification cost? Keep #154's policy
  choice and #178's portable-math resolution independently tracked.

## References

### Repository authority and inspected implementation

[issue186]: https://github.com/sagikazarmark/excalidraw-render.orig/issues/186
[issue154]: https://github.com/sagikazarmark/excalidraw-render.orig/issues/154
[issue178]: https://github.com/sagikazarmark/excalidraw-render.orig/issues/178
[context]: ../../CONTEXT.md
[cargo]: ../../Cargo.toml
[adr8]: ../adr/0008-deflate-pdf-streams-tag-chunk-programs-and-derive-the-file-identifier.md
[adr9]: ../adr/0009-share-pdf-raster-closure-planning-and-encoding.md
[spec-pdf]: ../renderer-spec-v1.md#86-vector-first-pdf
[spec-text]: ../renderer-spec-v1.md#83-typography
[spec-accounting]: ../renderer-spec-v1.md#132-semantic-accounting
[spec-safety]: ../renderer-spec-v1.md#133-safety-and-recovery
[spec-release]: ../renderer-spec-v1.md#15-release-and-upgrade-policy
[local-pdf]: ../../crates/excalidraw-render/src/render/pdf.rs
[local-finalization]: ../../crates/excalidraw-render/src/render/pdf.rs#L986-L1015
[local-content]: ../../crates/excalidraw-render/src/render/pdf/content.rs
[local-fonts]: ../../crates/excalidraw-render/src/render/pdf/fonts.rs
[local-streams]: ../../crates/excalidraw-render/src/render/pdf/streams.rs
[local-nested]: ../../crates/excalidraw-render/src/render/pdf/nested_svg.rs
[local-canonicalizer]: ../../crates/excalidraw-render/src/render/pdf/nested_svg.rs#L689-L898
[local-raster]: ../../crates/excalidraw-render/src/render/pdf/raster.rs
[local-raster-svg]: ../../crates/excalidraw-render/src/render/pdf/raster_svg.rs
[local-preflight]: ../../crates/excalidraw-render/src/render/pdf/raster_preflight.rs
[local-numbers]: ../../crates/excalidraw-render/src/render/number_format.rs
[local-encode]: ../../crates/excalidraw-render/src/render/encode.rs#L585-L690
[pdf-harness]: ../../tools/pdf-harness/README.md
[pdf-qualification]: ../../crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs
[probe-rust]: ../../crates/excalidraw-render/examples/pdf_writer_probe.rs
[probe-python]: ../../tools/pdf-harness/probe-pdf-writer.py

### Version-pinned official upstream primary sources

- pdf-writer **0.12.0**: [Primitive API][primitive-api], [Pdf API][pdf-api],
  [lib/finalization][pw-lib], [Chunk][pw-chunk], [objects/strings/streams][pw-object],
  [content/operators][pw-content], [fonts/CMaps][pw-font],
  [structure][pw-structure], [color][pw-color], [XObjects][pw-xobject],
  [files][pw-files], [annotations][pw-annotations], [actions][pw-actions],
  [renumber implementation][pw-renumber].
- svg2pdf **0.13.0**: [conversion entry points/options][svg-lib],
  [resource dictionary order][svg-resources], [content finalization][svg-context],
  [compression helper][svg-helper], [images][svg-image],
  [automatic filter rasterization][svg-filter].
- usvg **0.45.1**: [normalized tree][usvg-tree], [SVG serializer][usvg-writer].

[primitive-api]: https://docs.rs/pdf-writer/0.12.0/pdf_writer/trait.Primitive.html
[pdf-api]: https://docs.rs/pdf-writer/0.12.0/pdf_writer/struct.Pdf.html
[pw-lib]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/lib.rs.html#211-370
[pw-chunk]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/chunk.rs.html
[pw-object]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/object.rs.html
[pw-content]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/content.rs.html
[pw-font]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/font.rs.html
[pw-structure]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/structure.rs.html
[pw-color]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/color.rs.html
[pw-xobject]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/xobject.rs.html
[pw-files]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/files.rs.html
[pw-annotations]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/annotations.rs.html
[pw-actions]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/actions.rs.html
[pw-renumber]: https://docs.rs/pdf-writer/0.12.0/src/pdf_writer/renumber.rs.html
[svg-lib]: https://docs.rs/crate/svg2pdf/0.13.0/source/src/lib.rs
[svg-resources]: https://docs.rs/crate/svg2pdf/0.13.0/source/src/util/resources.rs
[svg-context]: https://docs.rs/crate/svg2pdf/0.13.0/source/src/util/context.rs
[svg-helper]: https://docs.rs/crate/svg2pdf/0.13.0/source/src/util/helper.rs
[svg-image]: https://docs.rs/crate/svg2pdf/0.13.0/source/src/render/image.rs
[svg-filter]: https://docs.rs/crate/svg2pdf/0.13.0/source/src/render/filter.rs
[usvg-tree]: https://docs.rs/crate/usvg/0.45.1/source/src/tree/mod.rs
[usvg-writer]: https://docs.rs/crate/usvg/0.45.1/source/src/writer.rs
