# Historical tooling retirement audit

## Approval-gate retirement (2026-09-26)

ADR-0019 replaces hash-bound issue-comment approval with ordinary Git/PR review
and reproducible tests across evidence families. Approval-writing CLI commands
and approval-only tests are retired. Old metadata remains historical and cannot
grant or block execution. Input/output descriptors, scope and tolerance checks,
native replay, completeness and downloaded-tool integrity remain active.

The standalone `verify-png-stage4.mjs` and `verify-six-case-application.mjs`
auditors are retired with their completed approval transactions. No current
Rust/CI runner calls them; their transition artifacts remain in the corpus and
their source is in Git history. Current ownership, candidate, snapshot and PDF
judgment verifiers exercise the corresponding substantive evidence.

**Final integration status (2026-09-25): resolved.** No proposal `.py` files
remain, including the last frozen PDFium adapter. Current first-party Python is
**two files**, both CI/release-owned, inventoried
individually with the completed representative CI handoff in
[python-inventory.md](python-inventory.md). Counts and retention decisions in the
earlier passes below are historical snapshots, not current blockers.

## Latest retirement: evaluations and final audit (2026-09-25)

Retired eight Python experiment runners: BMP `run-browser.py`, color
`{run,reference,compare}.py`, kurbo `{run,compare}.py`, and JPEG diagnosis
`run.py` / `hardening/run.py`. Their probe sources, patches, fixtures, observations
and research conclusions remain. Their README/research commands are explicitly
historical transcripts. Ordinary `jpeg_color_contract.rs` regressions cover
the sampling defect and `jpeg_release_limitation.rs` covers truncated entropy;
the unadopted hardening patch and mixed-scan discrepancy remain research inputs.

Moved the ninth remaining non-CI Python file, `webp191-verify-boundary.py`, to
Node, preserving the retained-data and semantic checks. Before deletion both
implementations reported 144 viewer comparisons, 336 independent-buffer and
336 paired semantic rejections, and 48 partial-alpha canonical failures. The
Node audit now runs automatically in the xtask integration suite.

Release qualification workflow/generator/runner descriptors now describe the
historical producer rather than demand current source bytes. Corpus, profile,
schema, measurement and aggregate checks remain live. JPEG reference capture
producers and the capture-time Cargo lock likewise stay historical; reference
inputs, profiles and real renderer comparisons remain live. No retained evidence
record or output bytes were rewritten to match new source hashes.

## Latest retirement: representative intake (2026-09-25)

The final **nine representative Python files** have been removed: `intake.py`,
`case.py`, `evidence.py`, `controls.py`, `viewer_assets.py`, and
`test_{intake,case,evidence,viewer_assets}.py`. The standalone comparator's three
files (`compare/Cargo.toml`, `compare/Cargo.lock`, `compare/src/main.rs`) have also
been removed; comparison now runs in-process in xtask. The Rust `viewer-fonts`
helper remains current.

The representative Dagger module now runs Rust tests and intake/case smoke
commands, and its dedicated GitHub Actions workflow has been deleted. Legacy v1
capture verification validates the historical tool descriptor without reading
`intake.py`. Retained observations, compact v1 records and their historical files
remain readable by the Rust compatibility paths.

New representative cases use normal PR review and may retain `approval: null`;
both registry and coverage independent-case validation accept it. Historical
non-null approvals still require valid metadata and the exact subject hash.
Materialization does not register a case: registry integration follows review.
Other evidence families retain their approval requirements. This latest note
records the completed migration, not a new CI or viewer-suite run.

## Earlier retirement audit

Audited 2026-09-25 against the working tree, including current fixtures, runtime
tests, reference-oracle consumers, and JSON path/hash descriptors. The proposal
Python inventory contained **108 tracked files** before this retirement (the
requested family was described as 109). Age alone is not a deletion criterion:
some completed scripts are still authenticated input bytes for other consumers.

## Retired: 25 Python files

Under `tests/conformance/proposals/`, retired **both `apply.py` and `collect.py`**
from each of these twelve directories (24 files):

