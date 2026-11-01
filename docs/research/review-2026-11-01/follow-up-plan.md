# Follow-up plan for the deferred items

The 2026-11-01 review landed its seven steps the same day (ADR-0025;
[README.md § Outcome](README.md#outcome)). This plan covers what the review
recommended and the branch did not do, as tickets in dependency order, in the
shape of the #127 round: what to build, the evidence behind it, the approach,
acceptance criteria, an effort mark (S under half a day, M one to two days, L
three or more or needing a decision), and whether a maintainer decision is
inside it. Everything here is optional for 0.1.0; tranche A is where the
remaining churn is.

## Tranche A: the remaining churn sources

Each of these still makes a routine change (a dependency bump, a Dependabot
group, a comment edit) touch a verifier or a record for no renderer reason.
They are independent of one another and of everything below.

### A1. Stop mirroring the rasterizer lock in the SVG harness — S

**What.** `parity_registered_case_svg::pins_are_measured_by_the_locked_rasterizer`
asserts `PINNED_RASTERIZER` (`resvg`, `tiny-skia`, `fontdb` versions) against
`Cargo.lock`, so every rasterizer bump fails the harness even when no pinned
measurement moves. Three bumps so far moved zero measurements and forced three
edits ([render-tier.md § 5](render-tier.md)).

**Approach.** Delete the test and the constant. The five raster pins are exact
in both directions already, so a bump that moves a measurement fails on the
measurement, with the observed values printed; a bump that moves nothing needs
no edit. Record in the module doc that the pins are measurements under the
workspace `resvg` at whatever version the lock holds, and that the pinned
pure-Rust viewer (`tools/svg-viewer-rust`, its own lock) is the canonical
judge. The alternative, judging the raster dimension through that viewer as a
subprocess, buys a frozen judge at the cost of 96 process spawns per run; keep
it in reserve for when a bump does move a pin.

**Acceptance.** A `resvg` bump with no measurement move touches no test file.
`cargo nextest run --test parity_registered_case_svg` passes unchanged.

### A2. Drop the provenance literals from `verify-all` — S

**What.** `xtask/src/conformance.rs:1546-1586, 1656` require every case's
`provenance.json` to name the exact container digest, `node_version
"v24.16.0"`, Chromium `"149.0.7827.55"` and `http://reference.invalid/` URLs.
These cannot change without regenerating the case, which rewrites the
provenance anyway, so the checks can only fire on a generator or image change,
where they duplicate the reference profile's own binding
([conformance-verifiers.md § 1](conformance-verifiers.md)).

**Approach.** Keep the structural checks (two fresh processes with distinct
PIDs, identical digests across them, the generated_at date against the
registry). Replace the literal comparisons with shape checks (a digest is 64
hex characters, a version is non-empty) and one comparison against the
reference profile JSON (`tests/oracle/reference-profiles/revision-1.json`
already records the container, Node and Chromium), so the profile is the one
place those identities live.

**Acceptance.** `verify-all` passes; a regenerated profile with a new Chromium
needs no xtask edit; the xtask unit tests that construct provenance fixtures
are updated to the profile-derived expectation.

### A3. One identity for each viewer: profile JSON, not xtask literals — M, needs Docker and a decision

**What.** `verify-svg-viewers`, `verify-pdf-viewers` and `verify-svg-font-subsets`
repeat the viewer profiles' version strings as Rust literals
(`svg_viewers.rs:126-163`, `pdf_viewers.rs:266-345`, `svg_font_subsets.rs:480-508`),
so a viewer bump edits the profile, its sidecar and xtask. And the PDF.js
profile names `6.1.200 at 6353acef` with a lockfile digest `887c2194…` while
`tools/pdf-harness/pdfjs/package.json` installs `6.2.108` (Dependabot
`96d5ff2a`): the profile describes a judge that is not the one running
([external-harnesses.md § 4](external-harnesses.md)). It passes only because
the lockfile is whitelisted as a historical producer.

**Approach.** (1) Regenerate the PDF viewer qualification with the installed
PDF.js (`just viewer-evidence pdf generate`, Docker, Linux x86-64 through
`restage.yaml`), so the profile says 6.2.108 and the rasters are its; the
V29 candidate already ran 6.2.108 with rasters identical to V28's, so no
measurement is expected to move. (2) Make the three verifiers read the
identities they check from the profile JSON and check the profile against the
live tool (the pure-Rust viewer's `Cargo.lock`, the pdfjs `package-lock.json`,
`pdfium-runtime.json`) rather than against a Rust literal. (3) Decide whether
the pdfjs lockfile stays whitelisted as historical or becomes a live binding
again now that the profile would match it; the review's position is live, so a
Dependabot bump of PDF.js is a visible viewer change.

**Acceptance.** No viewer version string appears in `xtask/src`; the PDF.js
profile, lockfile and `package.json` agree; `just conformance` passes; the
viewer checks reproduce on `restage.yaml`.

### A4. Retire the comparator closure currency test — S, ADR-0005 amendment

**What.** `crates/excalidraw-render-compare/tests/profile.rs::canonical_comparator_profile_schema_and_all_bound_bytes_are_current`
requires `canonical-visual-v1.closure.json` to equal the comparator's
dependency subgraph of `Cargo.lock`, and the profile's `implementation.sources`
digests to equal the comparator's own `.rs` files. Three of the seventeen CI
failures since 2026-10-13 were Dependabot bumps of `jsonschema` or `sha2`
tripping it ([fuzz-nightly-ci.md § 4](fuzz-nightly-ci.md)); a comment edit in
`compare/src` fails it until the profile is regenerated. `Cargo.lock` is the
pin; the comparator has no normal dependencies, so the closure records
dev-dependencies that cannot change a comparison.

**Approach.** Delete the closure file, the `materialize` test and the currency
test; make the `sources` digests historical as ADR-0019 made every other
producer digest. Keep the profile's limits and the sidecar check. Amend
ADR-0005 to say the closure was recorded from 2026-09-04 to 2026-11 and why it
stopped.

**Acceptance.** A Dependabot bump of a comparator dev-dependency and a comment
edit in `compare/src` both pass `just ci` without a profile edit.

### A5. Prune the unread candidates — S

**What.** Under ADR-0022's rule, `tests/conformance/candidates/` holds
directories nothing reads: `stage4-pdf-colorspace-bridge-v20` (576 KB, no
reader), `stage4-excalifont-canary` (156 KB, written by the
`qualify-excalifont-canary` command, read by no verifier or test), and
`stage4-frames-text-assets-v2` (408 KB, read only by
`tools/reference-oracle/scripts/stage4-representative-{v2,v4}.mjs`, which
nothing runs). The bare `stage4-frames-text-assets` is read by xtask and the
fast tier's `stage4_interaction_contract.rs`; V4 is the frozen authoring; V29
is registered; `stage4-pdf-backend-requalification-v2` is read by
`conformance_pdf_requalification.rs`. Those stay.

**Approach.** Delete the three directories and the `qualify-excalifont-canary`
command with its `qualify_text_canary` function; confirm the two `.mjs`
scripts have no caller and delete them too, or keep them and the V2 directory
together. Note the pruned paths in ADR-0022's list.

**Acceptance.** `just ci` and `just conformance-release` pass; `rg` finds no
reference to the pruned directories outside `docs/research`.

## Tranche B: what the parity harness can see

### B1. Exact paint and text attributes in the SVG harness — M

**What.** The SVG harness compares `<path d>` and the root extent exactly and
everything else only through a raster, where Pixelmatch flags a covered pixel
only above about 26 grey levels: a stroke `#1e1e1e → #333333` passes every
pixel ([render-tier.md § 8](render-tier.md)). The official SVG carries
`stroke`, `fill`, `stroke-width`, `stroke-linecap`, `stroke-dasharray`,
`opacity`, `transform` on `<path>`, `<g>` and `<rect>`, and `x`, `y`,
`font-size`, `text-anchor` on `<text>`; on the rectangle case the renderer's
sequence of these attributes is byte-identical to the official export today.

**Approach.** Add a fourth dimension, `attribute_divergence`: for each element
kind, the document-ordered sequence of the attributes above, compared as
exact strings, with the first differing attribute printed. Exclude
`font-family`, which the renderer spells as the spec's fallback list by
design (spec 8.4), and `clip-path`/`mask` references, whose ids differ.
Survey all 48 variants first; where an attribute differs for a documented
reason (a reference authority divergence, a compatibility exception), carry
it in a `KNOWN_ATTRIBUTE_DIVERGENCES` list pinned like the geometry pins, or
exclude that attribute for that element kind with the reason in the module
doc. Register nothing new in manifests.

**Acceptance.** Changing a stroke colour, a stroke width, an opacity or a text
anchor in any registered variant fails the SVG harness naming the attribute,
without a rasterizer; the survey's exclusions are each one sentence in the
module doc; the harness's runtime grows by under a second.

### B2. Frame-export visual class from the manifest — S

**What.** `parity_frame_export.rs:61-65` hard-codes the comparator class by
frame ordinal (4 and 5 mixed, 7 typography, the rest geometry); the frame
manifest carries no `visual_class` per target output, unlike the scene
manifests.

**Approach.** Add `visual_class` to each target's `svg` and `png` outputs in
`tests/conformance/cases/focused/frame-export/manifest.json` (canonical JSON,
sidecar, and the frame schema if it rejects the member), read it in the test,
and delete the table. Regenerate the inventory and pdf-judgments records and
rebind the ownership ledger (release tier) since the manifest moved.

**Acceptance.** The test has no ordinal table; the manifest validates;
`just conformance` and `just conformance-release` pass.

### B3. Retire the fast tier's whole-artifact self-pins — S

**What.** Seven sites still assert a SHA-256 of the renderer's own artifact
(`arrow_render.rs`, `outline_shape_render.rs`, `excalifont_text_render.rs`,
`cluster_text_pdf.rs`, `pdf_backend_qualification.rs` ×3). Each is re-pinned
on every intentional change ([fast-tier.md § 2](fast-tier.md)); the golden
record now holds the same promise for every registered target, and the
Excalifont scene is literally a golden key.

**Approach.** Where the scene is a registered case, delete the digest and keep
the behavioural assertions around it. Where it is not (the backend
qualification matrices, the cluster text PDF), either register it as a golden
"facade scene" (a second, small execution list in `golden_native` for
project-authored scenes) or keep the digest as a named characterization with
one comment stating what it guards. Prefer the golden list: one update
command, one review.

**Acceptance.** No `Sha256::digest(…artifact().bytes())` assertion remains in
`tests/fast`; the scenes that lost a pin are in the golden record or carry a
characterization note.

### B4. A characterization marker for known-defect pins — S

**What.** `jpeg_release_limitation.rs:21`, `jpeg_gain_map_contract.rs`
(`hdr_gain_maps_pin_known_color_defect_190`), `webp_container_contract.rs`
(`KNOWN_DIVERGENCES`) and `webp_sample_contract.rs` (`issue_191_…`) pin the
renderer's current wrong output for an open issue. A fix fails them like a
regression ([fast-tier.md § 8](fast-tier.md)).

**Approach.** A shared `common::characterization::known_defect(issue, observed,
pinned)` that fails with "known defect #N moved: if the output improved, close
the issue and drop the pin" and otherwise passes, used by the four sites, and
a module doc stating that these are characterizations, not parity evidence.

**Acceptance.** The four sites use the helper; its failure message names the
issue; CONTEXT.md's "Known defect" entry points at it.

### B5. Fold the one-test binaries into `fast` — S

**What.** `jpeg_matrix_inputs.rs`, `jpeg_release_limitation.rs`,
`pdf_image_colorspace.rs` and `stage4_typography.rs` are four binaries
re-linking the renderer for nine tests; `fast/main.rs` exists to avoid that.

**Approach.** Move each into `tests/fast/` as a module. `jpeg_matrix_inputs`
regenerates `tests/oracle/jpeg-color/inputs.json` with the `jpeg-encoder`
dev-dependency and compares bytes; it tests the dependency, not the renderer,
so either drop it or keep it as a module that runs only when the inputs file
is newer than its reference.

**Acceptance.** `crates/excalidraw-render/tests/*.rs` holds only `fast`,
`parity_*`, `golden_*`, `limits_*`, `harness_*`, `asset_fuzz`, `render_fuzz`
and `format_features`; the nextest tiers need no change.

## Tranche C: regeneration ergonomics, for the day Excalidraw moves

These are the prerequisites for a reference bump
([external-harnesses.md § 2](external-harnesses.md)). None is needed before
one is planned; C2 before C3 so the bump can be rehearsed.

### C1. Key the font-subset viewer qualification by glyph set — L, Docker

**What.** `svg-font-subset-viewer-qualification/revision-1.json` keys its 662
executions by `semantic_subset_identity`, a digest of the subset bytes, so a
profile-token rename or a subsetter bump rewrites 2.4 MB (a 5 MB diff) to say
every raster is unchanged; this happened twice in three days in October.

**Approach.** Key each execution by what is rendered (source font identity,
class, the scalar set), record the subset digest as a field, and have
`verify-svg-font-subsets` replay: regenerate the subset, compare its digest
with the recorded one, and compare the viewer rasters only when the record is
regenerated. A token change then edits one field per execution through a
small xtask command, and the rasters' digests stay. Needs one Docker run to
re-record under the new key.

**Acceptance.** A subset-profile token rename changes `revision-1.json` by one
line per execution and neither report; `verify-svg-font-subsets` still fails
when a raster or a subset digest moves.

### C2. `--supersede` for the case generators — S, Node

**What.** `conformance-case.mjs`, `typography-conformance-case.mjs` and
`frame-conformance-case.mjs` refuse to overwrite a manifested case, so
`just reference-cases generate` cannot regenerate the registry; today each of
37 manifests would be deleted by hand first.

**Approach.** A `--supersede` flag (and `just reference-cases generate
--supersede`) that lets a generator replace a manifested case, keeping the
refusal as the default. The generated manifest and sidecar are reviewed in Git
like any other change.

**Acceptance.** On Linux x86-64, `just reference-cases generate --supersede`
regenerates all 37 cases in one container session with a clean diff when the
reference has not moved.

### C3. `revision-N` as one configuration value — L, spec decision

**What.** The reference profile digest appears in 684 files and about 13
source constants (`conformance-case.mjs:47-49`,
`typography-conformance-case.mjs:94-96`, `scene-surface.mjs:19-21`,
`xtask/src/catalog.rs:10-30`, `conformance.rs:14-18`, `pdf_viewers.rs`,
`svg_viewers.rs`, `revision_1.rs`, `compatibility_identity.rs`,
`empty_scene_render.rs`), and `revision-1` is a path and a type name. Spec § 3
says a post-publication reference bump is a new revision coexisting with the
retained one; nothing is parameterised for two.

**Approach.** First the mechanical half: every constant reads the digest from
the profile sidecar or the catalog, and the `revision-1` paths go through one
function. Then the decision: whether revision 2 coexists in the tree (spec § 3
as written: a second catalog, case set and golden record) or supersedes
revision 1 while 0.x retains nothing (spec § 3's draft-revision clause). The
decision belongs to the maintainer and changes the size of the mechanical
half.

**Acceptance.** Changing the profile digest in the sidecar is the only edit a
profile update needs outside the regenerated evidence; the spec records the
coexistence decision.

## Tranche D: fuzzing that searches

### D1. A `fuzz/` crate with cargo-fuzz targets — M

**What.** Every fuzz family replays a fixed seed under lowered limits; every
hostile-input bug in the history (`822c97dc` and the others) was found by hand
([fuzz-nightly-ci.md § 1](fuzz-nightly-ci.md)).

**Approach.** A `fuzz/` crate outside the workspace members with three
targets, `parse_render_svg`, `parse_render_png`, `parse_render_pdf`, each
`SceneDocument::parse` → render under the default limits, with a 2 GiB
`rss_limit` and a 60 s per-input timeout; seeded from `tests/public-scenes`,
the registered scenes and the three regression corpora. Not in CI; a
`just fuzz <target> <minutes>` recipe and a weekly `workflow_dispatch` that
runs each for 30 minutes and uploads crashes. The nightly keeps the
deterministic families as the regression layer.

**Acceptance.** `just fuzz parse_render_png 1` runs; a crash reproducer path
is documented in `docs/render-fuzzing.md`; the first weekly run's findings are
triaged into issues.

### D2. One nightly family under the default limits — S

**What.** The nightly expansions run under the fuzzers' lowered limits (64 KiB
inputs, 31 elements, 65,536-pixel surfaces), which rule out the magnitudes
behind the October fixes; `render_fuzz` is the only family in the `test`
profile with debug assertions on.

**Approach.** An `EXCALIDRAW_RENDER_FUZZ_DEFAULT_LIMITS=1` mode for
`render_fuzz` that uses `ResourceLimits::default()` for a reduced case count
(512), run as a second nightly step with the 90-minute timeout as the
backstop. Measure peak memory on the first run and set the count from it.

**Acceptance.** The nightly runs the mode; a case that reaches a default
limit reports the typed `resource.*` code and no panic.

### D3. Settle the 2026-08-05 nightly finding — S

**What.** Nightly `30974954216` failed `font_subset_fuzz.rs:318` for seed
`6a09e667f3bcc908` case 3482 on `d31399a5` every night until runs stopped; no
commit cites it and no corpus entry records it.

**Approach.** Reproduce at `d31399a5` with the seed and case; bisect to the
commit that made it pass; if it fails on `main`, add it to
`font-subset-fuzz-regressions.json`; otherwise add the reproducer as a
regression entry anyway with the fixing commit named, so the finding is
recorded.

**Acceptance.** A regression entry or a one-paragraph note in
`docs/research` names the finding and its resolution.

## Tranche E: xtask hygiene

### E1. Host-tool tests to an opt-in group; Node out of the base shell — M

**What.** `xtask/tests/webp_boundary.rs` runs `node`;
`pdf_inspection.rs`, `pdf_inspection_expectations.rs`, `pdf_inspection_parity.rs`
and `pdf_harness_wrappers.rs` need qpdf 12.3.2, bash and `realpath`. Outside
devenv they fail for environmental reasons. Node is in the base devenv closure
for these and for the candidate gate's `stage4-viewer-assets.mjs verify`,
which is release tier now.

**Approach.** Mark the five test files `#[ignore = "host tools"]` in a
`host-tools` nextest group run by `just test-release` (qpdf is in the base
shell; node is not needed by the fast tier after this). Move
`languages.javascript` to a `release` profile that extends `browser`, so
`just release` runs under `devenv --profile release shell` and the base shell
holds Rust, just, nextest, cargo-deny, qpdf and PDFium only. Update
`release.yaml` and the runbook.

**Acceptance.** `devenv shell -- just ci` passes without Node on the PATH; the
five test files run in `just test-release`; the base closure shrinks by
Node's paths.

## Not planned, and why

- **Qualification through a nextest profile instead of the build-script cfg.**
  The log-scraped report that motivated it is gone; the cfg admits the two
  release tests without naming them, which a nextest filter would have to do.
- **The WASM golden test in two browser launches.** The separate-process pair
  caught nothing in nineteen baseline revisions; forward and reverse in one
  launch keep the ordering property, and `just wasm-packaging` keeps the
  packaged aspect.
- **Lowering the `geometry/image` SSIM floor.** The measured controls
  ([render-tier.md § 9](render-tier.md)) show the floor is beyond
  cross-rasterizer agreement in general but no identical-input control exists
  for the direct PNG path; the question stays with ADR-0014 and the two-sided
  exceptions carry the 22 variants exactly.
- **Branch protection.** A repository setting: require the six CI job names
  on `main`, or leave it off; the `Test (join)` job that stood in for it is
  gone.

## Order

1. A1, A2, A4, A5 (one PR each, about a day in total): they end the remaining
   per-bump and per-comment edits.
2. B1: the single largest gain in what the harness can see, at no churn cost.
3. A3 and B2, the two that need Docker or a manifest edit, in one restage.
4. D3, D2, D1: the fuzzing that searches, D3 first because it is a loose end.
5. B3, B4, B5, E1: tidy-up, as time allows.
6. C2, then C1 and C3, when an Excalidraw release is on the horizon.
