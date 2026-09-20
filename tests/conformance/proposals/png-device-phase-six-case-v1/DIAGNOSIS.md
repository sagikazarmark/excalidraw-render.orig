# Six focused PNG exceptions after per-glyph placement

This is a bounded requalification proposal. No protected live case, exception,
approval, renderer or xtask adapter is changed. The parent reported a complete
workspace run passing fast/library/xtask and approved representative execution,
with only the two parity targets failing at these six stale PNG exceptions.

## Controlled diagnosis and independent current replay

The parent's diagnosis in `/tmp/opencode/six-phase-probe` is archived under
`diagnosis/`: twelve raster sets, exact measurements, source bindings, the probe
driver/manifests, and the temporary renderer's PNG file and diff. All renderer
Rust sources other than `render/png.rs` were compared byte-for-byte to production.
That single temporary delta adds assertions for contiguous complete glyph-outline
partitions, valid fallback/bidi run ranges, finite origins, unchanged whole-device-Y
baseline and quarter-pixel X phases, plus a selectable former whole-line placement
branch. It does not alter the live renderer.

The preserved legacy mode exactly reproduces each old approved count and SSIM.
The current mode matches a fresh **unmodified release CLI** pixel-for-pixel for
all six cases. For each case, PNG and SVG artifacts and complete structured CLI
reports repeat in two fresh processes. Native SVG hashes equal the parent's
current and legacy hashes. Expected diagnostics and all declared options are
checked exactly. The actual build/profile/compiler/executable and current
producer sources are recorded in `native-build.json` and `native-producers.json`.

The standalone comparator uses the existing comparison crate without changes.
It independently recomputes all twelve old/new measurements from archived pixels,
checks the actual current release PNG against the diagnosis bytes, and verifies
all six prospective effective gates. It changes no global budget.

| Case | Old pixels / SSIM | New pixels / SSIM | Proposal |
| --- | --- | --- | --- |
| embeddable-links | 394 / 0.996641359605589 | 410 / 0.9967114218543928 | Exact 410 / `3fefe50f5a0d41c1` |
| emoji-fallback | 1187 / 0.8836183429943029 | 1184 / 0.8834829864365987 | Exact 1184 / `3fec457e1caa29ce` |
| bidi-text | 2353 / 0.8900035840752971 | 2375 / 0.8900222182569175 | Exact 2375 / `3fec7b0fe0040f08` |
| bidi-alignment | 1895 / 0.8357045887677239 | 1903 / 0.8363905899112435 | Exact 1903 / `3feac3b632cb3ae3` |
| text-line-endings | 15 / 0.9983753066942144 | 9 / 0.9990016606868246 | Withdraw; canonical pass |
| text-alignment-vertical | 70 / 0.9989756543412394 | 20 / 0.9993888979077394 | Withdraw; canonical pass |

All scopes remain `default/png/whole`, with no viewer profile. The four replacement
records use exactly the observed changed-pixel ceiling and SSIM floor, with no
margin. The two withdrawals do not introduce alternative exceptions.

## Non-defect causes and unchanged SVG qualification

- Embeddable labels retain the #105 glyph-coverage/Helvetica metadata and
  Liberation Sans light-autohinting/x-height cause.
- Emoji retains #121's monochrome Noto Emoji 3.003 versus reference Noto Color
  Emoji 2.047 shapes and advances, including displacement of following text.
- Bidi retains #152/#124/#125's deterministic basic-Latin question-mark fallback
  and different advances for unsupported scalars. Run order, outline ownership,
  physical alignment and paragraph dependencies are intact.
- The proposed reasons name the approved **per-glyph** rule: quarter-device-X
  origins with the existing integer-Y baseline for these axis-aligned scenes.
  They do not explain the current implementation as whole-line-only placement.

An isolated appended test called the **unchanged** `svg_divergences` implementation
for exactly these six staged cases, including its SVG geometry, raster and direct
PNG dependencies. All six passed. #168's separate bidi-alignment SVG harness
limitation remains **1190 changed pixels / `3fea564b9c97ab4f`**. The original
`RASTERIZER_LIMITATIONS` record is unchanged. The appended selector is archived
as `diagnosis/staged-six-svg-test.rs`; it exists only in the temporary copy and
was not proposed for application. See `svg-parity-check.log`.

## Normal case approval protocol

