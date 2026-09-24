# Shared coverage and execution contract (#85 / #86)

This is a **draft inventory**, not a release approval or a claim of complete
revision-1 coverage. It gives corpus closure and target collectors one checked
input set and one result key. The current inventory has:

- 202 scene-surface field rows and 118 distinct existing case coverage labels;
- 37 registered cases, 49 scene variants and 7 frame targets;
- 168 execution keys: each scene variant/frame target in SVG, PNG and PDF;
- 112 reference artifact descriptors and 56 explicit absent PDF reference
  descriptors (specialized PDF evidence is still a separate integration task);
- 27 labels with multiple candidate cases and eight unresolved milestones.

## Inventory lifecycle

The expanded global ownership proposal is in `ownership.json`, with derivation
and semantics documented in [OWNERSHIP.md](OWNERSHIP.md). It adds 928 format-scoped
obligations alongside this execution inventory and proposes single owners for the
27 collided labels. The ledger consistency check is separate from final approved
coverage closure; explicit gaps remain blocking.

`requirements.json` names the outstanding work and links existing tests/other
evidence by source digest and exact textual anchor. These anchors establish
where code exists, **not that it ran or completely covers a requirement**.
The fixed milestone set cannot silently shrink. Source-anchor updates are ordinary
reviewed edits; they are not additions to existing human approval subjects.

`inventory.json` is derived from those requirements, the case registry, current
case manifests, scene-surface fields, scene bytes, reference profiles and the
asset catalog. The verifier derives it again, checks referenced bytes and source
anchors, then verifies all registered-case approvals. New/deleted/changed cases,
variants, field rows, scene bytes, artifact bytes and source anchors cannot leave
a stale inventory green. Do not edit derived records by hand.

```sh
cargo run --locked -p xtask -- conformance inventory > target/inventory.json
# Review the staged diff, then copy target/inventory.json here.
cargo run --locked -p xtask -- conformance verify-inventory
cargo run --locked -p xtask -- conformance inventory-digest
dagger check excalidraw-render:conformance:coverage-inventory
```

Generation through the pinned Dagger environment:

```sh
dagger api call evidence inventory --output target/inventory.json
```

`surface:<recordKind>:<path>` rows come from the existing authority. `case-label:`
rows come from the approved manifests' local labels. Their `candidates` are sorted
case IDs, not globally selected owners. A single candidate is not automatically
approved as primary, and several candidates do not silently pick the first one.
Existing approved manifests are left intact during ownership design.

The draft does not yet expand every diagnostic, codec/profile, exception and
interaction into atomic inventory rows. `requirements.json` records that gap.
`conformance verify-closure` **always rejects this draft**, even if all current
rows have candidates or a synthetic result bundle validates. Completion needs
the approved global ownership/exclusion ledger and canonical snapshots; that is
the next schema transition, not flipping a free-standing `complete` flag.

## Execution key

Every result identifies the exact `key` object from an inventory execution:

```json
{
  "case": "focused.rectangle-outline.v1",
  "variant": "default",
  "export": {"kind": "scene"},
  "format": "svg",
  "inputs_sha256": "<64 lowercase hex characters>"
}
```

Frame keys use `{"kind":"frame","ordinal":1}`; ordinals are positive source
ordinals, not names. `inputs_sha256` binds canonical JSON containing the manifest
descriptor, scene descriptor, reference-profile descriptor, asset-catalog
descriptor, options and export target. Each descriptor hashes the actual file
bytes. The draft v1 digest uses this repository's `conformance::canonical_bytes`:
recursive sorted object keys, compact arrays/objects and serde_json 1.0.141
string/number spelling. It is independent of pretty printing but **not RFC8785
number normalization** (for example, `1.0` remains distinct from `1`). Collectors
must use `inventory-digest` rather than reserialize with another JSON library.
The approved closure/snapshot transition must settle RFC8785 number handling;
this draft digest must not be used as a canonical human approval subject.

Execution `options` preserve the reference manifest's request vocabulary. A
future public-facade collector must implement and validate its deterministic
mapping to RenderRequest/FrameRenderRequest (including frame zero-padding and
format-specific behavior); this draft does not claim those raw options are
already normalized Rust request values. PDF keys are explicit work items rather
than pretending official SVG bytes are a PDF canonical snapshot.

## Result bundle v1

`xtask/src/coverage.rs` defines strict serde types (`deny_unknown_fields` on every
record and tagged union). This is the executable wire contract, shared through
JSON with future native and WASM collectors:

