# Shared coverage and execution contract (#85 / #86)

**Current acceptance policy:** [ADR-0019](../../../docs/adr/0019-review-expectations-in-git-without-approval-hashes.md)
retires approval-subject gates and recording commands in favor of Git/PR review
and reproducible tests. Historical approval workflows below are transcripts.
Baseline bytes, diagnostic derivation, exact input keys, completeness and owner
selection still verify; approval metadata does not grant or block ownership.

This inventory is not a claim of complete revision-1 coverage. It gives corpus closure and target collectors one checked
input set and one result key. The current inventory has:

- 202 scene-surface field rows and 118 distinct existing case coverage labels;
- 37 registered cases, 49 scene variants and 7 frame targets;
- 168 execution keys: each scene variant/frame target in SVG, PNG and PDF;
- 112 reference artifact descriptors and 56 explicit absent PDF reference
  descriptors (specialized PDF evidence is still a separate integration task);
- 27 labels with multiple candidate cases and eight open closure requirements.

## Inventory lifecycle

The global ownership ledger is `ownership.json`, with derivation and
semantics documented in [OWNERSHIP.md](OWNERSHIP.md). It assigns one owner to
every format-scoped obligation alongside this execution inventory and selects
a single primary case for each of the 27 collided labels. The ledger
consistency check is separate from coverage closure; explicit gaps remain
blocking.

`requirements.json` names the outstanding work and retains historical links to
tests/other evidence by source digest and textual anchor. Those links describe
where code existed when recorded, **not that it ran or completely covers a
requirement**. Inventory derivation validates their metadata but does not read the
linked source files or require the old anchors to exist. Editing or moving linked
code therefore needs no source-hash refresh. The fixed requirement set cannot
silently shrink.

`inventory.json` is derived from those requirements, the case registry, current
case manifests, scene-surface fields, scene bytes, reference profiles and the
asset catalog. The verifier derives it again, checks render inputs and reference
artifacts, then verifies all registered-case approvals. New/deleted/changed cases,
variants, field rows, scene bytes and artifact bytes cannot leave a stale
inventory green. Changes to the requirements record itself still require
regenerating the inventory; changes to its historically linked source files do
not. Do not edit derived records by hand.

```sh
cargo run --locked -p xtask -- conformance inventory > target/inventory.json
# Review the staged diff, then copy target/inventory.json here.
cargo run --locked -p xtask -- conformance verify-inventory
cargo run --locked -p xtask -- conformance inventory-digest
just conformance-release
```

Generation, staged for review:

