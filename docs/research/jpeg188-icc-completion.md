# JPEG #188 ICC correction completion — 2026-09-21

The user explicitly approved both exact subjects. The recorded affirmative
maintainer decision is
[#85 / 5755392381](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5755392381),
by `sagikazarmark` on 2026-09-21. The agent recorded the user-granted decision;
the qualification scripts did not grant approval.

## Applied subjects

- Production, qualification and exact migration:
  `sha256:9ac73ac1c6ae2e3622c08d4254ba7dcb49d350047a12651e31590b1bacb2aa01`.
- Representative `representative.stage4-frames-text-assets.v10`:
  `sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15`.

The guarded v2 transaction persisted actual approval, comment and exact preimages
before changing the V10 manifest approval, supplemental registry and ledger.
It applied exactly **16 new ICC owners, 64 existing source-binding refreshes and
V9→V10**. It preserved all criteria, three exclusions, 168 full inventory keys
and the existing #187 sampling gaps. No rollback was needed.

Actual receipts are under `tests/conformance/coverage/jpeg188-application-v2/`.
Both `apply` and the independent fresh `verify-applied` passed. The latter checks
the genuine GitHub decision and reruns the six required commands, covering the
three affected test targets and approved candidate/registry/ownership/inventory
checks. All **109 tests (103 + 5 + 1)** pass, resolving the five pending binding
failures. Both consumers return exactly three approved SVG/PNG/PDF receipts.

## Correction and evidence

- One-component JPEG matrix/TRC profiles use 16-bit CMS and fixed-size scratch,
  processing backwards when expanding gray to RGB. Output is quantized once.
- A valid gray matrix/TRC profile on a three-component JPEG follows the measured
  reference behavior and is ignored. Missing TRC, unsupported PCS and non-image
  profile classes remain rejected; actual transform construction validates usability
  and preserves typed CMS errors. Gray LUT routing was not broadened.
- An RGB profile on gray JPEG remains applied, using the higher-precision path.
- Dependencies, both lockfiles and original reference evidence remain unchanged.

All **160 ICC vectors** pass their applicable gates: **104 canonical-color** and
**56 strict-recovery** vectors. Ten final ICC tests cover reference colors,
independent reconstruction, exact neutral anchors, scratch boundaries, mismatch
precedence, malformed recovery and negative artifact controls. The auxiliary
analytic gray ±1 check is not a relaxed canonical visual budget. Additional
YCbCr mismatch checks are metamorphic regression evidence, not new browser
qualification claims.

All **168 packaged artifacts/failures and reports match OLD `b01f2db` exactly**,
with repeated forward/reverse process/session collection. V10 artifacts/reports
and six viewer-pair rasters equal approved V9. Protected PDF structure, artifacts,
reports and repeated viewers are unchanged. Existing #187 evidence remains
64 canonical vectors plus 12 failures with 24 exact defect pins.

## Verification history and reviews

The full `cargo test --locked --workspace --no-fail-fast` invocation completed
with **1,226 passed, five pending stale-binding failures and 19 existing ignored
tests**, including all 595 fast tests. The raw result is preserved in the packet;
it is not described as a clean run. The subsequent 109-test rerun resolved those
five failures after actual approval/application. No redundant full suite ran.

Formatting and Clippy with warnings denied passed. Independent review corrected
an overly broad malformed-gray-profile bypass before source freeze. Final packet
review required actual post-approval success receipts, implemented in the strict
v2 successor with 17 passing negative/transaction tests. Standards and Spec
re-review found no remaining scoped blocker. Historical v1 and earlier approval
packets remain immutable.

## Final ownership and remaining work

| Measure | Count |
| --- | ---: |
| Obligations | 1,992 |
| Primary owners | 1,912 |
| Approved exclusions | 3 |
| Assigned/excluded | **1,915** |
| Explicit gaps | **77** |

All 80 bounded ICC matrix cells now have owners. This is not exhaustive JPEG ICC
or codec closure. Global ledger approval remains null. #187 baseline sampling,
#178/native-baseline work, broader codec/profile qualification and pre-existing
fixed-size CMS allocation/resource limitations remain open or deferred.

Current applied audit (requires GitHub access and executes bounded checks):

```sh
python3 tests/conformance/proposals/jpeg188-requalification-v2/verify.py verify-applied
```
