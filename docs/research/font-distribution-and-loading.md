# Font distribution and loading: primary-source research

Checked 2026-09-26 against repository commit
`adb6f8e211e3a331e56495b393811a3a1c95e372`. Three parallel research agents
investigated packaging, caller-provided fonts, and build-time acquisition. This
note consolidates their findings and a local architecture review. Two follow-up
agents checked Xiaolai selection and existing catalog abstractions after the user
clarified that the build helper would run in the application's own `build.rs`.
Recommendations are proposals, not changes to the rendering contract.

## Recommendation

**Implementation update:** the initial balanced-shard implementation was committed
as `391e68e`. The subsequent approved layout uses the shared `excalidraw-fonts-`
prefix with `excalidraw-fonts-xiaolai` (165 chunks, 9,596,296 payload bytes) and
`excalidraw-fonts-xiaolai-extended` (44 chunks, 3,071,196 bytes). The importer
reproduces the linked project's dominant-Unicode-block grouping and verifies its
published membership fingerprints. Both remain enabled together by `cjk`.
The common package has a targeted decimal 10 MB archive ceiling. Earlier balanced
layout recommendations and measurements below document the alternatives explored.

Separate **which font bytes define rendering** from **where those bytes come
from**. Keep a pinned metadata catalog and accept an explicit set of verified font
bytes. Optional bundled crates and an application-owned build helper can both
populate that set. For a strict no-font-bytes-on-crates.io distribution, the helper
can publish only code and metadata, acquire fonts externally, and generate
application-owned embedding code. Prebuilt CLI releases can still embed all fonts.

Publish per-family asset crates where independent reuse or selection warrants
them. Size alone does not justify eight small public packages: Xiaolai accounts
for about 97% of the current shard payload and needs its own distribution strategy.
A logical Xiaolai package can still use multiple internal data shards.

Start with a concrete `FontSet`, rather than requiring a public loader trait.
Applications can embed local files, use the build helper, read files at runtime,
or fetch them before rendering. Add a narrow exact-asset provider trait if lazy
loading or integration with existing application stores warrants it.

## Clarification: application-owned build helper

The intended workflow is different from downloading assets while compiling the
renderer dependency:

```text
Application build.rs
  -> helper crate (code + pinned manifest, no font payload)
  -> download/cache and hash-check selected font files
  -> application OUT_DIR: font files + generated registration code

Application compilation
  -> include generated registration code and font bytes
  -> construct FontSet
  -> pass FontSet to renderer
```

This works with Cargo's build order: the renderer dependency builds without font
payloads; the application's build script then generates assets for the application
itself. Cargo supports helper crates as build dependencies and generated files in
`OUT_DIR`. The earlier caveat about a consumer build script not preparing assets
for an already-built dependency does not obstruct this design.
[Official examples][cargo-build-examples]

For builds that render arbitrary runtime scene documents, the helper cannot infer
which characters future documents will contain. The application must select a
supported family/coverage bundle, embed everything, or provide additional assets
at runtime. A selected family must account for its compatibility fallbacks.

Immutable upstream identities, content hashes, a reusable local cache, and an
explicit preseeded/offline mode make this practical. The downloaded bytes still
increase the final executable or Wasm size; this changes distribution, not the
amount of data required for a chosen coverage set. Keeping payloads out of every
published package is necessary to achieve the no-registry-payload goal.

## Why Xiaolai exists, and when it is needed

Xiaolai is the pinned Excalidraw release's hand-drawn CJK fallback for Excalifont.
The browser font-family chain adds Xiaolai to Excalifont; it is not a general
fallback added to every family. Registering faces does not load every font file.
[Upstream fallback constants][excalidraw-constants]
[Font-family string][excalidraw-utils] [Font loading][excalidraw-fonts]

For upstream SVG export, Excalidraw collects `originalText` by family. If the
Excalifont character set satisfies `containsCJK`, it copies that complete set to
Xiaolai and prepends Xiaolai to the export embedding order. Individual faces are
selected by intersection with their declared Unicode ranges, then subsetted.
Browser fallback selection and this explicit SVG embedding trigger are distinct.
[Export selection][excalidraw-fonts] [Range filtering][excalidraw-face]

The trigger includes Han, Hiragana, Katakana, Hangul, and an explicit punctuation,
symbol, and currency list. It does not mean all non-Latin text. Some punctuation,
such as U+2025 `‥`, can trigger it in otherwise Latin text. Ordinary ASCII/Latin
Excalifont text without a triggering character does not require Xiaolai embedding.
[Trigger definition][excalidraw-cjk]

Xiaolai's 209 WOFF2 files are the available chunk catalog, not 209 files every
export needs. It is registered as fallback ID 100, outside the ordinary
`FONT_FAMILY` choices; the pinned normal family-string resolver does not support
selecting Xiaolai directly as an ordinary text family.
[Chunk index][excalidraw-xiaolai] [Constants][excalidraw-constants]
[Resolver][excalidraw-utils]

This renderer evaluates a target-wide CJK trigger over live Excalifont `text`,
then carries the outcome into both glyph fallback and SVG semantic demand.
Excalifont chunks get priority in glyph selection; Xiaolai is consulted when they
do not cover a character and the trigger is active. SVG chunk selection uses
semantic demand, so the required inputs cannot be inferred only from which font
eventually shaped each glyph.
[Trigger](../../crates/excalidraw-render/src/render/resolve/text.rs)
[Glyph selection](../../crates/excalidraw-render/src/render/text.rs)
[SVG demand](../../crates/excalidraw-render/src/render/font_demand.rs)