```text
schema = "excalidraw-render/coverage-results/v1"
compatibility_revision = 1
commit = 40 lowercase hex characters
inventory_sha256 = canonical inventory digest
tool_manifest_sha256 = SHA-256(tests/package-consumer/tool-manifest.json)
dagger_lock_sha256 = SHA-256(dagger.lock)
runs = exactly one run per (supported target, forward|reverse)
  target = x86_64-unknown-linux-gnu | wasm32-unknown-unknown
  order = forward | reverse
  process_id = nonempty collector-issued distinct process/instance identity
  results = every inventory key in its declared forward/reverse order
    key = exact inventory key
    samples = exactly two fresh-session samples
      outcome = {kind: artifact, bytes: {sha256, byte_length}}
             or {kind: failure, code: nonempty string}
      report = {sha256, byte_length}
      trace = {sha256, byte_length}
```

All byte digests are 64 lowercase hex; byte lengths must be positive. Each sample
carries the digest/length of the whole artifact (when present), complete serialized
compatibility report, and canonical trace. The trace comes from a test-only seam;
no public trace API is introduced. Both session samples, both process orders,
and both supported targets must agree exactly by key, outcome and byte descriptors.
Typed failures participate equally and cannot be substituted for artifacts on
only one target. Trace values are compared fresh, not against historical per-cell
pins (ADR-0002).

```sh
cargo run --locked -p xtask -- conformance verify-results target/results.json
```

The validator rejects missing/extra/duplicate/stale keys, wrong sequence, missing
targets/orders, reused process IDs, malformed/unknown fields, mismatched tooling
and unequal output/report/trace samples. It deliberately prints **draft equality**,
not release success: hashes and process IDs alone cannot attest real execution,
and no canonical snapshot oracle is connected yet. A future collector must own
fresh process/session creation, hash the captured bytes, retain reproducible
artifacts/logs, validate real commit/package provenance, and compare outcomes
against approved canonical expectations. Inventing equal hashes (as contract
tests do) proves only the validator's behavior.

## Next implementation

### Test-only trace collection

`dagger api call traces` collects the same 168 keys through the internal test-only
trace seam on both targets and orders. `dagger api call full-results` joins those
measurements to fresh packaged native and WASM artifact/report measurements;
`dagger check excalidraw-render:full-equality` rejects all combined disagreements.
The exported trace-collection schema requires `conformance verify-joined-results`,
not the generic draft `verify-results` command. No public trace API is added.
See [execution details and findings](../../../docs/research/trace-collection.md).
The initial run found 30 cross-target trace differences. A total semantic
release-record ordering fixes 27; three remain because PNG output lengths differ
under #178. The gate still rejects those differences; this is not a passing #86
closure or a waiver of the upstream blocker.

### Browser-WASM collection and replay

```sh
dagger check excalidraw-render:wasm:snapshot-replay
dagger api call wasm browser-snapshot-logs --output target/wasm-snapshot-logs
```

The first is a blocking comparison against the approved native baseline. The
second exports logs and `replay.json` even when comparison fails; check its
`decision`. Both execute all 168 keys twice in each of separate forward/reverse
browser runner invocations. The generated harness embeds the scenes and options
and reuses the approved native collector source's request adapter without editing
that approval-pinned file. Generated Rust/input files are ignored staging files.

Per-key digests/lengths cover complete artifacts, reports and typed failure
descriptors. The host rejects missing/extra/duplicate/out-of-order keys, absent
or duplicate completion, stale input binding, session drift, and any native
baseline difference. Logs are execution evidence, not trusted attestations when
supplied independently of the Dagger runner. No trace values are fabricated.

Initial execution found seven cross-target mismatches (four PNG byte differences
and image-codecs resource failures in all formats); 161 keys match. Both WASM
orders and all repeats agree. The ICO accounting fix resolves the three resource
failures; four PNG differences remain under #178. The automatic gate stays red
until those discrepancies are fixed. See [the findings](../../../docs/research/wasm-snapshot-mismatches.md).

### Canonical baseline approval and replay

`native-baseline/revision-1.json` contains the approved clean `ec30177`
recollection after JPEG began reading as the pinned libjpeg-turbo decoder does.
Exactly two keys changed, `focused.image-codecs.v1` PNG and PDF, with their
reports unchanged. The case's JPEG carries an sRGB-equivalent ICC profile,
which now draws untransformed as the reference draws it. It was approved on
2026-09-24 in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5808265754),
subject `sha256:9dfb7dc101a5bf5350248bee33028aa17e8b0b8228fb622f10839bb13985c2e7`.
See [the JPEG facet record](../../../docs/research/jpeg-umbrella-facets.md).

