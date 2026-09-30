# JPEG #189 EXIF correction completion — 2026-09-21

The user explicitly approved the exact production and representative subjects.
The user-granted affirmative maintainer decision was recorded at
[#85 / 5756275535](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5756275535)
by `sagikazarmark` on 2026-09-21. No qualification script grants approval.

## Applied identities and scope

- Production/qualification/migration:
  `sha256:d3dadca97b24fd4b2489b051fed00b9bb8540dc5f1305ac5c4ab7df50451ec61`.
- Representative `representative.stage4-frames-text-assets.v11`:
  `sha256:d8e086187884e412c78b98197957e2793b3f3a7f0c5fd11df72ceac4dbb3f87c`.

The guarded transaction persisted the actual decision and exact preimages before
applying **36 new EXIF owners, 46 source-only refreshes and V10→V11**. Only the
V11 manifest approval and reviewed registry/ledger metadata changed. All prior
criteria, other dispositions, three exclusions and 168 inventory keys survived.
Application receipts are in `tests/conformance/coverage/jpeg189-application-v1/`.

Both the application and independent applied audit passed. The audit verifies
the actual affirmative GitHub comment, exact successful receipts and a fresh
execution of the six bounded commands: **109 tests (103 + 5 + 1)**, candidate,
registry, representative ownership, inventory and ownership checks. All five
previously stale-binding failures are resolved. Candidate and registry each
return exactly three approved format receipts. No rollback was needed.

## Implementation and qualification

JPEG now selects the first complete EXIF APP1 prefix before the first SOS, even
when that block has no usable orientation. A JPEG-specific bounded TIFF reader
skips invalid numeric orientation values in IFD0 and chooses the first valid
value. Invalid entry type/count and malformed bounds retain the tested fallback.
Both readers use checked offsets, bounded slices and forward progress, with no
allocation. Shared PNG/WebP orientation interpretation remains unchanged.

All **164 frozen EXIF vectors** pass canonical gates, with exact SVG source
bytes, artifact/page/image dimensions, empty reports, replay, independent sample
permutations and orientation discrimination. The former 72 failing vectors now
have canonical assertions rather than accepted defect pins. Six focused EXIF
tests include malformed header/TIFF controls; all **34 JPEG regression tests**
pass. ICC160 and the previous JPEG color matrix retain their applicable gates;
the six #187 sampling cells remain explicit failures with 24 exact pins.

All **168 packaged artifacts/failures and complete reports equal OLD `7b432b6`**
under forward/reverse, two-process/two-session capture. V11 artifacts/reports and
six viewer-pair rasters equal approved V10. Protected PDF structure and repeated
native/viewer evidence are unchanged. Dependencies, original reference bytes and
existing exception ceilings were not changed.

## Verification history

The full `cargo test --locked --workspace --no-fail-fast` invocation completed
normally with **1,232 passed, five pending stale-binding failures and 19 existing
ignored tests**, including 601 fast passes. Its raw failures remain recorded;
the run is not described as green. After actual approval/application, the bounded
109-test rerun and independent repeated audit passed, resolving those failures.

Formatting and Clippy with warnings denied passed. Independent Standards and
Spec review found no scoped production or final packet blockers. The packet
includes negative authorization, migration, receipt and full-suite-classification
tests. No redundant full-workspace rerun was needed after metadata application.

## Final ledger and remaining work

| Measure | Count |
| --- | ---: |
| Obligations | 2,074 |
| Primary owners | 1,994 |
| Approved exclusions | 3 |
| Assigned/excluded | **1,997** |
| Explicit gaps | **77** |

All 82 bounded EXIF precedence cells now have owners. Broader TIFF/IFD-chain,
MPF/gain-map and codec qualification remain open. #187 sampling, #178/native
baseline work, global ledger approval and pre-existing fixed-size CMS resource
limitations remain separate. This completion record does not claim release closure.

Current applied verification:

```sh
python3 tests/conformance/proposals/jpeg189-requalification-v1/suite_gate.py verify-applied
```

It requires GitHub access and reruns the bounded approved checks.