**Conclusion:** Xiaolai is needed for compatible behavior in the relevant scenes,
but bundling every Xiaolai chunk into every renderer build is this project's
current distribution policy. Omitting it from a constrained application is
reasonable if missing demanded assets have defined behavior; it reduces the
application's supported scene coverage unless another source supplies them.

## Existing catalog libraries versus an exact font-byte set

The follow-up examined fontdb 0.24.0, Fontique 0.11.1,
`linebender_resource_handle` 0.1.1, font-kit 0.14.3, and Typst 0.15.1. These are
additional version-specific checks, separate from the earlier versions below.

| Library | Useful responsibility | Fit for this boundary |
| --- | --- | --- |
| fontdb | Family/style metadata, loading sources, matching | Useful as a downstream adapter; no pinned chunk hashes, declaration ranges/order, or fallback policy. |
| Fontique | Family discovery, matching, fallback, source caching | Useful when adopting its matching facilities; substantially more than exact-asset lookup. |
| font-kit | Platform-backed font discovery and loading | Poor fit for a pure-Rust, portable storage-only dependency. |
| Typst FontBook/World | Separates metadata from host font acquisition | Good design precedent, but coupled to Typst's types and compiler interface. |
| `linebender_resource_handle::Blob<u8>` | Shared, flexibly backed byte ownership | Reusable independently of matching if interoperability makes it worthwhile. |

[fontdb source][fontdb-current] [Fontique collection][fontique-collection]
[font-kit][fontkit] [Typst World][typst-world-current] [Blob][resource-blob]

Fontique's `SourceCache` supports memory blobs and filesystem paths; it is not a
custom HTTP/asset-loader interface. `FontInfoOverride` does not override declared
Unicode ranges. Its query selects best/default faces rather than walking all
same-style Unicode-range chunks according to our pinned ordering.
[Source cache][fontique-cache] [Font metadata][fontique-font]
[Query implementation][fontique-query]

fontdb and Fontique's normal registration paths do not decode WOFF2: they expect
SFNT/OpenType or collections. Holding compressed bytes in a generic memory
container is different from parsing them as fonts. Their parsed metadata also
cannot reconstruct external Unicode-range declarations or source provenance.
Keep the current WOFF2 decoding and reviewed-SFNT validation separately.
[fontdb parser][fontdb-parser] [Fontique scanning][fontique-scan]

A sufficient first API is a concrete set keyed by the catalog's exact asset ID,
accepting static slices and owned/shared byte buffers. Validate identities and
retain selection order in the catalog, rather than relying on map iteration.
The application helper can generate construction of this same set. No public
trait is required merely because applications obtain bytes in different ways.

If on-demand stores are needed, a small project-owned trait can expose only
`asset ID -> bytes or error`. Matching, fallback, decoding, and network scheduling
remain outside that operation. A synchronous trait does not solve asynchronous
browser acquisition; preloading a set remains useful.

`Blob<u8>` can avoid inventing a general byte-owner abstraction, but its generated
ID is not a content hash. It does not replace the catalog identity or SHA256
validation. Plain standard-library ownership is sufficient without broader
Linebender interoperability. [Blob implementation][resource-blob]

## Xiaolai components and concrete crate splits

CJK means **Chinese, Japanese, and Korean**. Many Han characters are shared;
character ranges cannot generally be assigned to exactly one language.
[Unicode explanation][unicode-cjk]

The pinned index describes Xiaolai SC Regular 3.11, generated with
`cn-font-split@5.2.2`. Its 209 files are character subsets of that one face, not
weights, styles, or independent language fonts. The index documents CJK-focused
preparation and changes to Hangul spacing and CJK centering. Declarations include
associated punctuation, symbols, and fullwidth forms.
[Pinned source and preparation notes][excalidraw-xiaolai]

A local analysis of the catalog and declarations measured:

| Property | Result |
| --- | ---: |
| WOFF2 bytes | 12,667,492 |
| Smallest chunk | 17,252 bytes |
| Largest chunk | 84,024 bytes |
| Declared codepoints per chunk | 114–233 |
| Distinct declared codepoints | 41,577 |
| Codepoints declared by multiple Xiaolai chunks | 0 |

These are declared ranges, not a count of internal glyphs. Some chunks contain
contiguous Hangul syllables or Han intervals; others mix scripts. For example,
chunk `_2` combines Hangul jamo and compatibility Han, while `_52` combines jamo,
Han, and Hangul syllables. The splitter uses character ranking and size-oriented
partitioning; the original complete invocation is not recorded. A clean
Chinese/Japanese/Korean package split is therefore not present in these inputs.
[Splitter implementation][cn-splitter] [Declared ranges][excalidraw-xiaolai]

Keeping whole original chunks in declaration order gives these candidate splits:

| Package layout | Chunk IDs, inclusive | Payload bytes |
| --- | --- | ---: |
| Two packages: first | `_0–_105` | 6,338,736 |
| Two packages: second | `_106–_208` | 6,328,756 |
| Three packages: first | `_0–_67` | 4,202,964 |
| Three packages: second | `_68–_134` | 4,214,612 |
| Three packages: third | `_135–_208` | 4,249,916 |

These are measured payload sums, not newly built `.crate` archive sizes. Two
packages have ample room under the existing 8 MiB raw target. A small
`font-xiaolai` facade could depend on both data packages. Their boundaries solve
packaging size, not language selection; disabling one does not mean disabling
one language. Preserve font bytes, declared ranges, identity, and selection order.

