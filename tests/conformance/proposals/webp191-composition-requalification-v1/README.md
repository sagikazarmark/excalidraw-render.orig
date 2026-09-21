# #191 first-frame WebP composition only — approved and applied

Current-production capture, V13/bridge-v7 evidence, final sealing, and independent
Standards/Spec reviews are complete. See [pending/README.md](pending/README.md) for
the exact approval subjects and preparation protocol. Both subjects were approved
in [issue #85 comment 5762673976](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5762673976).
The migration and fresh `suite_gate.py verify-applied` passed; actual decision,
preimages and successful 109-test/six-command receipts are in
`tests/conformance/coverage/webp191-application-v1/`. The preparation protocol below
is retained as a historical workflow description.

## Exact original evidence and source boundary

HEAD: `dd3389a`. `before/` and `basis.json` authenticate old WebP source/test, Cargo
inputs, ledger/registry/policy/inventory and unchanged historical helper sources.

| Record | SHA-256 |
| --- | --- |
| Frozen browser reference | `b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6` |
| Frozen independently encoded inputs | `21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4` |
| Original native defect observations | `d31850b8a23ed04039de976437a06b9922bbc963ee83f3b6269833d5cc24ba76` |
| New composition-only native observations | `6c9f48c8903604c7112085d64fd53325d30975f3552c01a3ace5e50dae5de668` |

The new observation record is native qualification evidence, not a refreshed
browser oracle or an automatic golden update. Both old and new records are
authenticated exactly. The new file is explicitly included in the completed freeze
even while untracked and outside the packet directory.

Fresh OLD `/tmp/opencode/webp191-old-dd3389a` captures all 168 packaged full keys,
two sessions per key in independent forward/reverse processes with complete
artifacts/failures/reports and archives. Normal snapshot verification passes.
`old/prior-capture-comparison.json` compares them to prior output while preserving
fresh HEAD provenance. This is not a baseline refresh or approval.

The only bound xtask edit is bridge-directory **v6→v7**; strict validation remains.
The frozen production change is first ANMF standalone decode and placement on
clear RGBA, not a static decoder, generic premultiplication or PDF-color change.
The standalone allocation/scratch/resource review is recorded in
`checks/FINAL-SOURCE-ANALYSIS.md`; it does not claim physical OOM closure.

## Exact 11 new cells and 12 residual cells

All eleven new owner IDs are listed in `basis.json`/`readiness.json`; the exact
selector is `webp_sample_contract::corrected_composition_cells_match_reference`.
They are opaque/binary animation composition cells explicitly named by the
reviewed test's `CORRECTED_CELLS`, never runtime-selected successes.

The 25 existing source-bound owners retain their three exact selectors, commands
and rationales: 13 original sample cells, ten strict-recovery controls and two
reserved-field accepted characterizations. Only their final source hash refreshes.
All twelve partial-alpha sample cells stay #191 gaps, and all other criteria,
dispositions, exclusions, catalog and 168 full inventory keys stay unchanged.

Projected totals: **2418 assignments / 2318 owners / three exclusions /
2321 assigned-excluded / 97 gaps**, after eleven additions plus 25 source refreshes
and V12→V13 representative selection. No policy-criterion amendment is needed or
permitted in this composition-only packet: successful cells pass the same gates.

## Measurement transition and residual semantics

The independent checker enforces exactly **30 changed vectors / 60 PNG/PDF format
records** within the 46 formerly unqualified measured vectors. All SVG records,
source identities, diagnostic records, dimensions and expected reference-view
hashes remain exact. Static and unaffected records cannot move. Alpha invariants
must pass. Eleven corrected cells (22 vectors) now pass both raster requirements;
**eight partial blend vectors improve but remain failing**, not eight promoted
partial cells. Full sample corpus pass counts are SVG72 / PNG48 / PDF48.

The residual 12 cells / 24 vectors retain exact new native hashes and 72 failed
views. Their failures remain canonical `SmallArtifactMismatch` (zero Pixelmatch
changed count does not grant a pass). Lossless straight-PDF sample invariants
remain required. No tolerance or comparator change can be smuggled through this
measurement transition; negative tests mutate these fields and must fail.

External `/tmp/opencode/webp191/REPORT.md` isolates two follow-ups: browser WebP
floor-premultiplied readback versus current rounded PNG projection, and a synthetic
PDF comparison between exact straight samples projected with the canonical rounded
rule and browser's floor-premultiplied captures. The latter is **not a measured
PDFium/PDF.js defect** or justification for changing exact straight PDF samples.
External diagnostics include a Rust 1.98.1 probe and system libwebp 1.3.2; they are
causal research, not canonical Rust 1.97.1 production qualification. A separate
PNG fix/comparison-boundary decision is required. None is authorized here.

## Completed preparation workflow — historical protocol

Parent finishes production/tests/resource review and fmt/Clippy, then provides
external JSON with `final_current_capture_authorized:true`, actual
`parent_freeze_message`, and `new_owner_selectors` mapping exactly the eleven
named IDs to `corrected_composition_cells_match_reference`.

```sh
P=tests/conformance/proposals/webp191-composition-requalification-v1
python3 "$P/workflow.py" freeze /tmp/opencode/webp191-parent-freeze.json
python3 "$P/workflow.py" stage /tmp/opencode/webp191-final-stage
python3 "$P/workflow.py" final /tmp/opencode/webp191-final-stage
python3 "$P/workflow.py" representative
python3 "$P/workflow.py" protected
python3 "$P/workflow.py" migration
```

Native collection pins Rust 1.97.1 release and memory-saving profiles. V13 performs
fresh format repeats, resources/font audit, all six viewer pairs, unchanged exact
PDF exception scopes, semantics and pending subject. Protected artifact/reports,
qpdf/viewers and V13 versus V12 artifacts/reports must remain identical. All 168
old/current records are compared; unexplained changes stop for diagnosis, not
golden refresh. The new producer data and standalone-frame allocation boundary
must be reviewed/tested by the parent before these commands.

`checks/parent-qualification.json` must bind actual final test and complete V13
producer sources, all reference/old/new measurement hashes, `composition_only:true`,
`static_or_alpha_policy_change:false`, 36 sample cells/24 passing/12 residual,
SVG72/PNG48/PDF48, the exact `measurement_transition` returned by the checker,
passed focused log descriptors, prior-codec regression results/known limitations,
and every `diagnosed_inventory_changes` row (empty only if full equality).
Full-suite outcomes and expected pending binding failures must be recorded honestly.
Only then may `workflow.py seal` create pending exact review subjects.

## Strict approval, successful receipts and rollback

The helpers are imported only after authenticating archived and original bytes.
The latest #188 v2 receipt validator is reused unchanged except explicit version/
count strings and #191 runtime identities: exact successful 103+5+1 test names,
six command vectors, zero exits, stdout/stderr hashes, current source/subject/
approved-metadata bindings, three native execution receipts and independent fresh
bounded reruns. Old approved scripts/packets are never edited.

After independent review and actual user approval of both final subjects,
designated maintainer **sagikazarmark** must record an issue #85 comment whose
entire body is one standalone JSON fence with exactly:

````text
```json
{
  "decision": "approve",
  "scope": "webp191-composition",
  "production_subject_sha256": "sha256:ACTUAL_FINAL_SUBJECT",
  "representative_subject_sha256": "sha256:ACTUAL_V13_SUBJECT"
}
```
````

This template is not approval. Real saved/fetched author/date/URL/ID/body and both
subjects must match. Rejections, hash mentions, other authors, alpha-wide scopes,
duplicate/unknown fields or stale decisions fail closed.

```sh
python3 "$P/receipt_gate.py" apply /absolute/path/to/actual-approval.json
python3 "$P/receipt_gate.py" verify-applied
```

The guarded transaction stores actual decision/preimages before writing exactly
three metadata files (V13 approval/registry/ledger). It runs required successful
109-test/six-command receipts before applied status; on failure all three files
are restored and byte-checked, with failed attempt/partial logs preserved and no
automatic overwrite retry. Applied verification authenticates first and independently
repeats the same bounded checks. No synthetic live approval is used in tests.

Original browser references, old native observations, all approved representatives,
baseline/#178, global closure, #187 sampling, deferred #190 HDR and CMS resource
limitations remain untouched. No actual approval, live migration or commit during
preparation. See `file-inventory.json` for exact packet contents.
