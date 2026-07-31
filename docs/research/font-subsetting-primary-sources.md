# Per-glyph SVG font subsetting: primary-source findings

Research for GitHub issue [#96, "Add Excalidraw-compatible per-glyph SVG font subsetting"](https://github.com/sagikazarmark/excalidraw-render.orig/issues/96). Sources are pinned upstream code, specifications, crate source, and this repository's normative artifacts. Checked 2026-07-31.

## Decision summary

Compatibility revision 1 is unpublished and remains a draft. It is being amended now to include direct-SVG font subsetting; first publication, not the earlier draft wording, is the immutability boundary. The pre-amendment specification at commit `9d6ce4f16b3c3a67d5e88f472519a1ad91af4978` required unmodified official Unicode-range chunks and forbade runtime subsetting, merging, and renaming. Those facts remain relevant historical constraints, but the approved draft amendment supersedes them as current policy. [Source: historical revision-1 typography contract](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/docs/renderer-spec-v1.md#L578-L588), [historical revision-1 dependency and asset contract](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/docs/renderer-spec-v1.md#L794-L826), [current draft boundary and status](../renderer-spec-v1.md#3-compatibility-identity)

The approved revision-1 policy applies request-derived subsetting only to direct scene and frame SVG. PNG and PDF font behavior and text/fonts inside sanitized nested SVG remain unchanged. This is renderer policy, not a claim that the reference renderer has the same bytes or architecture. [Source: approved typography scope](../renderer-spec-v1.md#83-typography)

The approved conformance target is:

- semantic parity for collection, Unicode-range face selection, glyph/layout closure, declaration ordering, and typography;
- renderer-owned byte identity for identical input, renderer release, and compatibility revision on every supported target;
- no claim of byte-for-byte identity with Excalidraw's generated sfnt or WOFF2.

The Excalidraw adapter collects first-use-ordered scalars from compatibility-normalized `originalText` for every live target text occurrence that survives local recovery and emission, followed by all visibly emitted generated labels. It groups them into resolved-family sets. Each official face is selected by declared-range intersection, but receives the complete family set. An Excalifont set containing any scalar from the frozen Chromium-149-equivalent CJK table is copied in full to Xiaolai, which is prepended to family order. Only qualified, digest-verified built-ins participate; used non-WOFF2 fallbacks remain complete, and custom fonts are excluded. [Source: approved semantic demand and face selection](../renderer-spec-v1.md#831-semantic-demand-and-face-selection)

The approved policy also fixes fatal typed failure with no complete-chunk fallback or partial SVG; deterministic rewritten names and Reserved Font Name aliases; retained legal records, complete applicable license texts, and per-subset notices; canonical resource transactions and cache replay under unchanged limits; focused post-base64 size-reduction thresholds; and the all-230-chunk, malformed-input, native/WASM, two-viewer, deterministic-byte and resource-trace release gates. Exact implementation artifacts still have to be materialized and qualified before production admission. [Source: approved derivation failure](../renderer-spec-v1.md#833-derivation-and-validation), [SVG emission](../renderer-spec-v1.md#84-svg), [resource accounting](../renderer-spec-v1.md#132-semantic-accounting), [release-blocking matrix](../renderer-spec-v1.md#142-corpus-and-coverage), [size gates](../renderer-spec-v1.md#152-gates)

Exact reference bytes are the wrong gate. WOFF2 permits encoder choices and explicitly says semantically equivalent reconstructed font data need not be a bitwise match. Excalidraw also vendors generated WOFF2 bindings whose exact Google WOFF2 source revision is not recorded. [Source: WOFF2 variable encodings](https://www.w3.org/TR/2024/REC-WOFF2-20240808/#DataTypes), [WOFF2 reconstruction differences](https://www.w3.org/TR/2024/REC-WOFF2-20240808/#table_dir_format), [fonteditor-core build instructions](https://github.com/kekee000/fonteditor-core/blob/582bba757aa2915ec2240c61717ff12c50594675/woff2src/Readme.md#L19-L36)

The strongest current pure-Rust spike candidate is exactly pinned `skera 0.5.1` plus `ttf2woff2 0.13.2`, reusing the repository's `wuff 0.2.8` decoder. It is not ready for production admission. `skera` differs from the pinned HarfBuzz configuration, has unsupported or unfinished paths, and contains reachable panics; neither candidate has yet passed malformed-input, semantic resource-bound, all-font, or cross-target execution gates.

## Source identities

| Component | Exact identity | Role and qualification boundary |
| --- | --- | --- |
| Excalidraw | `0.18.1`, commit [`a2ec2889babf7d2295469c6d90ebe77fae57df84`](https://github.com/excalidraw/excalidraw/tree/a2ec2889babf7d2295469c6d90ebe77fae57df84) | Reference collection, face selection, subsetting pipeline, and SVG injection |
| harfbuzzjs | `0.3.6`, tag commit [`54e9968f3e641534eb4a2480b2c118696974a522`](https://github.com/harfbuzz/harfbuzzjs/tree/54e9968f3e641534eb4a2480b2c118696974a522) | Excalidraw's subset WASM package; the package identifies itself as 0.3.6 [Source](https://github.com/harfbuzz/harfbuzzjs/blob/54e9968f3e641534eb4a2480b2c118696974a522/package.json#L1-L5) |
| HarfBuzz in harfbuzzjs | commit [`e079dd20304bacd8d753eee2ceacbaa1a3978a34`](https://github.com/harfbuzz/harfbuzz/tree/e079dd20304bacd8d753eee2ceacbaa1a3978a34) | Gitlink recorded by the harfbuzzjs 0.3.6 tree; Excalidraw's generated module identifies harfbuzzjs 0.3.6 as its source [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/harfbuzz/harfbuzz-wasm.ts#L1-L15) |
| fonteditor-core binding source | `2.4.1`, commit [`582bba757aa2915ec2240c61717ff12c50594675`](https://github.com/kekee000/fonteditor-core/tree/582bba757aa2915ec2240c61717ff12c50594675) | Excalidraw says its generated Emscripten binding is a slight modification of this file [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/woff2/woff2-bindings.ts#L4-L10) |
| Google WOFF2 used by fonteditor-core | unresolved revision | The documented build clones the repository without a revision; the build script names the C/C++ inputs but does not recover the revision used for the checked-in generated module [Source](https://github.com/kekee000/fonteditor-core/blob/582bba757aa2915ec2240c61717ff12c50594675/woff2src/build.sh#L4-L45) |
| wuff | `0.2.8`, commit [`0775526a3555e089d2fce140f755cb225e35cdc7`](https://github.com/nicoburns/wuff/tree/0775526a3555e089d2fce140f755cb225e35cdc7/wuff) | Existing pure-Rust, `no_std` WOFF2 decoder; decoder bounds Brotli output by table-derived size and rejects implausible ratios [Source](https://github.com/nicoburns/wuff/blob/0775526a3555e089d2fce140f755cb225e35cdc7/wuff/src/decompress_woff2.rs#L21-L30), [validation](https://github.com/nicoburns/wuff/blob/0775526a3555e089d2fce140f755cb225e35cdc7/wuff/src/decompress_woff2.rs#L75-L100) |
| skera | `0.5.1`, commit [`734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830`](https://github.com/googlefonts/fontations/tree/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera) | Pure-Rust general font-subsetting candidate; default features are empty [Source](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/Cargo.toml#L19-L33) |
| ttf2woff2 | `0.13.2`, commit [`8a9b82a00eb8af1e33af02706241c4b89106e01f`](https://github.com/0x6b/ttf2woff2/tree/8a9b82a00eb8af1e33af02706241c4b89106e01f) | Pure-Rust TrueType-sfnt to WOFF2 encoder candidate; use `default-features = false` to exclude its CLI [Source](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/Cargo.toml#L20-L33) |
| subsetter | `0.2.6`, commit [`c3f23c67bc0190587f639bda84dc0005ca3f54d5`](https://github.com/typst/subsetter/tree/c3f23c67bc0190587f639bda84dc0005ca3f54d5) | Not a candidate: its owner says output is generally usable only for PDF and deliberately removes information required elsewhere [Source](https://github.com/typst/subsetter/blob/c3f23c67bc0190587f639bda84dc0005ca3f54d5/README.md#L5-L17) |

Excalidraw's package manifest declares `fonteditor-core 2.4.1` and `harfbuzzjs 0.3.6`; its lockfile fixes the corresponding registry archive and integrity for harfbuzzjs. [Source: package manifest](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/package.json#L94-L113), [lockfile](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/yarn.lock#L6432-L6435)

The candidate code licenses fit this repository's application license: `skera` inherits `MIT OR Apache-2.0`, `ttf2woff2` declares `MIT OR Apache-2.0`, and `wuff` declares MIT. This does not resolve the separate licenses and Reserved Font Names of generated font content. [Source: fontations workspace](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/Cargo.toml#L18-L22), [ttf2woff2 manifest](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/Cargo.toml#L1-L10), [wuff manifest](https://github.com/nicoburns/wuff/blob/0775526a3555e089d2fce140f755cb225e35cdc7/wuff/Cargo.toml#L1-L13)

## Pinned Excalidraw behavior

### Collection and face selection

1. Excalidraw walks the supplied element list in order, ignores non-text elements, and inserts each JavaScript string iterator value from `originalText` into a per-`fontFamily` `Set`. This deduplicates Unicode-code-point strings in first-seen order; it does not collect the wrapped/rendered `text` field. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts#L395-L447)
2. A registered face participates only if at least one collected character intersects its `unicodeRange`. The range is converted to a Unicode regular-expression character class. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts#L32-L50), [range conversion](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts#L114-L131)
3. Every selected face receives the complete family codepoint set, not only the intersection with that face's declared range. `Array.from(...).codePointAt(0)` preserves non-BMP code points. Codepoints absent from that chunk's `cmap` are left for the subsetter to discard. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts#L37-L50), [generator call](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts#L263-L297)
4. When a participating hand-drawn family contains CJK, Excalidraw copies the same character set to Xiaolai and prepends that fallback family. Its comment makes fallback-before-primary declaration order intentional. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts#L165-L199)

### Decode, subset, encode, and failure behavior

For each selected face, Excalidraw fetches WOFF2, decodes it to sfnt, invokes HarfBuzz with a deduplicated codepoint set, re-encodes the subset to WOFF2, and returns a `data:font/woff2;base64,...` URL. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-shared.chunk.ts#L20-L58), [data URL encoding](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-shared.chunk.ts#L60-L81)

The HarfBuzz binding creates a default subset input, changes the layout-feature set to all tags, inserts the requested Unicode values, and calls `hb_subset_or_fail`. It does not change flags, name IDs, name languages, scripts, drop tables, variation axes, or layout closure. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/harfbuzz/harfbuzz-bindings.ts#L48-L104), [subset call](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/harfbuzz/harfbuzz-bindings.ts#L157-L197)

The harfbuzzjs subset build uses `HB_TINY`, internal qsort, no C++ exceptions or RTTI, a 65 MiB initial WASM memory, and explicitly re-enables CFF, layout, variation, style, and vertical support that `HB_TINY` would otherwise remove. [Source: build](https://github.com/harfbuzz/harfbuzzjs/blob/54e9968f3e641534eb4a2480b2c118696974a522/build-subset.sh#L4-L20), [configuration override](https://github.com/harfbuzz/harfbuzzjs/blob/54e9968f3e641534eb4a2480b2c118696974a522/config-override-subset.h#L1-L7)

The pinned HarfBuzz defaults therefore matter:

- flags are all false, so hinting and layout closure remain enabled and glyph IDs may be renumbered;
- name IDs `0..=6` and only language `0x0409` are selected;
- `morx`, `mort`, `kerx`, legacy `kern`, `JSTF`, `DSIG`, `EBDT`, `EBLC`, `EBSC`, SVG, PCLT, LTSH, and Graphite tables are dropped;
- `gasp`, `fpgm`, `prep`, and `VDMX` are passed through rather than subsetted; `DSIG` appears in both lists but the drop set wins;
- all layout scripts are selected, and Excalidraw's binding expands layout features from HarfBuzz's finite default list to every feature tag.

[Source: pinned HarfBuzz input defaults](https://github.com/harfbuzz/harfbuzz/blob/e079dd20304bacd8d753eee2ceacbaa1a3978a34/src/hb-subset-input.cc#L33-L79), [drop precedence](https://github.com/harfbuzz/harfbuzz/blob/e079dd20304bacd8d753eee2ceacbaa1a3978a34/src/hb-subset.cc#L402-L406), [feature and script defaults](https://github.com/harfbuzz/harfbuzz/blob/e079dd20304bacd8d753eee2ceacbaa1a3978a34/src/hb-subset-input.cc#L81-L190), [flag meanings](https://github.com/harfbuzz/harfbuzz/blob/e079dd20304bacd8d753eee2ceacbaa1a3978a34/src/hb-subset.h#L42-L98)

This does **not** preserve every name or all embedded legal metadata. OpenType assigns IDs 13 and 14 to license description and license information URL; both are outside the selected `0..=6` set. The approved renderer policy therefore separately retains English name IDs `0..=6`, `13`, and `14` and rewrites identity-bearing names; that policy is a renderer licensing decision, not a claim of Excalidraw parity. [Source: OpenType naming table](https://learn.microsoft.com/en-us/typography/opentype/spec/name#name-ids), [approved name policy](../renderer-spec-v1.md#832-subset-profile-names-and-identity)

If decode, subset, or encode throws, Excalidraw silently base64-embeds the complete original WOFF2 after logging. Worker failure first retries the same operation on the main thread. [Source: whole-font fallback](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-shared.chunk.ts#L25-L36), [worker fallback](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/subset/subset-main.ts#L20-L69)

The approved renderer policy deliberately does not copy that fallback. Every participating source chunk must be qualified in advance; any decode, plan, sfnt construction, encode, or validation failure is fatal `encode.svg-font-subset-failed`, with no complete-chunk fallback, partial family, partial SVG, or success diagnostic. [Source: approved failure policy](../renderer-spec-v1.md#833-derivation-and-validation)

### Ordering and SVG scope

Subsetting runs with concurrency three, but each yielded declaration carries a source-order integer. `PromisePool` stores by that integer and returns object values in integer-key order. Exact duplicate declaration strings are then removed by insertion-ordered `Set`. [Source: declaration order](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts#L191-L200), [order key](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/Fonts.ts#L263-L297), [PromisePool](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/utils.ts#L1200-L1233)

SVG export calls font generation with the original top-level element slice and inserts one `style.style-fonts` node under `defs`. It does not traverse nested SVG image text/font resources through this path. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/scene/export.ts#L410-L426)

The emitted declaration contains only `font-family` and `src`; it does not repeat the source face's `unicode-range`, style, weight, or display descriptors. Declaration order consequently participates in overlapping-codepoint fallback behavior. [Source](https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/fonts/ExcalidrawFontFace.ts#L43-L50)

## Candidate assessment

### `skera 0.5.1`

`skera` exposes the inputs needed to model HarfBuzz: Unicode and glyph sets, flags, drop tables, layout scripts/features, name IDs, and name languages. Its plan performs `cmap`, GSUB/GPOS, COLR, and composite-glyph closure and always starts with `.notdef`. [Source: plan inputs](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L395-L438), [closure](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L441-L614)

It is not drop-in equivalent to the pinned HarfBuzz invocation:

- The caller must explicitly pass an inverted/all layout-feature set. `skera`'s finite default list is not Excalidraw's `font-features=*` behavior.
- Its finite default list contains `palt`, while the pinned HarfBuzz list does not. Even using defaults on both sides would not be exact. [Source: skera defaults](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L108-L223), [pinned HarfBuzz defaults](https://github.com/harfbuzz/harfbuzz/blob/e079dd20304bacd8d753eee2ceacbaa1a3978a34/src/hb-subset-input.cc#L81-L190)
- `skera` marks CFF desubroutinization, legacy names, and IUP optimization unimplemented, skips MATH glyph closure, and has incomplete overflow-resolution paths. These gaps may be irrelevant to a proved official table corpus, but must be closed or excluded by manifest assertions. [Source](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L228-L271), [MATH limitation](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L555-L599), [overflow TODO](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L1181-L1200)
- Public planning can panic on malformed or inconsistent fonts: `maxp()` and `glyf()` use `expect`, and other planning paths use `unwrap`. Returning `SubsetError` from serialization does not make the complete public path panic-free. [Source](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L425-L435), [glyf expectation](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L578-L593), [maxp expectation](https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs#L967-L975)

Recommendation: qualify a narrow adapter against all 230 official inputs. If panic, closure, or table-retention gaps remain, use an exactly pinned controlled fork. Do not expose `skera` as a public extension point and do not admit arbitrary caller fonts to this path.

### `ttf2woff2 0.13.2`

The encoder's default is single-threaded and documented as deterministic. Multithreaded bytes vary with thread count; on WASM the option is ignored and the implementation forces one thread. [Source](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/src/woff2/encoder.rs#L20-L59), [compression branch](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/src/woff2/encoder.rs#L221-L260)

Only sfnt flavor `0x00010000` is accepted; CFF/OpenType flavor is rejected. [Source](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/src/woff2/sfnt.rs#L8-L39), [API contract](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/src/woff2/encoder.rs#L356-L373)

That restriction currently fits the sealed corpus: a local 2026-07-31 probe digest-verified and decoded every one of the revision-1 catalog's 230 official rendering WOFF2 assets with `wuff 0.2.8`; every decoded sfnt had flavor `0x00010000`. This is local qualification evidence, not an upstream guarantee. The catalog is the authoritative inventory. [Source: catalog](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render-assets-base/src/generated/revision_1.rs#L1-L11), [verified loading](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render-assets-base/src/lib.rs#L139-L150)

The encoder still needs a bounded adapter or fork. It allocates from untrusted table counts and derived lengths with infallible `Vec` operations, has an internal `unwrap`, and does not expose the renderer's preflight/working-memory accounting. [Source: sfnt allocation](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/src/woff2/sfnt.rs#L21-L65), [encoding allocations](https://github.com/0x6b/ttf2woff2/blob/8a9b82a00eb8af1e33af02706241c4b89106e01f/src/woff2/encoder.rs#L263-L328)

### Local spike results

- Native `cargo check` succeeded for `skera = 0.5.1` and `ttf2woff2 = 0.13.2` with default features disabled under Rust `1.97.0`.
- A four-codepoint Excalifont subset completed with all layout features selected; two release-mode single-threaded WOFF2 encodes were byte-identical (`1524` bytes, SHA-256 `9121e9cf0554864fbe76e9350d91cb0263e445bd913a3295849bd00e5b67d910`).
- A `wasm32-unknown-unknown` compile could not be run because that target's standard library is not installed in this environment. This is a test-environment gap, not evidence that the crates fail to compile. Rust documents that the target has no host imports, supports `core`/`alloc`, and cannot use `std::thread::spawn`. [Source: Rust target documentation](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html)

These probes establish only basic applicability. They do not establish HarfBuzz equivalence, all-font success, malformed-input safety, browser acceptance, visual parity, or native/WASM byte identity.

## Deterministic design

The approved draft revision freezes the following as one policy bundle, subject to implementation qualification before publication:

1. Collect first-use-ordered Unicode scalar values from compatibility-normalized `originalText` for every live target text occurrence that survives local recovery and emission, followed by all visibly emitted generated labels, keyed by resolved family.
2. Apply the approved family substitution and exact CJK copied-set rule, then select qualified official chunks in catalog face order by declared-range intersection.
3. For each selected chunk, pass the complete family scalar set and one explicit subset profile: flags, every layout feature and script, drop/no-subset tables, retained name IDs/languages, variation policy, and unknown-table policy. Never inherit crate defaults.
4. Process selected chunks sequentially in `(family order, face order)`. Parallel execution must not be introduced unless output and resource traces remain identical.
5. Decode only digest-verified official WOFF2, subset one decoded face, encode with one exact Brotli quality, glyf/loca transform setting, and single-thread mode, then validate the generated WOFF2 by decoding and parsing it before embedding.
6. Deduplicate by semantic selected-chunk identity before work, not by generated CSS string after work. Emit one declaration per selected chunk and one style block per SVG artifact.
7. Use the approved effective alias consistently in CSS and rewritten primary OpenType names, preserve metrics and shaping behavior, retain the required legal records, treat OFL subsets as Modified Versions, and apply the four fixed Reserved Font Name aliases.
8. Cache only within the request-scoped session. A cache hit must replay the same semantic resource charges as a miss.

The WOFF2 Recommendation requires checksums to be recalculated after reconstruction and requires DSIG removal because modifying transforms invalidate it. Generated output should be checked against those requirements, not compared to another encoder's bytes. [Source](https://www.w3.org/TR/2024/REC-WOFF2-20240808/#table_format)

## Approved module architecture

Direct SVG font work remains inside the renderer behind two package-private deep modules and a thin Excalidraw adapter:

- `font_subset` has a qualified-chunk-only, request-scoped session interface. It accepts one qualified built-in chunk plus its ordered scalar set under a hidden frozen profile and concrete resource account; it owns decode, plan, sfnt construction, WOFF2 encode, validation, and that inner resource transcript. It knows no SVG or scene type.
- `svg_fonts` accepts typed semantic SVG font demand, never serialized SVG. It owns resolved-family/range selection and order, calls `font_subset`, and returns a typed opaque SVG font bundle whose matching measurement and write operations cover CSS, data URLs, notices, and exact serializer/resource data. It owns family, range, bundle, CSS, and serialization transcript events.
- The thin Excalidraw adapter owns compatibility-normalized `originalText`, emitted generated-label collection, source-to-resolved-family decisions, omission/local recovery, and the exact CJK copied-set rule. Dependencies point strictly inward: adapter to `svg_fonts` to `font_subset`; reverse and lateral scene dependencies are forbidden.

The modules stay package-private in the renderer until a concrete second production caller or independently owned published fork creates an extraction need. There is no public font option, generic backend trait, or public subsetting API. [Source: approved architecture](../renderer-spec-v1.md#9-architecture), [canonical SVG font demand and bundle terms](../../CONTEXT.md#language)

## Licensing and names

OFL 1.1 permits modification and embedding but defines deletion or substitution as a Modified Version, requires the license/copyright to accompany redistributed copies, and forbids a Modified Version from using a Reserved Font Name as the primary user-visible font name without permission. [Source: repository's shipped OFL text](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render-assets-base/license-parts/OFL-1.1.txt#L10-L17), [definitions](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render-assets-base/license-parts/OFL-1.1.txt#L19-L33), [conditions](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render-assets-base/license-parts/OFL-1.1.txt#L38-L67)

The pre-amendment revision-1 draft permitted Reserved Font Names only on exact official bytes and approved lossless decodes; derived subsets are neither. The approved amendment preserves that source-byte rule and requires deterministic aliases for generated Modified Versions. [Source: historical source-byte rule](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/docs/renderer-spec-v1.md#L816-L836), [current approved aliases](../renderer-spec-v1.md#832-subset-profile-names-and-identity)

The approved revision-1 policy records:

- each OFL subset is a Modified Version;
- primary OpenType names and CSS aliases are `ExcalidrawRender-1`, `ExcalidrawRender-3`, `ExcalidrawRender-7`, and `ExcalidrawRender-9` for `Virgil`, `Cascadia Code`, `Lilita`, and `Liberation` respectively;
- English copyright, version, license-description, and license-URL records are retained under the frozen profile;
- SVG carries distinct derived-font notices and one complete applicable OFL or MIT license text per canonical license identity;
- MIT-licensed Comic Shanns remains under its separate license treatment.

These are approved release-policy decisions, not facts delegated to a subsetter default. Exact notice bytes and qualification evidence remain release artifacts.

## Resource accounting

The existing meter already has semantic phases, live `working_bytes`, cumulative `output_bytes` and `total_work_units`, acquire/release records, sharing keys, and a `RequestDerivedBuiltInBlob` record kind. [Source](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render/src/resource_accounting.rs#L14-L37), [phases and record kinds](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render/src/resource_accounting.rs#L135-L168), [working-byte release](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render/src/resource_accounting.rs#L347-L387)

The approved draft adds canonical transactions for each participating family's
selection and every semantically selected chunk, even on cache hits:

| Event | Canonical charge |
| --- | --- |
| `svg_font_family_scalar_set` | One set entry and work unit per unique scalar per resolved family; live set storage as working bytes |
| `svg_font_chunk_considered` | The complete candidate-face by family-scalar by declared-range comparison product in canonical order; no semantic short-circuit |
| `svg_font_chunk_selected` | One selection record per `(family, face-order, official asset identity)` |
| `svg_font_woff2_decode` | Work based on manifested source bytes plus validated decoded sfnt bytes; decoded sfnt as live working bytes |
| `svg_font_subset_plan` | Work based on manifested eligible table bytes/records, requested scalars, and semantic closure glyphs; plan structures as live working bytes |
| `svg_font_subset_sfnt` | Exact generated sfnt bytes as live request-derived built-in bytes |
| `svg_font_woff2_encode` | Work based on exact encoder input bytes and frozen quality/transform profile; compressed bytes as live request-derived built-in bytes |
| `svg_font_validate` | Work based on generated WOFF2 and decoded validation bytes; temporary decode as live working bytes |
| `svg_font_css_data_url` | Exact declaration and base64 expansion in working bytes, then exact serialized contribution in output bytes |

The approved specification assigns checked semantic ingredients, event order, first-crossing precedence, record lifetimes, and cache replay independently of timings, allocator/hash-table capacity, thread count, or physical caching. It requires conservative pre-reservation before a dependency can allocate, reverse-order release on failure, and fatal rejection without retained subset, complete chunk, or partial artifact. Qualification must manifest the exact conservative adapter and quality-11 encoder workspace maxima; it cannot invent an unverified formula. The existing default ceilings remain 256 MiB live working bytes, 128 MiB output, and 1,073,741,824 total work units. [Source: approved semantic accounting](../renderer-spec-v1.md#132-semantic-accounting), [existing default limits](https://github.com/sagikazarmark/excalidraw-render.orig/blob/9d6ce4f16b3c3a67d5e88f472519a1ad91af4978/crates/excalidraw-render/src/resource_limits.rs#L85-L112)

## Verification gates

Implementation should not be admitted until all of these pass:

- **Reference collection:** BMP and non-BMP scalars, duplicate scalars, combining marks, `originalText` differing from rendered text, multiple families, CJK fallback, deleted/omitted text, and generated labels.
- **Selection and order:** one family selecting multiple Unicode-range chunks, overlapping ranges, fallback-before-primary order, exact one-time embedding, and unchanged nested SVG image behavior.
- **Closure:** `.notdef`, composites, ligatures, contextual GSUB, marks/GPOS, variation selectors, bidi mirrors, color glyphs, and every retained table; compare source and subset shaping for all retained text and enabled feature/script combinations.
- **Names and metrics:** units-per-em, ascender/descender/line gap, advances, side bearings, vertical metrics where present, approved names, copyright, license records, and Reserved Font Name policy.
- **All-font corpus:** every selected-codepoint class across all 230 official chunks, including smallest, largest, empty-intersection, full-intersection, and closure-heavy cases.
- **Safety:** fuzz WOFF2 decode, sfnt parse, subset planning/serialization, WOFF2 encode, and generated-output validation; no panic, trap, unbounded allocation, whole-chunk fallback, or partial artifact on native or WASM.
- **Determinism:** repeated bytes and resource traces on all supported native targets and an executed `wasm32-unknown-unknown` harness, with single-thread encoding and exact dependency/feature/lock pins.
- **Typography:** Chromium and controlled Rust-viewer comparisons against the unmodified selected chunks; subset bytes may differ, glyph selection and appearance may not.
- **Size:** focused Latin, multi-chunk, and complete-CJK direct SVG fixtures must each be more than 20% and 4 KiB smaller after base64/CSS overhead than same-revision complete-chunk controls; full-intersection is exempt only from that focused threshold, no representative direct SVG may grow, and ordinary SVG/PDF size gates still apply.
- **Licensing:** artifact notices, internal names, license records, source provenance, and generated-font transformation records receive explicit approval.

## Confirmed draft-revision decisions

The unpublished revision-1 amendment has approved the direct scene/frame SVG scope; unchanged PNG, PDF, and nested-SVG behavior; semantic parity with renderer-owned bytes; exact semantic collection, resolved-family, declared-range/full-set, and CJK copied-set rules; qualified-built-in-only boundary; frozen profile, failure, naming/licensing, resource, size, architecture, and complete release-gate policies. The normative details are in [the current draft revision-1 specification](../renderer-spec-v1.md#83-typography); this report remains non-normative evidence and rationale.

The remaining work is implementation qualification and evidence materialization, not specification choice:

- freeze the exact CJK table bytes, digest, and selected-codepoint classes;
- freeze the complete subset-profile digest and corpus table-treatment manifest;
- establish exact conservative adapter and quality-11 encoder workspace bounds;
- record whether qualification admits the pinned `skera` and `ttf2woff2` adapters or requires controlled forks;
- produce all required release evidence, including the all-230-chunk matrix, malformed/fuzz safety, exact native/WASM bytes and resource traces, two-viewer semantic parity, licensing/notices, focused size thresholds, dependency audits, performance, footprint, packaged-source, and final approval records.
