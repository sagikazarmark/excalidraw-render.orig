# #191/#192 successor requalification — approved and applied

The user approved all three exact subjects below. Actual maintainer decisions:
[focused #44](https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5768106968)
and [successor #85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5768106992),
both dated 2026-09-21. The focused dependency update and three-file successor
migration are applied. Actual successor decision, preimages, successful receipts
and applied status are retained in
`tests/conformance/coverage/pdfium192-application-v1/`.

Application and independent `verify.py verify-applied` both passed the 109 bound
tests/six commands and the additional focused test. All six historical stale-
metadata failures are resolved. The full-suite log below remains an accurate
record of the pre-application run; it was not relabeled as green or rerun.
The remainder of this README preserves the reviewed proposal and procedure.

Reviewed final packet: **`manifest-v2.json`**. The earlier `manifest.json` is
superseded history, retained to expose review findings and corrections.

| Subject | SHA-256 |
| --- | --- |
| Successor/migration | `b0067aae018bec1787a8fa6d8d13aa73ca55211eead71956d4fd4c477eae65d7` |
| Representative V14 | `0b9f6a7d6eb1fd176d412855aecdb63b309ed260ae4d552054dbef794deb74ba` |
| Focused rectangle expectations | `a0351cf5b7b924e2262898ce888f9ff30e7b2eefeb99fe9f2906b56a6f0e3e16` |

## Verified scope

- Fresh V14 native outputs/full reports and all viewer-pair rasters equal V13.
- Bridge-v8 passes exact PDF structure and unchanged typography measurements.
- Fresh protected PDF/report/qpdf and two repetitions of both viewers match old
  evidence. `protected-bound.py` checks executable identity before and after each
  native run against V14's actual release build; verifier binds that proof.
- Fresh packaged capture at `0cbdad3`: all **168 full records unchanged**, including
  artifact/failure and complete report bytes, forward/reverse and two sessions.
  Subsequent xtask-only provenance changes do not change production sources.
- Proposed migration: **36 existing WebP source hashes**, V13→V14, and the two
  PDFium exception profile descriptors. Same three scopes and exact limits.
- **Zero new owners.** 2,418 obligations, 2,318 owners + three exclusions,
  97 gaps. Twelve partial-alpha WebP cells remain gaps; direct PNG72/PDF48.
- Historical diagnosis documents and producer bytes remain immutable. The new
  Rust check admits only the two exact archived→approved PDFium bindings under
  profile subject `c393a0dd…`, and still requires candidate-input/bridge equality.

## Verification results and corrections

The full workspace run is retained verbatim in `logs/full-workspace.raw.log`:
405 library and 627 fast tests passed; one focused-expectations test plus five
ownership/representative tests failed on stale metadata. Nineteen tests were
ignored. This run is **not green**. Clippy passed.

The existing focused rectangle ignored test was run explicitly and passed both
viewers and qpdf with unchanged exception limits. Its expectation manifest needs
renewed approval solely because the PDFium profile descriptor changed.

Independent Standards/Spec reviews initially found the focused-file freeze
conflict, omitted focused post-application check, and missing protected executable
binding. V2 fixes all three; read-only re-review found no remaining scoped blocker.
Three packet tests pass. The protected run was freshly repeated after its binding
fix. The historical full suite was not rerun.

The initial native snapshot attempt failed before collection because the output
parent was missing; `final-retry/` holds the actual successful capture, with failed
logs retained. Early representative exception preparation stopped on historical
PDFium bindings; the final consumer-only fix is explicitly recorded by the second
freeze. Native/viewer producer verification remained intact.

## Approval order and exact decisions

First approve the focused dependency update on **#44**:

```json
{
  "decision": "approve",
  "scope": "pdfium192-focused-rectangle",
  "approval_subject_sha256": "sha256:a0351cf5b7b924e2262898ce888f9ff30e7b2eefeb99fe9f2906b56a6f0e3e16"
}
```

Then approve both successor subjects on **#85**:

```json
{
  "decision": "approve",
  "scope": "pdfium192-successor",
  "production_subject_sha256": "sha256:b0067aae018bec1787a8fa6d8d13aa73ca55211eead71956d4fd4c477eae65d7",
  "representative_subject_sha256": "sha256:0b9f6a7d6eb1fd176d412855aecdb63b309ed260ae4d552054dbef794deb74ba"
}
```

Each must be the complete comment from the actual designated maintainer. No
approval has been manufactured or transferred. The verifier fetches decisions
from GitHub, rejects duplicate keys/prose/wrong scope, and authenticates exact
unsigned focused metadata and subject before applied verification.

After actual #44 approval, apply `pending/focused-pdf-expectations.json` with its
real approval record. Only the profile hash and approval block may change. The
verifier permits exactly that prerequisite transition and binds the applied file
in receipts. Then run:

```sh
python3 tests/conformance/proposals/pdfium192-successor-v1/verify.py apply /absolute/actual-successor-approval.json
python3 tests/conformance/proposals/pdfium192-successor-v1/verify.py verify-applied
```

The authenticated existing engine atomically applies the three successor metadata
files with rollback and requires actual successful **109-test/six-command**
receipts. Applied verification repeats them independently and additionally reruns
the exact focused-expectations test. Original receipt schema names remain from
the authenticated helper; all current subjects/paths/commands are explicitly bound.

The approved migration is complete. #178 baseline/native-WASM approval,
#187 sampling, #190 HDR and #191 partial-alpha comparison decisions remain open.