Each proposed manifest is canonical JSON with `approval: null` and its matching
normal `manifest.sha256` sidecar. The live-approved before files are archived.
Only the six exception fields and reset approvals differ; the central case
registry differs only in the same six exception declarations. Official artifact,
raster, scene, provenance, report expectations, options and other semantics are
unchanged and remain source-bound.

The unmodified staged xtask `conformance approval-subject CASE` verifies and
computes each prospective subject. The batch uses the repository's canonical
`excalidraw-render/conformance-approval-subject/v1` envelope, binding those six
subjects, native/diagnostic evidence, exact proposals/withdrawals, current source
bindings and the explicit migration plan. Python and the unchanged serde_json
canonical map serialization independently agree on its SHA-256. The batch is a
review wrapper, not a fabricated case approval or a new xtask approval endpoint.

## Explicit ownership/catalog migration

`migration-plan.json` and `preview/` are reviewed-plan outputs, not live updates.
The unmodified staged inventory/catalog derivation confirms:

- 1876 → 1874 obligations, retiring only the **unowned** whole-PNG scopes for
  `focused.text-line-endings.v1` and `focused.text-alignment-vertical.v1`.
- Every remaining criterion, identity, pointer and candidate list is preserved.
- 122 authority descriptors change: 118 case-label inventory authorities and
  the four remaining requalified exception authorities.
- 12 existing case-owner manifest descriptors need refresh; selections,
  commands, rationales, test sources and the representative owner are preserved.
- All three approved exclusions are byte-for-byte preserved, including their
  unchanged obligation subjects.
- Expected totals: **1803 assigned/excluded, 71 gaps, 1874 assignments**.

**Protocol constraint:** `CaseKey.inputs_sha256` includes the complete manifest,
including its approval metadata (`xtask/src/coverage.rs`, normal derivation).
Changing six manifests necessarily changes 18 full key digests—SVG, PNG and PDF
for each case—even though all 168 case/variant/export/format identities, options
and reference artifacts remain intact. The other 150 full execution records are
unchanged. Preserving all 168 keys literally is impossible under the unchanged
protocol. The batch review must explicitly authorize the enumerated input-binding
migration; it cannot be hidden as a digest refresh.

Final manifest hashes, inventory inputs, catalog hash and case-owner descriptor
hashes depend on the **future genuine case approval comments**. The pending
preview hashes are not final authorities. After approval, rederive those fields
using the same tools and enforce the semantic delta whitelist in the plan.

## Apply sequence after actual exact-batch approval

1. Revalidate this batch, its before bindings and the still-valid approved v7
   subject. Record the user's exact batch decision on #85. No approval is recorded
   by preparing this directory.
2. Record that same scoped user decision, naming the corresponding prospective
   subject, on each case's **registered approving issue**. The normal case verifier
   requires that issue's comment URL; a single #85 URL cannot replace these.
   Use the actual identity/date/comment URLs. Keep old cause-issue references and
   historical approvals in the batch archive.
3. Apply only the staged central-registry exception declarations, six manifests
   with null approvals and their sidecars. Check all six prospective subjects
   before recording the new case approvals.
4. For each case, use the unchanged normal command:
   `cargo run --locked -p xtask -- conformance record-approval CASE LOGIN YYYY-MM-DD ACTUAL_ISSUE_COMMENT_URL sha256:PROSPECTIVE_SUBJECT`.
   This computes the subject, checks its registered issue, writes only approval
   metadata and its digest sidecar, and verifies the approved case.
5. Derive the final inventory, catalog and ledger migration with actual approved
   manifest bytes. Retire exactly the two listed gaps; apply only the enumerated
   source/authority/owner/input-binding transitions. Preserve all other criteria,
   owners/exclusions and all 168 semantic execution identities. Do not copy the
   pending-preview catalog hash as final or preserve obsolete input digests.
6. Extend only excluded audit metadata for the approved migration; verify six
   cases, inventory, ownership, parity dependencies and unchanged v7 approval.
   Recheck actual post-approval subject consistency before broader testing.

No native baseline refresh/#178 or global ledger approval is granted. The fixed-size
moxcms allocation/resource gap remains explicit. No full suite or commit is part
of proposal preparation. If literal unchanged `inputs_sha256` values are required,
stop before application: that requirement needs a separate protocol decision,
which would violate the current bound-adapter freeze if implemented here.
