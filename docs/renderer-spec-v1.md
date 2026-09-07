# Excalidraw Static Renderer Specification

Document version: `1.2.1`

Compatibility revision: `1`

Status: Draft amendment approved; implementation and qualification pending

Approval: Revision-1 frame-export and SVG-font-subsetting decisions approved; final evidence approval required before publication

## 1. Authority

This document is the normative approved design contract for compatibility
revision 1 of `excalidraw-render`. It becomes implementation-ready when the
closed artifacts required by Section 16 are incorporated. It supersedes
conflicting wording in research notes, issue bodies, resolution comments, the
current MVP, and the repository README. Those sources remain rationale and
evidence, not parallel specifications.

The keywords MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are normative.

Normative ownership is exclusive rather than a global precedence ladder:

- The [scene-surface manifest](../tests/oracle/scene-surfaces/revision-1.json)
  owns exact field/value dispositions, diagnostic mappings, generated ordinals,
  notice texts, and fallback recipes.
- The [reference profile](../tests/oracle/reference-profiles/revision-1.json)
  and [reference asset manifest](../tests/oracle/reference-profiles/revision-1-assets.json)
  own exact environment, dependency, asset, and invocation identities.
- This document owns compatibility policy, public contracts, architecture,
  format behavior, safety, conformance, packaging, and release rules outside
  those closed domains.
- Approved conformance and release records instantiate this contract but never
  override it.

The performance and canonical WASM footprint protocols are aspirational and are
tracked in [docs/roadmap.md](roadmap.md). They are not normative, gate no
release, and bind nothing in this document until one of them is activated and
moved back here or into its own approved closed artifact.

An inconsistency between normative owners invalidates the handoff or affected
evidence. Implementations MUST NOT select whichever source is convenient. The
conflict requires an approved artifact or specification revision under the
compatibility rules below.

The canonical domain terms are defined in [CONTEXT.md](../CONTEXT.md). Serialized
artifact identifiers such as `oracle.*` and `baseline` MAY retain their frozen
spellings, but prose and new APIs MUST use the canonical terms.

## 2. Product Contract

The product is a self-contained pure-Rust renderer of local Excalidraw scene
documents. It produces scene exports and frame exports as static SVG and PNG
with format-specific perceptual parity to the pinned reference renderer, plus
vector-first PDF whose visual authority is the official SVG reference evidence.

The production library MUST:

- Be synchronous, byte-oriented, filesystem-free, network-free, and WASM-safe.
- Require no JavaScript, V8, Node, browser, OS service, system package, installed
  font, or runtime asset download.
- Treat every scene document, supplemental asset, nested SVG, and render option
  as untrusted input.
- Produce artifact byte stability for identical scene-document bytes, request,
  renderer release, and compatibility revision across all supported targets.
- Return explicit compatibility diagnostics for every declared repair,
  substitution, omission, fallback, or compatibility exception.
- Preserve understood content through best-effort rendering whenever the result
  remains a meaningful artifact.

The following are outside this specification:

- Browser bindings, web APIs, web UI, npm packages, JavaScript glue, a stable
  C-style WASM ABI, WASI, or a published raw WASM module.
- Interactive editor overlays, viewport state, collaboration, and editing.
- Production JavaScript, browser, network, filesystem-resource, or system-font
  dependencies.
- Ingestion of exported-image metadata, clipboard payloads, shared links,
  libraries, or any input other than a scene document.
- Pixel identity with the reference renderer or official SVG DOM identity.
- Multi-page, tagged, PDF/A, PDF/X, CMYK, spot-color, or print-production PDF.
- Installers, code signing, notarization, SBOM policy, or additional targets.

## 3. Compatibility Identity

Compatibility revision 1 is bound to official Excalidraw `v0.18.1`:

- Source commit: `a2ec2889babf7d2295469c6d90ebe77fae57df84`.
- npm integrity:
  `sha512-6i5Gt7IDTOH//qa0Z315Ly5iVRhjWpu2whrlQFqkuwrkKUWgRsMk0P5qdE7bpyDpai7jeLeWYkyj1eVAfni1lw==`.
- Reference-profile digest:
  `sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71`.

The source commit and npm integrity MUST both match. A material mismatch blocks
adoption. Excalidraw envelope version `2` is not an Excalidraw release or a
compatibility revision.

A compatibility revision is `draft` until the first package containing that
revision is published. A draft revision has no released compatibility promise
and MAY change through ordinary reviewed specification, manifest, evidence, and
implementation updates. First publication is the immutability boundary: the
published revision becomes `retained`, and its valid-request artifact bytes,
diagnostics, failures, profile identity, asset policy, and semantic resource
traces MUST NOT change. A contract-relevant change after that boundary creates a
new monotonic revision. Revision 1 remains draft because no package containing it
has been published.

### 3.1 Normative Artifacts

| Artifact | SHA-256 |
| --- | --- |
| [Reference profile](../tests/oracle/reference-profiles/revision-1.json) | `1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71` |
| [Reference asset manifest](../tests/oracle/reference-profiles/revision-1-assets.json) | `02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a` |
| [Scene-surface manifest](../tests/oracle/scene-surfaces/revision-1.json) | `c2beac04b6e5973093db128f57a3ad8a6cf3222faa583315b61baddb9bad43c8` |
| [Scene-surface schema](../tests/oracle/scene-surfaces/scene-surface-v1.schema.json) | `29871fad7bf7665142c8d625c3715b34603a6369a78ffa7baf96ade66be63362` |
| [Scene-surface evidence index](../tests/oracle/scene-surfaces/evidence/revision-1-harmless-fields.json) | `29984c198c9cb222d2aedbcca053e1829743ad686a09a266fdcdfdb192545aeb` |

Where present, adjacent `.sha256` files contain the authoritative lowercase
digest. The schema digest is fixed by this table and the repository verifier;
it has no adjacent sidecar. The JSON artifacts are exact canonical bytes.
Repository-relative links and digests are authoritative; access-scoped GitHub
URLs and schema `$id` URLs are not.

The scene-surface manifest freezes 202 semantic-path dispositions, every
revision-1 element kind, all 42 compatibility diagnostic codes, seven
normalization rules, and five content-addressed official fallback recipes. Its
coverage status MUST remain `complete` with an empty unresolved set.

### 3.2 Reference Profile

The reference profile MUST immutably bind every evidence-affecting input,
including Excalidraw identities, npm lock, rendering dependencies, official
assets and fonts, wrapper, browser bundle, invocation files, browser settings,
and host settings.

The frozen dependency identities include Rough.js `4.6.4`, Perfect Freehand
`1.2.0`, points-on-curve `1.0.1`, fractional-indexing `3.2.0`,
`@braintree/sanitize-url` `6.0.2`, and pako `2.0.3`.

The development reference renderer uses React and React DOM `19.2.3`, Vite
`8.1.5` with a production Oxc bundle targeting Chrome 149, Playwright `1.61.0`,
Node `24.16.0`, npm `11.13.0`, and Chromium `149.0.7827.55`. The canonical host
is Ubuntu 24.04 `linux/amd64`, fontconfig `2.15.0`, in Playwright image
`sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9`.

Browser execution MUST use headless software rendering, disabled GPU and font
hinting, forced sRGB, locale `en-US`, timezone `UTC`, device-pixel ratio `1`,
light color scheme, viewport `1280x720`, one worker, blocked service workers,
and origin `http://reference.invalid/`.

Profile generation and rendering MUST run with container networking disabled.
Browser routing MUST admit only that origin, the bound bundle, and
manifest-listed assets. The profile MUST locally serve and hash all 230 official
rendering WOFF2 files and 48 fontconfig-visible container font files. The four
Assistant UI faces remain excluded.

Every scene-export evidence invocation MUST explicitly provide background inclusion,
background color, padding, dark mode, embedded-scene choice, all four frame
controls, and exactly one sizing mode. `quality` MUST be omitted. Live
embeddables MUST be off, SVG font inlining and image reuse MUST be on, and
separate-frame export MUST be off. Elements MUST already have stable identity,
ordering, seed, version, nonce, and update fields before the official restore
path processes them.

Frame exports use a target-scoped revision-1 reference profile that inherits the
pinned release, browser environment, and asset identities while binding a
separate frame-capable wrapper and invocation. The scene reference profile and
its digest remain immutable. A frame invocation supplies one live `frame` or
`magicframe` source occurrence as `exportingFrame`, forces zero padding, frame
rendering enabled, frame names and outlines disabled, and uses the reference
renderer's format-specific clipping behavior.

## 4. Scene Admission and Preservation

### 4.1 Admission

`SceneDocument::parse` MUST admit exactly a JSON object with required own
members:

- `type` equal to `"excalidraw"`.
- `version` represented by the integer token `2`.
- `elements` as an array.
- `appState` as an object.
- `files` as an object.

`appState` and `files` MAY be empty but MUST NOT be absent or null. `source` and
all other root members are permitted and preserved. Bare arrays, scalars, null,
partial envelopes, clipboard or library payloads, another type, another envelope
version, or a wrong required container type MUST fail with
`scene.invalid-envelope`. Duplicate object members MUST be rejected.

After the envelope passes, parsing MUST remain compatibility-revision-neutral.
It MUST retain every occurrence, including scalar element entries, malformed
file records, unknown fields, deleted elements, and unreferenced files. It MUST
NOT validate nested rendering fields or apply compatibility normalization.

Every `elements` member counts as a source element occurrence. Every `files`
member counts as a scene asset entry. A future envelope version MUST NOT be
guessed into version 2.

### 4.2 Canonicalized Original Scene Document

Optional embedding MUST serialize the retained parse representation, never the
compatibility-normalized rendering view. It MUST preserve:

- Every admitted root, app-state, element, file-record, nested, unknown, deleted,
  malformed, unsupported, and unreferenced value.
- Array order and object-member source order.
- Exact token bytes of every valid JSON number.

It MUST exclude supplemental assets, resolved data, generated defaults,
repairs, compatibility placeholders, rendering-view changes, source whitespace,
the BOM, and original string-escape spelling.

Serialization MUST be minified UTF-8 without BOM or trailing newline. Strings
represent Unicode scalar sequences; escaped lone UTF-16 surrogates fail as
`scene.invalid-json`. Quote and backslash are escaped. Controls use the short
JSON escapes where available and lowercase `\u00xx` otherwise. Solidus,
non-ASCII scalars, U+2028, and U+2029 are emitted directly. Number tokens are
copied exactly without floating-point conversion.

The embedded-scene digest is SHA-256 over these exact uncompressed bytes,
spelled `sha256:<64 lowercase hex>`.

## 5. Compatibility Normalization

Compatibility normalization MUST reproduce deterministic rendering-affecting
behavior of the official public restore path, including migrations, defaults,
negative-dimension normalization, line and arrow point normalization, image and
frame defaults, and fractional-index repair in array order. It MUST NOT reproduce
random or current-time repair behavior.

The following deterministic repairs apply:

- Derive a missing element ID from canonical original element content and array
  position.
- Retain the first duplicate ID, deterministically rekey later occurrences, and
  resolve ambiguous references to the first occurrence.
- Use canonical zero for missing required timestamps.
- Replace Rough.js seed `0` with a nonzero value derived from compatibility
  revision and normalized identity.
- Detect relationship cycles in canonical source order and remove the edge that
  closes a recoverable cycle.

Every repair emits its declared source-located compatibility diagnostic.
Repairs affect only the rendering view.

### 5.1 Forward Compatibility

Field/value interpretation MUST use the frozen scene-surface manifest by
semantic record kind and path, never property-name heuristics. Callers cannot
extend or override it.

- Supported fields use their declared behavior.
- Static-export-irrelevant fields and opaque metadata are preserved silently.
- An unmatched property on a participating record is preserved in the
  canonicalized original scene document, ignored in the rendering view, and
  diagnosed as `compatibility.unsupported-property`.
- A known property with a wrong type, malformed recognized structure, or
  unsupported value not repaired by normalization is diagnosed as
  `compatibility.unsupported-property-value` at the narrowest pointer and uses
  the manifest fallback.

Root and application-state records participate globally. A non-deleted known
element participates when traversed. A file record participates only when
authoritative and referenced. Deleted elements and ignored or unreferenced file
records remain silent.

Diagnostic boundaries MUST prevent floods: one unknown property covers its
descendants; unknown children of a recognized object remain separate; a wholly
unknown element emits only its unsupported-element occurrence; and a wholly
malformed record emits only its malformed-record occurrence.

A malformed element with safe finite bounds uses the malformed-element visual
grammar. Without recoverable bounds it is omitted with the same warning. A
referenced malformed file record uses the format-appropriate resource fallback;
an unreferenced malformed file record remains silent.

A disposition change that alters compatibility diagnostics requires a new
compatibility revision. A field can become silent only after reviewed primary
source analysis and focused SVG and PNG reference evidence.

## 6. Public Rust API

The supported facade MUST have this topology:

```rust
SceneDocument::parse(&[u8]) -> Result<SceneDocument, SceneParseError>

Renderer::new() -> Renderer
Renderer::for_revision(CompatibilityRevision)
    -> Result<Renderer, UnsupportedRevisionError>
Renderer::supported_revisions() -> &'static [CompatibilityRevision]
Renderer::profile_identity(CompatibilityRevision)
    -> Result<&'static ReferenceProfileIdentity, UnsupportedRevisionError>
Renderer::selected_profile_identity(&self) -> &'static ReferenceProfileIdentity
Renderer::render(&self, &SceneDocument, &RenderRequest)
    -> Result<RenderOutcome, RenderError>
Renderer::render_frame(&self, FrameRef, &FrameRenderRequest)
    -> Result<RenderOutcome, RenderError>
Renderer::render_frames(&self, &SceneDocument, FrameRefs, &FrameRenderRequest)
    -> Result<FrameRenderCursor, FrameBatchError>

RenderRequest::builder(OutputFormat) -> RenderRequestBuilder
RenderRequestBuilder::build() -> Result<RenderRequest, RequestError>
FrameRenderRequest::builder(OutputFormat) -> FrameRenderRequestBuilder
FrameRenderRequestBuilder::build() -> Result<FrameRenderRequest, RequestError>
```

`SceneDocument` is opaque, immutable, cloneable, and `Send + Sync`.
`Renderer` is immutable, cloneable, and `Send + Sync`; each render creates a
fresh request-scoped session. Public structs have private fields and read-only
accessors. Evolvable enums and string-backed code enums are non-exhaustive.
Requests are constructible only through validated builders.

Every `Renderer` exposes its selected `ReferenceProfileIdentity` through a
read-only accessor. A successful outcome exposes read-only access to its
artifact, immutable ordered report, selected revision, Excalidraw release, and
profile digest. `RenderError` exposes the complete ordered report accumulated
before failure; pre-render typed failures expose no report.

`CompatibilityRevision` is a public string/number-parseable newtype with named
constants. `ReferenceProfileIdentity` exposes compatibility revision,
Excalidraw release, and canonical profile digest. `Renderer::new()` selects the
documented package default, which is compatibility revision `1` in the first
release. A named package-default constant is public and callers do not infer it
from list order. Every revision retained in the semver-major line
remains selectable. Exact reproduction requires both package release and
compatibility revision; a profile digest is discoverable but not a selector.

One target request produces exactly one `OutputFormat`: `Svg`, `Png`, or `Pdf`, spelled
`svg`, `png`, and `pdf` in serialized forms.

`SceneDocument::frames()` discovers every live ordinary and magic frame source
occurrence in source order. A borrowed `FrameRef` exposes its one-based frame
ordinal, kind, and optional source string name while keeping source occurrence
identity opaque. Frame requests expose background, sizing, color, metadata,
supplemental assets, resource limits, and PDF controls, but expose neither
padding nor whole-scene frame controls.

The lazy frame cursor validates one-scene ownership, rejects duplicate targets,
sorts targets by frame ordinal, permits an empty target list, and yields one
complete target result at a time without stopping after a target error. Scene-
global diagnostics are exposed separately from target diagnostics. Every frame
has an independent semantic resource ledger; physical cache sharing MUST NOT
change target bytes, reports, first crossings, or failures.

| Request setting | Contract |
| --- | --- |
| `include_background(bool)` | Default `true` |
| `padding(f64)` | Default `10`; finite and nonnegative |
| `sizing(Sizing)` | Exactly `Scale(f64)` or `MaxDimension(f64)`; finite and positive |
| `color_mode(ColorMode)` | `Light` or `Dark`; default `Light` |
| `frame_options(FrameOptions)` | `enabled`, `clip`, `name`, and `outline`; all default `true` |
| `metadata_options(MetadataOptions)` | `embedded_scene`, `provenance`, and `diagnostic_summary`; all default `false` |
| `supplemental_assets(SupplementalAssets)` | Owned bytes only |
| `resource_limits(ResourceLimits)` | Validated semantic limits |
| `pdf_options(PdfOptions)` | PDF-only, including raster-fallback policy |

Format options for another format are rejected. The API exposes no generic
option map, callback, async resolver, path, URL, writer, backend extension point,
live-embeddable control, external-font control, or font-inlining opt-out.
Mandatory notices are not optional metadata.

`SupplementalAssets` is an owned deterministic map from nonempty scene file ID
to bytes plus an optional advisory media type. Duplicate IDs are request errors.
The renderer sniffs and validates bytes. A supplemental entry fills only an ID
absent from the scene document. A conflicting scene payload remains
authoritative and emits `asset.supplement-conflict`; a claimed type mismatch
emits `asset.media-type-mismatch`.

## 7. CLI Contract

The command is:

```text
excalidraw-render [OPTIONS] <INPUT> --output <OUTPUT>
```

`INPUT` and `-o, --output` can be `-` for stdin and artifact stdout. Supplemental assets
are paths and never consume stdin. `-f, --format <svg|png|pdf>` MAY be omitted
only when a path output has a recognized case-insensitive extension. It is
required for stdout and unknown extensions. An explicit mismatch is a usage
error.