The preceding baseline contains the approved clean `4713c2e` recollection
after PDF began lowering dark mode into nested-SVG content (ADR-0013, #154).
Exactly one key changed, `focused.dark-mode-images.v1` dark-mode PDF, with its
report unchanged. It was approved on 2026-09-23 in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5789983701),
subject `sha256:e4dc77872c6479036b811c4641ab5ccafd14cd3d733e8b0a7494fecc2531c399`.

The preceding baseline contains the approved clean `59fda62` recollection
after the dark-mode root filter change (ADR-0012). 42 keys changed against the
prior baseline: the dark-mode SVG and PDF of four cases, isolated against a
parent-commit collection, and 34 keys from the approved WebP composition and
PDFium successor requalifications. It was approved on 2026-09-23 in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5788473732),
subject `sha256:46e23f43096064aef51dcd9f539ab7d31525aab610a520b2b41b410ee0d46965`.
See [the requalification record](../../../docs/research/dark-mode-root-filter-requalification.md).

The preceding baseline contains the approved clean `d96cddd` recollection
after the nested resource-limit amendment. All 168 artifact/report pairs and 187
blobs match the prior baseline exactly; refreshed input bindings and provenance
were approved on 2026-09-14 in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5671646910),
subject `sha256:177a9f4a2f901659189dac9b10e6e766e061cee2023024905a860f5bb51783aa`.
Canonical packaged replay passes. See [the amendment record](../../../docs/research/nested-resource-limit-amendment.md#native-baseline-refresh).

The preceding baseline contains the approved clean `73a1560` recollection
after nested-claim registry reapproval. All 168 artifact/report pairs and 187
blobs match the prior baseline exactly; refreshed input bindings and provenance
were approved on 2026-09-14 in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5662705380),
subject `sha256:3296d5a63885e443cd191fcacf6f0924656e81fb434640564a3e753b49b9f890`.
Canonical packaged replay passes.
See [the nested-claim binding refresh](../../../docs/research/nested-claim-binding-refresh.md).

The preceding baseline contains the approved clean `3836a21` recollection
after image registry reapproval. All 168 artifact/report pairs and 187 blobs
match the prior baseline exactly; updated input bindings and provenance were
approved on 2026-09-13 in [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5654932917),
subject `sha256:56aa7e51d3990375b31bea1198f2c262871c5a7dbfd59344b2254cdf35c52721`.
Canonical packaged replay passes.
See [the image binding refresh](../../../docs/research/image-registry-binding-refresh.md).

The preceding approved baseline contains the clean `b419e23` recollection
after the font registry/case-binding reconciliation. All 168 artifact/report
expectations and 187 blobs are byte-identical to the prior approved baseline;
input digests and provenance are refreshed. **Reapproved on 2026-09-13** in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5652971255) for subject
`sha256:f6e6a9e6a538b60b088038d19586c943b92166a18f5a9dee8a1b68c37984c957`.
The canonical packaged replay passes. See [the reconciliation record](../../../docs/research/font-registry-snapshot-reconciliation.md).

The original clean `2155a99` collection was **approved on 2026-09-11** in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5641266771),
subject `sha256:1f11007a925d0aa2f277454c09e411d553e0f6f600c695d0acdb673ccbe4af2c`.
The subject binds the complete expectation set,
inventory bytes, collector and producer source identities, candidate digest,
compiler, tooling and package provenance. Process IDs are left in the original
candidate provenance instead of becoming comparison expectations. The subject
contains strings/integers/null only; option JSON is bound by digest, avoiding the
draft inventory's floating-point canonicalization limitation.

```sh
cargo run --locked -p xtask -- conformance stage-native-baseline \
  target/native-snapshots-clean target/new-native-baseline
cargo run --locked -p xtask -- conformance native-baseline-approval-subject
cargo run --locked -p xtask -- conformance record-native-baseline-approval \
  <login> <UTC-date> <issue-85-comment-URL> <exact-subject>
cargo run --locked -p xtask -- conformance verify-native-baseline
dagger check excalidraw-render:packaging:snapshot-replay
```

Staging refuses existing output directories and modified-source candidates. It
checks source descriptors against the named Git tree and every authored archive
payload against that tree, including original manifests and VCS identity.
Cargo-generated normalized manifests/lockfiles are retained by archive digest
rather than expected to be identical to workspace files. The original archives
remain in `target/native-snapshots-clean/packages`; the baseline keeps their
digests, not duplicate archive copies. Source provenance checks run at staging;
approval recording rechecks the subject and Git ancestry.

