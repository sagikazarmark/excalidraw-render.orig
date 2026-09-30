# Krilla / krilla-svg: Stage 1 feasibility gates for the PDF backend

Research for [#201][issue201] (parent #199), 2026-09-26. Audited production
snapshot: `c9dd2f7`. Upstream sources: `krilla-v0.8.2` and `krilla-svg-v0.8.1`,
which are the **same commit** `3ffdf0588cf98050aad6edba51ca70162e1fb5b5`
(`git rev-parse` on both tags). Source line numbers below refer to that commit.
They were read from a clone at that commit. `cargo fetch` of `krilla =0.8.2` and
`krilla-svg =0.8.1` supplied the published crate sources for pdf-writer 0.15.0,
subsetter 0.2.6, usvg/resvg 0.47.0 and tiny-skia 0.12.0.

**Newer releases:** none. crates.io lists `krilla` 0.8.2 (2026-06-04) and
`krilla-svg` 0.8.1 (2026-06-04) as the latest versions ([K-ver], [KS-ver]).
Upstream `main` at `e412d76` (2026-09-24) moves Krilla's `simple-text` feature to
`harfrust ^0.12`, but it still pins `usvg`/`resvg` `0.47.0` and a git
`pdf-writer`. That snapshot has not been released.

## Question

Can Krilla 0.8.2 plus krilla-svg 0.8.1 own PDF serialization, font embedding and
nested-SVG conversion while keeping the essential behaviour: typed failures and
resource limits, the artifact manifest and notices, the embedded canonicalized
original scene document, occurrence-specific PDF raster closures, and
retirement of rustybuzz and svg2pdf? Or does a precise blocker justify deferring?

## Summary

| Gate | Verdict | One-line reason |
| --- | --- | --- |
| 1. Allocation / failure | **blocked** (contract-level) | No fallible allocation anywhere in Krilla, pdf-writer 0.15, subsetter, xmp-writer or the krilla-svg raster path. `finish()` returning `Result` does not change that. There are no resource hooks. |
| 2. Manifest / metadata | **viable-with-changes** | Embedded files work. There is no custom-XMP, catalog or Info extension point. `/AF` and `/AFRelationship` are written only for PDF 2.0 or PDF/A-3. The file ID is not settable, but a post-finish trailer rewrite is feasible. |
| 3. Raster-closure control | **viable-with-changes** | Any filtered usvg group is silently rasterized at Krilla's own scale, with a 5000 px cap. No switch exists. An adapter guard over the whole tree plus `Image::from_custom` for pre-admitted closures works. |
| 4. Dependencies / features | **blocked for released versions**; a patch is required | Krilla on its own without defaults drops rustybuzz. krilla-svg 0.8.1 re-enables Krilla's defaults, requires usvg's `text` feature (rustybuzz in 0.47), and cannot take our usvg 0.45.1 or 0.48 trees. |
| 5. Coordinates / fonts / determinism | **viable-with-changes** (explicit contract changes) | f32 throughout. Fonts are always re-subset. Fresh-process output is byte-identical. The settings are listed in §5. |

Recommendation for Stage 1: **defer the full rendering slice**. Gate 1 is a spec
decision (§13.3) that no prototype can answer, and gate 4 needs an upstream
release or a pinned krilla-svg patch before rustybuzz and the archived stack can
leave. The minimum changes are listed below. Gates 2, 3 and 5 have credible
adapter paths.

## Evidence method

- Source reading at the tagged commit. Each claim below cites file:line.
- Registry metadata from the crates.io API ([KS-deps], [K-deps], [usvg-feat],
  [resvg-feat], [fontdb-feat]).
- `cargo tree -e features` on a scratch crate depending on `krilla =0.8.2` and
  `krilla-svg =0.8.1`, and on a second crate using
  `krilla = { version = "=0.8.2", default-features = false, features = ["raster-images"] }`.
- A throwaway probe (§Appendix) built against the released crates. It draws an
  off-origin stroked path, pre-shaped Excalifont glyphs (`draw_glyphs`), a custom
  RGBA image, a filtered SVG through krilla-svg and an embedded
  `scene.excalidraw`. It ran twice in fresh processes and produced
  byte-identical output (15,201 B, SHA-256
  `d33bf3de40d61cbe5d4feba608beb4990dca803670450b0305f8aaf684af9702`). The
  harness-pinned qpdf 12.3.2 (`target/pdf-harness/qpdf/bin/qpdf --check`) exits
  0 with no warnings. Streams were inspected by inflating them with Python.
  Neither viewer was run. The probe is structural evidence, not qualification.

---

## Gate 1: Allocation and failure handling — **blocked (contract-level)**

**`finish` signature.** `Document::finish(self) -> KrillaResult<Vec<u8>>`
([document.rs:142-155][doc]). It calls `SerializeContext::finish` and then
`Pdf::finish`, which returns `Vec<u8>` and *panics* on a duplicated indirect
reference ([pdf-writer lib.rs:303-322][pw-lib]).

**Error variants.** `KrillaError` has `Font(Font, String)`,
`Validation(Vec<(ValidationError, Validators)>)`, `Limit(LimitError)`,
`DuplicateNamedDestination`, `DuplicateTagId`, `UnknownTagId`, and behind
`raster-images` `Image(Image, Option<Location>, String)` and `SixteenBitImage`,
plus `Pdf` behind the `pdf` feature ([error.rs:178-213][err]). `LimitError` is
`TooLargeFloat | TooLongArray | TooLongDictionary` ([error.rs:274-283][err]). It
is checked **only for PDF 1.4** ([serialize.rs:970-988][ser]).

**Fallible allocation.** None. `grep try_reserve|try_with_capacity` finds no
match in `krilla`, `krilla-svg`, `pdf-writer 0.15.0`, `subsetter 0.2.6`,
`xmp-writer 0.3.3` or `flate2 1.1.10/src`. The only match in the reached graph is
skrifa's own outline scratch buffers. Specific infallible sites:

- The output buffer is `Pdf::with_settings_and_capacity` with an f32 estimate
  `chunks_byte_len * 1.1 + 200` ([chunk_container.rs:122-127][cc]).
- Every stream is `deflate_encode` into `Vec::new()` with
  `write_all(..).unwrap(); finish().unwrap()` ([stream.rs:345-352][stream]).
- Images are copied and deflated inside `Deferred` closures
  ([image.rs:291-316][img]).
- The krilla-svg filter path allocates `tiny_skia::Pixmap::new(w, h)`
  ([filter.rs:61][filter]), which is `vec![0; data_len]` with an explicit "cannot
  check allocation" comment ([tiny-skia pixmap.rs:43-54][ts-pixmap]), plus a
  demultiplied RGBA copy ([filter.rs:79-88][filter]).

**Panics reachable from caller misuse** (an adapter can avoid them, but they
are not typed errors):

- Starting marked content twice ([content.rs:112-118][content]).
- A push/pop imbalance on `Surface` drop ([surface.rs:554-558][surface]).
- A `CustomImage` channel length mismatch, via `assert_eq!`
  ([image.rs:299-305][img]).
- `GlobalObjects::assert_all_taken` ([serialize.rs:1079-1087][ser]).

**Validation errors** are typed (`ValidationError`), but they are recorded only
when a configured PDF/A or PDF/UA validator prohibits them
([serialize.rs:503-507][ser]). With the default, empty validator a `.notdef`
glyph, a missing ToUnicode mapping or a missing embedded-file date is accepted
silently. No validator is diagnostic-only: selecting one also forces XMP,
device-independent colour and tagging ([serialize.rs:279-284][ser]).

**Limits and size hooks.** None. `SerializeSettings` has no size or budget field
([serialize.rs:46-119][ser]).

**Why this blocks.** Spec §13.3 requires "All untrusted-dependent allocations
are fallible. Physical allocation failure is fatal `resource.memory-unavailable`"
([spec:3190-3194][spec-safety]). It also requires injected allocator refusal to
be exercised ([spec:3211-3215][spec-safety]). Under Krilla, a refusal aborts the
process. A `Result`-returning finalizer does not establish fallible internal
allocation. The semantic ledger can still bound inputs *before* Krilla runs, but
it cannot turn a physical refusal inside Krilla into a typed failure. A prototype
cannot answer this. It needs one of:

1. an upstream fallible-allocation mode across krilla, pdf-writer, subsetter and
   the flate2 call sites (large and unlikely soon), or
2. an explicit spec amendment scoping §13.3 so the PDF serialization stage runs
   after conservative semantic preflight, with physical refusal inside it
   delegated to host process isolation (§13.3 already assigns deadlines to
   hosts; [spec:3185-3188][spec-safety]). That amendment would also have to state
   how "allocator refusal" conformance tests treat this stage.

## Gate 2: Arbitrary manifest and metadata integration — **viable-with-changes**

What the current backend writes ([render/pdf.rs:454-468, 676-690, 780-840][local-pdf],
[writer.rs:221-240][local-writer]):

- a catalog with `/OutputIntents [GTS_PDFA1 sRGB]`;
- a stored XMP packet whose sole payload is the **artifact manifest** (and so
  the notices) in `<er:manifest>` under `urn:excalidraw-render:pdf:metadata:1`
  ([render/pdf.rs:1492-1520][local-pdf]);
- `/Names/EmbeddedFiles [(scene.excalidraw) …]` plus catalog `/AF`;
- a Filespec with `/F`, `/UF`, `/AFRelationship /Data`,
  `/Desc (Canonical Excalidraw scene)`, and a Flate `/EmbeddedFile` with subtype
  `application/vnd.excalidraw+json`;
- no `/Info`;
- a trailer `/ID` equal to hex SHA-256/16 over all bytes before `trailer`
  (ADR-0008);
- `%PDF-1.7`.

Each piece, checked against Krilla:

| Need | Krilla 0.8.2 | Status |
| --- | --- | --- |
| Embedded canonical scene | `Document::embed_file(EmbeddedFile{path, mime_type, description, association_kind, data, modification_date, compress, location})` ([document.rs:119-126][doc], [embed.rs:31-52][embed]). Writes `/F`, `/UF` (≥1.7), `/EF`, `/Desc`, `/Subtype`, `/Params/Size`, with Flate when `compress: Some(true)` ([embed.rs:66-137][embed]). The probe emitted `/Subtype/application#2Fvnd.excalidraw+json` and `/Desc(Canonical Excalidraw scene)`. | ✓ |
| `/AFRelationship` and catalog `/AF` | Written only if `supports_associated_files()`, which means PDF 2.0 or PDF/A-3 ([embed.rs:122-124][embed], [chunk_container.rs:337-342][cc], [version.rs:97-102][ver], [validate.rs:880-890][val]). The default is 1.7 ([version.rs:21-22][ver]). The probe at 1.7 had neither. | ✗ at 1.7. Needs PDF 2.0 or an upstream change |
| Artifact manifest and notices in XMP | `XmpWriter` is private. Krilla writes only fixed fields (dc, xmpMM IDs, NPages, format, RenditionClass, PDFVersion, optional standard fields) and then `xmp.finish(None)` ([chunk_container.rs:177-219][cc], [metadata.rs:135-245][meta]). `Metadata` has fixed fields only ([metadata.rs:17-29][meta]). | ✗ **missing API**. Needs an upstream custom-XMP hook, or a spec §12.2 carrier change (for example a second embedded file) |
| Arbitrary Info or catalog keys | `/Info` carries only title, subject, keywords, author, creator, producer and dates ([metadata.rs:247-296][meta]). The catalog writer has no extension point ([chunk_container.rs:221-344][cc]). | ✗ **missing API** |
| `GTS_PDFA1` output intent | Written only when a PDF/A validator is set, using Krilla's sRGB v4 profile ([serialize.rs:687-717][ser]). A validator also adds claims and constraints. | ✗ without claiming PDF/A |
| XMP stored (unfiltered) | Metadata stream written without a filter ([chunk_container.rs:210-216][cc]). | ✓ |
| Deterministic file ID | `/ID = (document_id, instance_id)`, both base64 SipHash-128 literals. `instance_id` hashes the bytes before Info, XMP and catalog. `document_id` hashes `Metadata::document_id`, else title+authors, else `instance_id` ([chunk_container.rs:159-193][cc], [util.rs:152-180][util]). Probe trailer: `/ID[(r9KVEnaXOc/xjTT58G8eHA==)(P7tKhVZZCidc4BzSAHRsCg==)]`. | ✗ ADR-0008 formula. ✓ adapter: the trailer follows the xref table, so replacing `/ID` after `finish` moves no xref offset or `startxref`. The ADR-0008 digest over bytes before `trailer` stays computable. The XMP `xmpMM` IDs keep Krilla's values. |
| Metadata failure stages | Stage-specific omission (spec §12.3) cannot be driven inside `finish`. The adapter has to decide, before building the `Document`, which optional pieces it includes. | adapter (open question) |

Minimum to reach parity: (a) an upstream XMP extension hook, or an accepted
carrier change for the manifest, stated rather than silently substituted with
standard metadata; (b) either PDF 2.0 output, with its own review of the
Info/CIDSet/`/Lang` differences, or an upstream option to write `/AF` at 1.7;
(c) a trailer-ID adapter or an upstream ID override; (d) dropping the
non-claiming `GTS_PDFA1` intent, or an upstream hook for it.

## Gate 3: Raster-closure control — **viable-with-changes**

**krilla-svg behaviour.** `group::render` sends *any* group with non-empty
`filters()` to `filter::render` and returns ([group.rs:13-16][group]).
`filter::render`:

- picks `raster_scale = min(max_axis_scale(cur_transform) * filter_scale, 5000 / layer_bbox)`;
- rounds the pixmap size;
- renders with `resvg::render_node`;
- draws `Image::from_rgba8` at the layer bbox ([filter.rs:18-96][filter]).

`SvgSettings` has only `embed_text` and `filter_scale` (default 4.0)
([lib.rs:35-53][ks-lib]). No setting disables filters. `draw_svg` always returns
`Some(())` ([lib.rs:61-85][ks-lib]). A failed pixmap (`?` at
[filter.rs:54-61][filter]) silently omits the group, because `group::render`
ignores the result.

The same dispatch is reached from nested SVG images ([image.rs:41-46][ks-image]),
masks ([mask.rs:32][ks-mask]), clip paths ([clip_path.rs:209][ks-clip]) and
patterns ([util.rs:80][ks-util]). The probe confirmed it: a 100×100 SVG with
`feGaussianBlur` drawn at 100 pt produced a 384×384 RGB image plus a 384×384
gray `/SMask`, and `draw_svg` returned `Some(())`. The 5000 px cap is exactly the
"silently lowers resolution" behaviour spec §13.3 prohibits
([spec:3200-3205][spec-safety]), and it contradicts ADR-0009's single
occurrence-specific plan.

**Keeping our closure planning.** Feasible without upstream changes:

1. Keep ADR-0009 planning. Serialize the lowered tree with every admitted
   closure replaced by its PNG, then reparse it into a usvg 0.47 tree. This is
   the existing bridge, which has to be requalified on the new usvg version.
2. **Adapter guard (required):** before `draw_svg`, walk the tree (root, nested
   `ImageKind::SVG` trees, `Mask::root`, `ClipPath::root`, `Pattern::root`) and
   fail the occurrence if any `Group::filters()` is non-empty. This is the only
   way to ensure krilla-svg cannot rasterize independently. Upstream offers no
   refusal mode.
3. Alternatively, draw closures directly with
   `Image::from_custom(impl CustomImage, interpolate)`
   ([image.rs:129-142, 279-317][img]):
   - 8- or 16-bit RGB, Luma or CMYK;
   - optional separate alpha written as a DeviceGray `/SMask`;
   - optional ICC profile, honoured if the PDF version supports it and the
     channel count matches;
   - `/Interpolate` only when requested;
   - channels deflated at level 6 with no predictor ([image.rs:366-507][img]).

   The probe produced `/Subtype/Image /DeviceRGB /BitsPerComponent 8 /SMask` with
   a `/DeviceGray` mask. The colour space is DeviceRGB unless
   `no_device_cs = true`, in which case Krilla's own ICC sRGB is used
   ([image.rs:404-421][img]). Images are deduplicated by a u128 SipHash of the
   content ([image.rs:279-280][img]). That matches ADR-0009's per-identity
   sharing but is not keyed by our semantic identity.

Upstream improvement (not required): an `SvgSettings` filter policy of
`Rasterize | Error`, or a raster callback, so the guard is enforced in one place.

## Gate 4: Dependency and feature APIs — **blocked for released versions; patch required**

**Feature flags.** Krilla 0.8.2 has `default = ["raster-images", "simple-text"]`,
`simple-text = ["dep:rustybuzz"]`, `raster-images` (png, zune-jpeg, gif,
image-webp, imagesize), `pdf` (hayro-write), `rayon` and `comemo`
([krilla Cargo.toml:15-27][k-toml], [K-ver]). krilla-svg 0.8.1 declares no
features.

**Pre-shaped text without shaping.** `Surface::draw_glyphs(start: Point, glyphs:
&[impl Glyph], font: Font, text: &str, font_size: f32, outlined: bool)` is not
feature-gated ([surface.rs:270-283][surface]). The `Glyph` trait supplies
`glyph_id`, `text_range` (byte range into `text`), `x_advance`, `x_offset`,
`y_offset`, `y_advance` (em-normalized) and `location`
([glyph/mod.rs:89-106][glyph]). Only `draw_text` and `shape` are behind
`simple-text` ([surface.rs:381-395][surface], [text/mod.rs:25-32][text]).

Built with `default-features = false, features = ["raster-images"]`, the tree
contains no rustybuzz, harfrust, usvg, resvg or fontdb (`cargo tree` grep exit
1). Direct deps: base64, bumpalo, flate2, float-cmp, gif, image-webp, imagesize,
indexmap, pdf-writer 0.15.0, png, rustc-hash, siphasher, skrifa 0.42.1,
smallvec, subsetter 0.2.6, tiny-skia-path 0.12.0, xmp-writer, yoke, zune-jpeg.

**krilla-svg forces the problems back in.** From its published dependencies
([KS-deps]):

- `krilla ^0.8.2` with `default_features = true` plus `raster-images`, which
  enables `simple-text` and so rustybuzz 0.20.1;
- `usvg ^0.47.0` and `resvg ^0.47.0` with defaults
  (`text, system-fonts, memmap-fonts[, raster-images]`) ([usvg-feat],
  [resvg-feat]), where usvg `text` also pulls rustybuzz;
- `fontdb ^0.23.0` with defaults `std, fs, memmap, fontconfig` ([fontdb-feat]);
- `tiny-skia ^0.12.0`.

`cargo tree -i rustybuzz` shows both paths. Its source *requires* usvg `text`:
`tree.fontdb()` ([ks-lib lib.rs:103][ks-lib]) and `Text::layouted`
([text.rs:25][ks-text]) exist only under that feature ([usvg lib.rs:55-68,
tree/mod.rs:1669-1672][usvg-src]). A downstream crate cannot switch off features
that another crate enabled.

**Version coupling.** Our workspace pins `usvg`/`resvg =0.45.1`,
`tiny-skia =0.11.4`, `svg2pdf =0.13.0` and `pdf-writer =0.12.0`
([Cargo.toml:45-78][cargo]). krilla-svg takes only a usvg 0.47 `Tree`.
Adopting it therefore means:

- moving the renderer's direct usvg/resvg/tiny-skia to 0.47/0.12, which touches
  the PNG raster path, the nested-SVG sanitizer and ADR-0009's serializer
  assumptions, *and still carries rustybuzz through usvg 0.47 `text`*; or
- a krilla-svg on usvg/resvg **0.48.1**, whose `text` uses `harfrust ^0.12`
  and `fontdb ^0.24` ([usvg-048]). No such release exists, and `main` is still
  on 0.47.

**Status.** Adoption with rustybuzz retired needs a pinned `[patch.crates-io]`
krilla-svg (or a fork) that:

1. depends on `krilla` with `default-features = false, features = ["raster-images"]`;
2. depends on `usvg`/`resvg` 0.48 with `default-features = false, features = ["text"]`
   (no system-fonts or memmap);
3. uses `fontdb` without `fs`/`memmap`/`fontconfig`.

The exit condition is an upstream krilla-svg release with those edges.

Two further notes:

- krilla-svg contains one `unsafe` call, `fontdb::Database::make_shared_face_data`
  ([text.rs:177][ks-text]). Krilla itself has `#![forbid(unsafe_code)]`
  ([lib.rs:149][k-lib]).
- flate2 selects its miniz_oxide backend through the `rust_backend` feature
  (enabled here via usvg). A fresh resolve of flate2 1.1.10 uses
  **miniz_oxide 0.9.1**, while our pinned direct miniz_oxide is 0.8.9 (ADR-0008).
  Stream bytes would come from another compressor version unless the lockfile
  keeps flate2 on a 0.8-based release.

## Gate 5: Coordinates, fonts, determinism, settings

**f32 everywhere.** `Point{x: f32, y: f32}`, `Size`, `Rect`, `Transform`
(wrapping tiny-skia-path) and `PathBuilder::{move_to,…,cubic_to}(f32…)`
([geom.rs:8-10, 26-107, 177-245, 300-323][geom]). Transforms are composed in f32
(`root_transform.pre_concat(cur_transform)`, [content.rs:159-169][content]).
pdf-writer 0.15 has `impl Primitive for f32` only and writes the shortest ryu
f32 token ([pdf-writer buf.rs:51-77, object.rs:74][pw-buf]). Glyph x positions
accumulate in f32 (`cur_x += x_advance`, [content.rs:582-598][content],
[surface.rs:247-266][surface]).

Probe: path points `1000010.1234567` and `1000110.9876543` became tokens
`1000010.1` and `1000111`, errors of 0.0235 and 0.0123 user units. The page
translate went into one `cm` (`1 0 0 -1 -1000000 400 cm`). This conflicts with
spec §8.6's full-f64, no-second-quantization rule ([spec:1714-1719][spec-86]). The
issue already frames it as a precision/error budget that has to be decided
explicitly.

**Fonts.** Every CID font is re-subset with
`subsetter::subset_with_variations` ([cid.rs:237, 437-457][cid]). There is no
setting to embed the complete program, so ADR-0008 and spec §8.6 complete
reviewed programs cannot be kept. Details:

- **Kept tables:** glyf+loca (+cvt/fpgm/prep) or CFF/CFF2, head, hhea/hmtx,
  maxp, name, post ([subsetter lib.rs:221-247][subsetter]).
- **name table:** keeps Unicode records with IDs 0–9 and 11–14 unchanged
  ([subsetter name.rs][subsetter-name]). The original family names, including
  Reserved Font Names, survive unless the input is already renamed. ADR-0001
  aliases would require feeding Krilla our own renamed derived subsets.
- **BaseFont:** `<6-letter base-26 SipHash of GlyphRemapper>+<postscript name>`
  ([cid.rs:413-434][cid]). The probe produced `WQXESD+Excalifont-Regular`, and a
  61,948-byte sfnt became a 1,064-byte decoded FontFile2 (deflated in the file).
  That is not the ADR-0008 WOFF2-digest tag.
- **ToUnicode:** covers GIDs 1..n only, so a `.notdef` glyph gets no mapping
  ([cid.rs:378-398][cid]). In the probe, U+0301 (absent from Excalifont) mapped to
  GID 0 and dropped out of extraction silently: no ActualText and no error.
- **Type3 fallback:** used for fonts with no outline table or glyphs with
  COLR/SVG/bitmap data ([glyph/mod.rs:66-74][glyph], [font.rs:339-341][font]).
  SVG glyphs need `render_svg_glyph_fn`, which does nothing by default
  ([serialize.rs:115-121, 150][ser]).
- **Font identity:** hashed from `FontInfo` (checksumAdjustment, metrics,
  PostScript name and so on), not from the bytes ([font.rs:202-249][font]).
  Distinct derived subsets must differ in those fields.

**ActualText.** Automatic `/Span <</ActualText>>` appears only where cluster or
codepoint conflicts require it ([group.rs:40-143][tgroup],
[content.rs:760-808][content]). Line-level ActualText, which spec §8.6 requires
([spec:1792-1793][spec-86]), needs `start_tagged(ContentTag::Span(SpanTag{actual_text,..}))`.
That works only with `enable_tagging` (the default is true), writes an MCID
([surface.rs:178-227][surface], [tagging/mod.rs:355-380][tag]), and panics if
nested ([content.rs:112-118][content]).

**Determinism.**

- **Ordering:** fonts in an `IndexMap`, embedded files in a `BTreeMap`, named
  destinations sorted ([serialize.rs:1047-1077][ser],
  [chunk_container.rs:314-317][cc]). Objects are renumbered 1..n in a fixed
  chunk-category order (non-stream chunks, then embedded PDFs, then streams:
  fonts, shadings, patterns, pages, embedded files, ICC, XObjects, images), with
  the XMP and catalog last ([chunk_container.rs:100-142, 388-443][cc]).
- **Hashing:** `SipHashable` hashes `TypeId`, which is not stable across
  compilers ([util.rs:136-150][util]). It is used only for cache keys, and refs
  are renumbered, so it does not reach the bytes. IDs and subset tags use
  `stable_hash128`, with `usize` widened to u64 so 32- and 64-bit targets hash
  alike ([util.rs:157-180][util]).
- **Probe:** two fresh processes produced identical output
  (`sha256 d33bf3de…`, 15,201 bytes). WASM equality was not tested.

**`SerializeSettings`** ([serialize.rs:46-153][ser]):

| Field | Default | Notes |
| --- | --- | --- |
| `pretty` | false | pdf-writer formatting |
| `compress_content_streams` | true | Content streams and ToUnicode use Flate level 6 through flate2 ([stream.rs:233-244, 345-352][stream]). Font programs and images are always deflated regardless ([cid.rs:253][cid], [image.rs:308-309][img]). |
| `no_device_cs` | false | When true, colours use Krilla's embedded ICC sRGB/gray. Spec §8.6's ICC-sRGB fills need true. |
| `ascii_compatible` | false | Adds ASCIIHex to binary streams, plus a binary marker unless a validator needs one ([stream.rs:286-299][stream], [chunk_container.rs:130-137][cc]) |
| `xmp_metadata` | true | XMP always stored. Forced true by validators. |
| `cmyk_profile` | None | — |
| `configuration` | PDF 1.7, no validator | `with_version`, `with_archival_validator`, `with_accessibility_validator` |
| `enable_tagging` | true | Needed for `SpanTag` ActualText |
| `render_svg_glyph_fn` | no-op | krilla-svg offers `render_svg_glyph` |

There is no compression-level, predictor or object-stream setting. Output uses
a classic xref table ([pdf-writer lib.rs:307-322][pw-lib]).

Other behaviour to review:

- Paths with zero width *and* height are dropped, and zero-area or two-point
  paths are never filled ([content.rs:207-228][content]). A zero-length
  round-cap stroke (an Excalidraw dot) would disappear.
- Header: `%PDF-1.7` plus `%\x80\x80\x80\x80`, the same bytes the current
  writer uses.

## Minimum upstream or contract changes

1. **Spec §13.3 (contract):** either upstream fallible allocation in
   krilla/pdf-writer/subsetter, or an amendment that bounds the PDF
   serialization stage by semantic preflight and accepts process-level abort on
   physical refusal inside it, with matching conformance-test scope. This is
   blocking and cannot be resolved by prototyping.
2. **krilla-svg release or pinned patch:** Krilla with
   `default-features = false, features = ["raster-images"]`; usvg/resvg 0.48
   with `default-features = false, features = ["text"]`; fontdb without
   `fs`/`memmap`/`fontconfig`. Exit when upstream publishes these edges.
   Without this, rustybuzz and a second usvg/tiny-skia generation remain.
3. **Renderer dependency move:** direct usvg/resvg/tiny-skia to the version
   krilla-svg consumes, with the PNG path, sanitizer and ADR-0009 serializer
   requalified.
4. **Artifact manifest carrier:** an upstream custom-XMP (or catalog) hook, or a
   reviewed spec §12.2 change that states the new carrier.
5. **`/AF` and `/AFRelationship`:** PDF 2.0 output (a reviewed version change) or
   an upstream option to write them at 1.7.
6. **ADR-0008 revisions:** accept Krilla subset programs and tags (revisiting
   #120/ADR-0001, with derived-subset aliases and notices), the fixed Flate-6
   policy on flate2's miniz_oxide version, and either an adapter trailer-`/ID`
   rewrite or an upstream ID override.
7. **Spec §8.6 precision:** replace full-f64 with a measured f32 error budget at
   admitted coordinate and scale extremes.
8. **Adapter invariants:** refuse any residual usvg filter (§3); never pass
   `.notdef` or unmapped glyphs; enforce balanced push/pop and at most one
   tagged span at a time.

## Why further prototyping would not answer the blockers

- **Gate 1** is a question about what the contract requires, not about how
  Krilla behaves. The missing fallible paths are visible in the source
  (`Vec`/`BufWriter` growth in pdf-writer 0.15, subsetter and xmp-writer, and
  resvg pixmaps in krilla-svg). A rendering slice can only confirm them.
  Adopting Krilla requires either an upstream fallible mode or a reviewed
  amendment to §13.3. A prototype supplies neither.
- **Gate 4** comes from published dependency edges (the `krilla-svg` 0.8.1
  manifest keeps Krilla's default features and pins usvg/resvg `^0.47`). A
  prototype would have to invent the krilla-svg patch it was meant to evaluate,
  and its evidence would describe that local patch, not a releasable dependency
  graph.
- Everything else the representative export would measure (viewer rasters,
  text extraction, size, f32 error) depends on those two decisions. A result
  gathered before they are made would have to be requalified afterwards, so
  gathering it now would be wasted work.

## Acceptance-criteria disposition (#201)

| Criterion | Disposition |
| --- | --- |
| Resolve or characterize Stage 1 gates | Done: gates 1–5 above. Gate 1 and gate 4 are blockers. |
| Representative export (vectors, text, nested SVG, closure) | Not built. Blocked by the gates above. |
| Text extraction, notices, embedded scene, manifest | From source and the probe: embedded scene works. There is no manifest/XMP carrier (gate 2). Unmapped glyphs are dropped silently (gate 5). |
| Viewer / structural / repeatability / size comparison | Partial: fresh-process byte repeatability and qpdf `--check` on the probe only. |
| f32 error, subsets, aliases, IDs, allocation refusal, metadata stages | Characterized from source (gates 1, 2, 5). Nothing measured at the admitted extremes. |
| Nested SVG cannot silently rasterize | Characterized: it does silently rasterize. The required adapter guard is described in gate 3. |
| Removable responsibilities | Not measured. See [library-replacement-review][lrr] for candidate ranges. |
| Version/feature map, upstream prerequisites, patch exit | Done: gate 4 and the minimum-changes list. |
| Recommendation | **Defer.** Revisit when (a) §13.3 is amended or upstream adds fallible allocation, and (b) a krilla-svg release ships without Krilla's default features, on usvg/resvg ≥0.48. |

## Open questions for a later prototype (only once 1–2 are settled)

- Do tagged `SpanTag` MCIDs without a `TagTree` pass qpdf, PDFium and PDF.js? Is
  source-line ActualText extracted correctly when Krilla nests its own automatic
  `/Span` inside?
- Measured f32 placement error at the admitted coordinate and scale extremes,
  including accumulated `draw_glyphs` advances, compared with viewer tolerance.
- Does feeding already-renamed derived subsets through subsetter keep the
  ADR-0001 name records exactly? What do notices look like per Krilla subset?
- Output size compared with ADR-0008 figures (the Krilla subset should be much
  smaller), and viewer rasters for Latin, Arabic, bidi, combining marks and
  fallback.
- PDF 2.0 side effects: Info dropped, CIDSet deprecated, `/Lang` and viewer
  support.
- Browser-WASM build and byte equality, and fontdb `memmap` or `fs` exclusion on
  wasm32.
- How the §12.3 metadata failure stages map onto choices made before
  `Document::finish`.
- Actually removable production lines once adapters are counted. See the
  candidate ranges in [library-replacement-review][lrr]; no net total has been
  measured.

## Appendix: reproduction

```sh
S=$(mktemp -d) && cd "$S" && cargo new -q probe && cd probe
cat >> Cargo.toml <<'EOF'
krilla = "=0.8.2"
krilla-svg = "=0.8.1"
usvg = "=0.47.0"
skrifa = "=0.42.1"
EOF
cargo tree -e features -i rustybuzz        # both krilla/simple-text and usvg/text paths
# second crate: krilla = { version = "=0.8.2", default-features = false, features = ["raster-images"] }
cargo tree -e normal | grep -iE 'rustybuzz|usvg|fontdb'   # no match
```

The probe binary (`src/bin/probe.rs`, about 90 lines) builds one page:

- a stroked path at x≈1e6, drawn under `push_transform(translate(-1e6, 0))`;
- `draw_glyphs` for `"Hi\u{301} A"`, with the GIDs and advances taken from the
  skrifa charmap and hmtx of
  `crates/excalidraw-fonts/catalog/predecoded/Excalifont-Regular-a88b72a2….sfnt`;
- a 4×4 `CustomImage` with alpha;
- `draw_svg` of `<svg width=100 height=100><filter id=b><feGaussianBlur stdDeviation=3/></filter><rect … filter=url(#b)/></svg>`
  at 100 pt;
- `embed_file(scene.excalidraw, application/vnd.excalidraw+json, Data, compress)`
  and `Metadata::document_id("fixed-doc-id")`.

Two runs produced byte-identical PDFs. The observations are cited in the gates
above.

[issue201]: https://github.com/sagikazarmark/excalidraw-render.orig/issues/201
[lrr]: library-replacement-review-2026-09-26.md
[spec-safety]: ../renderer-spec-v1.md#133-safety-and-recovery
[spec-86]: ../renderer-spec-v1.md#86-vector-first-pdf
[cargo]: ../../Cargo.toml
[local-pdf]: ../../crates/excalidraw-render/src/render/pdf.rs
[local-writer]: ../../crates/excalidraw-render/src/render/pdf/writer.rs
[K-ver]: https://crates.io/api/v1/crates/krilla/0.8.2
[KS-ver]: https://crates.io/api/v1/crates/krilla-svg/0.8.1
[K-deps]: https://crates.io/api/v1/crates/krilla/0.8.2/dependencies
[KS-deps]: https://crates.io/api/v1/crates/krilla-svg/0.8.1/dependencies
[usvg-feat]: https://crates.io/api/v1/crates/usvg/0.47.0
[resvg-feat]: https://crates.io/api/v1/crates/resvg/0.47.0
[fontdb-feat]: https://crates.io/api/v1/crates/fontdb/0.23.0
[usvg-048]: https://crates.io/api/v1/crates/usvg/0.48.1/dependencies
[k-toml]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/Cargo.toml#L15-L27
[k-lib]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/lib.rs#L149
[doc]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/document.rs
[err]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/error.rs
[ser]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/serialize.rs
[cc]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/chunk_container.rs
[stream]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/stream.rs
[util]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/util.rs
[img]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/graphics/image.rs
[content]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/content.rs
[surface]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/surface.rs
[geom]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/geom.rs
[embed]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/interchange/embed.rs
[meta]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/interchange/metadata.rs
[tag]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/interchange/tagging/mod.rs
[ver]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/configure/version.rs
[val]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/configure/validate.rs
[text]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/text/mod.rs
[glyph]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/text/glyph/mod.rs
[font]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/text/font.rs
[cid]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/text/cid.rs
[tgroup]: https://github.com/LaurenzV/krilla/blob/krilla-v0.8.2/crates/krilla/src/text/group.rs
[ks-lib]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/lib.rs
[filter]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/filter.rs
[group]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/group.rs
[ks-image]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/image.rs
[ks-mask]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/mask.rs
[ks-clip]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/clip_path.rs
[ks-util]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/util.rs
[ks-text]: https://github.com/LaurenzV/krilla/blob/krilla-svg-v0.8.1/crates/krilla-svg/src/text.rs
[pw-lib]: https://docs.rs/pdf-writer/0.15.0/src/pdf_writer/lib.rs.html
[pw-buf]: https://docs.rs/pdf-writer/0.15.0/src/pdf_writer/buf.rs.html
[subsetter]: https://docs.rs/subsetter/0.2.6/src/subsetter/lib.rs.html
[subsetter-name]: https://docs.rs/subsetter/0.2.6/src/subsetter/name.rs.html
[ts-pixmap]: https://docs.rs/tiny-skia/0.12.0/src/tiny_skia/pixmap.rs.html
[usvg-src]: https://docs.rs/usvg/0.47.0/src/usvg/lib.rs.html