The render controls are exactly:

```text
--compatibility-revision <N>
--no-background
--padding <F64>
--scale <F64>
--max-dimension <F64>
--dark-mode
--embed-scene
--embed-provenance
--embed-diagnostics
--no-frame-rendering
--no-frame-clipping
--no-frame-names
--no-frame-outlines
--asset <FILE_ID>=<PATH>
--asset-mime <FILE_ID>=<MEDIA_TYPE>
--pdf-raster-fallback <allow-isolated|forbid>
--report-format <human|json|none>
--report-output <stderr|PATH>
--force
--list-revisions
```

`--scale` and `--max-dimension` are mutually exclusive. Report format defaults
to `human`; report output defaults to stderr. Asset options split on the first
`=` and reject empty or duplicate values. Every MIME declaration has exactly one
matching asset. The CLI has no resource override, preset, config file,
environment override, hidden override, schema selector, backend selector, or
extension map. Every render uses `ResourceLimits::default()`.

The frame export command is:

```text
excalidraw-render frames [OPTIONS] <INPUT> --output <DIRECTORY> --format <svg|png|pdf>
excalidraw-render frames [OPTIONS] <INPUT> --list
```

Frame artifact output is always a directory and cannot be `-`; input may still
be stdin. The exact token `frames` is reserved as a subcommand, while a scene
file with that basename remains addressable as `./frames` or after `--`. The
frame command reuses applicable background, sizing, color, metadata, asset, PDF,
report, revision, and force controls. It does not accept padding or whole-scene
frame controls. Its additional controls are one `--name-regex <REGEX>`, positive
`--max-frames <N>` up to 16384 with default 256, `--allow-partial`, and
`--list`.

`--list` is a dry run. `--output` and `--format` are required unless `--list` is
present. It applies the same selection an export would apply — name filter,
selected-frame cap, source ordering, and derived basenames — then reports
ordinal, kind, source name, and derived output basename without rendering,
reading supplemental asset bytes, creating the output directory, or writing any
artifact. It is exempt from performing those actions, not from validating them:
failure-ordering steps 1 through 5 apply unchanged, so request values and asset
declarations an export would reject are rejected here too. Without `--format`,
format-independent request values are still validated. With `--format` the basename carries its extension; without it, the
basename is reported as a stem. An empty selection is a valid answer and exits
`0`, unlike an export, which fails `cli.frame-no-match`. Exceeding
`--max-frames` still fails `cli.frame-limit-exceeded`. `--list` does not accept
`--report-format none`. The listing is delivered on the report channel: human
mode emits one tab-separated line per frame; JSON mode emits one schema-1
document whose top-level member order is `schema_version`, `status`,
`renderer_version`, `compatibility_revision`, `excalidraw_release`,
`reference_profile_digest`, `mode`, `frames`, with `mode` fixed to `list` and
each entry ordered `ordinal`, `kind`, `name`, `output_basename`.

The optional name regex uses Unicode-aware linear-time Rust regex search,
case-sensitive by default. It is applied only to nonempty source string names.
Selection enumerates live frames, filters names, fails with exit 1 on no match,
enforces the selected-frame cap, then derives source-ordered portable filenames.
The basename is an NFC-normalized Unicode letter/number slug or `frame-N` using
the unfiltered frame ordinal. Reserved device names gain `_`; components are at
most 120 UTF-8 bytes with an eight-hex SHA-256 truncation suffix; normalized
case-insensitive collisions gain `-2`, `-3`, and later suffixes.

A missing output directory is created only for publication. Existing
directories and unrelated entries are preserved. Existing selected destinations
fail unless `--force`, which replaces only selected paths. Default mode evaluates
all targets and stages all successful artifacts before publication; any handled
preflight, render, or staging failure publishes none. Multi-path commit failure
is reported honestly as possibly partial. `--allow-partial` instead publishes
independent successes after frame-local render, collision, staging, or delivery
failures and exits 1 when any target failed.

Frame JSON reporting uses a dedicated schema-1 batch envelope with frame profile
identity, format, overall and delivery status, scene-global diagnostics once,
and one ordinal-ordered entry per selected frame. Each entry contains target
identity, source name, intended path, artifact descriptor, target diagnostics,
error, and delivery state. Its top-level member order is `schema_version`,
`status`, `renderer_version`, `compatibility_revision`, `excalidraw_release`,
`reference_profile_digest`, `format`, `delivery_state`, `diagnostics`, `frames`.
Frame-entry member order is `ordinal`, `kind`, `name`, `output_path`, `artifact`,
`error`, `diagnostics`, `delivery_state`. Human clean success remains silent.

The CLI selects one failure in this order:

1. Deterministic report-mode preparse.
2. Command grammar and standalone-mode validation.
3. Compatibility-revision selection.
4. Explicit or inferred format selection.
5. Request values and asset declarations.
6. Path-conflict validation.
7. Existing output and report checks.
8. Bounded scene read and parse.
9. Supplemental reads by ascending file-ID UTF-8 bytes.
10. Render.
11. Artifact staging and delivery.
12. Report staging and delivery.

The report-mode preparser recognizes exactly one valid `--report-format json` or
`--report-format=json`. Missing, malformed, duplicate, or conflicting choices
remain human usage errors.

`--report-output` with `--report-format none` is `cli.invalid-arguments`.

Output and report paths MUST NOT alias input or supplemental assets, and output
and report MUST differ. Existing paths use filesystem identity; nonexistent
paths use normalized absolute lexical paths. `--force` permits replacing both
destinations but never overrides conflicts.

Both requested path outputs are fully staged in their destination directories
before either is delivered and are atomically replaced where supported. A
path-delivery failure leaves no new partial destination. The artifact is
delivered before the report. A report failure after artifact
delivery is still command failure and records `artifact_state: "complete"`.
Stdout write failure uses `possibly_partial` only if bytes may have escaped.

If JSON report-file delivery fails and stderr is not the failed channel, the CLI
emits one complete canonical `cli.report-write-failed` schema-1 envelope with
the accumulated diagnostics to emergency stderr and one LF. Human and `none`
mode emit one non-JSON stderr line. If stderr is the failed channel or its write
also fails, exit 1 without claiming successful reporting or trying another
channel. The failure envelope has top-level `artifact: null`; delivery details
record whether artifact bytes were already delivered.

Human mode prints all diagnostics for a diagnosed success and nothing for clean
success. JSON mode always emits one report document. `none` suppresses successful
reporting and emits one non-JSON family/code/message line on failure.

| Exit | Meaning |
| ---: | --- |
| `0` | Complete artifact delivered, including diagnosed or metadata-omitted success |
| `1` | I/O, bounded-read, parse, fatal render/resource, allocation, or delivery failure |
| `2` | Grammar, request, format, duplicate-ID, or unsupported-revision usage error |

`--list-revisions` accepts only human or JSON report controls and lists revisions
in increasing order. `--version` is human-only standalone stdout and prints
package version, default revision, Excalidraw release, and profile digest.

## 8. Rendering Semantics

### 8.1 Shared Controls

Defaults are white background included, 10 logical pixels of padding, scale 1,
light mode, optional metadata off, and every frame control on.

Explicit scale preserves logical scene bounds. SVG retains its logical
`viewBox` while multiplying width and height. PNG scales drawing and truncates
scaled pixel dimensions as the reference wrapper does. Maximum dimension is
downscale-only and never upscales. It applies to PNG and PDF; the official SVG
maximum-dimension invocation is a no-transform operation.

Scene-export bounds, paint order, clipping, masking, frames, bound text, links,
dark mode, application state, and embedded files participate according to the
scene-surface manifest.

In a scene export with frame rendering and frame names on, each live frame's
visible name is generated export text laid out as the text element the
reference synthesizes for it rather than as a decoration of the frame:
Helvetica at `FRAME_STYLE.nameFontSize` 14 with `nameLineHeight` 1.25, so a
17.5 px line box whose top sits `nameOffsetY` 3 px above the frame's top edge
at the frame's x, left-aligned, unrotated, fully opaque, unlinked, and never
frame-clipped, whatever the frame's own angle, opacity, link, or parent frame.
The name is positioned by Helvetica's metadata metrics and shaped with
Liberation Sans, colored `nameColorLightTheme` `#999999` or
`nameColorDarkTheme` `#7a7a7a` under the dark-mode filter, and truncated to the
frame's width with the reference's ASCII `...` when its measured advance is
wider; a frame too narrow for one character with the dots keeps its whole name
at the frame's width. The name's line box takes part in the scene bounds exactly
as that text element would. The frame outline is the frame rectangle with
`FRAME_STYLE.radius` 8, stroke `#bbb`, and width 2.

A frame export targets one live ordinary or magic frame and always uses the
rotation-aware frame AABB as its zero-padding viewport. The target frame name and
outline do not paint, even when persisted frame rendering is disabled. Candidate
elements are selected by inclusive rotated-AABB overlap, followed by one-hop
binding closure and immediate membership filtering: unframed elements and
direct target members may participate, while elements assigned to another frame
do not. Nested grandchildren are not recursively owned by an outer frame. SVG
uses the reference rectangular target clip, PNG relies on its cropped canvas,
and PDF applies equivalent vector-first visible semantics.

Frame artifact manifests use schema 2 and add a target block containing kind,
ordinal, and the source name when present. Target identity alone does not force
a manifest. Existing scene manifests remain byte-identical schema 1. Embedded
scene metadata remains the complete canonicalized original scene document.

Element colors use the CSS grammar recognized by the pinned Chromium profile and
serialize as canonical sRGB plus alpha. Invalid colors emit `color.invalid` and
substitute `#1e1e1e` for stroke, transparent for element background, and white
for scene background.

Safe absolute and relative links are preserved under the pinned sanitizer and
fixed origin. Unsafe links are omitted and diagnosed. PNG has no links.
Embeddable and iframe-like elements use the official offline fallback while
preserving a safe link where the format supports one.

### 8.2 Raster and Nested-SVG Assets

