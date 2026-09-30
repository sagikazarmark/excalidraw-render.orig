# Stage 4 binding reconciliation — applied, tooling ready for final capture

The parent explicitly authorized the finalized §8.5 PNG rule, locator-only
binding reconciliation, focused-tested owner source refreshes, and the single
native Excalifont PNG canary update. This unit applies those changes. It does
not modify production rendering, approved references, baseline approvals, the
protected PDF backend pin, or ownership dispositions. No commit was made.

## Confirmed production/spec identities

All five parent-supplied hashes matched before work and remain the release basis:

| File | SHA-256 |
| --- | --- |
| `docs/renderer-spec-v1.md` | `628acb6d7cf2e2756b1d0bacbab3aab9ee73b548df5252057747f7ea17270243` |
| `docs/research/stage4-png-device-phase-assessment.md` | `286a8aaf93d0050608baac9cb86c074fbd513bf20fdc616dc034a5db1cd24e40` |
| `src/render/png.rs` | `a089e88fb04f58f40ab186ead55a973048e5099995ad13a653be15006d0d2ade` |
| `src/render/png/paint.rs` | `c7d105d9dad483742df6e586fcd8fbee718d51fc95bce0144b669e8a8b9689ec` |
| `src/render/text.rs` | `f8f36b8377a9ce437c1734099e6118941e33081998463d1035361e26addc24f7` |

The last three paths are relative to `crates/excalidraw-render`.

## Locator, inventory, catalog and ledger

`fuzz.codecs` still names the same executable APNG test. The source changed only
at the independently qualified cropped-image PDF matrix assertion:
`-30 0 0 -20 30 20 cm` → `-30 0 0 20 30 0 cm`. The reconciliation checks that exact
single source replacement against commit `629930b`, retains the locator path and
anchor, and updates only its source digest. All 29 static-PNG tests, including the
APNG and crop controls, passed in release mode first.

`xtask ... reconcile-stage4-bindings` performed one guarded, explicitly scoped
transition. Its durable record is
`tests/conformance/coverage/stage4-binding-reconciliation.json`:

Record SHA-256:
`731cdcb4182fc017346a18b8ef193b3c57c0a292fc524df714a920170a3d77d9`.

- 1,007 spec-authority descriptors migrated for the approved amendment.
- Refreshing the locator necessarily changes the inventory file's byte identity;
  118 existing case-label authority descriptors therefore migrate too. This is
  recorded separately from the spec-only migration.
- All 168 execution keys **and their complete records/options** and all coverage
  rows are unchanged, checked by structural equality after normalizing only the
  reviewed locator bindings.
- 101 owner-source descriptors across five tested modules were refreshed. Owner
  selectors, exact commands, rationales, criteria and dispositions are unchanged.
- All 1,876 assignment records retain their disposition semantics; all three
  approved exclusion records and their approval subjects are preserved and
  revalidated. No representative owner was assigned; that interaction remains a gap.
- Helper hashes were already current and passed their consuming checks; none was
  blindly refreshed.

The record authenticates its exact before-ledger and fixed reviewed source
hashes, checks both catalog transitions by normalization/equality, validates the
resulting ledger, and refuses reapplication. It has no general digest override.

| Binding | Before | After |
| --- | --- | --- |
| Requirements file | `806d3fa98354cab1cff3b4103b4c262f47cfcd440805fe0fbdc709a7b6b53b44` | `f409dcd3b2e204fe5b4b825d4aa6bac2c576f45a5d82ae129f29b7d331718b9b` |
| Inventory file | `9db3087526fc25ca1a68ab5f3270ef863765e7f009684fe66d41446747869245` | `3c01839cc8ab14f7092591607b847a2c3d5cb3f797b0c233441f3f217782e91b` |
| Ledger file | `6882c37db5a89ebcccce0ee0b0a5bf399cbcc891cb20a10e19f3bfea1e0d7788` | `fdc86c8ab6e29371d13f95a0190735286aca764ef0c1c1f3e663a889a33402b5` |
| Derived catalog | `fb90d519e1ddb2a21b40357df48ab47f02efdce02403705f8890e9e1e7ef5001` | `59cf9f6032d7bf916f9e2e336f3cb567bf4ef09477c70470976ccf525b663fba` |

The intermediate spec-only catalog was
`1f6cc7cd79145cb47b928283356d2a941a9e36efde5f26d68fb8e299cbd6b7d8`.
Using that digest alone after refreshing the inventory locator would be wrong.