Currently data-000 holds 131 Xiaolai chunks plus all 21 other official chunks;
data-001 holds the remaining 78 Xiaolai chunks. Storage order follows upstream
path, including hashed filename order, and has no linguistic meaning.
[Catalog](../../crates/excalidraw-render-assets-base/catalog/revision-1.json)
[Importer](../../xtask/src/catalog.rs)

## Arbitrary fonts: format capability and implementation scope

The subsequent discussion set custom-family support aside in favor of packaging
the current inventory and assessing optional CJK. That narrower proposal is
recorded below; this section remains future-scope research.

### Excalidraw's scene format

In pinned 0.18.1, text elements store numeric `fontFamily` references, not font
names or byte references. Unknown numeric IDs survive upstream's ordinary restore
path, but its font-name resolver only recognizes its built-in table. The legacy
string `font` migration is a different path and defaults unknown names.
[Text type][excalidraw-text-type] [Restoration][excalidraw-restore]
[Name resolution][excalidraw-utils]

An external mapping from a scene's font ID to a custom font is technically
possible. The limitation is interoperability: stock Excalidraw will not know
that mapping. Its scene `files` collection does not define custom text-font
resources; simply inserting a font there does not make the editor use it, and
ordinary export filters unreferenced files. ID 4 has historical uses; 100 and
1000 are internal fallback IDs. None is a reserved general-purpose custom slot.
[Constants][excalidraw-constants] [File serialization][excalidraw-json]

Released 0.18.1 has no public arbitrary-font registration API. As checked on
2026-09-26, upstream PR #11601 proposes extended fonts, including provider-qualified
IDs and custom registration, but remains open. It is evidence of an evolving
direction, not a supported format/API to rely on today.
[Released exports][excalidraw-exports] [Proposal][excalidraw-custom-pr]

### Changes needed here

Supporting an additional static TrueType-outline font is feasible without
rewriting shaping or drawing, but supporting it across SVG, PNG, and PDF is a
cross-cutting feature rather than just changing the byte loader:

| Area | Current assumption | Needed generalization |
| --- | --- | --- |
| Normalization | Only IDs 1,2,3,5,6,7,8,9 admitted; others become Excalifont | Recognize registered custom IDs and define unregistered-ID behavior. |
| Family metrics | Static source-family table | Registration-time metrics/default line-height policy, retaining built-in overrides. |
| Text selection | `OfficialFont`, static names and asset records, fixed fallbacks | Registered faces, owned identities, coverage/selection and fallback rules. |
| SVG demand/subsets | Built-in-family switches and chunk-or-Noto source enum | General sources, names/notices, and input-format/table support policy. |
| PDF embedding | TrueType `FontFile2`/`CIDFontType2`, face index zero, fixed style assumptions | Support chosen custom-font scope; different handling for CFF, collections, or other formats. |
| Identity/accounting | Known lengths, digests, decoded inputs, notices | Preparation-time validation and bounded accounting for supplied fonts. |

Sources: [Normalization](../../crates/excalidraw-render/src/normalization/kinds.rs),
[metrics](../../crates/excalidraw-render/src/font_metadata.rs),
[text](../../crates/excalidraw-render/src/render/text.rs),
[SVG sources](../../crates/excalidraw-render/src/render/svg_fonts.rs),
[PDF fonts](../../crates/excalidraw-render/src/render/pdf/fonts.rs).

The current SVG subset path requires `glyf` and uses a sealed table policy;
rewritten font names assume a Regular face. A font parser being able to open a
file does not establish that all output formats can embed/subset it correctly.
Start with explicitly supported static TrueType-outline fonts. CFF/CFF2 outlines,
variable instances, collections, and color/bitmap fonts need additional decisions
and qualification rather than an unbounded promise of every OpenType font.
[Subset implementation](../../crates/excalidraw-render/src/render/font_subset.rs)
[Table policy](../../crates/excalidraw-render-assets-base/src/lib.rs)

Replacing a font also changes widths and fit inside the scene's stored text
geometry. Define whether a custom-font extension preserves existing line breaks
and boxes or performs new layout; loading the font does not settle that behavior.
Existing source-line and baseline logic can still be reused.
[Text layout](../../crates/excalidraw-render/src/render/text.rs)

### What metadata is actually needed upfront?

"Upfront" has two meanings: metadata supplied by the application, versus metadata
available to the renderer after registration/preparation. Most ordinary font
properties can be parsed once from bytes; callers should not transcribe them.

| Information | Source | Why it matters |
| --- | --- | --- |
| Scene font ID to registered face/family mapping | Application or built-in catalog | Gives the scene's numeric ID meaning. |
| Font bytes and face index (default zero) | Application/asset helper | Identifies actual font input and, if supported, collection face. |
| Container/outline format, family/style names, units per em, vertical metrics, character map | Parse font bytes | Decoding, scaling, initial selection, baseline policy, export capability. |
| Glyph advances, outlines, shaping/layout tables | Read from font as needed | Actual shaping and drawing; not hand-written catalog metadata. |
| Default line-height and any source-family metric override | Renderer policy, optionally application override | Font tables have multiple metric conventions; Excalidraw's source-family metrics are a separate compatibility input. |
| Fallback order and any variation/style selection | Application or renderer policy | Not inferable as the intended behavior from the bytes of one font. |
| Declared Unicode ranges and chunk precedence | External declarations/catalog, when using split fonts | Different from actual `cmap` coverage; preserves Excalidraw chunk selection. |
| Digest, stored/decoded lengths | Compute/validate during preparation | Cache identity, resource bounds, reproducibility. Expected hashes for built-ins stay pinned. |
| Notices, attribution, reserved-name/subsetting policy | Package metadata and applicable licenses | Self-contained export metadata and valid derived naming. Font name tables may contain some of this but are not a complete source. |
| Upstream URL/revision and reviewed transformation history | Built-in acquisition/catalog records | Reproducing the pinned built-in corpus; not a fundamental font-parser requirement. |