```sh
cargo run --locked -p xtask -- conformance inventory > target/inventory.json
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
recursive sorted object keys, compact arrays/objects, serde_json string
spelling, and (xtask enables `arbitrary_precision`) each parsed number's own
source spelling. It is independent of pretty printing but **not RFC8785
number normalization** (for example, `1.0` remains distinct from `1`). Collectors
must use `inventory-digest` rather than reserialize with another JSON library.
A future closure/snapshot transition must settle RFC8785 number handling
before this digest can serve as a canonical identity.

Execution `options` preserve the reference manifest's request vocabulary. A
future public-facade collector must implement and validate its deterministic
mapping to RenderRequest/FrameRenderRequest (including frame zero-padding and
format-specific behavior); this draft does not claim those raw options are
already normalized Rust request values. PDF keys are explicit work items rather
than pretending official SVG bytes are a PDF canonical snapshot.

## Result bundle v1 (retired 2026-11-01)

The `excalidraw-render/coverage-results/v1` wire contract carried the trace
collectors' results and retired with the trace pipeline. The golden record
(`tests/conformance/golden/native.json`, below) is the one result record the
inventory's keys now have.

## Next implementation

### Test-only trace collection (retired 2026-11-01)

The test-only trace collection (`just trace-logs`, `just full-equality`) once
joined the semantic resource ledgers of both targets to the packaged artifact
runs. It proved nothing the artifact and report equality does not already prove
(ADR-0002) at the cost of two further release builds, and it retired with the
2026-11-01 testing review. Its one finding was 30 cross-target trace
differences from an unstable sort, fixed by a total order.

### Golden record

`tests/conformance/golden/native.json` holds, for every inventory key, the
artifact and report bytes (by digest and length, with the bytes themselves
under `golden/blobs/`) the renderer produces on every supported host. The
render-tier test `golden_native` (`crates/excalidraw-render/tests/golden_native.rs`)
derives the keys from the registry and the manifests, renders each twice
(forward and reverse order, a fresh renderer each time) and holds them to the
record; an intentional output change is adopted with `just update-golden` on
any host and reviewed as the diff of the record and its blobs (ADR-0024: the
bytes do not depend on the host, and rendering all 168 keys on macOS arm64
reproduced every Linux-recorded blob). The same test under wasm32 in Firefox
(`just golden-wasm`) holds the browser build to the same record, with the
four PNG differences `golden/experimental-wasm.json` records for #178, which
must appear exactly as recorded and all appear.

The golden record replaced the native baseline (`coverage/native-baseline`,
collected through the packaged facade on Linux x86-64 and restaged through a
workflow) and the browser-WASM replay of it on 2026-11-01. The WASM mismatch
investigation is in [the findings](../../../docs/research/wasm-snapshot-mismatches.md).

### PDF judgments (`closure:pdf-judgments`)

`pdf-judgments.json` is one record, approved on #85, that closes the PDF
report gaps of the inventory and binds the specialized PDF judgment files no
ledger row binds. It carries:

- `executions`: an expected report for every PDF inventory execution (56),
  derived by `xtask/src/coverage/pdf_judgments/derive.rs` from approved
  declarations only, never from renderer output. Scene cases start from the
  registry's `expected_diagnostics.svg` (SVG is the PDF authority, spec 8.6),
  rewrite `format` to `pdf`, and require the PNG declaration to equal the SVG
  one minus PNG-silent codes (`link.unsafe-omitted`). Typography/frame cases
  transfer their single format-neutral manifest report, which must be the
  canonical empty report. Only codes in the spec-anchored `format_rules` table
  transfer. Each `sources` entry binds the registry member (canonical digest)
  and manifest report files (path + sha256); the report bytes follow the spec
  11.3 member order.
- `pdf_declarations`: the only authored input, one per PDF case, with its
  `pdf.*` diagnostics (all empty today), `insert_at`, spec anchor and
  justification. A scene whose nested SVG contains a pixel-domain filter must
  declare its `pdf.raster-fallback` warnings.
- `judgments`: path, sha256, role and existing approval references (embedded
  approval, recorded `approved_issue`, approved records naming the file and its
  digest) of every file under `tests/conformance` whose path contains `pdf`,
  outside `candidates/`, `coverage/` and `proposals/`, that matches a
  `judgment_scope` judgment pattern. Schemas, sidecars, READMEs, artifacts and
  viewer-qualification measurement evidence are listed as `non_judgment_files`.
  A discovered file that matches no pattern, or a judgment the record does not
  bind, fails verification.

```sh
cargo run --locked -p xtask -- conformance pdf-judgments-generate
cargo run --locked -p xtask -- conformance verify-pdf-judgments
```

The approved record owns `closure:pdf-judgments` through a `record` owner
([ADR-0015](../../../docs/adr/0015-own-closure-obligations-with-approved-judgment-records.md)):
`record-closure-owner closure:pdf-judgments pdf-judgments RATIONALE` binds its
digest after running this verification in-process, and
`verify-ownership` repeats it. See [OWNERSHIP.md](OWNERSHIP.md).

Generation keeps the authored declarations and rewrites everything else with a
null approval. Verification checks exact PDF execution coverage, report and
judgment digests, spec anchors, completeness, that derivation reproduces the
record, and cross-checks the golden record: every PDF report blob byte for
byte against the derived report, every SVG and PNG report blob as a JSON value
against the manifest-owned declarations.
The `implementation` descriptor records the derivation source used to generate
the record. Its path and hash are historical provenance, not a requirement that
today's source file have identical bytes or still occupy that path. Verification
rebuilds the record using today's derivation code, carrying forward only that
historical descriptor (and the accepted inventory provenance). The full
derivation rules, format rules, declarations, reports, file classifications and
judgment bindings must still match.

A source-only derivation refactor needs no record regeneration. Run
`conformance verify-pdf-judgments` to check the retained record
and golden-record agreement. Explicit generation still records today's source hash
and produces an unapproved record for review.

All 168 executions have an expected report: PDF from this record, SVG and PNG
from the manifests.

### Five-layer integration (`closure:all-five-layers`)

**Status: optional reporting.** The historical `layers.json` record and its
ownership assignments are no longer mandatory contribution checks.
`verify-ownership` excludes `layer:*` and `closure:all-five-layers` mappings from
ordinary validation, while continuing to check non-layer owners, PDF judgments,
registered case evidence and representative intake evidence independently.
Missing layer assignments are allowed. Actual test execution and input/output
verification remain separate requirements.

```sh
cargo run --locked -p xtask -- conformance layers-report
```

The command emits JSON with `advisory: true`, a freshly discovered `current`
record when discovery succeeds, and `findings`. It does not write `layers.json`
or require approval. New regressions appear in the current record; missing
owners and stale catalog bindings appear as findings. An undeclared corpus,
missing mapping file, or other discovery failure is reported with `current:
null`, rather than being presented as complete coverage. A zero exit status
means the report was produced, not that coverage or renderer correctness passed.

`verify-layers` without flags is a compatibility alias for this report.
`verify-layers --strict` and explicit global closure audits retain
strict historical verification for inspection only; a stale historical record
can fail those opt-in audits. The record's generation commands are retired
(ADR-0019); the report is the maintained form.

The description below documents the historical record format, not a required
update procedure.

The old live-repository five-layer audits are ignored by default. They can be
run explicitly with `cargo test --locked -p xtask --bin xtask
coverage::layers::tests:: -- --ignored`; they may fail against an evolved corpus.
Ordinary report tests use isolated inputs, so adding a corpus does not require
updating historical declarations or fixed coverage counts to run the test suite.

`layers.json` (`excalidraw-render/five-layers/v1`, approving issue #85) maps
every item of the five corpus layers of spec 14.2 to the ownership ledger.
`xtask/src/coverage/layers/derive.rs` discovers the items by rule and is bound
by digest. The only authored part is `declarations`, the reviewed mapping of
each exact-vector family, vector domain, stored vector corpus and oracle
directory to catalog rows. Selectors are exact ids or `prefix:*`.

| Layer | Items (discovered by rule) | Maps to |
| --- | --- | --- |
| 1 vectors | the 10 families parsed from "Exact vectors cover …"; `comparator-vectors/*/`, `crates/*/tests/fixtures/*-vectors.json`, `tools/*-vectors/`; every `tests/oracle/*/` | declared rows (`layer:vector:*`, `interaction:diagnostic-order`, `codec:*`); oracles are `vector`, `authority` or `profile` |
| 2 focused | every case in `cases/registry.json` | the case-label/layer rows naming it as a candidate |
| 3 representative | `candidates/registry.json` entries; `cases/representative/registry.json` #84 cases | coverage rows owned by that candidate's binding; #84 cases are checked by the verifier itself |
| 4 adversarial | `src/**/*_fuzz*`, `tests/*_fuzz*`, `tests/limits_*`, `**/frame_batch_property` | `layer:adversarial:<name>`, with the parsed `REQUIRED_RISKS` |
| 5 regressions | `crates/*/tests/qualification/*-regressions.json` | `layer:regression:<name>`, with digest, case ids and risks |

The record binds only what defines the layers' contents: `derive.rs` (the
rules), the text of the family sentence, the declarations, the tree digests of
the vector corpora, the registered case ids, each #84 case (`case.json`,
subject and intake), each fuzz target with its `REQUIRED_RISKS`, and the
regression corpora with their digests and ids. It binds no spec digest, catalog
digest, oracle bytes, fuzz source bytes or candidate registry. It never reads
the ledger either. Row selectors are expanded against the current catalog at
verification. The candidate registry is checked by rule: it must hold exactly
one entry, which must be the case bound by the owned
`interaction:frames-text-assets` representative owner. Every other
`candidates/*` directory, every unreferenced intake bundle and
`tests/public-scenes/*` is excluded by rule. So a representative successor, a
spec edit outside the sentence or a codec tranche does not stale the record. A
new corpus, oracle directory, case, fuzz target, risk or regression id does.

Verification rebuilds the record and requires equality. It then checks
completeness against the current catalog and the ledger's owners, using the ledger's own per-row owner
checks. Each vector item needs at least one owned row, every mapped `layer:*`
row must be owned, and every owned mapped row's owner must pass. Each
registered case must be the case owner of one of its rows. Candidate rows must
be owned by that candidate's representative binding. Every fuzz target and
regression corpus row must be owned, and no `layer:*` row may be left
unreferenced. Every problem is listed.

The retired writer/approval commands are not an update procedure. Historically,
the closure owner bound this record through the compiled-in `layers` verifier
(`verify-layers --strict` in-process today).