## Authorized native Excalifont PNG canary

The initial release test reproduced exactly:

```text
old expected: cb90c943bc71d5eec983bf2c933e1c3c16dd4ca7b5fc14ba79f0230404209534
new observed: 9b2da5d13b6a2c672ab80be51edbeeb9491195b57939dea9f48e80e3887f3b95
```

The existing direct-PNG reference-budget test passed. Additional independent
qualification used the exact rotated/translucent canary scene, not just the
unrotated focused reference: pinned official Excalidraw v0.18.1 under Chromium
149.0.7827.55, closed routes/fonts, verified reference profile, and two fresh
browser processes. The official oracle requires complete records; explicit
default fields were supplied and then proved to produce byte-identical native
outputs to the sparse canary scene for all three formats.
The initial sparse-source oracle attempt was rejected for missing required
`backgroundColor` before producing evidence; the successful complete-record
run and native equivalence checks are the qualification used here.

Result at 118×102: **0 changed pixels, SSIM 0.9993110230358684**, exact bits
`3feffa5b1c4f155c`, canonical typography pass. Native SVG and PDF remained exactly:

- SVG: `25ce18f298281f6a899de079bd4e065d3f0e62d0e863fb431f8e55cc3350a90c`
- PDF: `ed5dd7b2faf6fcd7c56c6afd5f8adbfb01d4326079414de7b16ef9d53cc2c6bb`

Only the native PNG canary literal and its explanatory comment changed. All six
Excalifont tests subsequently passed in release mode. The independent reference,
native artifacts and provenance are retained under
`tests/conformance/candidates/stage4-excalifont-canary/`; qualification record hash:
`f05f408b7187af1b2ae5edb7c451179afb50d0f1bcb752926e610067ab431e87`.
No protected reference artifact or approval was repinned.

## Actual release owner positive integration

`representative_owner_release_positive_path_uses_real_gate_in_isolated_fixture`
passed. It creates a fixture only under `target/`, renders fresh native release
SVG/PNG/PDF, and checks release SVG/PDF bytes equal the qualified viewer inputs
before reusing those known viewer rasters as synthetic verifier data. It rebuilds
resources/bindings, applies exact proposal checks and materializes through the
real gate. It verifies refusal without approval, then exercises the real approved
gate and three subject-bound owner receipts with an isolated synthetic approval.
Artifact corruption is rejected afterwards. The real registry and ledger are
never given that synthetic approval or owner. This is a verifier integration
test, not a final candidate collection or new viewer qualification.

## Checks and current state

Pinned Rust 1.97.1, debug0, incremental disabled. Focused release checks passed:
25 Stage-4 PNG tests, 29 static-PNG tests, five image-field tests, four interaction
tests, the full projection-boundary test, and six Excalifont tests after the
authorized canary update. The real release-owner positive integration passed;
four ownership CLI tests and five coverage CLI tests passed. xtask Clippy passed.

Normal verification now reports:

```text
verify-inventory: 320 coverage rows, 168 executions, 8 unresolved milestones
verify-ownership: 1876 assignments, 1802 assigned/excluded, 74 explicit gaps
stale locator/source audit: no stale locators; no stale owner sources
```

`verify-png-stage4.mjs` now verifies the explicit reconciliation rather than
assuming the spec/catalog never changed or treating retired v1 evidence as
eligible. It passes, including all frozen oracle/helper hashes, prior disposition
semantics, original execution records and protected reference preservation.

Tooling/bindings are ready to freeze for the next final **release** candidate
capture. This unit did not collect a final representative candidate. V6 remains
historical dev-profile evidence. Real candidate/exception approval and global
ownership approval remain pending. The protected PDF backend pin still requires
its separate structural/viewer qualification, and existing native baseline
approval remains intentionally stale/unchanged. Pre-existing fixed-size moxcms
execution-table allocations remain a global resource/OOM gap; this does not
claim end-to-end CMS OOM closure. No full-workspace suite or commit was run.

The preserved baseline was checked separately and still correctly reports
`stale snapshot source tests/conformance/coverage/inventory.json`. That protected
baseline approval was not rewritten; it is distinct from the now-clean live
inventory/ownership locator checks. Release builds also emitted the existing
unused `metadata::scratch_bytes` / `generation_work` warnings; production code
was left untouched.