- `stage4-v15-successor-v1/`
- `stage4-v16-successor-v1/`
- `stage4-v17-successor-v1/`
- `stage4-v18-successor-v1/`
- `stage4-v19-successor-v1/`
- `stage4-v20-successor-v1/`
- `stage4-v21-successor-v1/`
- `stage4-v22-successor-v1/`
- `stage4-v23-successor-v1/`
- `stage4-v24-successor-v1/`
- `stage4-v25-successor-v1/`
- `stage4-v26-successor-v1/`

Also retired: `tools/jpeg187-diagnosis/capture-hardening.py` (one file).

### Evidence for retirement

- Every V15–V26 proposal README records **approved and applied**. Each candidate's
  current `manifest.json` has an approval subject equal to the corresponding
  proposal's `logs/subject.stdout`. The scripts implement fixed, one-time
  predecessor-to-successor transactions and create-only collections.
- Repository reference searches found their full paths only in those twelve
  historical READMEs and twelve `freeze.json` inventories. Searches of executable
  source/configuration, including runtime tests, CI and Dagger definitions, found
  no caller of the successor directories or hardening capture script. A recursive
  JSON descriptor audit found no `{path, sha256}` consumer of the retired files,
  including packet-relative descriptors.
- The whole-tree `freeze.json` maps were consumed by the retired collectors'
  own `check()` functions. No remaining executable references these successor
  packets. Their maps remain historical observations, not current file-existence
  requirements. README application commands are preserved historical transcripts;
  they are no longer runnable maintenance instructions. Source is recoverable
  from Git history.
- `capture-hardening.py` was a create-only copy from fixed `/tmp/opencode` probe
  directories into the already-populated `hardening/` archive. Its only references
  were those successor freeze maps. The documented hardening reproduction uses
  `hardening/run.py`, which consumes the archived patch, sources and lockfile
  directly and does not call the capture script.
- ADR-0017 distinguishes approved output/replay from historical producer
  provenance. This retirement removes completed orchestration, preserving the
  candidate artifacts, approvals, logs, freeze maps and owner-tranche records.

## Initial-pass retained blockers and continuing uses (historical)

| Family | Why retained |
| --- | --- |
| `png-device-phase-six-case-v1/prepare.py` | `tools/reference-oracle/scripts/verify-six-case-application.mjs` iterates `batch.files` and reads/hashes each packet-relative descriptor; the batch binds `prepare.py`. It is data even when not executed. |
| `stage4-format-requalification-v1/` and `v2/` | V2's `predecessor()` authenticates and executes V1 source. `test_requalify.py` uses the real V1 templates and authenticated helper in disposable-root tests. |
| JPEG187 V1/V2, including `orchestration-successors/`, `nested*.py`, `retire_rc.py`, and `retired-rc-plan/` | V2 `verify.py:evidence()` checks every V1 manifest file; `load_frozen()` authenticates and executes helpers. The manifest binds even the completed orchestration and retired-RC scripts. `test_collect.py`, `test_qualify.py`, `test_split_verification.py`, and V2 tests read these helpers at runtime. |
| JPEG188 V1/V2 and JPEG189/190 workflows, receipt gates, suite gates and orchestration | Packet verifiers iterate manifest descriptors; later gates authenticate the original `suite-verification.py`, workflow and receipt helper source. Runtime tests reuse authenticated helpers and even earlier test source. Removing a script with no direct invocation can still break these readers. |
| Frozen `before/**.py` copies | These are preserved source identities used by authenticated helper loading and evidence descriptors, not duplicate executable clutter. Keep the exact bytes until their consuming packet can be retired as a unit. |
| `tools/bmp-evaluation/run-browser.py` | Documented native/browser-WASM differential replay against `expected.json`, backed by the isolated Rust probe. No completed one-time migration to retire. |
| `tools/color-evaluation/{run,reference,compare}.py` | Documented native/WASM/browser replay and read-only lock, observation and matrix checks; `compare.py` also reads current production source for line accounting. |
| `tools/kurbo-evaluation/{run,compare}.py` | Documented current/candidate and native/WASM replay, plus exact observation and matrix verification. |
| `tools/jpeg187-diagnosis/run.py` and `hardening/run.py` | Research documents retain explicit reproduction commands; result records bind the source/patch identities. These reproduce diagnostics in fresh external trees rather than perform a completed repository migration. |
| PDFium successor/alpha and WebP191 proposal Python | Retained evidence workflows with source-bound packets; no retirement in this pass. |