Replay requires explicit approval on #85, validates a newly collected candidate,
and compares every key's complete outcome/artifact/report descriptors with the
approved baseline. Missing cases, byte changes, artifact-to-failure changes and
report drift fail. New process IDs, candidate commit, and package archive metadata
are provenance, not output drift. The automatic Dagger check rejects an unapproved
baseline. After recording the original approval through Dagger, the complete native replay
passed in 5m32s: all 168 keys match the approved artifact/report expectations
across two sessions in each of two fresh process orders.
The working-tree replay verification is recorded in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5641348782).

Review of the initial clean collection: all 168 renders succeeded, 120 available
manifest-owned expected reports match as complete JSON values, all 56 PNG sizes
match reference dimensions, all 56 SVGs parse, and all 56 PDFs have header/EOF
markers. **48 PDF executions have no manifest-owned expected report**, and these
syntax/dimension checks are not full visual or PDF-structural qualification. The
baseline approval accepts exact regression expectations for current output; it
does not waive that missing conformance coverage or complete #85/#86. Full render
tier and conformance checks pass independently. See the release-readiness audit
for the remaining representative/PDF/ownership work.

### Native snapshot candidates

The native public-facade collector is implemented as the non-published
`tests/package-consumer/examples/native_snapshots.rs`. Run it in pinned Dagger:

```sh
dagger api call packaging snapshots --output target/native-snapshots
cargo run --locked -p xtask -- conformance verify-native-snapshots target/native-snapshots
```

Dagger verifies the inventory, packages the library/assets, runs request-adapter
tests, and builds the collector with `--release --locked` against those unpacked
crates. The parent spawns two separate worker processes in forward/reverse order.
Each key renders twice through fresh public-API sessions. The parent compares
both process outputs and only writes `snapshots.json` when all results agree.

The collector validates the complete option set rather than ignoring unfamiliar
values: background inclusion, scene padding, exactly one sizing mode, dark mode,
embedded-scene metadata and scene frame controls. Frame export uses the frame API's
zero padding and selected source ordinal. The existing white background option
is supported; other background overrides are rejected pending an explicit mapping.

Output is a separate `excalidraw-render/native-snapshots/v1` candidate bundle:
`snapshots.json`, forward/reverse result JSON, `blobs/<sha256>` containing full
artifact/report/failure-descriptor bytes, and `packages/` holding the exact four
library/asset archives. It reuses inventory execution keys and two-sample result
shape but omits the unavailable trace field and WASM runs. It must not be fed to
`verify-results`, which correctly requires both targets and traces.

Provenance records rustc, Git HEAD, source status, inventory, tool and consumer
locks, consumer manifest/configuration, collector source and package digests.
Cargo metadata must resolve each library/asset dependency to the packaged source.
Working-tree output is visibly
marked modified; Git HEAD alone is not claimed as its complete source identity.
The package archives preserve the actual compiled inputs. Output directory reuse
is refused by the collector; Dagger exports to a caller-selected staging location.
The verifier rehashes every artifact/report/failure blob, checks exact execution
coverage/order and repeat equality, rejects extra blobs, verifies packaged archive
digests, exported worker-report equality and current input/tool/collector bindings. This does not grant approval or
prove semantic parity with official evidence: it stages reproducible candidates
for that review. Typed render failures retain their error descriptor and full
report instead of inventing an artifact.

Initial Dagger execution on the working tree based on `cf4f64d` passed all 168
keys (56 each of SVG, PNG and PDF), with 672 successful render calls across
the two process orders and no render failures. The candidate manifest has null
approval. The staged output in `target/native-snapshots` includes about 22 MB
of blobs and package archives; generated candidate files are not committed.

1. Expand the authority into complete atomic risks and approve ownership and
   exclusions; connect exact vectors and specialized PDF/frame/regression owners.
2. Maintain the approved canonical artifact/report expectations and native replay
   gate as the inventory grows; changed expectations require explicit reapproval.
3. Resolve the exposed WASM byte and cross-target trace differences; both collectors
   and their combined gate now execute the current inventory in both orders.
4. Promote representative cases and expand codec/renderer fuzz reach, then make
   complete collection/equality/snapshot checking a Dagger release gate. Only then
   replace the draft closure rejection with a verified approved closure state.