Accepted raster families are PNG/APNG, JPEG/JFIF, GIF, WebP, BMP, and ICO.
AVIF is not accepted in revision 1 (#68): an AVIF payload takes the
`image.unsupported-format` path below and no platform decoder or weakened
fallback stands in for it. Stored payload bytes are authoritative; the renderer
MUST NOT replay editor import, Pica, or canvas transformations or infer import
history.

Rendering never resolves network or filesystem resources. Nested SVG is parsed
and sanitized locally before any viewer library. DTDs, external entities,
scripts, event attributes, active animation, `foreignObject`, unsafe actions,
unsupported foreign namespaces, external CSS/fonts, `@import`, and non-fragment
URLs are removed or rejected under the declared diagnostics. Only same-document
fragments and admitted bounded data images can resolve.

Direct SVG embeds validated compressed raster payload octets exactly. It does
not decode, select a frame, apply orientation, transform color, tone-map,
premultiply, strip metadata, or re-encode. Profiles, metadata, source alpha, and
live animation survive silently. Data URLs use the validated canonical media
type and canonical base64 spelling; source lexical spelling is not preserved.

PNG selects the pinned default frame, including ICO's largest-area then
highest-depth rule; applies the format-specific orientation, profile/chunk/CICP
and gain-map precedence, color management, HDR tone mapping, transfer behavior,
and premultiplied-alpha compositing; and writes deterministic untagged sRGB
RGBA8 with RGB zero wherever alpha is zero. Source bytes, profiles, metadata, animation, and source alpha
representation are discarded silently.

PNG and PDF emit one informational `image.animation-flattened` occurrence for
each actually rendered multi-frame asset occurrence. Static, one-frame, and
unreferenced assets remain silent. PDF uses the same default-frame, orientation,
and color interpretation and embeds lossless unpremultiplied canonical sRGB with
a grayscale soft mask. Source raster images are inherent raster content, not
PDF raster fallback.

A missing local image is omitted in SVG and PDF, uses the official image/error
fallback in PNG, and is diagnosed in every selected format. Other invalid,
forbidden, unsupported, or exhausted resources use the format-specific recipe
and diagnostics in the scene-surface manifest.

### 8.3 Typography

Direct scene and frame SVG exports embed request-derived subsets of required
official Unicode-range WOFF2 chunks under the contract below. PNG and PDF font
artifacts, typography, diagnostics, failures, notices, and resource ledgers are
unchanged. Text and fonts inside sanitized nested SVG are unchanged and do not
participate in direct-export subsetting. PNG uses deterministic shaping and
controlled glyph-outline rasterization. No format consults installed fonts or
OS text services.

Liberation Sans and Noto Emoji replace local-only Helvetica/Arial and Segoe UI
Emoji behavior for user text and generated labels, with one informational
substitution diagnostic per affected occurrence. Missing codepoints use the
bundled deterministic replacement glyph at defined metrics and identify the
element and codepoint.

Ordinary scene text and generated embeddable labels take their baselines from
the source family's metadata metrics: the `unitsPerEm`, `ascender`, and
`descender` the pinned Excalidraw release declares for that family, which
coincide with every bundled face's `hhea` table. Substituted Helvetica text
(`fontFamily` `2`) keeps Helvetica's declared metrics (`2048`, `1577`, `-471`)
while Liberation Sans shapes its glyphs, as the official export does; the face
that shapes the glyphs never positions them.

A text element whose `lineHeight` is missing or non-positive takes the pinned
restore path's derivation rather than one constant. When the
compatibility-normalized `height` is positive, `lineHeight` is `height /
lineCount / fontSize`, where `lineCount` counts the lines of the wrapped `text`
after EOL normalization (`\r\n` and `\r` become `\n`; tab expansion adds no
lines) and `fontSize` is the compatibility-normalized font size, so a missing
or invalid `fontSize` detects with `20` rather than reproducing the reference's
`NaN`. Otherwise it is the source family's metadata line height: Excalifont,
Virgil, and Comic Shanns `1.25`; Helvetica, Lilita One, Liberation Sans, and
Xiaolai `1.15`; Cascadia `1.2`; Nunito `1.35`, keyed on the source family so
Helvetica keeps `1.15` while Liberation Sans shapes it. An unrecognized family
has already normalized to Excalifont. A `lineHeight` of `0` or a negative value
is unusable and derives the same way, keeping its unsupported-value diagnostic;
an absent `lineHeight` is silent. Generated embeddable labels have no stored
height and take Helvetica's `1.15`.

Ordinary scene text breaks into lines where the pinned reference does:
`text.replace(/\r\n?/g, "\n").split("\n")`, so `\r\n`, `\r`, and `\n` each end
a line, `\r\n` counts once, a trailing terminator leaves an empty last line, and
empty text is one empty line. Every format lays out that many lines, one line
step apart, and no line terminator reaches the shaper, face selection, or
missing-codepoint reporting. The stored `text` and `originalText` are not
rewritten: the per-line source text a format carries beside its glyphs (the
SVG `data-source-text` attribute, the PDF line-level `ActualText`) keeps each
line's own terminator bytes, so the lines concatenate back to the stored
`text`. This is the same line count the restored line height divides by.

Within each source line, bidirectional text follows UAX #9 as the pinned
`unicode-bidi 0.3.18` implements it (#122): the line's base level is detected
from its first strong character (left-to-right when it has none), and its
resolved levels are reordered into visual runs. Every format lays those runs
out left to right in visual order, and each visual run is split further by the
face that shapes its scalars under 8.3.1, so one visual run of one face is one
glyph run: SVG emits it as one `<tspan>` carrying that face's effective family,
`direction="rtl"` or `"ltr"`, and one `x` and `y` per glyph; PNG and PDF place
the same glyph positions. Within a run, glyph order and placement are the
deterministic shaper's for the run's display text. The text a format carries
beside the glyphs stays in logical order (`data-source-text`, `ActualText`, and
each run's `<tspan>` content), as do missing-codepoint diagnostics. An
occurrence with no right-to-left content whose shaped scalars the whole-text
face all maps is one run of that face per line and emits no direction.

#### 8.3.1 Semantic Demand and Face Selection

The Excalidraw adapter MUST collect first-use-ordered Unicode scalar values from
the compatibility-normalized `originalText` of every live target text occurrence
that survives local recovery, followed by every visibly emitted generated label
in deterministic emission order. Omitted text and omitted labels contribute
nothing. Deduplication preserves first use. Values are grouped by resolved
bundled family; source family IDs `2` and `9` resolve to the same Liberation
family demand, while occurrence diagnostics remain source-ID-specific.

The revision binds an exact CJK scalar table equivalent to the pinned Chromium
149 predicate. The CJK subset trigger is evaluated once per target, before
layout and before any local recovery, over the stored `text` of every live
Excalifont text occurrence (#122). The stored `text` is the trigger's
format-neutral carrier: compatibility normalization keeps `originalText` only
for SVG, a well-formed scene's wrapped `text` keeps every non-whitespace scalar
of its `originalText`, and the table contains no whitespace scalar. When any
such scalar belongs to the table, Xiaolai is selected for the target: glyph
face selection in every format extends every Excalifont occurrence's fallback
chain with Xiaolai, and the SVG demand copies the complete Excalifont scalar
set to Xiaolai and prepends Xiaolai to resolved family order. That one outcome
is shared and never re-derived from survivors, so an occurrence that local
recovery later omits still counts toward it, the faces glyph runs use and the
faces the SVG declares cannot disagree, and a Xiaolai chunk's own coverage of a
scalar never selects Xiaolai. The table bytes, digest, and selected-codepoint
classes MUST be frozen in the compatibility asset manifest before release.

For each resolved family in canonical fallback order, `svg_fonts` compares every
unique scalar with every official face's declared Unicode range and selects each
intersecting WOFF2 chunk in pinned face order. Every selected chunk receives the
complete ordered family scalar set, not only its range intersection. A used
non-WOFF2 fallback, including Noto Emoji, remains complete and is emitted once;
it is never subset. Only qualified, digest-verified built-in assets participate.
Custom fonts and a public subset interface are forbidden.

Glyph face selection is the same in every format, for ordinary scene text and
generated embeddable labels alike, and consults declared Unicode ranges before
any `cmap`. A resolved family's whole-text face is fixed once per target, when
the family's first live occurrence is laid out: the first official font chunk
of the family, in pinned face order, whose declared ranges cover every shaped
scalar of that occurrence and whose `cmap` maps them all; failing that, the
family's first chunk declaring and mapping `?` and `A`. Every later occurrence
of the family shapes with that face. A shaped scalar the whole-text face maps
takes it. Any other shaped scalar consults, in this order and stopping at the
first face whose `cmap` maps it: each official font chunk of the family whose
declared range covers the scalar, in pinned face order; for Excalifont when the
trigger selected Xiaolai, each Xiaolai chunk likewise; the complete Noto Emoji
face; and finally the Liberation Sans chunk declaring `?`, which shapes the
replacement glyph `?` and reports `font.missing-glyph` for the scalar. A chunk
whose declared range does not cover the scalar is never consulted, so its
`cmap` never selects it.

Each of those consultations is one font fallback probe, the event 13.2 charges
32 work units (#122): one for each consulted chunk of the family and, when
selected, of Xiaolai, one for the Noto Emoji consultation, and one for the
replacement. Every shaped scalar occurrence pays its own probes, repeated
scalars included. Once the family's whole-text face is fixed, whether a
consulted face already serves the target changes neither the count nor the
order; and neither the whole-text face's initial `cmap` test nor a
declared-range test is a probe. So an untriggered Excalifont scalar that no
Excalifont chunk declares and Noto Emoji lacks pays two probes; a triggered
Excalifont scalar that the one Xiaolai chunk declaring it maps pays one; an
emoji Noto Emoji maps pays one; a Liberation Sans scalar its one chunk declares
but does not map pays three.

#### 8.3.2 Subset Profile, Names, and Identity

Subset output is renderer-owned cross-target bytes with semantic parity; it is
not required or permitted to claim Excalidraw WOFF2 byte identity. The revision
MUST pin the complete profile and its digest. The profile is HarfBuzz-like: keep
all layout scripts and features, ordinary layout and TrueType composite closure,
hinting, and glyph renumbering; do not pin variation axes. Drop `morx`, `mort`,
`kerx`, legacy `kern`, `JSTF`, `DSIG`, `EBDT`, `EBLC`, `EBSC`, `SVG `, `PCLT`,
`LTSH`, and Graphite tables. Pass through `gasp`, `fpgm`, `prep`, and `VDMX`.
Clear `OVERLAP_SIMPLE` while rebuilding canonical `glyf`/`loca`; this is an
explicit profile normalization applied before WOFF2 encoding, not an encoder
side effect. When subsetting would otherwise produce a zero-length `glyf`
table, encode an outline-free simple `.notdef` glyph so Chromium OTS admits the
font without changing any selected visible glyph.
Every other table occurring in the sealed font corpus MUST have an explicit
keep, subset, rewrite, pass-through, or drop classification in a frozen manifest;
dependency defaults and unknown-table defaults MUST NOT decide output.

Retain English name records for IDs `0` through `6`, `13`, and `14`. Copyright,
version, license description, and license URL remain semantically unchanged.
Family, full, unique, and PostScript identities are rewritten deterministically.
An OFL subset is a Modified Version. Reserved Font Name families use these CSS
aliases and primary OpenType family names:

| Source family | Required alias |
| --- | --- |
| Virgil | `ExcalidrawRender-1` |
| Cascadia Code | `ExcalidrawRender-3` |
| Lilita | `ExcalidrawRender-7` |
| Liberation | `ExcalidrawRender-9` |

The licensing and compatibility rationale is recorded in
[ADR 0001](adr/0001-treat-font-subsets-as-modified-versions.md).

Non-Reserved-Font-Name families retain their canonical family names. The same
effective alias MUST be used by SVG text, CSS, and primary OpenType names.

Subset semantic identity is SHA-256 over this exact preimage, where digests are
their 32 raw bytes and asset identity is UTF-8:

```text
UTF-8("excalidraw-render:svg-font-subset:v1")
NUL
u64-le(compatibility revision)
full 32-byte subset-profile digest
u64-le(source asset identity byte length)
source asset identity
source asset digest
u64-le(unique scalar count)
u32-le(first scalar) ... u32-le(last scalar)
```

The full 64-digit lowercase digest appears in the mandatory subset notice and
name ID `3` as `excalidraw-render;<effective-alias>;<full-digest>`. Name ID `1`
is the effective alias, ID `2` is `Regular`, ID `4` is
`<effective-alias> Regular <digest-prefix>`, and ID `6` is
`<postscript-alias>-Regular-<digest-prefix>`. `digest-prefix` is the first 128
digest bits as 32 lowercase hexadecimal digits; `postscript-alias` replaces each
ASCII space in the effective alias with `-`. IDs `0`, `5`, `13`, and `14` retain
their approved source text. Identity is computed and semantically deduplicated
before decode or subset work.

#### 8.3.3 Derivation and Validation

The decoder is pinned `wuff 0.2.8`. The sfnt subset adapter MUST first qualify
`skera 0.5.1`; otherwise a pinned controlled fork is required. The encoder MUST
first qualify `ttf2woff2 0.13.2`; otherwise a pinned controlled fork is required.
WOFF2 encoding uses quality 11, `glyf`/`loca` transformation, one thread, and no
metadata or private block. Families and faces are processed sequentially in
canonical order.

Every generated WOFF2 MUST be decoded and parsed before use. Validation checks
container and sfnt structure, checksums, the frozen table policy, required
`cmap`, layout and composite closure, metrics, names, and legal records. It also
requires byte equality after the generated sfnt and WOFF2-decoded sfnt are each
canonicalized by the same renderer-owned serializer to tag-sorted tables,
recomputed table and font checksums, canonical `head.checkSumAdjustment`, and
zero alignment padding. Raw sfnt reconstruction bytes need not be identical.
Any decode, plan, sfnt construction, encode, or validation failure is fatal
`encode.svg-font-subset-failed`, with stage, resolved family, and source asset
identity. There is no complete-chunk fallback, partial family, partial SVG, or
success diagnostic.

### 8.4 SVG

SVG MUST be semantic and self-contained. It preserves selectable text, safe
anchors, logical transforms and bounds, clipping, masks, frame appearance,
paint order, accepted images, required fonts, deterministic IDs, notices,
metadata, and deterministic serialization.

`<defs>` contains exactly one font style element, `<style class="style-fonts">`,
when direct-export fonts are used. It contains one `@font-face` declaration for
each selected subset chunk and used unchanged fallback in canonical
fallback/family/face order. A declaration contains only the effective
`font-family` and data-URL `src`; selected subsets use WOFF2 and unchanged
fallbacks retain their manifested format. Range and source-family properties are
not emitted. CSS, data URL, base64, declaration, and style serialization are
canonical. All selected chunks and used unchanged fallbacks share this one style
block; no second font style element is emitted. The artifact manifest carries
one complete applicable OFL or MIT license text per canonical license identity
and distinct mandatory subset notices in canonical subset-identity order.

Before the style element, `<defs>` defines each referenced frame clip once, in
first-use order, as a `<clipPath>` holding the frame rectangle with the frame's
transform and, in a scene export, `FRAME_STYLE.radius` corners; the rectangular
frame-export clip has none. Every clipped element is wrapped in one
`<g clip-path>` referencing its frame's definition, so a frame's clip is never
repeated per child. Clip IDs are renderer-owned and keyed by the frame's index
in the source `elements` array; an element id is untrusted input and is not
used as an XML ID. The clip rectangle rotates by the frame's angle in degrees,
as the outline does; revision 1 does not reproduce the reference's
radians-valued `rotate()` on its clip definition.

Request-scoped and frame-batch physical caches MAY retain qualified source
decodes and identical derived bytes. Each SVG target nevertheless derives the
same semantic demands and replays the same complete semantic resource transcript
as an uncached render; cache hits cannot change bytes, reports, first crossings,
failures, or release points.

An owned typed serializer controls namespace and attribute order, number
formatting, IDs, embedded assets, escaping, links, and bytes. A backend tree
followed by canonicalization is not permitted.

### 8.5 PNG

PNG MUST execute its canvas-equivalent typed plan directly. Serialized SVG is
never its rendering oracle. Controlled adapters own paths, glyph outlines,
images, masks, clips, compositing, dark mode, dimensions, color type, filters,
compression, chunk order, metadata order, and exclusion of host/time/random
data.

### 8.6 Vector-First PDF

PDF is a direct semantic projection, not a serialized-SVG pivot. Official SVG
reference evidence is the transitive visual authority for geometry, paint order,
frames, clipping, text, links, and images. Official PNG is not authoritative
where formats differ, and no official PDF reference evidence is implied.

When an approved compatibility exception intentionally changes SVG appearance,
PDF authority is official SVG reference evidence plus that exception's exact
reviewed delta. Its crops, expectations, and ceiling apply independently to
candidate SVG and PDF. Outside the declared delta both retain the ordinary SVG
gate. Candidate SVG and renderer-owned snapshots never become reference
evidence.

Each PDF has one page. One logical pixel maps to `0.75` points. Padding precedes
sizing. `Scale` scales page and content; `MaxDimension` is downscale-only.
`MediaBox` and `CropBox` equal final bounds at `(0,0)`; no other page boxes,
rotation, or `UserUnit` are emitted. Included background paints the full page
first; excluded background emits no page paint.

Apply only these pinned SVG reductions in local logical units before transforms,
sizing, and the 0.75-point conversion:

- Rough path operation numbers use `fixedDecimalPlaceDigits = 2`.
- Freedraw uses the exact `getSvgPathFromStroke` two-decimal token-trimming
  algorithm; generic decimal rounding cannot replace it.
- Image viewport width and height use official SVG integer rounding exactly
  where the reference renderer applies it. Crop offsets, transforms, clips,
  corners, and other image values do not inherit that rounding.

All other geometry retains full `f64`, including text placement, bounds, clips,
masks, transforms, crop geometry, frames, effect regions, and link
quadrilaterals. Reductions occur in local coordinates; transforms and sizing
follow, then point conversion. PDF numbers use deterministic non-exponential
round-trip decimals, strip redundant zeros, serialize negative zero as `0`, and
add no second quantization.

Compatibility revision, not backend capability, owns this closed vector matrix.
Every project-owned operation remains native PDF content:

- Move, line, quadratic, cubic, and close segments, with deterministic lowering
  of quadratics and arcs to cubics where required.
- ICC-sRGB solid fills and strokes with alpha, nonzero/even-odd fill, caps,
  joins, miter limits, dash arrays, and dash offsets.
- Affine transforms, nested vector clips, alpha and luminosity soft masks,
  source-over isolated transparency groups, and ordinary opacity.
- Rough strokes/fills, freedraw, arrowheads, frames and frame clipping, image
  and arrow-label masks, generated labels, and compatibility placeholders.
- Shaped scene text and generated labels under the selectable `ToUnicode` and
  `ActualText` contract.
- Safe URI annotations over deterministic transformed bounds.

Dark mode is lowered into vector colors and image pixels before this matrix.
Failure to encode a required operation is fatal, never fallback. Admitted scene
raster pixels are lossless image XObjects and inherent source content.

After deterministic expansion and lowering, sanitized nested SVG has exactly
this recursively vector-native subset:

- Containers, nested viewports and `viewBox`/`preserveAspectRatio`,
  `symbol`/`use`, all basic shapes/path commands, and expanded markers.
- Solid paints; linear/radial gradients with stop alpha and spread modes; and
  finite tiling patterns whose referenced content is vector-eligible.
- Affine transforms, opacity, nested clips, and alpha/luminosity masks.
- PDF 1.7 blend modes `Normal`, `Multiply`, `Screen`, `Overlay`, `Darken`,
  `Lighten`, `ColorDodge`, `ColorBurn`, `HardLight`, `SoftLight`, `Difference`,
  `Exclusion`, `Hue`, `Saturation`, `Color`, and `Luminosity`.
- Deterministically resolved nested-SVG text lowered to glyph outlines.
- Admitted nested raster image nodes represented as source image XObjects.

A gradient, pattern, mask, marker, reused subtree, or other resource is
vector-native only when all required content is vector-eligible.

Additional rasterization is allowed only for isolated pixel-domain operations
inside sanitized nested SVG that lack exact PDF representation, such as blur,
convolution, morphology, displacement, turbulence, lighting, post-composite
color transforms, and non-native blends. Ordinary opacity, clips, masks,
gradients, patterns, native blends, text, and source raster images are not
fallback-eligible.

A fallback closure is the smallest isolated subtree containing the innermost
effect, descendants, resources, inputs, intermediates, and required local
backdrop. Dependent closures merge only to their least common required
nested-SVG ancestor. Appearance-equivalent ancestor transforms, clips, opacity,
masks, and native blends remain outside. A closure never crosses the nested-SVG
root. An effect requiring external or page backdrop capture fails as
`pdf.raster-fallback-unisolatable` with no artifact.

Closure bounds include deterministic effect bleed, intersect the viewport and
unchanged ancestor clips, and floor/ceil to the final 96-DPI device grid. The
closure renders transparent canonical sRGB RGBA8 at one pixel per final pixel,
preserves subpixel placement and alpha with a grayscale soft mask, gives a
nonempty subpixel region at least one pixel, and never downscales.

`PdfRasterFallback` has `AllowIsolated` and `Forbid`; the default is
`AllowIsolated`. Complete-plan preflight precedes fallback rendering. `Forbid`
succeeds exactly when no closure remains after lowering, expansion, culling,
and provable no-op elimination. The first remaining closure fails as
`pdf.raster-fallback-forbidden`, with no artifact and no fallback warning.

`AllowIsolated` emits exactly one `pdf.raster-fallback` warning per emitted
closure occurrence, ordered by scene occurrence and expanded nested-tree order.
It identifies the causative node, logical bounds, final pixel dimensions, and
all cause pointers. Deduplicated bytes do not deduplicate diagnostics.

PDF text uses deterministic `rustybuzz` shaping, explicit glyph positions,
stored source lines, the reviewed decodes of official font chunks, deterministic
character codes, complete `ToUnicode`, and line-level `ActualText`. It does not
reflow, outline, rasterize, subset, rename fonts, or consult host services.
Only the reviewed decodes of chunks selected for glyphs actually used are
embedded, each as one complete `FontFile2` stream, byte for byte; the complete
Noto Emoji fallback, when selected, is embedded the same way as its exact
digest-verified manifested bytes (Section 10), which have no decode.

An official font chunk's reviewed decode is the sfnt the pinned `wuff 0.2.8`
decode of the chunk's exact manifested WOFF2 bytes produces (#120). The revision
catalog records every chunk's reviewed decode beside its source digests, as a
decoded length and SHA-256, and the reviewed-decodes record
(`catalog/reviewed-decodes/revision-1.json`) binds all 230 under one maintainer
approval on the Section 14.4 subject pattern: the decoder's name, version, and
invocation; the reference asset manifest and 8.3.3 qualification-corpus
digests; each chunk's source and decoded length and digest in catalog order;
and the decodes that are additionally checked in byte for byte. The review is
the 8.3.3 qualification of that decoder over all 230 chunks on every supported
target, not a per-chunk table comparison; a decode may additionally be reviewed
independently and checked in, as the Excalifont basic-Latin decode is, and then
must equal the recorded decode. `xtask catalog verify` re-derives every decode
from the checked-in shards, requires each to parse and to have the length its
container declares, requires the record and catalog to state exactly those
decodes, and holds the decoder version to the lockfile. The renderer admits a
decode for shaping or embedding only after matching its recorded length and
digest; any other bytes are a sealed-asset failure, never a font. A decoder
change, a moved decode, or a chunk added or removed moves the subject and
requires a new approval.

Every safe linked element generates one borderless URI annotation from
deterministic transformed-bound quadrilaterals, clipped to the page and frame
clips. Empty areas are omitted; overlaps are ordered topmost to bottommost.
Relative links use the reference-profile origin. JavaScript, Launch, file, and
other active actions are forbidden.

Output is PDF 1.7 with one project-generated ICC v2.1 matrix/TRC sRGB profile
and matching output intent. Images and fallback pixels are lossless canonical
sRGB, unpremultiplied, with invisible RGB canonicalized and alpha in a grayscale
soft mask. Source profiles, lossy re-encoding, timestamps, random IDs,
encryption, signatures, incremental revisions, and linearization are forbidden.

Complete bytes are stable through fixed semantic object numbering, classic
xref/trailer, one revision, canonical decimals and zero, fixed stream filters
and Flate settings, and fixed dictionary and metadata order.

PDF backend qualification MUST proceed in this order:

1. First qualify an exactly pinned project-owned controlled fork of
   `svg2pdf 0.13.0` with `pdf-writer 0.12.0` behind the replaceable adapter. The
   typed PDF plan lowers directly into its bounded tree; serialized SVG is
   forbidden.
2. The fork deterministically sorts resources/objects, removes reachable panic
   and `unwrap`, accepts only resolved assets, lowers dark mode first, and
   exposes every required operation. The outer writer owns page, annotations,
   output intent, XMP, attachment, and notices.
3. Admit it only after the conformance spike proves every vector, text, color,
   transparency, metadata, safety, and cross-target byte-stability gate without
   whole-page rasterization or hidden resource access.
4. If satisfying a gate would effectively rewrite `svg2pdf`, direct
   `pdf-writer` over the same plan is mandatory. This mechanical result does not
   reopen the PDF contract.

## 9. Architecture

The implementation MUST use one-way layers:

1. Preserve and parse the scene document.
2. Apply compatibility normalization.
3. Resolve built-in and scene-local assets.
4. Resolve shared semantics and geometry.
5. Build a typed format-specific projection plan.
6. Encode artifact bytes.

Compatibility revision 1 is a draft bundle binding the reference profile,
defaults, normalization, compatibility algorithms, asset catalog, and projection
policies. First publication seals and retains that complete bundle; later
revisions add bundles rather than scatter version checks.

Each render owns a request-scoped session containing semantic resource
accounting, an ordered diagnostic sink, scene-local asset state, and
deterministic insertion-ordered caches. Hidden mutable globals are forbidden.
One scene renders sequentially; independent scenes can render concurrently.
Internal parallelism SHOULD NOT be introduced without demonstrated need.

The shared resolved representation is an immutable paint-ordered semantic graph
that retains source identity/location, frame/group/clip/mask/order relations,
sanitized links and hit geometry, text and stored lines, fonts and placement,
asset provenance/fallback state, styles, opacity, transforms, and resolved
geometry. It contains no backend-native object.

Geometry uses owned target-neutral `f64` points, transforms, bounds, and
move/line/quadratic/cubic/close paths. Operation order is deterministic,
target-specific fast paths are forbidden, non-finite and negative-zero handling
is canonical, and quantization occurs only at declared projection boundaries.
No observable order may depend on hash iteration.

SVG, PNG, and PDF have separate typed plans. A universal display list,
scattered backend conditionals, and serialized-SVG pivot are forbidden.

Direct SVG font work uses two package-private deep modules and one thin adapter.
`font_subset` exposes request-scoped derivation from one qualified chunk and
ordered scalar set under its hidden frozen profile and concrete resource account;
it owns decode-plan-sfnt-encode-validation and knows no SVG or scene type. `svg_fonts`
consumes typed semantic family demand and owns family/range selection, bundling,
measurement, CSS/data-URL writing, and its resource transcript; it knows no
Excalidraw scene type. The Excalidraw adapter owns `originalText`, generated-label
collection, resolved source families, omission and local-recovery decisions, and
the CJK subset trigger, which it evaluates once per target over the stored
`text` of live Excalifont occurrences and shares between glyph face selection
and the typed demand it bridges inward. Dependencies are strictly
adapter to `svg_fonts` to `font_subset`; reverse or lateral scene dependencies
are forbidden. Transcript ownership is `svg_fonts` for family/range/CSS/bundle
events and `font_subset` for decode/plan/sfnt/encode/validate events.

These modules remain inside the renderer until a real second production caller
or an independently owned and published fork justifies extraction. There is no
backend trait, public font option, or public subsetting API.

The current MVP is evidence only, not a migration constraint. Its permissive
model, discarded fields, local defaults, approximate geometry, SVG pivot,
backend defaults, silent skips, and current bounds/text/frame/link behavior do
not define compatibility.

## 10. Dependencies, Assets, and Distribution

A production component qualifies only if it operates natively and on
`wasm32-unknown-unknown` without production JavaScript, browser, OS service,
system package, network, or host font; preserves artifact byte stability; is
bounded and panic-free on the renderer-controlled surface; has compatible
licensing and provenance; and passes compatibility-critical vectors. Remedy
failure in this order: adapter, pinned controlled fork, focused owned port.

The renderer MUST directly port the used Rough.js `4.6.4` paths and fillers,
preserving JavaScript `f64`, signed 32-bit `Math.imul` PRNG behavior, operation
order, and PRNG-consumption order, without `rand`. `roughr` cannot define
compatibility. `perfect_freehand` MAY be used unchanged only after exact vectors
against Perfect Freehand `1.2.0`; otherwise use a focused owned port with the
official stroke-to-path wrapper.

Raster codecs use individually pinned pure-Rust adapters behind one bounded,
deterministic contract. When a later revision admits AVIF it follows the same
gate; if no permissively licensed pure-Rust/WASM decoder qualifies, the project
completes or owns one rather than using a platform decoder or weakening
compatibility. `usvg`/`resvg` can process only sanitized bounded nested SVG
under deny-all resource access.

Compatibility revision 1 ships all 230 byte-identical official rendering WOFF2
source files across eight families, exact Liberation Sans and Noto Emoji
fallback inputs, qualified decoder inputs, retained deprecated families, and
exact Font Awesome fallback paths. The asset manifest's source lengths and
digests are authoritative. Assistant remains excluded. Direct SVG may derive
only the deterministic subsets and rewritten identities specified in Section
8.3; the source assets remain unchanged.

Installed fonts, network fallback, system font services, official nested
HarfBuzz/WOFF2 WASM helpers, custom-font subsetting, merging, and unspecified
renaming are forbidden. Each qualified source decode and subset toolchain records
source, intermediate, and output digests; exact versions and invocation; complete
profile and corpus-table-manifest digest; table/name/metric/license comparison;
workspace bounds; reviewer; and date. Any unprofiled content, metric, table, or
name change needs new approval. For the source decode that record is the Section
8.6 reviewed-decodes record. A WOFF2 decode is a lossless container
reconstruction: it has no intermediate and no profile or workspace of its own
beyond the 8.3.3 qualification corpus whose digest it binds; its table, name,
metric, and license comparison is byte identity with the recorded decode, which
`xtask catalog verify` re-derives on the verifying host and which the 8.3.3
corpus carries across supported targets through the subsets derived from it;
and its reviewer and date are the approval's.

Reserved Font Names `Virgil`, `Cascadia Code`, `Lilita`, and `Liberation` remain
only on exact official source bytes and approved lossless decoding equivalents,
the reviewed decodes; generated Modified Versions use the Section 8.3 aliases.
Excalifont source content carries Excalifont and Virgil provenance. Noto Emoji
`3.003` uses the exact manifested path/hash without a stronger historical-origin
claim.

Fonts are OFL-1.1 except MIT-licensed Comic Shanns. Font Awesome geometry is CC
BY 4.0. Copied or ported Excalidraw, Rough.js, Perfect Freehand, and
points-on-curve code retains its MIT notices. New project-authored code is
`MIT OR Apache-2.0` with complete license texts.

The asset catalog records identity, profile, upstream path/revision/URL, complete
and segment digests and lengths, order, license/copyright, Reserved Font Names,
transform history including the reviewed decode, Unicode range or role, and
output formats. Imported bytes,
indexes, manifests, and notices are checked in. A repository-owned Rust `xtask`
is the sole importer; `build.rs` never downloads or generates production assets.

Asset shards SHOULD remain at or below 8 MiB raw and MUST produce `.crate`
archives below 9.5 MB. Reassembly verifies the complete digest before parsing.

The published virtual workspace exposes only `excalidraw-render` as supported
Rust API and `excalidraw-render-cli` as the native CLI. Doc-hidden unsupported
`excalidraw-render-assets-base` and numbered
`excalidraw-render-assets-data-*` packages carry asset shards. All packages use
one lockstep semver and exact internal versions.
Owned ports/assets publish first, then library, then CLI; partial publication
requires a new version. Production git dependencies are forbidden.

The library always includes SVG, PNG, PDF, every retained compatibility
revision, and the complete built-in catalog. Public features cannot remove them
or alter behavior.

Every distributing crate carries applicable notices. The library exposes
`license_bundle() -> &'static str`; CLI `--licenses` writes exactly those UTF-8
bytes and exits without render paths. Native archives contain the binary,
README, both application licenses, `THIRD_PARTY_NOTICES.md`, and `LICENSES/`.
Linux and macOS use target-named `.tar.gz`; Windows uses `.zip`; one external
`SHA256SUMS` covers all archives.

First-party production code sets `unsafe_code = "forbid"`. Direct dependencies
and every transitive package capable of changing rendering or serialization are
exactly constrained. `Cargo.lock`, `--locked`, packaged-consumer graph checks,
`cargo-deny`, `cargo-vet`, RustSec, provenance, feature, build-script,
native-link, and WASM graph checks are release gates.

Every transitive unsafe use has a source audit in the dependency ledger that
justifies the bounded renderer-facing surface and absence of a safer qualifying
alternative. Production build scripts are denied by default. An exception is
recorded, pure Rust, deterministic, offline, and independent of host time,
locale, path, and randomness; it cannot download, compile C/C++, probe optional
system libraries, or generate production assets.

A checked tool manifest pins every Rust CI executable by exact version and
checksum. WASM runner and `wasm-bindgen-test` versions are exact and compatible.
Third-party GitHub Actions use full commit pins; downloaded tools use exact
version plus checksum or immutable image digest. Workflows use least privilege,
never expose release secrets to untrusted pull requests, and update pins only
through ordinary reviewed dependency changes.

Supported targets are exactly:

- `x86_64-unknown-linux-gnu`.
- `wasm32-unknown-unknown` in the pinned Chromium harness.

Every target executes renderer and deterministic-output tests. Cross-compilation
alone is insufficient. Adding a target is a spec amendment that requires the
target to execute the qualification with the pinned tool identities and agree
on the committed cross-target measurement first (ADR-0006). `rust-version`
equals the official stable compiler. The
first release promises no explicit native deployment floor beyond the target OS
ABI; release notes record toolchain and immutable builder identity.

SVG carries the Section 8.3 complete license texts and distinct subset notices,
plus complete notices for unchanged embedded fallbacks and rendered Font Awesome
fallbacks. PNG carries Font Awesome attribution in deterministic `iTXt` only
when that geometry appears; rasterized font glyphs alone carry no OFL notice.
Official reference evidence remains byte-identical and carries notices in
sidecars and evidence packages, not injected into official artifacts.

## 11. Outcomes and Serialized Contracts

`Artifact` owns exactly one `Vec<u8>` and exposes format, canonical media type,
borrowed bytes, and consuming `into_bytes()`. Media types are `image/svg+xml`,
`image/png`, and `application/pdf`.

A successful `RenderOutcome` contains exactly one complete artifact, the full
ordered `CompatibilityReport` even when empty, compatibility revision,
Excalidraw release, and profile digest. Recoverable repair, replacement,
omission, local loss, or optional-metadata failure returns that outcome.
`RenderError` never contains partial artifact bytes.

The five typed library failures are `SceneParseError`, `RequestError`,
`FrameBatchError`, `UnsupportedRevisionError`, and `RenderError`. They implement
`std::error::Error`, typed accessors, and `descriptor() -> ErrorDescriptor`.
Descriptors, diagnostics, reports, compatibility identity, and stable views
implement `serde::Serialize`. Opaque scene documents, artifacts, outcomes,
failure values, source chains, backtraces, and implementation types have no
public serialization contract.

A standalone empty report is exactly this JSON, with no trailing LF:

```json
{"schema_version":1,"diagnostics":[]}
```

CLI JSON transport appends exactly one LF.

### 11.1 Canonical JSON

Public report and artifact-manifest JSON is minified UTF-8 without BOM, uses
direct non-ASCII scalars, only required escapes, lowercase `\u00xx` for other
controls, shortest round-trip finite decimals, lowercase exponent when needed,
no redundant zeros, canonical `0` for negative zero, integer counters, and the
declared member order.

Schema-1 errors have this exact member order and explicit nulls:

```json
{
  "family": "scene_parse | request | frame_batch | unsupported_revision | render | cli",
  "category": "input | resource_limit | no_meaningful_artifact | encoding | internal | null",
  "code": "stable.code",
  "message": "human explanation",
  "format": "svg | png | pdf | null",
  "element_id": "normalized element id | null",
  "json_pointer": "RFC 6901 pointer | null",
  "details": null
}
```

`category` is non-null only for render failures. A scene-global render error uses
root pointer `""`. Human messages are non-normative.

`json_pointer` is RFC 6901 into the admitted scene document. Tokens escape `~`
as `~0` and `/` as `~1`. Errors always include pointer or explicit null;
diagnostics always contain a non-null pointer and use root `""` when no narrower
scene source exists. A missing required member points to its expected path.

Parse pointer rules are:

| Code | Pointer |
| --- | --- |
| `scene.invalid-utf8` | null |
| `scene.invalid-json` | Narrowest reliable parser-established path, otherwise null |
| `scene.invalid-envelope` | Narrowest offending/expected member; root for wrong top-level kind |
| `scene.input-too-large` | null before a reliable path exists |
| JSON depth/structure/string/number/parse-memory excess | Value/member whose admission crosses when reliable, otherwise null |
| `scene.object-key-too-large` | Expected member, containing object, or null in decreasing reliability |
| `scene.duplicate-object-key` | Expected path of the later duplicate member |

Malformed syntax never causes an invented path. Request, unsupported-revision,
CLI grammar/path/transport/delivery, and supplemental transport failures use
null. CLI-preserved library errors preserve the library pointer.

Every render error has an admitted scene. Source-owned failures point to the
narrowest owner; scene/output-global failures use root. PDF fallback failures
point to the outermost causative effect node, with nested identity in details.
SVG font-subset and mandatory-notice carrier failures use root. The pointer
belongs to the earliest canonical failed transaction, never later speculative
work.

Diagnostic pointer rules include:

| Diagnostic | Pointer |
| --- | --- |
| Missing ID/timestamp | Expected member path |
| Duplicate ID or zero seed | Later element's `id` or affected `seed` |
| Relationship cycle | Source member for the closing edge |
| Unsupported/malformed element | Whole element occurrence |
| Unsupported property/value | Narrowest affected property |
| Known element replacement | Whole element when no narrower primary cause exists |
| Supplement conflict | Authoritative `/files/<escaped-id>`, or root for request-only source |
| Media mismatch | Narrowest authoritative payload/media member, or request root |
| Malformed file record | Whole authoritative file record |
| Missing image | Referring element's `fileId` |
| Unsupported/decode-failed image | Authoritative `dataURL` member |
| Animation flattening | Referring element's `fileId` |
| Forbidden/unsafe nested content | Owning scene resource plus ordinal nested details |
| Unsafe link, invalid color | Affected property |
| Font substitution/missing glyph | Affected text/name/label source |
| Local resource recovery | Narrowest element/property or authoritative/referring asset source |
| Optional metadata and `metadata.*` | Root |
| PDF raster fallback | Outermost causative effect node |

Per-reference diagnostics belong to the referring occurrence; intrinsic asset
diagnostics belong to the authoritative file record. Supplemental request-only
diagnostics use root. Generated events use their source owner. Shared content
never redirects occurrence-specific pointers to a cache or identity.

Diagnostics have this member order:

```json
{
  "code": "stable.code",
  "severity": "info | warning",
  "format": "svg | png | pdf",
  "effects": ["input_repair | visual | semantic | metadata"],
  "element_id": "normalized element id | null",
  "json_pointer": "required RFC 6901 pointer",
  "message": "human explanation",
  "details": null
}
```

Effects are nonempty and ordered `input_repair`, `visual`, `semantic`,
`metadata`. They respectively mean submitted input was repaired, selected, or ignored,
appearance may differ, links/text/animation/vector semantics may differ, or
carried metadata may differ.

Schema-1 `details` is null or exactly one closed object below. Every member is
required in the displayed order and non-null unless marked nullable. Unknown
members and kinds are invalid.

```text
{"kind":"parse_limit","field":<string>,"configured":<number>,"value_kind":<value-kind>,"measurement":<measurement>}
{"kind":"request_value","field":<string>,"asset_id":<string|null>,"option":<string|null>}
{"kind":"revision","requested":<integer>}
{"kind":"compatibility","recovery":<recovery>,"recipe_id":<string|null>}
{"kind":"asset","asset_id":<string>,"claimed_media_type":<string|null>,"validated_media_type":<string|null>,"recovery":<recovery>,"recipe_id":<string|null>}
{"kind":"resource_limit","field":<string>,"configured":<number>,"value_kind":<value-kind>,"measurement":<measurement>,"recovery":<recovery|null>}
{"kind":"memory_unavailable","subject_kind":<string>,"subject":<string>}
{"kind":"missing_glyph","codepoint":<integer>,"recovery":"substituted"}
{"kind":"svg_font_subset_failure","stage":<svg-font-subset-stage>,"resolved_family":<string>,"asset_identity":<string>}
{"kind":"metadata","component":<string>,"operation":<string>,"recovery":"metadata_omitted"}
{"kind":"nested_source","asset_chain":[<asset-chain-entry>],"paths":[<nested-path>],"recovery":<recovery>}
{"kind":"pdf_raster_fallback","recovery":<"rasterized"|null>,"logical_bounds":<logical-bounds|null>,"pixel_width":<integer|null>,"pixel_height":<integer|null>,"causes":[<nested-path>]}
{"kind":"cli_argument","argument":<string>,"value":<string|null>}
{"kind":"cli_path","entries":[<cli-path-entry>]}
{"kind":"cli_io","role":<"input"|"asset">,"asset_id":<string|null>,"operation":<cli-operation>,"path":<string>}
{"kind":"delivery","role":<"output"|"report">,"operation":<cli-operation>,"path":<string>,"artifact_state":<delivery-state>}
```

Nested closed shapes are:

```text
measurement = {"kind":<"attempted"|"at_least">,"value":<number>}
asset-chain-entry = {"asset_id":<string|null>,"occurrence":<nonnegative integer>}
nested-path = {"segments":[{"kind":<nested-segment-kind>,"ordinal":<nonnegative integer>,"name":<string|null>}]}
logical-bounds = {"x":<finite number>,"y":<finite number>,"width":<finite number>,"height":<finite number>}
cli-path-entry = {"role":<"input"|"asset"|"output"|"report">,"asset_id":<string|null>,"path":<string>}
svg-font-subset-stage = <"decode"|"plan"|"sfnt"|"encode"|"validate">
```

Nested segment kinds are `node`, `attribute`, `css_rule`, `css_declaration`,
`resource_slot`, and `effect_input`. Ordinals, not invented XML JSON pointers,
define identity. `pdf_raster_fallback.causes` uses the same nested-path shape.
Fallback bounds use logical units; bounds and pixel dimensions can be null only
when an unisolatable closure cannot safely determine them before failure.

Measurements are only `attempted` and `at_least`. `at_least` is restricted to
bounded streaming stopped one unit over a ceiling or checked overflow. Value
kinds are only `bytes`, `count`, `depth`, `pixels`, `work_units`, `dimension`,
and `coordinate`. Recovery values are only `repaired`, `ignored`, `placeholder`,
`omitted`, `removed`, `substituted`, `flattened`, `rasterized`, and
`metadata_omitted`. Delivery states are `none`, `complete`, and
`possibly_partial`. CLI operations are `open`, `read`, `create`, `write`,
`flush`, and `replace`.

Byte/count/depth/pixel/work values are nonnegative integers. Dimensions and
coordinates are finite numbers. Machine paths retain caller lexical spelling.
Host canonical paths, OS error numbers, Rust type names, and allocator details
are forbidden from schema-1 fields.

Detail bindings are fixed, not producer-selected:

| Code family or code | Required details |
| --- | --- |
| `scene.invalid-utf8`, `scene.invalid-json`, `scene.invalid-envelope`, `scene.duplicate-object-key` | null |
| `scene.input-too-large` | `parse_limit`, `max_input_bytes`, bytes |
| `scene.json-depth-exceeded` | `parse_limit`, `max_json_depth`, depth |
| `scene.json-structure-exceeded` | `parse_limit`, `max_json_structure_entries`, count |
| `scene.string-too-large` | `parse_limit`, `max_total_string_bytes` or `max_string_bytes`, bytes |
| `scene.object-key-too-large` | `parse_limit`, `max_object_key_bytes`, bytes |
| `scene.number-token-too-large` | `parse_limit`, `max_number_token_bytes`, bytes |
| `scene.parse-memory-exceeded` | `parse_limit`, `max_parse_working_bytes`, bytes |
| Every `request.*` | `request_value`; canonical rejected field and applicable asset/option |
| `revision.unsupported` | `revision` with parsed requested revision |
| `scene.unbounded-geometry`, encoder/internal failures other than `encode.svg-font-subset-failed` | null |
| `encode.svg-font-subset-failed` | `svg_font_subset_failure`; first failed stage, resolved family, source asset identity |
| Fatal PDF fallback | `pdf_raster_fallback`, null recovery, first closure |
| `resource.memory-unavailable` | `memory_unavailable`; no semantic limit measurement |
| Other fatal resources | `resource_limit`, null recovery, mapping in Section 11.2 |
| Normalization diagnostics | `compatibility`; `repaired`, except cycle is `removed`; null recipe |
| Unsupported/malformed/replaced element | `compatibility`; declared placeholder recipe or omitted/null recipe |
| Unsupported property | `compatibility`; `ignored`; null recipe |
| Unsupported property value | `compatibility`; manifest recovery and recipe |
| `asset.supplement-conflict` | `asset`; `ignored`; null recipe |
| `asset.media-type-mismatch` | `asset`; `repaired`; both media types |
| Asset/image failures | `asset` or `nested_source`; format recovery and manifest recipe |
| `image.animation-flattened` | `asset`; `flattened`; null recipe |
| `image.unsafe-svg-content-removed` | `nested_source`; `removed` |
| `link.unsafe-omitted` | `compatibility`; `omitted`; null recipe |
| `color.invalid` and font substitution | `compatibility`; `substituted` |
| `font.missing-glyph` | `missing_glyph`; Unicode scalar; `substituted` |
| Recoverable resource diagnostics | `resource_limit`; exact limit, measurement, and non-null recovery |
| Metadata failures | `metadata`; fixed component/operation; `metadata_omitted` |
| `pdf.raster-fallback` | `pdf_raster_fallback`; `rasterized`; finite bounds, dimensions, causes |
| CLI argument/path/I/O/delivery codes | Matching `cli_argument`, `cli_path`, `cli_io`, or `delivery` shape |

Fatal resource field/value-kind mappings are the Section 11.2 table.
Recoverable mappings are:

| Code | Field | Value kind |
| --- | --- | --- |
| `resource.element-complexity-replaced` | `max_geometry_segments_per_element` or `max_text_scalars_per_element` | count |
| `resource.element-work-exceeded` | `max_element_work_units` | work_units |
| `resource.asset-bytes-exceeded` | `max_asset_bytes` | bytes |
| `resource.asset-work-exceeded` | `max_asset_work_units` | work_units |
| `resource.image-dimensions-exceeded` | `max_raster_dimension` | dimension |
| `resource.image-pixels-exceeded` | `max_decoded_pixels_per_asset` or `max_raster_surface_pixels` for replaceable fallback | pixels |
| `resource.nested-svg-complexity-exceeded` | `max_nested_svg_nodes` | count |
| `resource.nested-asset-depth-exceeded` | `max_nested_asset_depth` | depth |
| `resource.coordinate-range-exceeded` | `max_coordinate_magnitude` | coordinate |
| `resource.optional-metadata-omitted` | exact crossed limit | that limit's kind |

Metadata component/operation bindings are provenance/serialization,
diagnostic_summary/serialization, embedded_scene/serialization,
embedded_scene/compression, embedded_scene/embedding,
manifest/serialization, and manifest/embedding, matching their stable codes.
A mandatory-notice failure uses fatal `encode.<format>-failed`, not metadata
details.

`cli_io.asset_id` is non-null only for asset reads. `cli_path` asset entries have
non-null asset ID; other roles use null. CLI pre-parser scene transport excess
uses family `cli` with `scene.input-too-large` and parse-limit details.
Supplemental transport excess uses family `cli`, the precise resource code, and
fatal resource-limit details.

### 11.2 Error Registry

Scene-parse codes are:

```text
scene.invalid-utf8
scene.invalid-json
scene.invalid-envelope
scene.input-too-large
scene.json-depth-exceeded
scene.json-structure-exceeded
scene.string-too-large
scene.object-key-too-large
scene.number-token-too-large
scene.duplicate-object-key
scene.parse-memory-exceeded
```

Request and revision codes are:

```text
request.invalid-padding
request.invalid-scale
request.invalid-max-dimension
request.empty-asset-id
request.duplicate-asset-id
request.invalid-asset-media-type
request.incompatible-format-options
request.invalid-resource-limit
revision.unsupported
```

Frame-batch construction codes use family `frame_batch` and null details:

```text
frame-batch.foreign-target
frame-batch.duplicate-target
frame-batch.shared-preparation-failed
```

Non-resource render codes are:

```text
scene.unbounded-geometry
pdf.raster-fallback-forbidden
pdf.raster-fallback-unisolatable
render.no-meaningful-artifact
encode.svg-failed
encode.svg-font-subset-failed
encode.png-failed
encode.pdf-failed
render.internal-failure
```

`render.no-meaningful-artifact` is reserved and unreachable in compatibility
revision 1. Resource failures use these exact code-to-field mappings:

| Code | Canonical field |
| --- | --- |
| `resource.scene-bytes-exceeded` | `max_scene_bytes` |
| `resource.element-count-exceeded` | `max_elements` |
| `resource.asset-count-exceeded` | `max_asset_entries` |
| `resource.total-asset-bytes-exceeded` | `max_total_asset_bytes` |
| `resource.relationship-depth-exceeded` | `max_relationship_depth` |
| `resource.total-geometry-exceeded` | `max_total_geometry_segments` |
| `resource.total-text-exceeded` | `max_total_text_scalars` |
| `resource.total-decoded-pixels-exceeded` | `max_total_decoded_pixels` |
| `resource.raster-surface-exceeded` | `max_raster_dimension` for width/height; `max_raster_surface_pixels` for area |
| `resource.total-rasterized-pixels-exceeded` | `max_total_rasterized_pixels` |
| `resource.svg-dimensions-exceeded` | `max_svg_dimension` |
| `resource.pdf-page-dimensions-exceeded` | `max_pdf_page_points` |
| `resource.diagnostic-count-exceeded` | `max_diagnostics` |
| `resource.working-bytes-exceeded` | `max_working_bytes` |
| `resource.output-bytes-exceeded` | `max_output_bytes` |
| `resource.total-work-exceeded` | `max_total_work_units` |
| `resource.memory-unavailable` | no semantic configured/measured value |

CLI-native codes are `cli.invalid-arguments`, `cli.format-required`,
`cli.format-mismatch`, `cli.invalid-asset-option`,
`cli.asset-mime-without-asset`, `cli.path-conflict`,
`cli.destination-exists`, `cli.invalid-name-regex`,
`cli.invalid-output-directory`, `cli.frame-no-match`,
`cli.frame-limit-exceeded`, `cli.invalid-frame-selection`,
`cli.input-read-failed`, `cli.asset-read-failed`,
`cli.output-write-failed`, `cli.report-write-failed`, and
`cli.memory-unavailable`. The CLI reuses precise library codes when applicable
and never substitutes a generic code.

### 11.3 Compatibility Diagnostic Registry

The scene-surface manifest is authoritative for exact format/effect/detail rules.
Its complete revision-1 vocabulary is:

| Code | Required core behavior |
| --- | --- |
| `normalization.missing-element-id` | Warning; repair identity |
| `normalization.duplicate-element-id` | Warning; repair identity and affected relations |
| `normalization.missing-timestamp` | Warning; repair to canonical zero |
| `normalization.zero-rough-seed` | Warning; deterministic visual repair |
| `normalization.relationship-cycle` | Warning; remove cycle-closing edge |
| `compatibility.unsupported-element-type` | Warning; unknown-element fallback |
| `compatibility.malformed-element` | Warning; malformed fallback or omission |
| `compatibility.unsupported-property` | Warning; ignore with visual/semantic/metadata effects |
| `compatibility.unsupported-property-value` | Warning; manifest fallback/effects |
| `compatibility.known-element-replaced` | Warning; known-element fallback or omission |
| `asset.supplement-conflict` | Warning; scene payload wins |
| `asset.media-type-mismatch` | Warning; validated type wins |
| `asset.malformed-file-record` | Warning when referenced |
| `image.missing-file` | Warning; format-specific omission/fallback |
| `image.forbidden-resource` | Warning; remove or replace |
| `image.animation-flattened` | Info; PNG/PDF rendered multi-frame occurrence |
| `image.unsupported-format` | Warning; format-specific omission/fallback |
| `image.decode-failed` | Warning; format-specific omission/fallback |
| `image.unsafe-svg-content-removed` | Warning; remove active/unsafe content |
| `link.unsafe-omitted` | Warning in SVG/PDF; PNG silent |
| `color.invalid` | Warning; deterministic color substitution |
| `font.liberation-sans-substituted` | Info; local-font substitution |
| `font.noto-emoji-substituted` | Info; emoji-font substitution |
| `font.missing-glyph` | Warning; replacement glyph |
| `resource.element-complexity-replaced` | Warning; local fallback/omission |
| `resource.element-work-exceeded` | Warning; local fallback/omission |
| `resource.asset-bytes-exceeded` | Warning; local resource fallback/omission |
| `resource.asset-work-exceeded` | Warning; local resource fallback/omission |
| `resource.image-dimensions-exceeded` | Warning; local resource fallback/omission |
| `resource.image-pixels-exceeded` | Warning; local resource fallback/omission |
| `resource.nested-svg-complexity-exceeded` | Warning; smallest fallback/omission |
| `resource.nested-asset-depth-exceeded` | Warning; stop and replace/omit |
| `resource.coordinate-range-exceeded` | Warning; local fallback/omission |
| `resource.optional-metadata-omitted` | Warning; omit optional metadata |
| `metadata.provenance-serialization-failed` | Warning; omit provenance |
| `metadata.diagnostic-summary-serialization-failed` | Warning; omit summary |
| `metadata.scene-serialization-failed` | Warning; omit embedded scene |
| `metadata.scene-compression-failed` | Warning; omit embedded scene |
| `metadata.scene-embedding-failed` | Warning; omit embedded scene |
| `metadata.manifest-serialization-failed` | Warning or fatal notice failure |
| `metadata.manifest-embedding-failed` | Warning or fatal notice failure |
| `pdf.raster-fallback` | Warning; isolated closure rasterized |

Only animation flattening and the two declared font substitutions are
informational; all other codes are warnings. A narrower field, value, or
resource diagnostic suppresses a duplicate generic replacement diagnostic.
Occurrences are never deduplicated.

Final diagnostic order uses this total key:

1. Canonical source owner and preserved source position.
2. Canonical nested occurrence path.
3. RFC 6901 pointer bytes.
4. Stable code UTF-8 bytes.
5. Canonical phase/event ordinal.

Original owners use depth-first source order, arrays by index, objects by source
member order, and member before value. Generated events follow their owner at
the published ordinal. Per-reference asset diagnostics belong to the referring
element; intrinsic authoritative-asset diagnostics belong to the file record.
Supplemental request values follow scene file occurrences by file-ID bytes.
Root metadata events follow content in provenance, summary, embedded-scene
order. Report order does not alter semantic-accounting transaction precedence.

### 11.4 Meaningful Artifacts and Renderer-Owned Fallbacks

A valid empty scene, deleted/nonpainting-only content, or fully diagnosed local
fallbacks and omissions remains a meaningful artifact.

Renderer-owned element fallbacks use fill `#f1f3f5`; stroke and diagonal cross
`#868e96`; stroke width 2 logical pixels; dash `6 4` at phase zero; and centered
Liberation Sans 14 logical pixels in `#495057`. Labels are:

- `Unsupported: <type>` with `renderer.unknown-element.v1`.
- `Malformed element` with `renderer.malformed-element.v1`.
- `Unavailable: <type>` with `renderer.known-element-unavailable.v1`.

They preserve transform, opacity, paint order, frame clipping, and sanitized
safe link. Centered strokes are clipped to source bounds. Bounds are never
enlarged and no minimum footprint is invented. Labels escape source text and
use one U+2026 only if it fits; otherwise omit the label. Components that do not
fit are omitted. Without safe finite bounds, omit the occurrence under the same
primary diagnostic.

Official image, image-error, forbidden-resource, nested-resource, and
embeddable fallbacks MUST use the content-addressed recipes in the scene-surface
manifest, not the generic box.

### 11.5 CLI JSON

Render report schema 1 has this exact top-level order:

```json
{
  "schema_version": 1,
  "status": "success | failure",
  "renderer_version": "package semver",
  "compatibility_revision": 1,
  "excalidraw_release": "v0.18.1",
  "reference_profile_digest": "sha256:...",
  "format": "svg | png | pdf | null",
  "artifact": {
    "media_type": "image/svg+xml",
    "byte_length": 0,
    "sha256": "sha256:..."
  },
  "error": null,
  "diagnostics": []
}
```

Exactly one of artifact and error is non-null. Identity and format become
populated as soon as selected. Artifact descriptors contain no bytes.

Revision-discovery schema 1 contains schema version, renderer version, and
increasing revision entries with compatibility revision, Excalidraw release,
profile digest, and default flag in that order. An incompatible shape or new
detail kind after first publication requires a new report schema; a new stable
code can then remain in schema 1 only with null details or an existing detail
kind. The unpublished schema 1 defined here already includes
`svg_font_subset_failure`.

## 12. Artifact Manifest

The conditionally present artifact manifest is format-neutral schema-1 canonical
JSON:

```json
{
  "schema_version": 1,
  "provenance": {
    "renderer_version": "<Cargo package semver>",
    "compatibility_revision": 1,
    "excalidraw_release": "v0.18.1",
    "reference_profile_digest": "sha256:<64 lowercase hex>"
  },
  "diagnostic_summary": {
    "snapshot": "pre_metadata",
    "total": 3,
    "by_severity": {"info": 1, "warning": 2},
    "by_code": [
      {"code": "<stable diagnostic code>", "count": 3}
    ]
  },
  "notices": [
    {"asset_identity": "<canonical built-in identity>", "text": "<complete canonical notice>"}
  ],
  "embedded_scene": {
    "name": "scene.excalidraw",
    "media_type": "application/vnd.excalidraw+json",
    "sha256": "sha256:<64 lowercase hex>"
  }
}
```

Unrequested, omitted, or failed provenance, summary, and embedded scene are
literal null. `notices` is always an array, sorted by canonical asset identity,
and deduplicates complete license texts by canonical license identity; the
license notice object's `asset_identity` is that canonical license identity.
Every subset identity still contributes one distinct existing-schema notice object:
its `asset_identity` is the full subset semantic identity, and its canonical
`text` contains source identity and digest, profile digest, output digest and
length, effective alias, and transformation statement; OFL output is identified
as a Modified Version. Equal subset notice text is not deduplicated.
`by_severity` always contains info then warning. `by_code` contains positive
counts sorted by stable-code bytes.

The manifest exists when an optional component survives or a mandatory notice
applies; otherwise it is absent. Schema 1 is closed and caller-unselectable.

The diagnostic summary is frozen exactly once before metadata processing. It
includes all pre-metadata diagnostics and excludes summary, optional-metadata,
and metadata-failure events produced later. It is never regenerated.

### 12.1 SVG and PNG Scene Codec

SVG and PNG embed this exact-key-order wrapper:

```json
{"version":"1","encoding":"bstring","compressed":true,"encoded":"<byte string>"}
```

The project-owned zlib stream uses RFC 1950 header `78 01`, no dictionary, RFC
1951 stored blocks in source order, at most 65,535 source bytes per block,
little-endian `LEN` and one's-complement `NLEN`, exactly one final block even for
empty input, and big-endian Adler-32. Each zlib byte maps to the same
U+0000-U+00FF codepoint in `encoded`. SVG applies official Latin-1-byte-to-base64
transport; PNG stores wrapper JSON as Latin-1 `tEXt`. Failure never switches
compression mode, compressor, or carrier.

### 12.2 Format Carriers

SVG root children begin, in order, with `<!-- svg-source:excalidraw -->`, one
`<metadata>`, then `<defs>`. `<metadata>` always exists. An embedded scene starts
with this child sequence in order:

```xml
<!-- payload-type:application/vnd.excalidraw+json -->
<!-- payload-version:2 -->
<!-- payload-start -->
BASE64_TEXT
<!-- payload-end -->
```

`BASE64_TEXT` denotes one unwrapped XML text node, not an element.

The manifest follows as one
`excalidraw-render:manifest` element in namespace
`urn:excalidraw-render:artifact-manifest:1`, containing one canonically escaped
JSON text node. No second metadata element is emitted.

PNG application metadata follows final `IDAT` and precedes `IEND`. Embedded
scene is the first `tEXt`, keyword `application/vnd.excalidraw+json`, followed by
one uncompressed `iTXt` manifest with keyword
`excalidraw-render:artifact-manifest`, compression flag/method zero, and empty
language and translated keyword. No other `tEXt` or `iTXt` is emitted.

PDF attaches raw canonical scene bytes as `scene.excalidraw` with media type
`application/vnd.excalidraw+json`; it does not use the wrapper. The artifact
manifest is the sole custom value in this exact bare, unpadded UTF-8 XMP tree:

```xml
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:er="urn:excalidraw-render:pdf:metadata:1"><er:manifest>...canonical manifest JSON...</er:manifest></rdf:Description></rdf:RDF></x:xmpmeta>
```

There is no XML declaration, BOM, processing instruction, indentation, trailing
LF, `xpacket`, or duplicate information-dictionary metadata. Only required XML
character-data escaping changes JSON spelling, and XML unescaping reproduces
the canonical manifest bytes. XMP is absent when no manifest is required.

### 12.3 Atomicity and Failure

The embedded scene, carrier/attachment, and matching descriptor are one atomic
component. Retention priority is required visual/container bytes and notices,
then provenance, diagnostic summary, and embedded scene. Failure of one optional
component does not prevent consideration of later components.

After pre-metadata diagnostics, freeze `S0`. For each optional component in
priority order:

1. Precheck exact generation work and scratch.
2. If either cannot fit, reject it and emit
   `resource.optional-metadata-omitted`.
3. Otherwise commit measurement work and measure exact marginal encoded bytes
   against the already accepted component set.
4. Precheck output-buffer growth, emission work, total work, and output bytes.
5. If retention cannot fit, reject only retention, release scratch, preserve
   committed measurement work, emit the omission diagnostic, and continue to
   every later component.

The omission diagnostic is required report content and can itself cause fatal
diagnostic, working-byte, or total-work exhaustion. Required visual/container
bytes and mandatory notices are never omitted. Non-resource serialization or
compression failure emits its component diagnostic, continues, and never
regenerates `S0`.

Provenance or summary serialization failure nulls only that component. Scene
serialization, compression, or carrier failure omits the complete embedded
scene. Final manifest failure with no notices omits the manifest and all optional
metadata, including prepared scene payload. Final manifest failure with a
mandatory notice is fatal `encode.<format>-failed` with no artifact.

Recoverable failures emit exactly one current-format warning with metadata
effect, root pointer, and no element ID. Resource omission uses
`resource.optional-metadata-omitted`. Physical allocation refusal is fatal
`resource.memory-unavailable`. No path exposes partial in-memory metadata or
artifact bytes.

## 13. Resource Limits and Robustness

`ResourceLimits` is immutable, has private fields and read-only accessors, and
is built from `ResourceLimits::default()`. Integer fields use target-independent
`u64`/`u32`; coordinate fields use finite positive `f64`. There is no unlimited
or target-specific preset.

| Field | Default | Absolute maximum |
| --- | ---: | ---: |
| `max_scene_bytes` | 32 MiB | 64 MiB |
| `max_elements` | 16,384 | 65,536 |
| `max_asset_entries` | 1,024 | 4,096 |
| `max_asset_bytes` | 32 MiB | 64 MiB |
| `max_total_asset_bytes` | 128 MiB | 256 MiB |
| `max_relationship_depth` | 64 | 128 |
| `max_nested_asset_depth` | 4 | 8 |
| `max_nested_svg_nodes` | 65,536 | 262,144 |
| `max_geometry_segments_per_element` | 262,144 | 1,048,576 |
| `max_total_geometry_segments` | 4,194,304 | 16,777,216 |
| `max_text_scalars_per_element` | 262,144 | 1,048,576 |
| `max_total_text_scalars` | 4,194,304 | 16,777,216 |
| `max_raster_dimension` | 16,384 | 32,768 |
| `max_decoded_pixels_per_asset` | 16,777,216 | 67,108,864 |
| `max_total_decoded_pixels` | 33,554,432 | 134,217,728 |
| `max_raster_surface_pixels` | 33,554,432 | 134,217,728 |
| `max_total_rasterized_pixels` | 67,108,864 | 268,435,456 |
| `max_svg_dimension` | 1,000,000 | 10,000,000 |
| `max_pdf_page_points` | 14,400 | 14,400 |
| `max_coordinate_magnitude` | `1e9` | `1e12` |
| `max_diagnostics` | 65,536 | 262,144 |
| `max_working_bytes` | 256 MiB | 512 MiB |
| `max_output_bytes` | 128 MiB | 512 MiB |
| `max_element_work_units` | 67,108,864 | 268,435,456 |
| `max_asset_work_units` | 134,217,728 | 536,870,912 |
| `max_total_work_units` | 1,073,741,824 | 4,294,967,296 |

MiB means 1,048,576 bytes. Equality is accepted. Checked `u64` arithmetic is
mandatory; overflow is a crossing. Defaults and maxima remain invariant across
revisions in one semver-major line.

`ResourceLimits::absolute_maxima()` exposes the immutable maximum profile. The
builder starts from `ResourceLimits::default()` and public read-only accessors
expose every configured field.

The builder rejects zero, non-finite, negative, above-maximum values and exactly
these cross-field inconsistencies:

```text
max_asset_bytes > max_total_asset_bytes
max_geometry_segments_per_element > max_total_geometry_segments
max_text_scalars_per_element > max_total_text_scalars
max_decoded_pixels_per_asset > max_total_decoded_pixels
max_raster_surface_pixels > max_total_rasterized_pixels
max_element_work_units > max_total_work_units
max_asset_work_units > max_total_work_units
```

Other aggregate infeasibility is evaluated at render time.

`max_svg_dimension` applies to logical `viewBox` extents and final scaled width
and height. `max_pdf_page_points` applies after padding and sizing; PDF cannot
evade it with `UserUnit`. Raster side and area limits apply after sizing and
integer conversion. Main PNG side or surface exhaustion is fatal; an
independently replaceable fallback surface uses local recovery.

`max_coordinate_magnitude` applies to every normalized coordinate, dimension,
control point, translated transform coefficient, and intermediate exposed to a
later semantic operation.

### 13.1 Fixed Parser Envelope

`SceneDocument::parse` always uses these cross-target ceilings:

| Ceiling | Value |
| --- | ---: |
| Input bytes | 67,108,864 |
| JSON depth, root at 1 | 128 |
| JSON structural entries | 1,048,576 |
| Total decoded UTF-8 string bytes | 67,108,864 |
| One decoded string | 67,108,864 |
| One object key | 4 KiB |
| One number token | 128 bytes |
| Canonical parser live bytes | 256 MiB |

A structural entry includes every container, scalar, and object member name.
Large valid number tokens can remain lexical until consumed. Parser logical
memory uses `A(n) = 64 * ceil(n / 64)`, with `A(0) = 0`, and these records:

| Parser record | Canonical logical bytes |
| --- | ---: |
| Array or object node | `128` |
| Scalar node | `64` |
| Object member | `64 + A(key_utf8_bytes)` |
| Decoded string storage | `64 + A(decoded_utf8_bytes)` |
| Preserved number token | `64 + A(token_bytes)` |
| Active parser frame | `64` |
| Active token scratch | `64 + A(current_token_bytes)` |

The input slice is excluded. Records are acquired before constructing the value
and released when the opaque document takes ownership or parsing fails.
Allocator/hash capacity, physical layout, RSS, WASM pages, and reserve capacity
do not affect this ledger.

CLI scene input streams to the 64 MiB parser ceiling without trusting file
metadata. One extra byte reports `scene.input-too-large` with
`at_least = 67,108,865`. A complete scene above the 32 MiB default but within
the parser envelope parses, then fails `resource.scene-bytes-exceeded`.
Supplemental files stream by file-ID order against 32 MiB per-file and 128 MiB
aggregate transport ceilings.

### 13.2 Semantic Accounting

The canonical phase order is admission, JSON traversal and normalization,
authority/reference census, intrinsic assets and nested discovery, shared
semantic resolution, format planning, projection/fallback, metadata, then
encoding/outcome assembly. Implementations can run ahead only while buffering
results until all earlier canonical transactions settle.

Original JSON traversal is depth-first lexical source order. Relationship sources
order by element, then frame, group, binding, clip, mask, asset reference, and
link; ties use target source position and pointer bytes. Format projection orders
page/background, scene paint order, then published suboperation ordinal.

Every scene file payload, supplemental mapping payload, and accepted nested data
payload is a separate asset payload occurrence before conflict resolution or
deduplication. Each occurrence pays exact valid decoded bytes, or the valid
prefix before the first malformed quantum, against local and aggregate limits.
Nested bytes count on every discovery.

Top-level occurrences order scene file records by preserved member order, then
supplemental mappings by file-ID UTF-8 bytes. Inside an accepted authoritative
parent, nested slots order by source-byte offset then attribute/property ordinal,
recursively depth-first. Top-level depth is 0. Recursive discovery occurs only
for a referenced authoritative parent that passed admission, validation,
sanitization, node, depth, and asset-work checks. Ignored, unreferenced,
malformed, unsupported, oversized, replaced, or conflicting parents are not
traversed.

Semantic asset identity is SHA-256 over this exact preimage:

```text
UTF-8("excalidraw-render:semantic-asset:v1")
NUL
u64-le(compatibility revision)
u64-le(validated media-kind tag)
u64-le(decoded payload length)
decoded payload octets
```

Revision-1 media-kind tags are frozen as `1` PNG/APNG, `2` JPEG/JFIF, `3` GIF,
`4` WebP, `5` BMP, `6` ICO, `7` reserved for AVIF (not accepted in revision 1,
so never emitted), and `8` sanitized SVG. Claimed MIME, ID, pointer, lexical
spelling, parameters, path, fragment, and provenance do not participate.
Malformed content without a validated kind has no identity.

Intrinsic operation kinds are exactly `validate`, `sanitize-svg`,
`decode-raster`, `select-static-frame`, `color-normalize`,
`derive-decoded-rgba`, `derive-sanitized-svg`, and `derive-format-resource`.
They charge once per `(identity, operation kind)`. Occurrence admission,
references, projection, masking, and compositing do not share charges.

`max_elements` counts all source occurrences. Geometry counts every move, line,
quadratic, cubic, close, rough operation, clip, mask, and glyph-outline command.
Text counts normalized Unicode scalars before shaping. Decoded pixels share only
per identity and decode policy. Each selected raster surface pays its full area;
the main PNG surface precedes fallbacks.

Every event charges total work and at most one local owner: element-local JSON,
normalization, relations, geometry, text, format, and diagnostics belong to that
element; occurrence admission belongs to that occurrence's asset owner;
intrinsic work belongs to the semantic identity; nested projection belongs to
the parent identity; optional metadata is total-only. Scene-global/container
and output work is total-only. Events matching several rows pay every row but
retain one owner.

The revision-1 work ledger is:

| Weight | Canonical event and multiplicity |
| ---: | --- |
| 1 | Each original scene source octet, once |
| 1 | Each supplemental raw or data-URL lexical payload octet |
| 1 | Each decoded occurrence octet in `D(o)` |
| 1 | Each identity payload octet for every selected intrinsic operation |
| 1 | Each codec-measurement input octet |
| 1 | Each codec-emission input octet when retained |
| 1 | Each canonical raster sample operation |
| 1 | Each retained artifact octet |
| 4 | Each selected decoded source pixel |
| 8 | Each original JSON node/member |
| 8 | Each generated normalized field |
| 8 | Each format-plan operation |
| 8 | Each retained diagnostic occurrence |
| 16 | Each nested XML/CSS token |
| 16 | Each nested SVG element/text node |
| 16 | Each accepted, removed, or rejected nested attribute/CSS declaration |
| 16 | Each canonical selector attempt |
| 16 | Each accepted or rejected local nested reference |
| 32 | Each considered relationship traversal, including rejected/cycle edge |
| 32 | Each source/generated geometry command |
| 32 | Each normalized Unicode scalar |
| 32 | Each shaped glyph |
| 32 | Each line-break step |
| 32 | Each font fallback probe |
| 32 | Each glyph-outline command |

A font fallback probe is one face consultation of the per-scalar chain in
8.3.1. Each shaped scalar occurrence pays its own probes; once the family's
whole-text face is fixed, the faces already serving the target change neither
their count nor their order, and neither the whole-text face's initial `cmap`
test nor a declared-range test is a probe.

Direct SVG font subsetting adds these canonical events for each target. First,
all family-scalar-set events occur in resolved family order. Then each candidate
face produces one considered event in pinned face order and, when selected, an
immediately following selected event. Only after every family completes
selection are selected chunks derived in `(family order, face order)`. An event
that does not apply is absent rather than represented by a zero transaction.

| Event | Required semantic ingredients |
| --- | --- |
| `svg_font_family_scalar_set` | Every unique first-use scalar and its ordered `u32` storage |
| `svg_font_chunk_considered` | For every candidate face in pinned order, the complete Cartesian product of first-use-ordered family scalars and normalized declared ranges in source order |
| `svg_font_chunk_selected` | One selected source identity/order record when at least one considered pair intersects |
| `svg_font_dependency_envelope` | Before the first opaque dependency call, the frozen conservative work and live-record envelope for every selected chunk in derivation order |
| `svg_font_woff2_decode` | Exact compressed input bytes, manifested decoded maximum, exact decoded sfnt bytes, and every decoded table record |
| `svg_font_subset_plan` | Profile/table-manifest records, every requested scalar, and every closure glyph charged before expansion |
| `svg_font_subset_sfnt` | Exact generated sfnt bytes and table records |
| `svg_font_woff2_encode` | Exact sfnt input, conservative qualified quality-11 workspace, and exact generated WOFF2 bytes |
| `svg_font_validate` | Exact generated WOFF2 decode/parse bytes, table records, closure, metrics, names, legal records, and sfnt-agreement comparisons |
| `svg_font_css_data_url` | Exact WOFF2, base64, declaration, style-block, and resulting SVG output bytes |

Each ingredient pays every applicable existing per-byte, scalar, glyph, table,
format-plan, retained-output, working-byte, and total-work row. Source compressed
bytes and immutable catalog storage remain excluded from working bytes, but work
over them is charged. Decoded and generated variable bytes use the existing
request-derived built-in blob record `256 + A(n)`; selected chunks use built-in
selection state; scalar, range, table, and closure entries use built-in
glyph/range or format-plan records as applicable. Exact checked arithmetic is
mandatory. Pinned dependencies that do not expose incremental budget hooks run
under one revision-defined conservative dependency envelope. For a selected
chunk, let `S` be its exact compressed byte length, `D` its manifested decoded
sfnt length, `U` the family scalar count, `T = D / 16`, `G = 65,535`,
`F = 256 * D`, `W = 2 * D`, and `E = 8 MiB`. `F` is the maximum
adapter-produced sfnt and adapter workspace, `W` is the maximum generated WOFF2,
`E` is the quality-11 encoder workspace, `T` is the sfnt table-record ceiling,
and `G` is the TrueType glyph ceiling. The per-chunk work envelope is:

```text
(S + D + 8T)
+ (32U + 32G)
+ (F + 8T)
+ (F + W)
+ (W + F + 32G)
+ (W + 4 * ceil(W / 3))
```

The live envelope is the sum of retained `256 + A(W)` records for all selected
chunks plus the maximum, over one sequentially active chunk, of
`256 + A(D) + 256 + A(F) + E + 64 * (U + G)`. The envelope formulas and
constants are compatibility semantics rather than measurements of allocator
behavior. Qualification MUST prove that every admitted sealed chunk and
selected-codepoint class remains within them. Exceeding `F`, `W`, `T`, or `G`
after an accepted envelope is an internal qualification failure, never a larger
late resource transaction.

All `svg_font_chunk_considered` work for every candidate face in a family is
charged before that family's first selected-chunk derivation. The semantic charge
does not short-circuit after an intersection; physical range indexing may vary
only if it preserves the complete candidate/scalar/range transactions and first
crossing.

Codec measurement and retained emission are two semantic passes even when
physically fused. Dependencies must prove untrusted-dependent loops are bounded
by prepaid semantic events.

Raster operations cumulatively charge `P`, `2P`, `4P`, `6P`, `8P`, `12P`,
`16P`, or `32P` samples for clear, copy, solid background or clip/composite,
nearest projection, alpha-mask generation, bilinear projection, vector/glyph
coverage, and bicubic projection respectively. `P` is the clipped conservative
device-space integer bounding rectangle area using floor minima, ceil maxima,
and surface clamping.

A PNG frame clip mask is generated once per distinct frame clip set in a target,
in first-use element order and before any element projection. Its `P` is the
clip's own bounding rectangle, it charges `P` rasterized pixels and `8P`
total-only work, and its mask surface record lives from generation through
final composite. An element clipped by a frame charges only its own projection
plus any mask that is its own: a bound-label exclusion pays the label's `P`, an
image clip pays the image's `P`.

Canonical logical working bytes, not physical allocations, define
`max_working_bytes`. All variable records use the `A(n)` function above:

| Canonical record | Logical bytes | Lifetime |
| --- | ---: | --- |
| Request session/meter | `4,096` | Render start to outcome handoff |
| Normalized element | `256` | Normalization through plan completion |
| Scalar/reference/style field | `64` | Normalization through plan completion |
| Owned UTF-8/byte field | `64 + A(n)` | Normalization through plan completion |
| Relationship edge | `64` | Admission through plan completion |
| Geometry path header/command | `128` / `64` | Geometry admission through plan completion |
| Text run | `256 + A(source_utf8_bytes)` | Text resolution through plan completion |
| Glyph/line-break record | `64` / `32` | Text resolution through plan completion |
| Asset occurrence state | `256 + A(file_id_or_pointer_bytes)` | Admission through authority/discovery or parent completion |
| Semantic identity and authoritative payload | `512 + A(decoded_payload_bytes)` | Identity processing through last projection use |
| Decoded RGBA | `256 + 4 * width * height` | Decode through projection completion |
| Sanitized SVG | `512 + 128N + 64(A_t + T + S + R + C)` | Sanitize through projection completion |
| Format-plan base/operation/reference | `1,024` / `128` / `64` | Planning through encoding completion |
| Format clip/mask/link | `128` | Planning through encoding completion |
| Raster color/mask surface | `256 + 4*pixels` / `256 + pixels` | Selection through final composite/encoding |
| Diagnostic | `512 + A(pointer) + A(element_id)` | Acceptance through outcome handoff |
| Metadata scratch | `512 + A(uncompressed) + A(compressed)` | Measurement through acceptance/omission |
| Raster decoder workspace | `1 MiB + 32 * max(width, height)` | Active decode |
| SVG sanitizer workspace | `1 MiB` | Active sanitizer |
| SVG serializer workspace | `256 KiB` | Measurement/emission through encoding |
| PNG encoder workspace | `2 MiB + 16 * output_width` | Measurement/emission through encoding |
| PDF stream workspace | `1 MiB` | Active stream |
| Metadata compression workspace | `1 MiB` | Active compression |
| Built-in selection state | `256` per selected resource | First use through encoding |
| Built-in glyph/range state | `64` per selected glyph/range | First use through encoding |
| Request-derived built-in blob | `256 + A(derived_bytes)` | Derivation through encoding |
| Output buffer | `512 + A(final_output_bytes)` | Exact admission through handoff |

For sanitized SVG, `N` is accepted nodes; `A_t` accepted/removed
attributes/declarations; `T` retained tokens; `S` retained selectors; `R`
retained local-reference slots; and `C` retained nested geometry commands.

Preexisting opaque scene/request/supplemental storage and immutable built-in
catalog bytes are excluded. Derived and retained state remains charged for its
published lifetime. Work never refunds.

Sharing is permitted only by these keys: identity payload/state by semantic
identity; sanitized SVG by `(identity, sanitizer revision)`; RGBA by
`(identity, decode policy)`; intrinsic work/pixels by `(identity, operation)`;
element geometry by `(source element, semantic role)`; selected built-ins by
identity/glyph/range; a PNG frame clip mask by `(target, frame clip set)`; and
format resource body only where that format permits. Occurrence admission,
relationships, per-reference projection, element-owned masking, compositing,
repeated nested occurrences, plan references, surfaces, fallback, diagnostics,
optional metadata occurrences, and output octets never share.

SVG family scalar and selection records are acquired before selection work. The
dependency envelope is acquired after selection and before the first selected
chunk is decoded; it covers all opaque decode, closure, subset, encode, and
validation dependency work. The exact stage events that follow are observations
validated against the accepted envelope and do not add work or working bytes.
Family scalar and selection records release after bundle measurement; the
dependency envelope and generated WOFF2 records release after serializer
emission; output remains through outcome handoff. Failure releases all live
records in reverse dependency order. A physical cache hit replays and charges
this same sequence and these same boundaries; it is not `shared_intrinsic` in a
target's semantic trace.

For an SVG containing subsets, measurement first reserves the complete final
output buffer. Encoding then commits output runs in ordinary serializer order,
including font declarations with their base64 data URLs, notices, and remaining
SVG bytes.
At each transaction, working-byte crossing precedes output-byte crossing, which
precedes total-work crossing. No subset, complete chunk, or partial artifact is
retained after a rejected fatal transaction. Existing default and absolute
resource limits are unchanged.

Before work, determine an exact delta, a permitted lower bound, or the canonical
SVG-font dependency envelope, evaluate every applicable limit, reserve accepted
work/records, and perform only covered work. A rejected transaction commits no
work, output, local count, or record. An accepted dependency envelope is not
refunded when exact stage measurements are smaller; this keeps first crossing
and cache transcripts independent of dependency allocation details.
`at_least = limit + 1` is allowed only for bounded streaming stopped one unit
after the ceiling or checked overflow.

For jointly evaluated occurrence bytes, geometry, text, and decoded pixels,
aggregate crossing wins and neither observation nor local admission commits. If
only local crosses, the aggregate observation commits, local admission does not,
and local recovery begins. If neither crosses, both commit. Rejected local work
commits neither local nor aggregate work because it is not performed.

At one source point, fatal aggregate crossings beat local recovery. Fatal
precedence is scene bytes, elements, asset entries, total asset bytes,
relationship depth, total geometry, total text, total decoded pixels, required
main raster, total rasterized pixels, SVG dimensions, PDF page points,
diagnostics, working bytes, output bytes, then total work. Local precedence is
nested depth, occurrence bytes, image side, per-identity pixels/fallback area,
nested-SVG complexity, coordinate, element complexity, element work, asset work,
then optional metadata.

For multiple raster crossings, width precedes height, then area. Coordinate
components order `x`, `y`, `width`, `height`, affine `a`, `b`, `c`, `d`, `e`,
`f`, then revision-defined components. Across source points the earliest
canonical phase/source wins. A rejected fatal transaction emits no recovery
diagnostic.

Fallback is a new fully charged transaction. If it exceeds the same local work
allowance, omit instead of recursively falling back and retain the original
diagnostic.

### 13.3 Safety and Recovery

Public fallible operations return typed results rather than panic, unwind, hang,
recurse without checked bounds, read external resources, or expose partial
in-memory artifacts. The core exposes no clock, timeout, cancellation callback,
or async resolver. Hosts needing deadlines isolate calls in a terminable process
or worker. Elapsed time never affects bytes or semantic outcomes.

`catch_unwind` is not the safety boundary. Renderer-controlled paths remove
unchecked indexing, user-dependent recursion, unchecked conversion, and
`unwrap`. All untrusted-dependent allocations are fallible. Physical allocation
failure is fatal `resource.memory-unavailable`; trusted-catalog corruption is
internal failure.

Relationships, clipping/masking, nested assets, XML, CSS, and backend trees use
checked explicit stacks where practical. Cycles are detected in source order.
Recoverable cycles remove the closing edge; excess relationship depth is fatal.

Local recovery is permitted only for an independently replaceable element,
asset, nested-image subtree, PDF effect closure, or optional metadata component
when the remainder stays faithful. The renderer never truncates points, text,
elements, bytes, diagnostics, scene order, or output; silently lowers
resolution; selects cheaper filters; returns an encoder prefix; or translates
allocation failure into a fallback.

Aggregate exhaustion is fatal, discards all in-memory artifact bytes, and
preserves the already accumulated report. If finite scene bounds cannot be
established after local recovery, fail `scene.unbounded-geometry`.

Conformance MUST exercise N-1, N, and N+1 for every limit, combined aggregate
crossings, exact code/pointer/order/artifact behavior, cycles and deep graphs,
expansion and decompression bombs, numeric excess, oversized dimensions,
overdraw, diagnostic floods, metadata omission, allocator refusal, output
exhaustion, fuzzing, and every supported target.

Tests expose an uncharged byte-stable canonical resource trace. Its header has
revision, profile, format, limits, and input digests. Each ordered transaction or
release records phase/source ordinal, pointer/element/identity, event, local
owner, sharing key, record kind, deltas, before/attempted/after values,
exact-or-at-least, decision, code, fallback parent, and acquire/release. Decisions
are only `committed`, `rejected_local`, `rejected_fatal`, `shared_intrinsic`, and
`released`. Allocator, backend, cache, thread, and timing facts are excluded.

Alternate-representation tests vary borrowed/copied payloads, aliases, caches,
eager/streamed records, fused/split passes, deterministic internal traversal,
codecs, backends, and allocator growth while preserving artifact/report,
canonical trace, first crossing, measurement form, fallback, and stable code.
Only injected physical allocation refusal may diverge to
`resource.memory-unavailable` after the same committed trace prefix.

## 14. Conformance

### 14.1 Development Reference Renderer

The development-only reference renderer is a repository-owned Playwright wrapper
around the exact official package. It calls `exportToSvg()` for official SVG and
independently calls `exportToBlob({ mimeType: "image/png" })` for browser-canvas
PNG. PNG is never derived from SVG.

Official SVG, direct-canvas PNG, and provenance sidecars are immutable checked-in
reference evidence. Every evidence tuple is generated twice in fresh browser
processes with no shared mutable state; encoded SVG hashes, PNG hashes, and raw
raster hashes must match. Existing approved bytes cannot change while profile,
scene hash, options, format, and animation offset are unchanged.

Ordinary Rust tests consume approved evidence without requiring Node,
JavaScript/V8, Playwright, Chromium, OCI, networking, or system fonts. A
repository `xtask` owns explicit generation and verification. Monthly and
pre-release regeneration occurs in an isolated location and never automatically
refreshes evidence. `excalidraw-image 0.2.0` remains diagnostic only.

### 14.2 Corpus and Coverage

Conformance uses independent hard gates. Exact invariants, parsed semantics,
complete ordered reports, visual budgets, and artifact/report determinism all
pass per applicable case and region; corpus averages cannot offset failures.

The conformance corpus has five layers:

1. Exact compatibility-critical vectors.
2. Focused scene documents with official SVG/PNG evidence.
3. Representative licensed or project-authored scene documents.
4. Adversarial and resource-boundary scene documents.
5. Permanent minimized regressions.

Each case is a stable manifested directory recording identity, schema, purpose,
layer, feature/risk tags, coverage identities, compatibility revision, profile
digest, complete scene and hash, variants/formats, provenance/license,
sanitization, assets, evidence hashes/dimensions/generation data, complete
ordered expected report, semantic expectations, visual class, crops,
exceptions, determinism tags, and originating issue where applicable.

External scenes require consent or redistribution-compatible licensing and are
sanitized. Only the immutable sanitized corpus scene is retained. Synthetic
cases are complete scene documents and avoid random/time restore repairs.

A machine-readable coverage ledger assigns every rendering-affecting `v0.18.1`
behavior, format-specific difference, compatibility exception, diagnostic row,
raster/decoder/orientation/alpha/animation class, and declared interaction to
exactly one primary record or approved exclusion. Exclusions require proof of
unreachability or no static-export effect. Difficulty, insignificance, or absent
evidence are not exclusions.

Raster coverage blocks release until every accepted profile and color path,
HDR/gain-map path, decoder, orientation, alpha representation, and animated
container class is characterized under the canonical viewer and direct-PNG
rules. Missing coverage is not a tolerance or runtime best-effort case.

CI rejects unknown identities, stale links, missing variants/formats, duplicate
primary owners, dead cases, unapproved exclusions, and uncovered inventory.

Exact vectors cover Rough.js, the qualified Perfect Freehand path, PRNG
consumption, normalization, geometry, color, shaping/fallback, serialization,
diagnostic order, and format planning. Fixed-seed property tests run on every
pull request for parsing, normalization idempotence, bounded geometry, stable
order, repeated rendering, and panic resistance. Parser, normalization,
image/SVG, and renderer fuzzing runs at least nightly. Confirmed failures become
permanent minimized regressions.

SVG font subsetting is a release-blocking matrix over all 230 source chunks and
every manifested selected-codepoint class. It includes exact fixtures for text
and generated-label collection, first-use order, omission and local recovery,
CJK propagation, family/range selection, layout and composite closure, names,
Reserved Font Name aliases, semantic identity, complete licenses, distinct
notices, CSS, and cache transcript replay. Malformed generated/source structures
and fuzz cases MUST return typed failures without panic, trap, unbounded
allocation, whole-chunk fallback, or partial output.

Repeated artifacts and canonical resource traces MUST be exact on every
supported native target and executed `wasm32-unknown-unknown`, including cold,
warm, request-cache, and frame-batch-cache paths. Artifact bytes are evidence
per qualification cell. Trace stability is instead evidenced as a cross-target
equality property: each target computes its trace measurement fresh, the
aggregation compares every supported target, and the agreed measurement is
committed as a reviewable artifact that every aggregation re-asserts. The
supported-target aggregate itself is recorded verbatim into the release evidence
bundle, bound to the corpus, font subset profile, viewer approval, and committed
measurement it was produced for; the bundle's decision stays pending until it is
recorded and fails closed once any of those four bindings moves. Per-cell trace
digests MUST NOT be pinned. Canonical Chromium and the controlled pure-Rust
viewer each
compare source chunks with generated subsets
for shaping, glyph choice, metrics, line layout, selectable text, and rasterized
typography. Both viewers MUST pass independently; source-vs-subset semantic
parity, not Excalidraw subset bytes, is authoritative.

Every variant first passes exact dimensions and complete report comparison. SVG
semantic checks cover declared consumer-visible semantics rather than official
DOM identity. PNG must decode and pass dimensions, color/alpha, deterministic
chunks/metadata, embedded-scene, and notice checks before visual comparison.

### 14.3 Canonical Raster and Comparators

Every viewer and PNG decoder produces tightly packed, top-left, row-major,
straight-alpha sRGB RGBA8 with exact dimensions and SHA-256. Resizing,
translation, alignment search, and color fitting are forbidden. RGB is zeroed
where alpha is zero. For each `(r,g,b,a)`, create:

```text
alpha: (a, a, a, 255)
black: ((r*a+127)/255, (g*a+127)/255, (b*a+127)/255, 255)
white: ((r*a+255*(255-a)+127)/255,
        (g*a+255*(255-a)+127)/255,
        (b*a+255*(255-a)+127)/255,
        255)
```

Operations are unsigned integers in displayed order; division truncates after
the addition. These views are the complete Pixelmatch and SSIM inputs.

Changed pixels use an exact port of Mapbox Pixelmatch `7.2.0`, commit
`9faed09302aaecec130b4ce0e8505d5ed5221393`, source blob
`be64dc92dae160d6d5ac21b9778b70553c023292`, fetched SHA-256
`52c6026c4916a07548d4ce55090ec4ed918d353e15095406dabc3a8a5135d788`.
Options are threshold binary64 `0x3fb999999999999a` (`0.1`), `includeAA = false`, and
`checkerboard = true`. The port preserves YIQ coefficients, source operation and
traversal order, strict delta comparison, anti-alias ties, ECMAScript `ToInt32`
and remainder, and binary64 rounding. FMA, reassociation, fast math, approximate
operations, and parallel reductions are forbidden.

Pixelmatch runs separately on alpha, black, and white. The changed mask is their
coordinate union after anti-alias suppression. Crops are extracted before
comparison. Fraction limits use exact widened integer cross multiplication;
equality passes.

SSIM is single-scale Wang/Bovik/Sheikh/Simoncelli SSIM with no filtering or
downsampling, an 11x11 Gaussian window at sigma 1.5, population moments, valid
centers only, and an unweighted row-major arithmetic mean.

Black/white luma uses binary64 in this source order, with every multiply/add
rounded separately and no FMA or reassociation:

```text
Y = 0x3fd32119cb582d94*r
  + 0x3fe2c59c7c77f5b0*g
  + 0x3fbd4eb519d2b9f7*b
```

Alpha uses the alpha byte directly. These 121 row-major binary64 coefficient
words are normative. Their concatenated little-endian `u64` digest is
`b931d31602d633632c7a2f29b24dbe5812a49a8e8a016002b07dc8c0ce93efd3`:

```text
3eb1be35ac1c34c6 3ee063548b08c281 3f036912a05519ac 3f1d7b5ae8a4fc11 3f2cb61ec7cbd51a 3f31ed983c6c179d 3f2cb61ec7cbd51a 3f1d7b5ae8a4fc11 3f036912a05519ac 3ee063548b08c281 3eb1be35ac1c34c6
3ee063548b08c281 3f0e45e1f47286ac 3f31ed983c6c179d 3f4b3afa92035fc1 3f5a84ce6c9db06d 3f608f18b8573f7e 3f5a84ce6c9db06d 3f4b3afa92035fc1 3f31ed983c6c179d 3f0e45e1f47286ac 3ee063548b08c281
3f036912a05519ac 3f31ed983c6c179d 3f553c0d18eb2ec1 3f702050fb9dc09d 3f7f68dcbd7dd57d 3f839ce92f03301e 3f7f68dcbd7dd57d 3f702050fb9dc09d 3f553c0d18eb2ec1 3f31ed983c6c179d 3f036912a05519ac
3f1d7b5ae8a4fc11 3f4b3afa92035fc1 3f702050fb9dc09d 3f887e5bdea2018b 3f97da7ee69c2aee 3f9dca171b0b58b1 3f97da7ee69c2aee 3f887e5bdea2018b 3f702050fb9dc09d 3f4b3afa92035fc1 3f1d7b5ae8a4fc11
3f2cb61ec7cbd51a 3f5a84ce6c9db06d 3f7f68dcbd7dd57d 3f97da7ee69c2aee 3fa73aea2efb5b99 3fad02cc3c609d96 3fa73aea2efb5b99 3f97da7ee69c2aee 3f7f68dcbd7dd57d 3f5a84ce6c9db06d 3f2cb61ec7cbd51a
3f31ed983c6c179d 3f608f18b8573f7e 3f839ce92f03301e 3f9dca171b0b58b1 3fad02cc3c609d96 3fb21d7958fce8e0 3fad02cc3c609d96 3f9dca171b0b58b1 3f839ce92f03301e 3f608f18b8573f7e 3f31ed983c6c179d
3f2cb61ec7cbd51a 3f5a84ce6c9db06d 3f7f68dcbd7dd57d 3f97da7ee69c2aee 3fa73aea2efb5b99 3fad02cc3c609d96 3fa73aea2efb5b99 3f97da7ee69c2aee 3f7f68dcbd7dd57d 3f5a84ce6c9db06d 3f2cb61ec7cbd51a
3f1d7b5ae8a4fc11 3f4b3afa92035fc1 3f702050fb9dc09d 3f887e5bdea2018b 3f97da7ee69c2aee 3f9dca171b0b58b1 3f97da7ee69c2aee 3f887e5bdea2018b 3f702050fb9dc09d 3f4b3afa92035fc1 3f1d7b5ae8a4fc11
3f036912a05519ac 3f31ed983c6c179d 3f553c0d18eb2ec1 3f702050fb9dc09d 3f7f68dcbd7dd57d 3f839ce92f03301e 3f7f68dcbd7dd57d 3f702050fb9dc09d 3f553c0d18eb2ec1 3f31ed983c6c179d 3f036912a05519ac
3ee063548b08c281 3f0e45e1f47286ac 3f31ed983c6c179d 3f4b3afa92035fc1 3f5a84ce6c9db06d 3f608f18b8573f7e 3f5a84ce6c9db06d 3f4b3afa92035fc1 3f31ed983c6c179d 3f0e45e1f47286ac 3ee063548b08c281
3eb1be35ac1c34c6 3ee063548b08c281 3f036912a05519ac 3f1d7b5ae8a4fc11 3f2cb61ec7cbd51a 3f31ed983c6c179d 3f2cb61ec7cbd51a 3f1d7b5ae8a4fc11 3f036912a05519ac 3ee063548b08c281 3eb1be35ac1c34c6
```

For each center initialize `mu_x`, `mu_y`, `E[x^2]`, `E[y^2]`, and `E[xy]`
to positive binary64 zero. Visit coefficients row-major and execute, in order:

```text
mu_x   += w*x
mu_y   += w*y
E[x^2] += (w*x)*x
E[y^2] += (w*y)*y
E[xy]  += (w*x)*y
```

Every displayed operation rounds separately. Then compute in source-line order:

```text
var_x  = max(0, E[x^2] - mu_x*mu_x)
var_y  = max(0, E[y^2] - mu_y*mu_y)
cov_xy = E[xy] - mu_x*mu_y

ssim = ((2*mu_x*mu_y + C1) * (2*cov_xy + C2))
     / ((mu_x*mu_x + mu_y*mu_y + C1) * (var_x + var_y + C2))

C1 = 0x401a028f5c28f5c3
C2 = 0x404d42e147ae147b
```

Sum local scores with plain binary64 addition in center row-major order and
divide once by center count. Run views independently and take their minimum.
Any non-finite value fails. Threshold exact values, never displayed rounding.
Declared crops MUST be at least 11x11. Only a complete artifact smaller than
11x11 can use the exception: it passes only when all three complete view buffers
are byte-identical, producing exact score 1.

| Visual class | Maximum changed pixels | Minimum SSIM | Pure-Rust SVG typography allowance |
| --- | ---: | ---: | ---: |
| Exact | `0/1` | `0x3ff0000000000000` (`1.000`) | Same |
| Geometry/image | `1/400` | `0x3feff7ced916872b` (`0.999`) | Same |
| Typography | `1/40` | `0x3fefd70a3d70a3d7` (`0.995`) | `3/100`, `0x3fefae147ae147ae` (`0.990`) |
| Mixed representative | `1/100` | `0x3fefe76c8b439581` (`0.997`) | Same |

Whole artifacts and every crop pass both metrics. Mixed cases include focused
typography and non-text crops. Compatibility revision 1 permits no ignored
pixel masks, excluded rectangles, transparent don't-care regions, denominator
removal, alignment search, or metric weighting.

Canonical Chromium renders official and candidate SVG independently as isolated
top-level documents in fresh contexts using exact dimensions, transparent
capture, virtual time zero, completed images, `document.fonts.ready`, and two
lifecycle updates without advancing time. It uses the exact reference-profile
browser/host/launch/color/origin identity, DPR 1, no margins, disabled GPU and
font hinting, blocked service workers, and a route allowlist limited to the fixed
origin, bundle, and manifested assets. Every other request fails.

Virtual time is installed before navigation. Parsing, every admitted data-image
decode, and fonts complete at offset zero before the two lifecycle updates and
transparent capture. DevTools MUST report every SVG text font as custom
embedded; platform-font use is a hard failure before metrics. Official and
candidate runs never share document, cache, mutable context, or clock.
Live-animation evidence samples the initial point and one integer-microsecond
midpoint from every source-frame interval in the first loop relative to its
fresh resource-ready epoch, each in a fresh context and reproducible in both
oracle runs.

The pure-Rust SVG gate uses test-only `resvg`/`usvg 0.47.0`, commit
`3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707`, separate from any production
version, with a complete lockfile and exact transitive checksums. Default
features, system/memory-mapped fonts, ambient fonts, and file-backed faces are
disabled; only reviewed text and raster-image features are enabled.

Its explicit profile is 96 DPI; languages `["en-US"]`; default Liberation Sans
12; shape `GeometricPrecision`; text `OptimizeLegibility`; image
`OptimizeQuality`; fallback viewport 100x100; no resource directory or injected
stylesheet; admitted-data-URL-only image resolver; transparent sRGB output;
native tree dimensions; identity transform. Local and network resolution fail.
The image resolver covers every accepted raster family using exact-pinned
pure-Rust decoders and revision-1 frame/orientation/color conversion, then
supplies a deterministic in-memory PNG. Nested SVG remains sanitized SVG.

Each artifact gets a fresh empty font database. The viewer parses every embedded
`@font-face` and WOFF2 data URL; verifies canonical CSS, effective alias, subset
identity, payload hash, source/profile/output notice fields, and media type;
decodes and validates the generated font under Section 8.3; and loads only
artifact-declared faces in canonical fallback/family/face order. It rejects
duplicate effective face tuples and maps every generic family explicitly to the
artifact's Liberation alias. An unchanged non-WOFF2 fallback is matched to its
manifested complete bytes. If Liberation is absent, no ambient replacement is
added. Full-catalog preload, scanning, undeclared aliases, host fallback, shared
databases, and any other face are forbidden.

Premultiplied output sets RGB zero at alpha zero; otherwise each channel is
`min(255, (premultiplied_channel*255 + floor(alpha/2))/alpha)` with integer
arithmetic.

Candidate PNG compares directly to official browser-canvas PNG. Candidate PDF
uses exact-pinned PDFium and PDF.js renderings at 96 DPI against canonical
Chromium rendering of official SVG. Both viewers must pass independently.

PDF visual classes use the same exact, geometry/image, typography, and mixed
thresholds for PDFium; PDF.js typography permits 3% changed pixels and 0.990
SSIM. Structural parsing also asserts page boxes, vector/text operators, fonts,
`ToUnicode`, `ActualText`, annotations, clips/masks/transparency, output intent,
XMP/notices/attachments, and exact image-XObject inventory. A full-page image,
rasterized ordinary text/geometry, undeclared image, missing semantic object,
parser warning, or viewer failure is a hard failure.

Focused PDF evidence proves every project-owned and nested-SVG vector family
emits neither `pdf.raster-fallback` nor fallback image XObjects. Every allowed
fallback family is tested alone and in merged, clipped, transformed, reused,
root-minimal, and unisolatable forms. Source images and statically empty or fully
clipped effects succeed under `Forbid`. Required-vector backend failures remain
distinct fatal failures and never become fallback.

### 14.4 Approval and Determinism

One designated compatibility maintainer judges case class, crops, coverage,
exclusions, tolerance exceptions, and reference-evidence acceptance. Recorded
self-approval is allowed when there is one maintainer. Approval records GitHub
identity, UTC date, issue/PR URL, and SHA-256 of a canonical approval subject.

The subject is a canonical bundle containing the case manifest without its
approval object, every evidence/raw-buffer hash, semantic and complete ordered
report expectations, whole/crop classes, crops, coverage records, exceptions,
and all reference/comparator/viewer profile identities. The approval object has
only `approved_by`, `approved_at`, `approval_url`, and
`approval_subject_sha256`; it is excluded from its own digest. Any
judgment-bearing or referenced-byte change invalidates approval. Multi-case
approval lists every case and digest. Coverage-ledger changes additionally
approve the ledger digest. CI validates schema, hashes, identities, links,
reproducibility, approval presence, and scope but makes no judgment or rewrite.

A tolerance exception names one case, variant, format, viewer, and region;
documents a non-defect cause and immutable approval; and sets ceilings to the
exact observed count and binary64 score with no margin.

Canonical renderer-owned snapshots are generated on
`x86_64-unknown-linux-gnu` with the pinned Rust toolchain, `--release --locked`,
and only the public API. Every corpus variant renders twice through fresh
sessions. The risk set also runs in a fresh process with reversed or
deterministically shuffled order.

Exact artifact and report bytes match canonical snapshots on every supported
target. An aggregation directly compares the targets' reports. The risk set
covers every backend, numeric/PRNG path, font/fallback, decoder/nested-SVG path,
frame/clip/mask path, metadata/compression path, diagnostic behavior, fallback,
and target-sensitive dependency. Pull requests run the release-mode risk set
without path filters. Releases rerun from the protected tag and packaged crates.

## 15. Release and Upgrade Policy

One complete lockstep release ships through crates.io, GitHub source, and one
native archive. It includes all formats, retained compatibility revisions,
assets, notices, and supported-target evidence.

Every pull request without path filters runs asset/manifest/license/provenance,
package-consumer, dependency, unsafe/build-script/native-link, and WASM graph
checks. It runs `cargo package` and `cargo publish --dry-run` for every published
package, enforces compressed-size and package-file allowlists, verifies complete
licenses/notices and lockstep exact internal dependencies, and builds/tests from
packaged sources. It also runs full canonical Linux conformance; release-mode
artifact/report risk tests and direct aggregation across every supported
target; and the browser public-API smoke. Each supported target
executes tests rather than only cross-compiling. Release CI repeats the complete matrix from the protected tag
and packaged crate sources.

The browser WASM smoke uses a non-published `wasm-bindgen-test` adapter calling
only public Rust API in exact-pinned headless Chromium. One checked scene renders
through SVG, PNG, and PDF and exercises rough geometry, bundled and fallback
text, an image, frames/clipping, and ordered diagnostics. Complete artifact bytes
and serialized report data match checked native expectations from the same
package. Repeated calls expose global state or cache nondeterminism. Inputs are
served locally with network and system-font resolution disabled.

The only release trigger is a protected semver tag on a fully green commit. The
workflow completes package dry runs, requires maintainer environment approval,
publishes exact lockstep crates in dependency order, builds archives from that
tag on pinned builders, reruns supported-target checks, then creates the GitHub
release.

Every non-prerelease stable Excalidraw release is evaluated by release-note and
source-diff review. When needed, create a complete candidate reference profile,
regenerate affected evidence, classify all changes, prove two-process stability,
make renderer behavior and diagnostics conform, obtain explicit approval, and
activate atomically with conformance and release documentation.

Before first publication, a draft revision MAY incorporate reviewed
contract-relevant changes, including disjoint export targets, by updating its
complete specification, profile, manifests, evidence, approvals, and expected
bytes together. First publication of any package containing the revision makes
it retained. After that point, every contract-relevant change, including a new
target, creates the next monotonic compatibility revision; the retained bundle
and all valid pre-existing request behavior remain immutable. Target-scoped
reference profiles may share a draft revision when their scopes are disjoint or
have no contract-relevant differences where they overlap. Reproducibility
failure alone never authorizes refreshed retained evidence.

Exactly one reference-evidence tree MUST be active. Superseded evidence remains
recoverable through immutable history, release tags, and archived release
artifacts.

Release is blocked by unmanifested assets, unapproved byte changes, missing
provenance/transformation/license records, improper Reserved Font Names, missing
output-traveling attribution, font-bearing evidence without notice packages,
production installed/network font access, dependency-audit failure, incomplete
target execution, or incomplete corpus coverage. Checkpoints belong to the
aspirational protocols in [docs/roadmap.md](roadmap.md) and gate a release only
if a protocol is activated and moved back into this specification first.

Concrete compiler, builder, PDF viewer, WASM tool, compressor, ICC profile,
qualified decoder/port, backend-adapter, and fixture identities are immutable
implementation or release evidence selected under the closed gates above. Their materialization is implementation work, not an unresolved product,
compatibility, architecture, API, robustness, platform, or test strategy
decision. Where qualification fails, this document already specifies
the required fallback or release block.

## 16. Approval and Completion

The original scene-export contract was approved through
[Publish and approve the canonical renderer specification](https://github.com/sagikazarmark/excalidraw-render.orig/issues/26).
The frame-export amendment is specified by
[Export Excalidraw frames as separate artifacts](https://github.com/sagikazarmark/excalidraw-render.orig/issues/97).
The direct-SVG font-subsetting amendment is specified by
[Add Excalidraw-compatible per-glyph SVG font subsetting](https://github.com/sagikazarmark/excalidraw-render.orig/issues/96).
The 8.3 clarifications of the CJK subset trigger's evaluation point, the font
fallback probe, and bidirectional layout are specified by
[Record the CJK subset trigger's evaluation point, the fallback probe, and bidi in spec 8.3 and CONTEXT.md](https://github.com/sagikazarmark/excalidraw-render.orig/issues/122).
The 8.6 definition of the reviewed decode PDF embeds for every official font
chunk is specified by
[Decide how PDF satisfies "reviewed predecoded sfnt chunks" for the 229 runtime-decoded chunks](https://github.com/sagikazarmark/excalidraw-render.orig/issues/120).
Changing these bytes invalidates the original adjacent approval subject; a new
maintainer approval is required before release.

The specification is implementation-ready when all of these remain true:

- Every accepted decision is represented here or in an explicitly incorporated
  immutable closed artifact.
- The reference profile and scene-surface manifest validate and match their
  adjacent digests.
- No compatibility, architecture, API, robustness, platform, or test-strategy
  choice is left to implementation discretion.
- Remaining choices are only qualified implementation selection under a fixed
  gate, evidence materialization, or ordinary private code organization.
- Implementation and implementation-ticket decomposition remain outside this
  wayfinding effort.

## Appendix A. Decision Provenance

These linked ticket resolutions are the rationale incorporated by this
specification. They do not override its normalized wording.

- [Inventory official Excalidraw static-export behavior](https://github.com/sagikazarmark/excalidraw-render.orig/issues/6)
- [Evaluate deterministic reference-oracle options](https://github.com/sagikazarmark/excalidraw-render.orig/issues/13)
- [Survey pure-Rust and WASM parity paths](https://github.com/sagikazarmark/excalidraw-render.orig/issues/2)
- [Audit required asset licensing and distribution](https://github.com/sagikazarmark/excalidraw-render.orig/issues/4)
- [Choose the reference baseline and upgrade policy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/9)
- [Define the static-export compatibility contract](https://github.com/sagikazarmark/excalidraw-render.orig/issues/3)
- [Choose the native rendering architecture](https://github.com/sagikazarmark/excalidraw-render.orig/issues/5)
- [Define the public library and CLI contracts](https://github.com/sagikazarmark/excalidraw-render.orig/issues/14)
- [Set platform, packaging, and dependency policy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/10)
- [Design the conformance and regression strategy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/15)
- [Set performance budgets and benchmark method](https://github.com/sagikazarmark/excalidraw-render.orig/issues/11)
- [Define vector-first PDF behavior](https://github.com/sagikazarmark/excalidraw-render.orig/issues/12)
- [Set robustness and resource-limit policy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/7)
- [Confirm the implementation-ready rendering specification](https://github.com/sagikazarmark/excalidraw-render.orig/issues/8)
- [Research reference raster-asset semantics](https://github.com/sagikazarmark/excalidraw-render.orig/issues/19)
- [Define semantic resource accounting and CLI controls](https://github.com/sagikazarmark/excalidraw-render.orig/issues/17)
- [Materialize compatibility revision 1 reference profile](https://github.com/sagikazarmark/excalidraw-render.orig/issues/22)
- [Define scene admission and forward-compatibility boundaries](https://github.com/sagikazarmark/excalidraw-render.orig/issues/18)
- [Approve the distributable font asset plan](https://github.com/sagikazarmark/excalidraw-render.orig/issues/23)
- [Define canonical scene preservation and artifact metadata](https://github.com/sagikazarmark/excalidraw-render.orig/issues/20)
- [Define the performance workload and statistics protocol](https://github.com/sagikazarmark/excalidraw-render.orig/issues/24)
- [Clarify vector-first PDF conformance and fallback boundaries](https://github.com/sagikazarmark/excalidraw-render.orig/issues/25)
- [Set the first-release WASM footprint policy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/27)
- [Choose compatibility revision 1 raster normalization and SVG passthrough policy](https://github.com/sagikazarmark/excalidraw-render.orig/issues/28)
- [Define public outcomes and serialized contracts](https://github.com/sagikazarmark/excalidraw-render.orig/issues/16)
- [Define the conformance comparison and approval protocol](https://github.com/sagikazarmark/excalidraw-render.orig/issues/21)
- [Materialize the compatibility revision 1 scene-surface manifest](https://github.com/sagikazarmark/excalidraw-render.orig/issues/29)
- [Add Excalidraw-compatible per-glyph SVG font subsetting](https://github.com/sagikazarmark/excalidraw-render.orig/issues/96)