At that point, further JPEG/format packet retirement needed an explicit decision
about their read-only audit consumers and authenticated test fixtures. Merely removing
Python execution or rewriting a manifest to omit source files would not preserve
those contracts.

## Verification boundary

Results: **1,892 Python lines removed**, proposal Python count **108 → 84**;
all remaining proposal scripts parsed without imports or bytecode writes.
The post-deletion reference/descriptor audit passed, including tracked files and
current untracked configuration/source (excluding unrelated user research).
The retained-script descriptor census was identical before and after: 84 matching
identities and five historical noncurrent identities. These old identities were
not refreshed. **29 existing read-only tests passed** from JPEG187 split
verification, JPEG188 suite verification, and JPEG189/190 suite gates. Only
classification, scope, inventory and authenticated-loader tests were selected;
workflow startup and transaction/rollback tests were excluded. Scoped
`git diff --check` passed.

Use static reference/descriptor audits, syntax parsing without imports, and
read-only log-classification tests. Do not run collection, sealing, application,
or migration commands as a cleanup check. Existing whole-tree freezes describe
past repository states; they are not a clean-tree test for current development.

## Subsequent retirement: completed workflow families (2026-09-25)

The user explicitly approved retiring the completed audit/approval workflows as
whole families, including their self-referential source-hash chains and workflow
tests. This supersedes the proposal/diagnostic retention decisions above; the
earlier audit is retained as history. Archived source descriptors still describe
the original capture and have not been rewritten to match this cleanup.

### Scope and counts

This pass deletes **97 files / 14,459 lines** beyond the earlier retirement:

- **83 proposal `.py` files**: all remaining PNG six-case preparation, stage4
  format V1/V2, JPEG187 V1/V2, JPEG188 V1/V2, JPEG189, JPEG190, WebP191 and
  PDFium192 successor workflow code. These include **16 workflow-only Python
  test files** and **11 frozen `before/` Python copies**. Migration/rollback,
  approval, receipt classification, source inventory and seal tests retire with
  their implementations; they are not renderer or fixture-semantic tests.
- **Three extensionless Python helpers**, `pdfium192-successor-v1/helpers/{auth,
  receipt,snapshot}`. Their only runtime consumer was the retired successor
  verifier's authenticated loader.
- **One** `coverage/jpeg187-application-v2/transaction.py`, the completed,
  create-only application/rollback orchestrator.
- **Nine PDF diagnostic producers**: `pdfium192-{audit,proposal}.py`,
  `stage4-image-colour{,-package}.py`, `stage4-pdf-approved-transition.py`,
  `stage4-typography.py`, and `webp191-{comparison-boundary,partial-viewers,
  pdfium-raw}.py`.
- **One** `tools/pdf-harness/probe-pdf-writer.py`, the completed #186 bounded
  serializer investigation. Its Rust example and research/results remain.

Proposal `.py` inventory reached **108 → 1** across those two passes (**84 → 1** in
that pass); final integration below removes the last copy, reaching **zero**.
All evidence JSON, fixtures, logs, outputs, approval records, source
inventories, Rust controls and research prose are preserved. Historical prose
commands naming retired scripts are transcripts, with source available in Git
history, rather than supported maintenance commands.

### Current consumers and meaningful checks

- Current Rust, Node, Python, shell, CI/configuration and Dagger source searches
  found no remaining executable caller of the retired proposal workflow family.
  Rust representative retention/viewer tests and Python intake readers still
  consume proposal **data**; that data was preserved.
- `tools/reference-oracle/scripts/verify-six-case-application.mjs` still serves
  `verify-png-stage4.mjs`. Only the exact packet-relative `prepare.py` descriptor
  is historical now. The unchanged pinned approval subject authenticates it;
  every other `batch.files` entry still requires actual bytes, length and hash.
  Artifact, metadata, approval and transition assertions remain in place.