For one full custom font file, default to a single face covering its actual
character map rather than requiring a manually authored chunk manifest. Detailed
declared-range metadata is needed when reproducing split-font semantics or making
selection decisions before downloading bytes. Font databases can become useful
if the application wants family/style discovery over many fonts, but a direct
ID-to-face registration still does not require generic matching.

This suggests two related APIs: an exact-asset `FontSet` for the existing pinned
catalog, and future custom-family registration that parses bytes and records
application policy. Avoid forcing custom fonts to pretend they are reviewed
upstream Excalidraw assets.

## Comparison with `excalidraw-image-fonts-cjk` and optional CJK now

The user proposes `excalidraw-fonts` for all current fonts except Xiaolai, plus
two CJK crates. Inspection of the linked published crate revealed that it already
has a companion, `excalidraw-image-fonts-cjk-extra`:

| Published 1.0.0 package | Chunks | WOFF2 payload | `.crate` bytes |
| --- | ---: | ---: | ---: |
| `excalidraw-image-fonts-cjk` | 165 | 9,596,296 | 9,643,375 |
| `excalidraw-image-fonts-cjk-extra` | 44 | 3,071,196 | 3,088,261 |

The research agent downloaded both archives, verified their registry checksums,
and compared font paths, lengths, SHA256 hashes, and bytes against this repository's
catalog and shards. Together they contain exactly the same 209 Xiaolai chunks,
with no overlap or omissions. Their archive size is explained by partitioning,
not a different or more compact representation of the complete font.
[CJK registry record][image-cjk] [Extra registry record][image-cjk-extra]

