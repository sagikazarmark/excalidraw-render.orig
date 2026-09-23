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
| [Scene-surface manifest](../tests/oracle/scene-surfaces/revision-1.json) | `93029f2dab63c203dee69da4497ea4c2988a72d73f2a0017e62c529a4caac202` |
| [Scene-surface schema](../tests/oracle/scene-surfaces/scene-surface-v1.schema.json) | `c8af2987f69e320bbc2673e252284fda77a18ddaf667bf998055c47cd3aa7341` |
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
records remain silent. The manifest describes a file record's map key as the
pseudo-field `file_record` `/$key`; it binds the key of the `files` entry and
never a member of the record, so a member literally named `$key` is an
unmatched property like any other (#156).

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

The scene-export bounds are the union of the reference's `getElementBounds`
over every element frame clipping leaves visible, each kind measured as the
reference measures it, in its arithmetic, since the last ulp of the extent is
what the SVG `viewBox` prints and what decides a raster dimension near a half
pixel (#172, #173). A rectangle, embeddable, iframe, image, text element or
frame contributes the four corners of its box rotated about the box's centre
in `pointRotateRads`' order, `(x - cx) cos - (y - cy) sin + cx`. An ellipse
contributes its exact rotated extent, `hypot(w cos, h sin)` by
`hypot(h cos, w sin)` about its centre for its half extents, with `Math.hypot`
as V8 evaluates it (the arguments normalized to the larger and the root scaled
back). A diamond contributes its four vertices at the exact midpoints of the
box's sides, each rotated about the centre; the drawn vertices at
`floor(extent / 2) + 1` do not enter the bounds. A freedraw element contributes
its points rotated, in local coordinates, about the centre of their box. A
line or arrow contributes the analytic extent of its drawn Rough.js outline's
cubics after every control point is placed on the page and rotated about the
centre of the element's points' box — the centre `getElementAbsoluteCoords`
reads while the shape cache is cold, as it is while `getCanvasSize` measures a
freshly restored scene — where each cubic's extent is `getCubicBezierCurveBound`:
its end points and, for each axis, the values at the roots of the derivative
in `[0, 1]` from `solveQuadratic`'s spelling of the coefficients, with
`Math.pow(x, 3)` rounded once (the platform `pow` the pinned V8 defers to) and
`Math.pow(x, 2)` as `x · x`. Arrowheads and bound text take no part in a
linear element's own extent. An elbow arrow contributes the same analytic
extent of a Rough.js shape the reference measures it with (#176): not its
drawn elbow path but `generateLinearElementShape`, the shape
`getLinearElementRotatedBounds` generates while the shape cache is cold — a
`linearPath` over its route points with the element's own
`generateRoughOptions` (`preserveVertices` at the raw `roughness < 2`, its
control points jittered by the element's roughness and seed), or a `curve`
over them when the element carries a `roundness` record — rotated about the
centre of its points' box like every other linear kind. At roughness 0 the
shape has no jitter and its extrema are the route points; above it the extent
includes the jitter. The renderer generates that shape for the measurement
alone and charges it as generated geometry (Section 13.2).

The scene document's own export options — `appState.exportBackground`,
`exportWithDarkMode`, `exportScale`, and `exportEmbedScene` — are validated and
diagnosed under that manifest but never consumed: background, color mode,
scale, and embedding come from the request alone. In the pinned release's
`APP_STATE_STORAGE_CONF` all four carry `export: false`, so
`cleanAppStateForExport` strips them from every scene document the official
application writes; a value found in one is hand-edited, and honoring it would
darken or rescale a render nobody asked for. `frameRendering`, stored under the
same flag, is the one application-state export option a scene export honors:
each frame control is on only when the persisted member and the request's
control are both on.

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
`FRAME_STYLE.radius` 8, stroke `#bbb`, and width 2; the stroke passes through
the dark-mode filter like every other color, in every format. The reference SVG
strokes every frame kind `#bbb`, which SVG and PDF follow; the reference canvas
recolors a magic frame's outline `#7affd7` in the light theme and `#1d8264` in
the dark theme before the same filter, which PNG follows. The frame's own render
opacity (its `opacity` times its containing frame's, like every element's)
reaches the outline only where the reference applies it (#137): the reference
canvas paints the outline under `globalAlpha` set from that opacity, which PNG
and PDF follow; the reference SVG's frame `<rect>` carries no opacity
attribute, which SVG follows, so an SVG frame outline is always fully opaque.
In every format the frame's opacity still multiplies its members' opacity.

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

An element's `fillStyle` names one of the seven fillers Rough.js `4.6.4`'s
`getFiller` dispatches — `solid`, `hachure`, `cross-hatch`, `zigzag`,
`zigzag-line`, `dots`, and `dashed` — and the reference passes the value
through `generateRoughOptions` unvalidated, so every format renders all seven
through the ported fillers (#137); a value outside that domain takes the
manifest fallback, `solid`, with `compatibility.unsupported-property-value`.
Every pattern filler scans the element's fill polygon with `hachureGap` four
times the element's own `strokeWidth` and strokes its pattern at `fillWeight`
half of it, whatever a dashed or dotted stroke style adds to the outline's
width; the dots filler's ellipses alone also read `disableMultiStroke` and the
element's `curveFitting`. The dots filler is the one place the reference draws
from the unseeded `Math.random()`, for each dot's centre, so its own export of
a dotted fill is not reproducible from one run to the next. Revision 1 draws
that scatter from a second Rough.js generator seeded with the element seed and
leaves the seeded stream the dot ellipses consume untouched: the dot shapes are
the reference's own and the centres are one deterministic sample of its jitter.
For the same reason the dots fill has no official evidence in the conformance
corpus: the reference profile's generator requires two fresh browser processes
to export identical bytes and rejects a dotted scene, so the JS-derived unit
vectors in `render/tests.rs` and the fast test over the withdrawn dots scene
(`tests/fixtures/rough-fillers-dots.excalidraw`), both pinned against the npm
Rough.js through `tools/rough-filler-vectors/derive.mjs`, are its evidence
(#137). Every pattern filler's lines keep their endpoints on the fill polygon
or jitter them as the reference's `preserveVertices` decides: the option is
`continuousPath || roughness < 2` on the element's raw `roughness`, with
`continuousPath` set only for the shapes the reference draws as a Rough.js
path — the rounded rectangle, the rounded diamond and the elbow arrow — so a
rounded line or arrow, whose closed loop the reference fills as a curve, and a
freedraw loop drop their fill vertices at cartoonist roughness like an
unrounded shape does (#158). A freedraw loop's background is the reference's
`generator.curve` with `stroke: "none"` over the points-on-curve
simplification of its points at 0.75, drawn with the element's own seed and
adjusted roughness, which the scene surface admits for the freedraw kind as for
every other shape although the perfect-freehand stroke reads neither; the curve
generates its outline before the fill and `stroke: "none"` only drops it
afterwards, so the fill reads the shared seeded stream from after the
outline's `2 · (n + 2)` offsets for `n` simplified points (#170).

An arrow's heads are generated through the one Rough.js options object the
reference's `getArrowheadShapes` shares between the outline and both heads,
the start head before the end head, and each head kind reads and changes that
object as the reference does (#171): a `dot`, `circle` or `circle_outline`
draws at the lesser of 0.5 and the shared roughness, a `triangle`,
`triangle_outline`, `diamond` or `diamond_outline` at the lesser of 1 and it,
each on a copy, and each deletes the shared `strokeLineDash`; a `bar`,
`arrow`, `crowfoot_many` or `crowfoot_one_or_many` assigns the shared
roughness to the lesser of 1 and itself and the shared dash to the shortened
dotted pattern `[1.5, 4 + strokeWidth]` for a dotted element or to none,
so the head that follows inherits both; a `crowfoot_one` draws with the shared
object as it stands, its roughness unclamped and a dashed or dotted element's
own pattern kept unless an earlier head deleted or replaced it. The clamp is
therefore never carried from a dot-kind start head to the end head, and a
cartoonist `crowfoot_one` is as rough as its shaft.

Dark mode is the reference export's `THEME_FILTER`,
`invert(93%) hue-rotate(180deg)`, which the reference SVG carries on its root
element and the reference canvas sets on its context. Both apply it to the
composited image, so an antialiased pixel is the filter of a blended color,
which no per-color lowering reproduces where the filter clamps or rounds (#84).
SVG therefore carries the same root filter, as the attribute value
`filter="invert(93%) hue-rotate(180deg)"` on the root `<svg>`, and paints light
colors beneath it; only the frame name switches to the dark-theme constant, as
the reference SVG's does. PNG and PDF emit no filter and lower it into every
color they paint — scene colors, renderer-owned fallbacks, frame outlines and
names, and the PNG image placeholder — evaluated as the pinned browser
evaluates CSS filter functions: in gamma-encoded sRGB, with the Filter Effects
hue-rotation matrix, and each function's result clamped to the channel range.
PNG, whose authority is the canvas, floors the result to 8 bits; PDF, whose
authority is the official SVG, rounds it, as the pinned browser does for the
official SVG's root filter. Alpha is untouched. Images are the exception: Section 8.2 records how
each format lowers dark mode onto them.

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

PQ and HLG PNG content is tone mapped with the reference's default
reference-white tone map, whose peak is the content's positive `cLLI` maximum
content light level, else its positive `mDCV` maximum luminance, else 1000
nits, as the pinned codec reads them before the first `IDAT`: a malformed chunk
counts as absent, the first valid one wins, and `mDCV` must precede `PLTE`.
Content whose peak does not exceed the 203-nit reference white is not tone
mapped.

WebP is read as the pinned reference's demuxer reads it:
- Bytes after the declared RIFF end are ignored.
- A reserved VP8X flag bit rejects the file. Reserved VP8X bytes and reserved
  `ANMF` bits are ignored.
- A frame's size is its bitstream's.
- `EXIF`, `XMP ` and unknown chunks are ignored, whatever the VP8X flags
  declare.

WebP has no orientation: its EXIF is never applied. An embedded ICC profile
applies only to a still image whose VP8X ICC flag is set, and only when it
parses as an RGB profile; otherwise it is ignored silently.

GIF is read as the pinned reference's decoder reads it:
- Blocks:
  - Any extension is skipped whatever its label or block size, also in
    GIF87a.
  - Application data such as ICC or XMP is never applied.
  - A byte other than an extension or image introducer ends the stream.
  - A malformed graphic control extension rejects the file before the first
    image; after it, anything malformed ends the frame count.
  - Reserved bits, disposal values and the background index are ignored.
- Canvas: the logical screen grown to the first frame's extent. A zero-size
  first frame, or no image at all, draws a transparent screen.
- Pixels: an index outside the palette, or any index without a palette, is
  opaque black unless it is the transparent index.
- First frame: once its pixels are complete, the rest of its data is ignored.
  Where its data ends or fails before the pixels are complete, the file is
  rejected with the usual decode-failed recovery, although the reference
  paints the partial frame.
- Later frames only count; their data is not decoded.

BMP is read as the pinned reference's BMP reader reads it:
- Headers:
  - The file header needs only its signature and pixel-data offset.
  - The accepted info headers are OS/2 1.x, Windows V3 and later, and OS/2
    2.x, each with the depths and compressions the reference accepts.
  - A width or height of 65536 or more rejects the file.
- Pixels:
  - A palette index outside the palette is opaque black.
  - Channel masks may be empty and keep a wider channel's top eight bits.
  - Alpha comes only from a V3 or later alpha mask. An alpha channel that is
    zero throughout is opaque.
  - Run-length holes and anything after an early end of bitmap are
    transparent. A stream that ends or turns invalid keeps what it drew.
- Color:
  - Calibrated RGB is color managed, and an embedded ICC profile applies. An
    embedded profile that lies outside the file or does not parse rejects the
    file.
  - Every other color space type is sRGB.
- Non-run-length pixel data that ends before the last row, or masks that
  overlap or have gaps, reject the file with the usual decode-failed recovery,
  although the reference loads it and paints what it could decode.

ICO and CUR are read as the pinned reference's icon decoder reads them:
- Directory:
  - The directory's reserved fields, planes, byte sizes and bit counts may
    disagree with the images.
  - Every entry's image must start after the directory.
- Selection: the largest area wins, then the larger bit count, then the
  earlier entry. Only the selected entry is decoded.
- Entries:
  - An entry must decode to exactly the directory's size.
  - A DIB entry is read as BMP. Its AND mask applies unless a 32-bit entry
    has non-zero alpha.
  - A PNG entry follows the PNG rules, but its EXIF orientation is ignored.
- Divergences:
  - An entry that fails, or whose image lies outside the file, rejects the
    file with the usual decode-failed recovery, although the reference paints
    it transparent.
  - A PNG entry that needs a color transform draws as the same PNG would
    standalone, although the reference paints it fully transparent.

PNG and PDF emit one informational `image.animation-flattened` occurrence for
each actually rendered multi-frame asset occurrence. Static, one-frame, and
unreferenced assets remain silent. PDF uses the same default-frame, orientation,
and color interpretation and embeds lossless unpremultiplied canonical sRGB with
a grayscale soft mask. Source raster images are inherent raster content, not
PDF raster fallback.

SVG and PDF place an image the way the reference SVG export does, and PNG the
way its canvas export does. The reference draws an image through a `<use>` of
`Math.round(element.width)` by `Math.round(element.height)`, the image
viewport, and that is the only image value that rounds (Section 8.6); the flip
mirrors the viewport about the element's unrounded center, and the rotation
center, corner clip, and link rectangle keep the unrounded size. An uncropped
image fills the viewport, as the reference's `width="100%"` symbol image does
under the `reuseImages` export default the reference profile pins. A cropped
image is drawn whole at the reference's unrounded `getUncroppedWidthAndHeight`
size, shifted by the unrounded `normalizedCrop` offset so the crop's corner
sits at the element's origin, and is clipped twice as the reference clips it:
by a viewport-sized `<mask>` on the unflipped group and by the `<use>`
viewport, which spans from the crop offset to the rounded viewport's far edges
and flips with the image. SVG shares the payload the way the reference does
and inlines the rest of the geometry: each distinct asset is one `<symbol>` in
`<defs>` holding the `<image>` once, with the reference's
`preserveAspectRatio="none"` and `width="100%" height="100%"` (Section 8.4),
and each element draws it through a `<use>` sized to its content rectangle;
the mask and the `<use>` viewport are nested `<svg overflow="hidden">`
viewports around that `<use>`, and the flip a `translate(width height)
scale()` group about the same center. PDF, whose clips all live in
the element's space, installs the intersection of the two as one rectangle
clip before the content matrix that maps the asset onto its content rectangle,
and a nested SVG's form, whose Y axis svg2pdf inverts, is inverted back within
that rectangle. PNG maps the crop onto the element's unrounded rectangle, as
`drawImage(..., 0, 0, element.width, element.height)` does, so the geometry
stays fractional there while SVG and PDF round it; which device pixels that
rectangle paints follows the canvas as well (#162). For an image whose device
transform keeps rectangles rectangles — rotation 0, either flip, any export
scale — the pinned Chromium draws without edge antialiasing once the
destination is at least one device pixel wide and high, rounding each edge to
`floor(edge + 0.5)`: a device pixel is painted if and only if its centre lies
inside the destination rectangle, a centre on the left or top edge falling
outside and one on the right or bottom edge inside, so a fractional edge is a
whole row or column or none and never a blend, while the image is still
sampled bilinearly at the exact fractional mapping and the strict source
rectangle pads the snapped edge with the crop's own edge pixels. PNG paints
the same whole pixels, and resamples the image as the canvas does: once per
destination pixel, a bilinear sample at the pixel's centre under that mapping
(#165). The renderer's fixed supersample takes no part in an axis-aligned
raster image's sampling — each destination pixel paints one constant block of
subpixels, which the box filter returns unchanged — so its interior pixels are
the canvas's rather than a footprint one destination pixel wider; a 1:1 image
is its own texels. A rotated image, whose edges the supersample antialiases,
is sampled at every subpixel centre. A rotated image keeps antialiased edges, and the
rounded-corner clip and frame clips stay antialiased masks whatever the
transform, as the canvas's `clip` is, so a rounded image's straight fractional
edge is the whole column attenuated by the clip's coverage. A nested-SVG image
is not snapped: the canvas draws an SVG image as vectors (Blink
`SVGImage::DrawInternal`), antialiasing the document's content edge and its
viewport clip alike, inside a non-antialiased `clipRect` of the element
rectangle enlarged to whole element-space pixels, which Skia snaps to device
pixels by the same rounding. PNG follows: an axis-aligned nested-SVG image's
fractional edge row or column carries the square of its coverage, and a pixel
whose centre the snapped enclosing clip excludes carries nothing, as the
pinned reference profile measured for #162 (a 96x96 nested SVG at a quarter-
and half-pixel offset: 0.5625 in the left column, 0.25 in the bottom row, and
nothing in the right column or top row).

An image element with a `roundness` record is clipped to its rectangle with
`getCornerRadius(min(width, height))` corners in every format, as the
reference canvas clips it with `roundRect` before `drawImage`: SVG defines the
clip in `<defs>` beside the frame clips as a `<clipPath>` holding a `<rect>` of
the element's size with `rx`/`ry`, keyed by the image's source index, and
references it from the image's translated and rotated group; PNG intersects the
image's paint mask with the same rounded rectangle; PDF installs it as the
element-space clip in place of the rectangular one. The clip lives in the
element's own space, before crop, `scale` flips, and rotation, so those leave
its corners at the element's corners; a radius that is not positive clips
nothing the rectangle does not already clip and defines none. The clip
geometry is charged once per target in the semantic phase like a frame clip's.
The image/error fallback is never corner-clipped, as the reference canvas clips
only the image it draws. The reference SVG declares the same `<clipPath>` but
does not offset its rectangle by the crop and sets `clip-path` as a namespaced
attribute, as it does for frame clips, so browsers leave its rounded images
square while the reference canvas rounds them; revision 1 follows the canvas
and clips in every format. The namespaced attribute is the same reference
authority divergence Section 8.4 records for frame clips, and a `usvg`-based
comparison is blind to it in the same way for an uncropped rounded image:
only the canonical Chromium raster of the reference SVG shows the difference.
For a rounded and cropped image the clip `usvg` honours is the misplaced one,
declared at the crop-shifted group origin with the element's size, so under
one `resvg` the reference SVG shows only the image's top-left quadrant with one
rounded corner while revision 1 clips at the element rectangle as the canvas
does. The registered-case SVG harness sees that divergence and holds it as a
reference authority divergence pin (#163): the exact measurement, this
section, the issue, and the requirement that the variant's direct PNG passes
the canonical budget against the official canvas raster.

In dark mode the reference SVG puts `IMAGE_INVERT_FILTER`,
`invert(100%) hue-rotate(180deg) saturate(1.25)`, on each raster image's `<use>`
under its root `THEME_FILTER`, so a raster image keeps roughly its authored
appearance with its contrast compressed to about `#111111`–`#ededed`, while a
nested-SVG image receives the root filter alone and inverts with the scene. The
reference canvas export resets its filter to `none` for a raster image
(`shouldResetImageFilter`) and keeps the theme filter for a nested-SVG image
and for the image placeholder. Each format follows its authority: SVG defines
the image filter once in `<defs>` as Filter Effects primitives in sRGB (Section
8.4) and references it from each raster image element's `<use>`, where the
reference puts `IMAGE_INVERT_FILTER`, under the root theme filter, leaving the
shared payload octets exactly as validated; a nested-SVG image has no filter of
its own and inverts with the root; PNG leaves raster pixels alone, passes the pixels
it rasterizes from a nested-SVG image through the theme filter, and lowers the
placeholder's `#E7E7E7` rectangle and `#888` glyph the same way; PDF, whose
authority is official SVG (Section 8.6), lowers the image filter followed by
the theme filter into each visible raster pixel it embeds, leaving invisible
RGB at its canonical zero and the soft mask untouched, rounding the image
filter's result to 8 bits as the browser's intermediate surface does before the
theme filter reads it, and rounding the theme filter's result as Section 8.1
describes; an SVG viewer evaluates the filters itself. None of
this changes the semantic resource ledger: like every other color lowering, it
is not a charged event. PDF also lowers dark mode into a nested-SVG image's
content before `svg2pdf` converts it (#154): the sanitized tree is serialized
through the backend's lossless normalized writer, every `fill`, `stroke`,
`stop-color`, `flood-color` and `lighting-color` color and every visible pixel
of a nested raster (a normalized PNG) passes through the rounded theme filter,
alpha untouched, and the result is parsed again. `<mask>` content keeps its
light-mode colors, because a mask acts through their luminance while the
reference filters only the composite. Nested-SVG images inside the image are
inlined by that writer and lowered in the same pass. The content stays vector;
no fork of `svg2pdf` is involved.

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
controlled glyph-outline rasterization, with each line's baseline snapped to
the device pixel grid as the canvas snaps it (Section 8.5, #169); SVG and PDF
carry the exact baseline. No format consults installed fonts or OS text
services.

Liberation Sans and Noto Emoji replace local-only Helvetica/Arial and Segoe UI
Emoji behavior for ordinary scene text and generated export text. Ordinary
scene text and compatibility placeholder labels report the Liberation Sans
substitution with one informational `font.liberation-sans-substituted`
diagnostic per affected occurrence; frame names and generated embeddable
labels, whose Liberation Sans face is renderer-owned chrome rather than a
substitute for a source family the scene named, report none (#175). The Noto
Emoji substitution is reported with one informational
`font.noto-emoji-substituted` diagnostic per affected occurrence of ordinary
scene text or generated embeddable label. Missing codepoints use the
bundled deterministic replacement glyph at defined metrics and identify the
element and codepoint. A default-ignorable code point is not a missing
codepoint (#167): the class is the pinned shaper's (`rustybuzz 0.20.1`,
HarfBuzz's `is_default_ignorable`, the Unicode `Default_Ignorable_Code_Point`
property less the Hangul fillers U+115F, U+1160, U+3164 and U+FFA0) — the
bidi controls U+200E, U+200F, U+202A–U+202E and U+2060–U+206F, the joiners
U+200B–U+200D, the soft hyphen U+00AD, U+034F, U+061C, U+17B4–U+17B5,
U+180B–U+180E, the variation selectors U+FE00–U+FE0F, U+FEFF, U+FFF0–U+FFF8,
U+1D173–U+1D17A and U+E0000–U+E0FFF. The browser draws nothing for any of
them, whether or not a face has a glyph, and so does revision 1 in every
format: such a scalar stays in the display text and in its bidi visual run
(the bidi marks are exactly what UAX #9 reads), is shaped by the face of the
nearest other scalar of that run (the preceding one in logical order, else the
following, else the whole-text face), is hidden by the shaper as one glyph of
zero advance that draws nothing, pays no font fallback probe, is excluded from
the scalars the whole-text face must declare and map, and reports no
diagnostic. The paragraph separator U+2029 is not in the class and stays the
missing codepoint #125 records.

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
`unicode-bidi 0.3.18` implements it (#122), at a base level that is the text
element's, not the line's (#166). The reference detects the direction once per
element with its `isRTL(element.text)` and sets the canvas `dir` attribute and
every line's SVG `direction` from it, so revision 1 detects the base level
once over the stored `text` with the same rule and lays out every source line,
and every UAX #9 paragraph within one, at that level (`BidiInfo::new(line,
Some(level))`), never from a line's own first strong character. The rule is
the reference's regex, not P2/P3: reading the text's UTF-16 code units from
the start and skipping every unit in neither class, the element is
right-to-left when the first unit in either class is in
`\u0591-\u07FF\uFB1D-\uFDFD\uFE70-\uFEFC` and left-to-right when it is in
`A-Za-z\xC0-\xD6\xD8-\xF6\xF8-\u02B8\u0300-\u0590\u0800-\u1FFF\u2C00-\uFB1C\uFDFE-\uFE6F\uFEFD-\uFFFF`
or the text has no unit in either class; digits, spaces, punctuation and line
terminators are skipped, combining marks and, through their surrogates, every
supplementary scalar count as left-to-right, and Arabic digits as
right-to-left. A right-to-left element therefore reorders a line that starts
with Latin the way the canvas does (`Hello <Arabic> 123 world` reads
`world 123 <Arabic> Hello`), and a left-to-right element keeps a trailing
neutral of a Hebrew line at the right; every line of a right-to-left element
takes the UAX #9 layout even when none of its scalars is right-to-left. The
line's resolved levels are reordered into visual runs. Only the line
terminators above end a source line, so a UAX #9 paragraph separator inside
one (U+2029, U+0085, U+001C–U+001E) is admitted content that splits the line
into paragraphs (P1): every paragraph is laid out, in order, at the element's
level, and the separator itself is shaped with the paragraph it ends, as the
missing codepoint it is in every bundled face (#125). Every format lays
those runs out left to right in visual order, and each visual run is split
further by the face that shapes its scalars under 8.3.1, so one visual run of
one face is one glyph run; the face runs of a right-to-left visual run follow
in visual order too, the logically first rightmost (#126). SVG emits each
glyph run as one `<tspan>` carrying that face's `font-family` list (Section
8.4, the effective family first), `direction="rtl"` or `"ltr"`, and one `x`
and `y` per character of its content, in logical order, each the position of
the glyph that shows that character; PNG and PDF place the same glyph
positions. Within a run, glyph
order and placement are the deterministic shaper's for the run's display text
shaped in the run's direction (#126): a right-to-left run's glyphs are placed
in visual order, leftmost first, so an asymmetric neutral inside it sits at
its visual end, and a mirrored character shows its mirror glyph (UAX #9 L4)
when the face has one. The text a format carries beside the glyphs stays in
logical order (`data-source-text`, `ActualText`, and each run's `<tspan>`
content), as do missing-codepoint diagnostics. A left-to-right occurrence with
no right-to-left content whose shaped scalars the whole-text face all maps is
one run of that face per line and emits no direction.

The reference SVG's anchoring of a right-aligned right-to-left element is a
reference authority divergence (#168), of the same shape as the frame clip and
rounded-image clip of Sections 8.2 and 8.4. The exporter writes every
right-to-left element (its `isRTL`, above) as
`<text direction="rtl" text-anchor="end">` with `x` at `0`, `width / 2` or
`width` for `textAlign` left, center or right
(`textAlign === "center" ? "middle" : textAlign === "right" || direction ===
"rtl" ? "end" : "start"`). Under SVG semantics, and in the pinned Chromium,
`end` under `direction: rtl` places the text's left edge at `x`: a left-aligned
element therefore sits where the canvas draws it, while a right-aligned one has
its left edge at the element's right edge and overflows it, where the canvas
(`textAlign = "right"`, a physical alignment) right-aligns it; Chromium's
raster of the official SVG shows only the first glyph's columns inside the
export. Revision 1 follows the canvas in every format: its SVG positions every
glyph of a right-to-left or mixed line explicitly (Section 8.4), and PNG and
PDF place the same glyphs, so a right-aligned right-to-left line ends at the
element's right edge. Like the clip divergences this is recorded here rather
than diagnosed or excepted, and a harness observes it only when the official
SVG's browser raster is its authority.

The registered-case SVG harness cannot judge right-to-left alignment on its
raster dimension at all, for a reason of its own (#168): `usvg 0.45.1` (and
0.47) parses `direction` but never reads it, resolving `text-anchor` as if
every text were left-to-right and running UAX #9 at a left-to-right base level,
and it lays out only the first UAX #9 paragraph of a `<text>`. Under it the
official document's left-aligned right-to-left elements land off the canvas,
its right-aligned ones coincide with the canvas by accident, and a
right-to-left line's digits and neutrals take left-to-right positions; a
`<text>` holding a U+2029 shows its first paragraph only. The SVG document
parity of that harness (extent and path data) and the direct PNG against the
official canvas raster are the gates for right-to-left text; the raster
dimension holds such a variant as a rasterizer limitation pin
(`tests/conformance/README.md`), at its exact measurement, requiring the
variant's direct PNG to be where its record says it is.

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
complete ordered family scalar set, not only its range intersection. The Noto
Emoji fallback is demanded last, after every resolved family, as its own
one-face family whose single declared range covers all of Unicode (#150): its
scalar set is the first-use-ordered set of scalars glyph face selection shaped
with it, collected from the emoji glyph runs of ordinary text in source order
and then of generated embeddable labels in paint order, so it is selected
exactly when an emoji run exists and receives no scalar a family face mapped.
It goes through the same subsetter as the chunks; the one difference is that
its source is the manifested TrueType sfnt rather than a WOFF2 container, so
its decode stage is the identity and its decoded length is its manifested
length. Only qualified, digest-verified built-in assets participate. Custom
fonts and a public subset interface are forbidden.

Glyph face selection is the same in every format, for ordinary scene text and
generated embeddable labels alike, and consults declared Unicode ranges before
any `cmap`. A resolved family's whole-text face is fixed once per target, when
the family's first live occurrence is laid out: the first official font chunk
of the family, in pinned face order, whose declared ranges cover every shaped
scalar of that occurrence other than its default-ignorable code points (8.3,
#167) and whose `cmap` maps them all; failing that, the family's first chunk
declaring and mapping `?` and `A`. Every later occurrence of the family shapes
with that face. A shaped scalar the whole-text face maps takes it, and a
default-ignorable code point takes its run's face without consulting any. Any
other shaped scalar consults, in this order and stopping at the
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
scalars included; a default-ignorable code point consults no face and pays
none. Once the family's whole-text face is fixed, whether a
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

Non-Reserved-Font-Name families retain their canonical family names, Noto
Emoji among them: its OFL reserves no font name, so its subset's alias is
`Noto Emoji`. The same
effective alias MUST be used by SVG text, CSS, and primary OpenType names; SVG
text names it first in the `font-family` list Section 8.4 fixes per family.

Subset semantic identity is SHA-256 over this exact preimage, where digests are
their 32 raw bytes and asset identity is UTF-8; for the Noto Emoji fallback the
source asset identity is its manifested path and digest
(`scripts/woff2/assets/NotoEmoji-Regular.ttf@sha256:…`) and the source asset
digest is the digest of the sfnt itself:

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
`skera 0.7.0`; otherwise a pinned controlled fork is required. The encoder MUST
first qualify `ttf2woff2 0.13.3` with `brotli 9.0.0` and `portable-float` enabled;
otherwise a pinned controlled fork is required.
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

Every element's group carries the reference's
`translate(x y) rotate(degrees cx cy)`, the rotation about the centre of the
box `getElementAbsoluteCoords` returns for the element. For a line or arrow
(an elbow arrow included; a freedraw is not a linear element to that
function) the box depends on the state of the reference's `ShapeCache` when
it is read, and the two official export paths read it in opposite states
(#177): `renderElementToSvg` reads the coordinates before it generates the
shape, on a freshly restored element whose cache is cold, so the official SVG
rotates a linear element about the centre of its **points'** box, while the
canvas `renderElement` generates the shape first and reads the coordinates
after, so the official PNG rotates it about the centre of the drawn
**curve's** box, the cubic extent of its Rough.js outline (Sections 8.5 and
8.6). The two centres differ by the offset the roughness jitter gives the
curve's box against the points' box — up to a couple of pixels at roughness 1
to 2, zero at roughness 0 — and rotation about the one rather than the other
moves the drawn shape by `(R − I)(c₁ − c₂)`, so a rotated rough line or arrow
sits at a slightly different place in the official SVG than in the official
PNG. That is a format-specific difference the reference itself carries, and
revision 1 preserves it: the SVG target writes `rotate(degrees cx cy)` about
the points' box centre, in the reference's own arithmetic
(`(x2 - x1) / 2 - (element.x - x1)` over the page coordinates of the points'
box, so the attribute matches the official export to the last digit), and
PNG and PDF rotate about the curve's box centre. It is not a reference
authority divergence: neither export is defective, each rotates about the box
it read. A rotated rough linear element's SVG and PNG are therefore judged
against the official artifact of their own format, never against each other.

`<defs>` contains exactly one font style element, `<style class="style-fonts">`,
when direct-export fonts are used. It contains one `@font-face` declaration for
each selected subset — the resolved families' chunks in canonical
fallback/family/face order, then the Noto Emoji fallback's when an emoji run
selected it (Section 8.3.1). A declaration contains only the effective
`font-family` and data-URL `src`, and every declaration's payload is a WOFF2
subset; no complete face is embedded (#150). Range and source-family
properties are not emitted. CSS, data URL, base64, declaration, and style
serialization are canonical. All selected subsets share this one style block;
no second font style element is emitted. The artifact manifest carries the
Section 12 font notices: one attribution notice per subset family, each
applicable license text once, and distinct mandatory subset notices, all in
canonical identity order.

Before the style element, `<defs>` defines each referenced frame clip once, in
first-use order, as a `<clipPath>` holding the frame rectangle with the frame's
transform and, in a scene export, `FRAME_STYLE.radius` corners; the rectangular
frame-export clip has none. Every clipped element is wrapped in one
`<g clip-path>` referencing its frame's definition, so a frame's clip is never
repeated per child. Clip IDs are renderer-owned and keyed by the frame's index
in the source `elements` array; an element id is untrusted input and is not
used as an XML ID. The clip rectangle rotates by the frame's angle in degrees,
as the outline does; revision 1 does not reproduce the reference's
radians-valued `rotate()` on its clip definition. The frame outline itself is a
`<rect>` in a translated and rotated group that carries no `opacity`, as the
reference SVG's frame `<rect>` carries none (Section 8.1); the frame name
precedes it as its own fully opaque text group.

Nor does revision 1 reproduce the reference's attribute spelling. The reference
SVG sets each clipped group's `clip-path` through `setAttributeNS` with the SVG
namespace, so it serializes as
`<g xmlns:nsN="http://www.w3.org/2000/svg" nsN:clip-path="url(#id)">`, and
the pinned Chromium does not read a namespaced attribute as the `clip-path`
presentation attribute: the reference SVG paints a frame's children unclipped
there while the reference canvas clips them. This is a reference authority
divergence, not a format difference the revision preserves. Revision 1 emits
the plain `clip-path` attribute and follows the canvas in every format, as it
does for the rounded-image corner clip the same spelling affects (Section 8.2)
and for the anchoring of right-aligned right-to-left text (Section 8.3, #168),
whose `text-anchor="end"` under `direction="rtl"` the reference SVG writes and
browsers place off the element.
A `usvg`-based rasterization accepts an attribute in the SVG namespace as the
property it names and clips the reference SVG anyway, so neither the in-process
`resvg` raster dimension of the registered-case SVG harness nor the pure-Rust
SVG viewer (Section 14.3) observes the divergence; only the canonical Chromium
raster of the reference SVG shows it. A scene-export PDF gate that took that
raster as its authority (Section 8.6) would therefore disagree with a correct
PDF wherever a frame's children overflow the frame. The frame-export gate is
unaffected today: in every target it exports whose children are clipped, the
clip is an unrotated frame rectangle coinciding with the zero-padding page, so
what the clip removes lies off the page; a rotated target with overflowing
children would expose the divergence there too.

The frame clips are followed in `<defs>` by each rounded image's corner clip
(Section 8.2), in paint order, keyed `image-clip-<source index>`; then by the
shared image definitions; and then, in a dark-mode export with a raster image,
by the dark-mode image filter `dark-mode-image-invert`, defined once and
standing for `IMAGE_INVERT_FILTER` (Section 8.2). It spells its CSS filter
functions as the Filter Effects primitives they are defined as —
`feComponentTransfer` tables for `invert(100%)`, `feColorMatrix` `hueRotate` for
`hue-rotate(180deg)`, and for `saturate(1.25)` an explicit `feColorMatrix
type="matrix"` carrying the Filter Effects saturate matrix at `s = 1.25`
(`1.19675 -0.17875 -0.018 0 0 -0.05325 1.07125 -0.018 0 0 -0.05325 -0.17875
1.232 0 0 0 0 0 1 0`), never `type="saturate"`, whose SVG 1.1 domain is
`[0, 1]` and which `usvg`-based viewers clamp to the identity while the CSS
function keeps any positive amount (#161) — with
`color-interpolation-filters="sRGB"`, and each raster image element's `<use>`
references it by `filter="url(#…)"`. A dark-mode export's root `<svg>` carries
`filter="invert(93%) hue-rotate(180deg)"` after its `height`, the reference's
own spelling (Section 8.1); a viewer that ignores CSS filter functions shows the
light-mode colors. A light-mode export defines and references no filter.

Every accepted image asset is defined once, as the reference's `reuseImages`
export defines it (#151): `<symbol id="image-<source index>"><image
href="data:…" preserveAspectRatio="none" width="100%"
height="100%"/></symbol>`, in first-use paint order, one per semantic asset
identity, so two file records or nested payloads that decode to the same
validated payload share one definition whatever their ids, and two elements
that crop or size the same asset differently share it too. The key is the
source index of the first element in paint order that draws the asset, never a
file or element id. Each image element draws its definition through
`<use href="#image-<source index>" width height>` sized to its content
rectangle (Section 8.2), inside its own translated and rotated group, corner
clip, crop viewports and flip. The symbol carries no `overflow` attribute,
like the reference's, so its viewport clips as a symbol's does by default
(#164). That clip is load-bearing for a nested-SVG payload whose content
overshoots its own `viewBox`: a `usvg`-based viewer paints the overshoot
outside the image viewport when the symbol declares `overflow="visible"`, and
clips it, as the reference canvas and Chromium do, when it does not. It also
keeps the document rasterizing like the official one under the same viewer:
under the in-process `resvg` of the registered-case SVG harness the
`focused.image-codecs.v1` document is pixel-identical to the official export
with the default overflow and differs by 144 pixels with `overflow="visible"`;
under the pinned Chromium the two spellings paint the official evidence's
integer-edged images identically, and at a fractional image position the
symbol's viewport clip attenuates the image's own antialiased edge a second
time in both the official document and revision 1's, which is the reference
SVG's appearance rather than a divergence from it. The viewports that clip a
cropped image, its mask and `<use>` viewport, are the nested
`<svg overflow="hidden">` elements around the `<use>`. The reference keys its
symbols by file id and gives a cropped image a second symbol per uncropped
size; revision 1 does not reproduce either.

Text markup targets browser rendering, where the embedded fonts resolve and the
document renders as the reference SVG does; a consumer that ignores
`@font-face`, embedded fonts, or per-character positioning is served on a
best-effort basis and is not a parity authority (#151). Ordinary scene text is
one `<text>` per element carrying `data-source-text` (Section 8.3),
`xml:space="preserve"`, `fill`, `font-family`, `font-size`, and `text-anchor`,
with one `<tspan x y>` per source line whose content is the line's display text
and, for a line with right-to-left content or more than one shaping face, one
`<tspan>` per glyph run instead (Section 8.3). Nothing separates one line's
`<tspan>`s from the next: under `xml:space="preserve"` a separator would be a
rendered character joining the preceding text chunk, and a trailing space would
shift every middle- or end-anchored line off the position the reference gives
it, so the DOM `textContent` of a multi-line element concatenates its lines
while the line breaks live in `data-source-text`. A right-to-left or mixed-face
line lists one `x` and `y` per character of each run (Section 8.3), which fixes
the glyph positions the deterministic shaper chose and disables a consumer's
own kerning and bidi reordering for that run. Generated export text (frame
names, embeddable labels, compatibility-placeholder labels) is written the same
way.

Every `font-family` attribute the renderer writes, on `<text>` and `<tspan>`
alike, is a list (#150, #151): the effective family of the face that shapes the
text first, so a viewer that honors the embedded fonts, and the parity harness,
resolve exactly that face; then, where the effective family is a Reserved Font
Name alias, the canonical family name; for Liberation Sans also `Helvetica`,
the local family it substitutes (Section 8.3); then the reference export's own
fallback chain for the family, as `getFontFamilyFallbacks` orders it (`Xiaolai`
for Excalifont, then `Segoe UI Emoji` for every family); and last a generic
family of the same class, `monospace` for Cascadia Code and Comic Shanns and
`sans-serif` otherwise. Names are spelled bare and comma-separated, as the
reference spells `Segoe UI Emoji`. The lists are fixed per resolved family:

| Face | `font-family` |
| --- | --- |
| Excalifont | `Excalifont, Xiaolai, Segoe UI Emoji, sans-serif` |
| Virgil | `ExcalidrawRender-1, Virgil, Segoe UI Emoji, sans-serif` |
| Cascadia Code | `ExcalidrawRender-3, Cascadia Code, Segoe UI Emoji, monospace` |
| Lilita One | `ExcalidrawRender-7, Lilita One, Segoe UI Emoji, sans-serif` |
| Liberation Sans | `ExcalidrawRender-9, Liberation Sans, Helvetica, Segoe UI Emoji, sans-serif` |
| Nunito | `Nunito, Segoe UI Emoji, sans-serif` |
| Comic Shanns | `Comic Shanns, Segoe UI Emoji, monospace` |
| Xiaolai | `Xiaolai, Segoe UI Emoji, sans-serif` |
| Noto Emoji | `Noto Emoji, Segoe UI Emoji, sans-serif` |

A list names families the document does not embed (Xiaolai when the CJK subset
trigger did not select it, Segoe UI Emoji always); they are reached only by a
viewer that cannot use the embedded face, since every scalar a run contains is
mapped by the face its list names first. The `@font-face` declarations keep
naming one effective family each.

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

A line or arrow, an elbow arrow included, rotates about the centre of its drawn
Rough.js outline's cubic extent — the box `getElementAbsoluteCoords` returns to
the canvas `renderElement`, which generates the element's shape before it reads
the coordinates — where the SVG rotates it about its points' box (Section 8.4,
#177). Every other kind rotates about its element box, a freedraw about its
points' box, in every format alike.

Edge coverage follows the reference canvas: paths, glyphs, clips and rotated
images are antialiased by area coverage, and an axis-aligned image's
destination rectangle is painted by pixel-centre coverage with no edge
antialiasing (Section 8.2, #162), which the fixed supersample reproduces
exactly because the snapped edges land on whole subpixels. Inside that
rectangle an axis-aligned raster image is resampled once per destination
pixel centre and painted as constant subpixel blocks (Section 8.2, #165), so
the supersample neither blurs nor widens it.

Text placement follows the canvas's text rasterization as well (#169). The
canvas draws every line of ordinary scene text, every frame name and every
generated embeddable label with `fillText`. PNG MUST position each shaped
glyph independently on the reference canvas's device-phase grid; a single
whole-line shift does not reproduce the different phases of a rotated run.
The phase is selected after the complete element transform and export scale,
in final device pixels, before multiplication by the internal supersample.

For local glyph origin `(gx, gy)` and local-to-device affine transform
`(a,b,c,d,e,f)`, let `X = a*gx + c*gy + e` and `Y = b*gx + d*gy + f`.
An axis-aligned baseline uses quarter-pixel phases along the baseline and
whole pixels on the perpendicular device axis: a horizontal baseline uses
`(qx,qy) = (4,1)`, and a vertical baseline uses `(1,4)`. Alignment is in device
space; a quarter-turn baseline is vertical, not an oblique baseline merely
because the source angle is nonzero. Other, non-axis-aligned rotated
baselines use `(4,4)`.

Classify alignment using the pinned Skia font-cache copy of the device matrix,
not an arbitrary angular epsilon or raw `a == 0` / `b == 0` comparisons. For
each coefficient `v`, admit it to binary32, multiply by `1024` in binary32,
promote that product to binary64, then compute `floor(product + 0.5)`. The
cache coefficient is that rounded value divided by `1024`; only whether it
is zero affects classification. Test cached `b == 0` first for horizontal
alignment, otherwise cached `a == 0` for vertical alignment, otherwise neither.
This matrix includes export scale, but excludes font size and the internal
supersample. In particular, admitted `+1/2048` rounds away from zero while
`-1/2048` rounds to zero. The cache copy is used only to choose the phase grid:
the actual device transform, glyph contours, and path-cutoff calculation retain
their original precision.

For either device coordinate, apply:

```text
snap(v, q) = floor(v*q + 0.5) / q
dx = snap(X, qx) - X
dy = snap(Y, qy) - Y
```

Translate only that glyph's transformed contours by `(dx,dy)`. Do not round
each contour point or change advances, kerning, shaping offsets, or layout.
Halfway ties choose the more positive device coordinate, including at
negative coordinates: `snap(0.125,4) = 0.25`, `snap(-0.125,4) = 0`, and
`snap(-0.5,1) = 0`. This is nearest-phase rounding with the pinned Skia
eighth-pixel bias on quarter-pixel axes, not ties-to-even or ties-away-from-zero.

Glyphs drawn through the large-text path route retain exact transformed
origins. For font size `F` in local logical units, bypass the phase snap if
either `F*F*(a*a+b*b) > 256*256` or `F*F*(c*c+d*d) > 256*256`. Equality stays
on the mask-phase route. The threshold is in final device pixels, not local
font units or supersampled pixels; a bypass adds no origin displacement.

These are PNG raster-placement operations alone. Shared layout, glyph origins,
advances and outlines remain exact; SVG and PDF retain exact fractional
coordinates, including baselines (the official SVG declares fractional `y`
too). The compatibility placeholder's label, which the canvas never draws,
keeps its exact placement. Semantic pixel-coverage charges continue to use
the exact outlines (Section 13.2). The scoped draft amendment, pinned source
citations, boundary evidence and implementation assessment are recorded in
[`stage4-png-device-phase-assessment.md`](research/stage4-png-device-phase-assessment.md).

Residual coverage differences include the rasterizers' antialiasing of placed
outlines and, for Helvetica-family text, the reference profile's own
rendering of it: the reference has no bundled Helvetica and draws it with the
container's system Liberation Sans, whose render parameters the pinned
Chromium takes from fontconfig rather than from `--font-render-hinting=none`,
so that face is drawn with FreeType's light autohinting — its x-height snapped
to the pixel grid (`7.40 → 8` px at 14 px, `10.57 → 11` px at 20 px) and the
glyph image stretched upward from the snapped baseline. The renderer, whose
Liberation Sans shapes at the exact outline, follows the baseline but not the
hinting, so a Helvetica-family line's bottom row agrees and its x-height row
sits one row lower than the reference's.

### 8.6 Vector-First PDF

PDF is a direct semantic projection, not a serialized-SVG pivot. Official SVG
reference evidence is the transitive visual authority for geometry, paint order,
frames, clipping, text, links, and images. Official PNG is not authoritative
where formats differ, and no official PDF reference evidence is implied.

One placement follows the canvas rather than the official SVG: a line or arrow,
an elbow arrow included, rotates about the centre of its drawn Rough.js
outline's cubic extent, as PNG does (Section 8.5), where the official SVG
rotates it about its points' box (Section 8.4, #177). The two coincide at
roughness 0; for a rotated rough linear element the PDF's shape sits where the
official PNG's does, up to a couple of pixels from where the official SVG's
does, so a PDF gate whose authority is the official SVG raster sees that
displacement there exactly as the direct-PNG harness would see the opposite one
if PNG followed the SVG.

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

Dark mode is lowered into vector colors and image pixels before this matrix,
rounded as the official SVG's root filter rounds, nested-SVG content included
(Sections 8.1 and 8.2). Failure to encode a required operation is fatal, never
fallback. Admitted scene raster pixels are lossless image XObjects and inherent
source content.

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
reflow, outline, rasterize, subset, or consult host services, and never
rewrites a program's own name table; the PDF-level names of an official font
chunk's program carry the subset tag below. Only the reviewed decodes of
chunks selected for glyphs actually used are embedded, each as one complete
`FontFile2` stream, byte for byte before the stream filter; the complete Noto
Emoji fallback, when selected, is embedded the same way as its exact
digest-verified manifested bytes (Section 10), which have no decode. This is
deliberate and stays so after direct SVG began subsetting the fallback (#150):
#120 admits only reviewed bytes into a PDF and ADR-0001 makes any subset a
Modified Version, so PDF subsetting of the fallback is foreclosed on the same
grounds as PDF subsetting of the chunks and is out of scope; the Flate filter
below is what bounds the fallback's PDF cost.

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

Every stream the renderer itself writes is one `/Filter /FlateDecode` zlib
stream produced by the pinned `miniz_oxide 0.8.9` at compression level 6 with
the default strategy, the configuration the PNG encoder uses for `IDAT`
(ADR-0004, ADR-0008): the page content stream, the fallback Form XObject
streams, the ICC profile, the `scene.excalidraw` embedded file, every
`FontFile2` program (whose `/Length1` is the complete sfnt length), every
`ToUnicode` CMap and `CIDToGIDMap`, and each image's RGB plane and gray soft
mask. Nested-SVG Form chunks are requested from `svg2pdf` with compression on,
which deflates their content streams through the same crate at the same
level. The one stored stream is the XMP metadata stream: PDF/A requires the
metadata stream to be readable without a filter, and the file carries a
`GTS_PDFA1` output intent. No other filter, predictor, or filter chain is
emitted.

Every embedded program that is an official font chunk — a Unicode-range chunk
of its family, including the single Liberation Sans chunk — is named with a
subset tag on the Type0 `/BaseFont`, the CIDFontType2 `/BaseFont`, and the
descriptor `/FontName`: six uppercase letters and `+` before the family's PDF
name, the letters spelling the first six nibbles of the chunk's manifested
WOFF2 digest with `A` for 0 through `P` for 15, so two chunks of one family
never share a name (`OEECDL+Excalifont`, `AAGKCL+LiberationSans`). The
complete Noto Emoji face is not a subset and carries no tag. The `ToUnicode`
`/CMapName` is the untagged `<PDF name>-UCS`.

The trailer carries `/ID [<hex> <hex>]` with both entries equal: the first 16
bytes of the SHA-256 over every byte of the file preceding the `trailer`
keyword (the header, the objects, and the cross-reference table), as 32
uppercase hexadecimal digits. No `/Info` dictionary is written, and the XMP
carries no `pdfaid` schema: the output intent is stated, but no PDF/A
conformance is claimed.

Complete bytes are stable through fixed semantic object numbering, classic
xref/trailer, one revision, canonical decimals and zero, the fixed stream
filter and Flate configuration above, the content-derived file identifier, and
fixed dictionary and metadata order.

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
Transcendental functions (`sin`, `cos`, `atan2`, `pow`, `hypot`, `cbrt`, and any
later addition) are called only through one package-private module that routes
them to the pinned pure-Rust `libm` crate so every supported target computes the
same double, `powi` is not used, and only the functions IEEE 754 makes exact or
correctly rounded (`sqrt`, `abs`, rounding, `min`/`max`, `copysign`,
`rem_euclid`, `mul_add`) may be called on `std` (ADR-0007). No observable order
may depend on hash iteration.

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

SVG and PDF carry the Section 12 font notices for every embedded face, each
license text once and one attribution per family, SVG additionally the
distinct subset notices, the Noto Emoji fallback's included when its subset is
embedded, plus complete notices for rendered Font Awesome fallbacks. PNG
carries Font Awesome attribution in deterministic `iTXt` only
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

For nested resource-limit recovery in SVG/PDF, a rejected SVG payload carries
`semantic` even when the limit prevents inspecting its text/vector content.
Classification uses a fixed prefix of at most 48 decoded bytes without parsing
or fully decoding the rejected payload: a recognized raster signature selects
visual-only loss; SVG or unidentified content conservatively selects potential
semantic loss. A media-type claim does not override the signature. PNG adds no
semantic effect for this recovery. This applies to placeholders and omissions.

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
{"kind":"nested_resource_limit","field":<string>,"configured":<resource-value>,"value_kind":<value-kind>,"measurement":<resource-measurement>,"asset_chain":[<asset-chain-entry>],"paths":[<nested-path>],"recovery":<"placeholder"|"omitted">,"recipe_id":<string|null>}
{"kind":"nested_asset_claim","asset_chain":[<asset-chain-entry>],"paths":[<nested-path>],"claimed_media_type":<string>,"validated_media_type":<string>,"recovery":"repaired"}
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
| `asset.media-type-mismatch` | `asset` for top-level claims, `nested_asset_claim` for nested data-URL claims; `repaired`; both media types (top-level values nullable until validation) |
| Asset/image failures | `asset` or `nested_source`; format recovery and manifest recipe |
| `image.animation-flattened` | `asset`; `flattened`; null recipe |
| `image.unsafe-svg-content-removed` | `nested_source`; `removed` |
| `link.unsafe-omitted` | `compatibility`; `omitted`; null recipe |
| `color.invalid` and font substitution | `compatibility`; `substituted` |
| `font.missing-glyph` | `missing_glyph`; Unicode scalar; `substituted` |
| Recoverable resource diagnostics | `resource_limit` for top-level crossings; `nested_resource_limit` for nested image slots, with exact limit/measurement, full source chain/path, placeholder or omitted recovery and the nested-resource recipe only when painted |
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
| `resource.image-pixels-exceeded` | `max_decoded_pixels_per_asset` | pixels |
| `resource.nested-svg-complexity-exceeded` | `max_nested_svg_nodes` | count |
| `resource.nested-asset-depth-exceeded` | `max_nested_asset_depth` | depth |
| `resource.coordinate-range-exceeded` | `max_coordinate_magnitude` | coordinate |
| `resource.optional-metadata-omitted` | exact crossed limit | that limit's kind |

A PNG compatibility placeholder's raster surface has no local ceiling: it is
the placeholder's rectangle clamped to the canvas, so it is at most the main
raster surface `max_raster_surface_pixels` has already admitted (a fatal
`resource.raster-surface-exceeded` when it has not), and it is charged against
`max_total_rasterized_pixels` alone. Revision 1 carried an unreachable local
crossing of that ceiling for the placeholder while a draft (#157, #159); no
retained report can have carried it.

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
| `resource.coordinate-range-exceeded` | `max_coordinate_magnitude`; selected frame viewport only |
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
Top-level image-dimension and per-asset decoded-pixel limit diagnostics are
intrinsic: emit once per authoritative file ID per target, with null
`element_id`, even when several image elements reference that file. Each
reference independently keeps its format- and status-selected recovery.
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
and carries every identity once. A font's notice is two objects (#151): an
attribution notice whose `asset_identity` is the family's canonical license
identity (`font.cascadia-code.ofl-1.1`) and whose `text` is the family's
copyright and Reserved Font Name statement, the license sentence, and the
identity of the license body it refers to; and the license body itself, whose
`asset_identity` is the canonical identity of that license text
(`license.ofl-1.1` for the SIL Open Font License 1.1, `license.comic-shanns.mit`
for the Comic Shanns MIT text, which carries its own copyright line) and whose
`text` is the license text as the license bundle distributes it. Every family
licensed under the same text refers to the one body, so an artifact embedding
four OFL fonts carries the OFL once and four attribution notices; OFL §2's
"the above copyright notice and this license" is met by the attribution
notices together with the body. The same two objects cover an official font
chunk a PDF embeds complete and a subset direct SVG derives from it.
Every subset identity still contributes one distinct existing-schema notice object:
its `asset_identity` is the full subset semantic identity, and its canonical
`text` contains source identity and digest, profile digest, output digest and
length, effective alias, and transformation statement; OFL output is identified
as a Modified Version under the family's license identity. Equal subset notice
text is not deduplicated.
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

PDF attaches the raw canonical scene bytes as `scene.excalidraw` with media
type `application/vnd.excalidraw+json`; it does not use the wrapper. The
`EmbeddedFile` stream body is Flate-filtered like every other renderer-written
stream (Section 8.6); its decoded content is the canonical scene bytes and
nothing else. The artifact manifest is the sole custom value in this exact
bare, unpadded UTF-8 XMP tree, carried in the one stream the file leaves
stored:

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
integer conversion. Main PNG side or surface exhaustion is fatal; a
compatibility placeholder's surface, clamped to that admitted canvas, is
charged against the aggregate only (#159).

`max_coordinate_magnitude` applies to every normalized coordinate, dimension,
control point, translated transform coefficient, and intermediate exposed to a
later semantic operation.

A scene frame outside this ceiling is omitted together with descendants that
depend on its clipping, so recovery cannot expose formerly clipped content.
A standalone frame target whose viewport exceeds the ceiling fails with fatal
`resource.coordinate-range-exceeded`, resource-limit details and null recovery,
before viewport selection. Its local scene counterpart remains a warning.

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
An elbow arrow's geometry is its drawn elbow path together with the Rough.js
shape the reference measures its scene extent with (Section 8.1, #176): the
extent shape's commands are generated geometry commands of the element like the
drawn shape's, counted in its geometry segments, charged 32 work units and a
64-byte command record each on the same ledger row and in the same
shared-semantic-resolution transaction, and subject to the same per-element
segment and work ceilings, so a scene's canonical trace moves only where it has
an elbow arrow and identically on every path. Text counts normalized Unicode
scalars before shaping. Decoded pixels share only
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

The closed revision-1 nested-resource placeholder uses a conservative dependency
envelope of 8,192 work units and 8,192 temporary bytes before constructing its
two fixed Font Awesome paths and data-SVG wrapper. Its generated data URL is a
request-derived built-in blob (`256 + A(n)`) retained through projection. Each
rejected occurrence pays this envelope, including cached-parent replay; a local
asset-work ceiling below the envelope selects omission instead. Root recovery
viewer preparation additionally admits the generated source byte work, a
`512 + A(n)` identity record and the 1 MiB sanitizer workspace before parsing.
These bounded recovery envelopes are semantic constants, not allocator samples.

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
exact observed count and binary64 score with no margin. A complete artifact
smaller than 11x11 has no score (Section 14.3); its exception instead records,
per failing view, the exact Pixelmatch count, the exact count of differing
view pixels and the exact maximum channel difference, and holds the
small-artifact mismatch outcome itself.

Representative cases (layer 3) apply one further rule before exceptions
(ADR-0014, #195). A region that fails its class budget still passes when it is
no larger, on both metrics, than the worst **identical-input control** at that
same region. Controls are pairs of canonical renderings of the same input:
the official SVG in the canonical Chromium viewer vs the pure-Rust viewer, the
official SVG vs the official canvas raster, and, for PDF, the candidate PDF in
PDFium vs PDF.js. They are computed in the same verification run and through
the same comparator and crops, never stored as tolerances. The rule
admits only disagreement the reference authorities and canonical viewers
themselves show at that spot; the class budgets, and their use for every other
layer, are unchanged. A region beyond its controls needs an exception, and an
exception whose region passes by budget or controls is dead and rejected.

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
[Record the CJK subset trigger's evaluation point, the fallback probe, and bidi in spec 8.3 and CONTEXT.md](https://github.com/sagikazarmark/excalidraw-render.orig/issues/122);
its paragraph-separator clause by
[Lay out every UAX #9 paragraph of a source line on the fallback text path](https://github.com/sagikazarmark/excalidraw-render.orig/issues/125)
and its run-direction and per-character SVG position clauses by
[Shape RTL visual runs in their own direction and map SVG per-glyph x to logical characters](https://github.com/sagikazarmark/excalidraw-render.orig/issues/126).
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