- `tools/pdf-harness/diagnostics/webp191-verify-boundary.py` remains a meaningful
  offline data verifier. It checks both pinned measurement records, every output
  hash, reference/inputs, viewer profiles and qualification evidence, all 144
  paired viewer results, and the negative color/alpha/compositing checks. The
  exact six recorded producer source paths and historical CLI digest no longer
  demand installed old source/binary bytes. It does not invoke Cargo or a viewer.
- The only proposal Python retained at that intermediate point was
  `pdfium192-alpha-requalification-v1/before/tools/pdf-harness/pdfium/render.py`.
  At that intermediate audit, Rust
  `conformance_candidate_exceptions.rs::validate_pdfium_alpha_successor()` and
  its test read those exact archived bytes. This was a live external consumer,
  unlike the deleted workflow-internal copies.

### Final integration: former parent-owned blockers resolved

- `conformance_candidate_exceptions.rs::proof()` now passes only the exact
  retired typography/PDFium producer paths to `validate_retired_producer()`.
  It pins `stage4-typography.py` to **15,075 bytes** /
  `dd168deb66a8027c3088a24eaabc68f204bd391193c72c5c564527e005b6791b`
  and `pdfium/render.py` to **6,188 bytes** /
  `2b3f28ba7633f739cdd1b8617361f8d1a82f51f62204286598e38a5d29ad1c6d`.
  The report/evidence digest pins, other live repository bindings, exact
  scene/PDF/SVG/raster inputs and colour bridge remain checked. Tests reject
  mutations of each retired descriptor's path, digest and length.
- `validate_pdfium_alpha_successor()` now handles the archived **profile JSON**,
  current approved profile digest and exact approval subject, not the frozen
  Python adapter. The final
  `pdfium192-alpha-requalification-v1/before/tools/pdf-harness/pdfium/render.py`
  copy is deleted; profile JSON and evidence remain. Proposal Python count is
  **108 → 0** over the full retirement.
- `conformance_candidate_gate.rs` validates historical source descriptors for
  safe relative paths, uniqueness, lowercase SHA-256 shape, allowed fields and
  optional unsigned byte length. Required source membership remains enforced;
  `producer_sets()` requires the exact expected set and still hashes captured
  candidate inputs. Historical provenance remains inside the approved subject.
  Retired source bytes are not restored merely to satisfy historical hashes.
- `conformance_pdf_requalification.rs` is now test-only historical policy code.
  The old production `harness_bindings()` reader is gone, and the historical
  requalification-subject command explicitly reports its retirement. Current
  `generate-fixtures.py`, `structure/inspect.py`, `pdfium/render.py` and
  `pdfium/test_alpha.py` have all been removed; Rust harness/adapter sources and
  wrapper tests own their replacements.
- The preserved `crates/excalidraw-render/examples/pdf_writer_probe.rs` module
  comment names the retired probe driver; it is historical documentation, not
  an executable caller.

### Verification

- `python3 -B tools/pdf-harness/diagnostics/webp191-verify-boundary.py` passed:
  **144** independent viewer comparisons, **336** independent-buffer and **336**
  paired semantic mutations rejected; **48** partial-alpha judgments remain
  canonical failures.
- Node syntax check passed. Read-only packet validation passed for **115**
  retained six-case artifact descriptors and the unchanged approval subject.
- Full `verifySixCaseApplication()` reaches its live metadata checks but fails
  on `tests/conformance/coverage/inventory.json`: the archive expects
  `5d4c81d5cc17b5153fe5b0a6784c0e2499a947c77eb32d600c6a65e6a7130faa`,
  while the already-dirty working tree has
  `89fa1f7931e5f75c791e085f5671052d92a60a64d106f2fe9dd659f55cae6cb6`.
  This existing historical-state mismatch is not a missing producer file and
  was not bypassed by broadening the author-file exception.
- Scoped `git diff --check` and the deleted-file type audit passed. The only
  deleted non-`.py` files are the three extensionless Python helpers. No
  collection, migration, application, seal, or rollback script was executed.
- The original pass did not run the full Rust suite. Final source inspection
  confirms the integration blockers above are resolved; that is not a new claim
   of full Rust-suite success. The latest remaining Python inventory and completed
   representative handoff are recorded in [python-inventory.md](python-inventory.md).