Their synchronization script classifies each complete chunk by its dominant
Unicode block. Extension A, Compatibility Ideographs, Extension B+, and a
catch-all Other bucket go into `cjk-extra`; the remaining chunks go into `cjk`.
Mixed chunks stay intact. Thus "common" and "extra" are approximate labels, not
an exact common-character repertoire or language boundary.
[Publisher's script][image-sync]

The CJK crate's build script only enumerates packaged WOFF2 files and generates
an `include_bytes!` table. The parent renderer separately predecodes selected
fonts for its rasterizer. The published parent defaults to no CJK; `cjk` enables
the 165 chunks, and `cjk-full` adds the remaining 44. Its omitted-font behavior
differs from this repository's self-contained export contract: SVG font-fetch
failures can leave external font URLs, while its rasterizer only has enabled
embedded fonts. This is a packaging precedent, not a behavior to copy implicitly.
[Font build script][image-cjk-build] [Parent manifest][image-parent-manifest]
[Parent build script][image-parent-build] [Fetch shim][image-fetch]

### Recommended package and feature boundary

For one all-or-nothing CJK capability, use balanced CJK-only storage packages:

```text
excalidraw-fonts
  non-Xiaolai official chunks + Noto Emoji + predecoded Excalifont
  complete catalog/selection metadata
  cjk feature -> excalidraw-fonts-cjk-000 + excalidraw-fonts-cjk-001
```

The measured `_0–_105` / `_106–_208` split above leaves both payloads near 6.04 MiB
and comfortably below the current 8 MiB target. Use the linked project's 165/44
split instead only if common-only CJK coverage is a desired public feature; its
first package exceeds both this repo's 8 MiB raw target and 9.5 MB archive ceiling,
although it fits the registry's default 10 MiB cap.

Enable `cjk` by default initially in the renderer and explicitly forward it to
the font crate with dependency defaults disabled. This preserves current default
behavior while permitting a lean consumer. Decide separately whether the CLI
also exposes a no-CJK build configuration. Cargo features are additive, so another
dependency can reactivate CJK; graph checks need an isolated consumer rather than
assuming a workspace-wide no-default-features run proves omission.

### Scope of implementing optionality during the split

**Assessment: medium, bounded work; appropriate to do with this reorganization.**
It does not require custom-family registration or an external-provider API.

1. Repack the small always-on payload separately from the two Xiaolai payloads.
2. Update importer/materializer/verifier and generated byte references. Current
   `AssetSegment` embeds a static shard slice directly; intentional absence needs
   an explicit representation or a separate feature-gated storage lookup.
3. Retain all metadata, ranges, ordering, and the existing CJK trigger even when
   payloads are absent. Metadata helpers currently reading WOFF2 headers must
   instead use generated metadata with verified equivalent bounds.
4. Add unavailable-font handling distinct from corrupt-font and missing-glyph
   handling. Current shaping errors become `render.internal-failure`, while SVG
   can collapse loading failures into subset-decode errors; neither correctly
   describes a deliberately disabled capability.
5. Forward features, update renamed package references in generation/CI/consumer
   checks, and amend the always-bundled-font specification.
6. Preserve default-feature artifact behavior and add targeted no-CJK tests for
   Latin/emoji/generated labels, CJK demand errors, dependency omission, and both
   native and browser-Wasm compilation.

Sources: [Importer](../../xtask/src/catalog.rs),
[asset representation and metadata helpers](../../crates/excalidraw-render-assets-base/src/lib.rs),
[SVG loading](../../crates/excalidraw-render/src/render/svg_fonts.rs),
[shaping](../../crates/excalidraw-render/src/render/text.rs),
[specification](../renderer-spec-v1.md#L2025-L2038).

### Missing-CJK behavior remains a decision

Two strict policies are possible:

- **Trigger-level rejection:** fail when the existing target-wide Excalifont CJK
  trigger activates. Simpler and format-independent, but can reject text whose
  primary font could shape it, or a triggering occurrence later omitted.
- **Demand-level failure:** preserve normal selection and fail only when an absent
  Xiaolai source must be consulted or embedded. More precise, with additional
  propagation through shaping and SVG demand. "Consulted" does not prove the font
  would win glyph selection, since its actual coverage requires reading it.

Recommendation: demand-level failure if the public promise is "renders everything
that does not require Xiaolai." Trigger-level rejection is a reasonable simpler
contract only if described explicitly. In either case use a stable unavailable-font
error, without silently changing to question marks or viewer-dependent fonts.
This decision should be settled before implementation.

## Current inventory and measured sizes

The [catalog](../../crates/excalidraw-render-assets-base/catalog/revision-1.json)
contains 230 WOFF2 source files across eight families:

| Family | Files/chunks | Stored bytes |
| --- | ---: | ---: |
| Xiaolai | 209 | 12,667,492 |
| Liberation Sans | 1 | 70,668 |
| Cascadia Code | 1 | 65,732 |
| Excalifont | 7 | 64,768 |
| Nunito | 5 | 57,564 |
| Virgil | 1 | 56,156 |
| Comic Shanns | 4 | 31,452 |
| Lilita One | 2 | 12,092 |
| **Total** | **230** | **13,025,924** |

The non-Xiaolai WOFF2 payload is only 358,432 bytes. Additionally,
[`assets-base`](../../crates/excalidraw-render-assets-base/src/lib.rs) embeds
862,680 bytes of Noto Emoji TTF and 61,948 bytes of predecoded Excalifont
basic-Latin SFNT, plus metadata, source records, and notices.

Freshly generated archives, using `cargo package` with `--no-verify --locked
--offline`:

| Package | Compressed `.crate` bytes | MiB |
| --- | ---: | ---: |
| `excalidraw-render-assets-base` | 933,223 | 0.89 |
| `excalidraw-render-assets-data-000` | 8,357,109 | 7.97 |
| `excalidraw-render-assets-data-001` | 4,705,191 | 4.49 |
| **Total** | **13,995,523** | **13.35** |

These measurements establish package size, not a packaged-consumer build test.
No alternative per-family archives were built. WOFF2 already uses Brotli
compression, so assuming substantial additional archive compression would be
unwise. [WOFF2 specification][woff2]

## What other Rust projects do

### egui/epaint: optional bundled fonts and caller-owned bytes

In egui/epaint **0.32.3**, `default_fonts` is enabled by default and selects the
optional `epaint_default_fonts` package. Applications supplying their own fonts
can disable it. `FontDefinitions::default()` becomes an empty definition when
that feature is absent. This is a direct precedent for optional convenience
assets without removing text-rendering functionality. [Manifests][egui-manifest]
[epaint manifest][epaint-manifest] [Implementation][epaint-fonts]

`FontData::from_static(&'static [u8])` and `FontData::from_owned(Vec<u8>)` support
compile-time and runtime data through the same registration model.
`FontDefinitions` maps application-chosen names to font data and families to
ordered fallback lists. The documented inputs are TTF/OTF; this is not a
replacement for this renderer's WOFF2 decode pipeline. [Implementation][epaint-fonts]

### Standalone font crates exist

`ttf-noto-sans` packages one font family and uses features to select regular,
bold, italic, and bold-italic faces. It embeds bytes and has no normal rendering
dependencies. This validates the basic `font-excalifont` idea, but does not show
that every font family will fit in one registry archive.
[Manifest][noto-manifest] [Source][noto-source]

### Typst: asset package plus a host-controlled font interface

`typst-assets` **0.15.1** explicitly separates assets to keep the compiler package
small. Its `fonts` feature enables embedding font files; without it, `fonts()`
returns an empty iterator. The font files still belong to the published asset
package regardless of the consuming application's feature selection.
[Manifest][typst-assets-manifest] [Source][typst-assets-source]

The host interface is a separate useful precedent. In Typst **0.13.1**, `World`
exposes `book()` for metadata and `font(index)` for obtaining a selected font.
The host owns acquisition and caching. `typst-kit` provides native convenience
search across explicit directories, system fonts, and optional embedded fonts;
that searcher is distinct from the portable compiler interface.
[World][typst-world] [Native searcher][typst-search]

### usvg/fontdb: explicit databases and optional discovery

In usvg **0.45.1**, `Options` owns an `Arc<fontdb::Database>` and selection/fallback
callbacks. Its default database is empty. Callbacks can add fonts on demand, but
must not remove entries or replace the database underneath existing references.
[Options][usvg-options] [Resolver][usvg-resolver]

fontdb **0.23.0** accepts owned bytes or shared binary sources independently of
filesystem loading. File-backed sources can be reread or remapped later; loading
a byte snapshot is preferable when the renderer must use immutable inputs.
Database face IDs are local handles, not persistent content identities.
[Source][fontdb-source]

usvg's defaults enable system-font and memory-mapping support, but `default-features
= false, features = ["text"]` retains text processing without that filesystem
discovery support. Enabling the support and actually scanning fonts are separate
actions. [Manifest][usvg-manifest]

### cosmic-text: convenience constructors can add ambient inputs

In cosmic-text **0.14.2**, `FontSystem::new_with_fonts` also loads system fonts
under `std`. A controlled setup instead starts with an explicit database and
locale, and can supply a custom fallback implementation. Even a configured
fallback list is insufficient if the database itself includes uncontrolled
faces. Its reusable `FontSystem` owns font and matching caches.
[Constructors and caches][cosmic-system] [Fallback implementation][cosmic-fallback]

## Evaluating the three proposals

### 1. One crate per font

**Sound where it creates useful selection or reuse boundaries.** It allows
optional dependencies to exclude entire payload packages from compilation and
lets unchanged fonts be versioned independently of renderer releases. The latter
would change this repo's current lockstep release policy.

It does not remove the registry limit. Cargo's publishing guide describes a
10 MB archive limit; crates.io's current implementation defaults to exactly
`10 * 1024 * 1024` compressed bytes and supports per-crate overrides. A larger
allowance would require registry agreement, not just a manifest setting.
[Publishing guide][cargo-publish] [Server limits][registry-limits]

Xiaolai's existing files total 12.08 MiB. A single-family archive would need over
17% further compression before overhead to fit the default limit. The measured
current shards barely compress. Keep multiple internal data packages unless an
actual single-family package measurement, different qualified representation,
or granted size exception establishes another solution.

Cargo features do not produce feature-specific registry archives. `include` and
`exclude` determine package contents. To change package distribution granularity,
use separate packages; putting font files behind `cfg` inside one package changes
compilation, not what is uploaded. [Manifest packaging][cargo-include]
[Package operation][cargo-package]

Two additional qualifications:

- Features are additive and unified. Another dependency can re-enable bundled
  fonts despite one consumer specifying `default-features = false`.
  [Cargo features][cargo-features]
- Optional dependencies are not necessarily absent from a lockfile or from
  `cargo fetch`: lockfile resolution considers all workspace-member features,
  and fetch downloads locked dependencies. Distinguish compilation/binary savings
  from guaranteed download savings. [Resolver][cargo-resolver] [Fetch][cargo-fetch]

### 2. Optional font crates and external loading

**The strongest general-purpose design.** There are two different scopes:

| Scope | Consequence |
| --- | --- |
| Exact pinned assets supplied externally | Storage/acquisition changes; the same validated inputs can preserve artifact bytes and reports. |
| Arbitrary font files, versions, or system discovery | New rendering inputs and font-selection behavior; requires a broader contract. |

A matching family name is not sufficient evidence of equivalent bytes. Replacing
the catalog's Unicode-range WOFF2 chunks with a full TTF is also not automatically
equivalent: declared ranges, precedence, source identities, and subsetting inputs
matter. [Current typography and asset rules](../renderer-spec-v1.md)

Suggested responsibilities:

1. **Catalog:** expected font identities, lengths/hashes, formats, declared
   ranges, face order, fallbacks, reviewed decoded identities, and notices.
2. **Provider or explicit font set:** immutable bytes keyed by catalog asset ID.
   Static embedded slices and caller-owned shared bytes should both work.
3. **Renderer:** validation, decoding, shaping, subsetting, and derived caches.
4. **Application:** filesystem or network acquisition. Browser applications can
   fetch before calling the synchronous rendering API.

Missing required data should be an explicit error, distinct from a font that
loaded successfully but lacks a character. A partial bundle needs a defined
policy: validate completeness when constructing the renderer, or fail when a
request demands an absent asset. Supporting partial sets cannot silently preserve
the current promise that every built-in font is always available.

An application that wants build-time embedding can then use `include_bytes!`
on its own local fonts and register those slices. This already avoids requiring
a renderer-owned downloader or build script. Rust embeds those bytes at compile
time on both native and Wasm targets. [Rust macro documentation][include-bytes]

### 3. `build.rs` without font uploads

**Possible, but it moves asset provisioning onto every source consumer.** It
avoids registry font uploads only when the bytes are excluded from the published
archive and separately supplied during a consumer build. Copying packaged fonts
into `OUT_DIR`, generating Rust arrays, or changing compression does not itself
avoid uploading their source representation. [Packaging][cargo-package]

For a controlled build workflow, prefer explicit preparation followed by a
network-free build script:

1. A tool or CI step provisions an exact, hash-pinned font bundle.
2. The consumer sets an asset-directory path through Cargo configuration or an
   environment variable.
3. The script validates the bundle and stages files under `OUT_DIR`.
4. Rust embeds those staged bytes. The script tracks the environment variable
   and input files with `rerun-if-env-changed` and `rerun-if-changed`.

Cargo's `[env]` supports repository-relative paths using `relative = true`.
Build scripts run on the host, including when compiling for Wasm. Provisioning
must happen before Cargo if a dependency's build script consumes the fonts: the
consumer application's own ordinary `build.rs` runs after its dependencies have
been built. This restriction does not apply to the clarified application-owned
embedding workflow above, where only the application consumes generated assets.
[Configuration][cargo-env] [Build-script lifecycle][cargo-build]

A downloader is a real ecosystem pattern: `utoipa-swagger-ui` downloads a UI ZIP,
supports a local-file override, extracts into `OUT_DIR`, and generates embedding
code. It also offers a vendored asset-crate fallback, used for docs.rs builds.
This is an asset precedent rather than a font-specific precedent.
[Build script][utoipa-build] [Manifest][utoipa-manifest]

The costs are concrete:

- `--locked` controls Cargo resolution, not fonts fetched by arbitrary script
  code. `cargo fetch`/`cargo vendor` do not provision arbitrary font URLs.
  [Cargo flags][cargo-package] [Fetch][cargo-fetch] [Vendor][cargo-vendor]
- Cargo's offline mode is not a network sandbox for independently implemented
  build-script downloaders; their acquisition policy must be explicit.
  [Cargo build scripts][cargo-build] [Downloader example][utoipa-build]
- docs.rs blocks network access and builds with read-only sources. A downloader
  requires an asset-free documentation configuration or packaged fallback; merely
  skipping the download will fail if code still includes missing files.
  [docs.rs build policy][docsrs]
- Successful verification with a locally configured asset directory does not
  demonstrate an unconfigured registry consumer can build. Test from the extracted
  package with the documented provisioning procedure. [Packaging][cargo-package]
- `xtask` is explicit project automation, not a dependency build hook. A publisher's
  preparation command does not automatically run for downstream users.
  [cargo-xtask limitations][xtask]

This approach can produce self-contained binaries, but it gives up a
self-contained source package unless a bundled fallback remains available.

## Fit with this repository

The current specification explicitly requires the complete built-in catalog,
forbids features removing it, requires checked-in imported bytes, and prohibits
`build.rs` downloading or generating production assets. It also requires lockstep
internal package versions. Optional payloads, independent font package versions,
and build-time acquisition therefore need deliberate amendments to those rules.
These are existing project choices, not Rust requirements.
[Specification, dependency and asset contract](../renderer-spec-v1.md#L1983-L2058)

ADR-0016 permits reviewed changes between 0.x releases and makes browser Wasm
experimental; publication does not automatically freeze a compatibility revision.
Accordingly, this proposal need not assume permanent historical-revision support
or promise unqualified native/Wasm byte equality.
[ADR-0016](../adr/0016-release-0.1.0-with-a-version-scoped-contract.md)

The implementation needs more than optional Cargo flags:

- `AssetSegment` directly stores a `&'static [u8]` shard pointer, and generated
  catalog records reference data-crate statics. Metadata is currently coupled to
  storage. [Catalog types](../../crates/excalidraw-render-assets-base/src/lib.rs)
  [Generated records](../../crates/excalidraw-render-assets-base/src/generated/revision_1.rs)
- `Renderer` currently contains only a static catalog reference and has a
  `const` constructor. Provider ownership and constructor behavior need an explicit
  design. [Renderer](../../crates/excalidraw-render/src/compatibility.rs)
- Font loading occurs in ordinary/generated text, SVG subset loading, and nested
  SVG handling. Noto Emoji and predecoded Excalifont use specialized loading paths.
  A provider must cover these as well as official WOFF2 chunks.
  [Text](../../crates/excalidraw-render/src/render/text.rs)
  [SVG fonts](../../crates/excalidraw-render/src/render/svg_fonts.rs)
  [Nested SVG](../../crates/excalidraw-render/src/render/asset/svg.rs)
- Keep cache-independent resource accounting and test externally supplied exact
  assets against bundled assets on each target: artifact bytes, reports, missing
  and corrupt assets, fallback cases, and cold/warm/frame-batch behavior.
  This is provider equivalence on a target, not a stronger cross-target promise.
  [Accounting decision](../adr/0002-scope-the-determinism-promise-to-artifact-bytes.md)
- Distribution changes do not remove font notices or subset naming rules.
  Preserve the existing treatment of modified OFL subsets and each font's actual
  license. [ADR-0001](../adr/0001-treat-font-subsets-as-modified-versions.md)
  [Official OFL text][ofl]

## Suggested next decision

Agree on **optional embedded assets plus caller-supplied exact fonts** first.
Choose package granularity after deciding whether independent font reuse is a
goal. Keep arbitrary custom fonts as a separately scoped extension, since accepting
bytes alone does not define their selection, metrics, fallback, or subset policy.

The clarified application-owned helper directly supports eliminating registry
font uploads. The remaining product questions are whether to also offer optional
font packages, and whether applications may intentionally supply partial font
sets. Those choices determine defaults and missing-asset behavior; they do not
require replacing the existing renderer-specific selection catalog with a general
font discovery library.

## Primary sources

Tagged links describe those versions, not necessarily the latest release. Sources
linked to `master`/`main` are development snapshots checked on the date above.

[woff2]: https://www.w3.org/TR/WOFF2/#table_format
[egui-manifest]: https://github.com/emilk/egui/blob/0.32.3/crates/egui/Cargo.toml
[epaint-manifest]: https://github.com/emilk/egui/blob/0.32.3/crates/epaint/Cargo.toml
[epaint-fonts]: https://github.com/emilk/egui/blob/0.32.3/crates/epaint/src/text/fonts.rs
[noto-manifest]: https://github.com/gifnksm/ttf-noto-sans/blob/master/Cargo.toml
[noto-source]: https://github.com/gifnksm/ttf-noto-sans/blob/master/src/lib.rs
[typst-assets-manifest]: https://github.com/typst/typst-assets/blob/v0.15.1/Cargo.toml
[typst-assets-source]: https://github.com/typst/typst-assets/blob/v0.15.1/src/lib.rs
[typst-world]: https://github.com/typst/typst/blob/v0.13.1/crates/typst-library/src/lib.rs
[typst-search]: https://github.com/typst/typst/blob/v0.13.1/crates/typst-kit/src/fonts.rs
[usvg-options]: https://github.com/RazrFalcon/resvg/blob/v0.45.1/crates/usvg/src/parser/options.rs
[usvg-resolver]: https://github.com/RazrFalcon/resvg/blob/v0.45.1/crates/usvg/src/text/mod.rs
[usvg-manifest]: https://github.com/RazrFalcon/resvg/blob/v0.45.1/crates/usvg/Cargo.toml
[fontdb-source]: https://github.com/RazrFalcon/fontdb/blob/v0.23.0/src/lib.rs
[cosmic-system]: https://github.com/pop-os/cosmic-text/blob/0.14.2/src/font/system.rs
[cosmic-fallback]: https://github.com/pop-os/cosmic-text/blob/0.14.2/src/font/fallback/mod.rs
[cargo-publish]: https://doc.rust-lang.org/cargo/reference/publishing.html#packaging-a-crate
[registry-limits]: https://github.com/rust-lang/crates.io/blob/main/src/config/publish_limits.rs
[cargo-include]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields
[cargo-package]: https://doc.rust-lang.org/cargo/commands/cargo-package.html
[cargo-features]: https://doc.rust-lang.org/cargo/reference/features.html
[cargo-resolver]: https://doc.rust-lang.org/cargo/reference/resolver.html#features
[cargo-fetch]: https://doc.rust-lang.org/cargo/commands/cargo-fetch.html
[cargo-vendor]: https://doc.rust-lang.org/cargo/commands/cargo-vendor.html
[include-bytes]: https://doc.rust-lang.org/std/macro.include_bytes.html
[cargo-env]: https://doc.rust-lang.org/cargo/reference/config.html#env
[cargo-build]: https://doc.rust-lang.org/cargo/reference/build-scripts.html
[utoipa-build]: https://github.com/juhaku/utoipa/blob/master/utoipa-swagger-ui/build.rs
[utoipa-manifest]: https://github.com/juhaku/utoipa/blob/master/utoipa-swagger-ui/Cargo.toml
[docsrs]: https://docs.rs/about/builds
[xtask]: https://github.com/matklad/cargo-xtask#limitations
[ofl]: https://openfontlicense.org/open-font-license-official-text/
[cargo-build-examples]: https://doc.rust-lang.org/cargo/reference/build-script-examples.html
[excalidraw-constants]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/constants.ts
[excalidraw-utils]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/utils.ts#L91-L115
[excalidraw-fonts]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts
[excalidraw-face]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts
[excalidraw-cjk]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/textWrapping.ts
[excalidraw-xiaolai]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Xiaolai/index.ts
[fontdb-current]: https://github.com/RazrFalcon/fontdb/blob/v0.24.0/src/lib.rs
[fontdb-parser]: https://github.com/RazrFalcon/fontdb/blob/v0.24.0/src/ttf_parser/mod.rs
[fontique-collection]: https://github.com/linebender/parley/blob/v0.11.1/fontique/src/collection/mod.rs
[fontique-cache]: https://github.com/linebender/parley/blob/v0.11.1/fontique/src/source_cache.rs
[fontique-font]: https://github.com/linebender/parley/blob/v0.11.1/fontique/src/font.rs
[fontique-query]: https://github.com/linebender/parley/blob/v0.11.1/fontique/src/collection/query.rs
[fontique-scan]: https://github.com/linebender/parley/blob/v0.11.1/fontique/src/scan.rs
[fontkit]: https://docs.rs/font-kit/0.14.3/font_kit/
[typst-world-current]: https://github.com/typst/typst/blob/v0.15.1/crates/typst-library/src/lib.rs
[resource-blob]: https://docs.rs/crate/linebender_resource_handle/0.1.1/source/src/blob.rs
[unicode-cjk]: https://www.unicode.org/faq/han_cjk.html
[cn-splitter]: https://unpkg.com/cn-font-split@5.2.2/dist/cn-font-split.js
[excalidraw-text-type]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/element/types.ts#L232-L259
[excalidraw-restore]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/restore.ts#L227-L273
[excalidraw-json]: https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/data/json.ts#L22-L70
[excalidraw-exports]: https://unpkg.com/@excalidraw/excalidraw@0.18.1/dist/types/excalidraw/index.d.ts
[excalidraw-custom-pr]: https://github.com/excalidraw/excalidraw/pull/11601
[image-cjk]: https://crates.io/api/v1/crates/excalidraw-image-fonts-cjk/1.0.0
[image-cjk-extra]: https://crates.io/api/v1/crates/excalidraw-image-fonts-cjk-extra/1.0.0
[image-sync]: https://github.com/rickardp/excalidraw-image/blob/c33f236d9167845c327893b6e1a19728e763d173/src/scripts/sync-fonts.mjs
[image-cjk-build]: https://docs.rs/crate/excalidraw-image-fonts-cjk/1.0.0/source/build.rs
[image-parent-manifest]: https://docs.rs/crate/excalidraw-image/0.1.0/source/Cargo.toml.orig
[image-parent-build]: https://docs.rs/crate/excalidraw-image/0.1.0/source/build.rs
[image-fetch]: https://github.com/rickardp/excalidraw-image/blob/c33f236d9167845c327893b6e1a19728e763d173/src/core/shims/fetch-fonts.mjs
