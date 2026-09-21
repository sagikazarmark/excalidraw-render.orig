# JPEG #188 ICC requalification infrastructure — awaiting parent freeze

No current-production capture, V10/bridge-v4 collection, pending final subject,
approval or live ledger/registry migration has been created. Parent owns production
and tests; this directory owns the unapproved evidence/checker only.

## Before state and OLD evidence

Base: commit `b01f2db`. `before/` and `basis.json` authenticate the original JPEG
producer/test, policies, ledger, inventory, registry, all Cargo inputs and the
existing #187 snapshot collector/public-facade helper. Frozen ICC reference:

```text
7ffd57eaa8ddfe4a7d311c5a55c57b84a001463864ce6bd235613faab85e76d5
```

A fresh detached `/tmp/opencode/jpeg188-old-b01f2db` builds/packages that exact OLD
source. `old/snapshots/` captures all 168 full keys, two sessions per key in fresh
forward/reverse processes. The normal snapshot verifier and authenticated existing
collector are reused without editing historical helper files. Old archive payloads
and source bindings identify b01f2db honestly; prior #187 output bytes are compared
but are never relabeled as fresh producer evidence. `old/prior-capture-comparison.json`
records the equality result. No baseline approval is granted.

The only bound xtask change is the bridge directory constant **v3→v4**, preserving
strict structure/viewer/producer validation and all historical bridges/approvals.

## Exact pending migration

Ledger before: **1992 assignments / 1896 owners / 3 exclusions / 1899 assigned or
excluded / 93 gaps**. The final reviewed transaction may change only:

- Sixteen exact `codec:jpeg:icc:*` gaps: twelve gray gamma-1 version/mode/layout
  cells and four opposite-model cells, all currently #188.
- Sixty-four existing ICC owner-source hashes, preserving both existing selectors,
  commands and rationales (36 color and 28 strict-recovery owners).
- One approved representative registry/owner binding V9→V10.

Projected counts: **1992 assignments / 1912 owners / 3 exclusions / 1915 assigned
or excluded / 77 gaps**. All 80 ICC policy criteria, other assignments, three
exclusions, catalog and 168 inventory keys are unchanged. Six #187 sampling cells
and 24 exact known-failure pins remain gaps/failure evidence; no new JPEG-wide,
ICC-wide, whole-PDF or CMS resource closure is implied.

Parent must finalize the new executable owner selector(s) before freeze. The
freeze includes an exact 16-ID→selector map. It rejects selectors still named
`unqualified`, `diagnostic` or `failure`, and missing/non-test functions. Do not
assign the current draft's old unqualified function as a passing owner. The
existing 64 selectors remain unchanged unless independently reviewed scope is
delegated; this packet will fail closed rather than silently rename them.

## Final freeze and commands — not authorized yet

Parent first completes production/test review, formatting and Clippy. Then supplies
an external JSON with `final_current_capture_authorized: true`, the actual
`parent_freeze_message`, and `new_owner_selectors` mapping exactly the sixteen
qualified #188 obligation IDs to their real final test function(s).

```sh
P=tests/conformance/proposals/jpeg188-requalification-v1
python3 "$P/workflow.py" freeze /tmp/opencode/jpeg188-parent-freeze.json
python3 "$P/workflow.py" stage /tmp/opencode/jpeg188-final-stage
python3 "$P/workflow.py" final /tmp/opencode/jpeg188-final-stage
python3 "$P/workflow.py" representative
python3 "$P/workflow.py" protected
python3 "$P/workflow.py" migration
```

The commands use pinned Rust 1.97.1 release, disabled incremental/debug profiles,
and exact existing packaged collector semantics. Final staging matches frozen live
bytes; output parents are created before collection. The collector hash-checks its
archived and original source before importing it, changes only its output-root
parameter and retains all ordinary snapshot checks. No historical script is edited.

Representative pipeline is fresh V10/bridge v4, native SVG/PNG/PDF repeated capture,
resource derivation, font audit, all six viewer pairs, exact existing three PDF
scopes, inspection/materialization and pending subject. Protected PDF is replayed
twice with exact artifact/report equality, qpdf and both repeated viewers. V10
artifacts/reports must equal approved V9; any movement stops for review without
changing a protected pin or tolerance.

The 160 ICC vectors include strict recovery controls: state **160 gates passed**,
not that rejected malformed inputs have visual parity. Parent evidence must bind
actual final test/producer bytes, complete ICC reconstruction/precedence/color/
report/recovery results, and the 76-vector #187 regression contract (64 canonical,
12 known-failure vectors/24 exact pins). The raw16 fixed-scratch/backward expansion,
RGB-profile-on-gray and ignored valid-gray-profile-on-three-component routes need
their focused causal checks in the parent's final source-bound evidence.

## Parent evidence and sealing

Provide `checks/parent-qualification.json` with exact `test_source`, full final V10
`producer_sources`, `icc_reference_sha256`, `icc_vectors:160`, `icc_gates_passed:true`,
`jpeg187_vectors:76`, `jpeg187_canonical_pass_vectors:64`,
`jpeg187_known_failure_vectors:12`, `jpeg187_exact_failure_pins:24`, and actual
passed focused `checks` with log descriptors. `diagnosed_inventory_changes` must
enumerate every changed old/current full key/sample pair, empty only if all 168
are equal. New differences require explicit diagnosis; equality is not assumed.
Keep expected pending ownership/representative failures separate from passed
checks; do not relabel them or invent a clean broad-suite result.

Only after this evidence exists run `workflow.py seal`. It emits a pending
production/qualification/migration subject binding all evidence and the separate
V10 subject. Read-only verification is `workflow.py verify`.

## Strict approval and parent-owned atomic transaction

Only after independent review and actual explicit user approval of both exact
subjects, designated maintainer **sagikazarmark** records an issue #85 comment
whose entire body is one standalone fenced JSON decision with exactly:

````text
```json
{
  "decision": "approve",
  "scope": "jpeg188-icc",
  "production_subject_sha256": "sha256:ACTUAL_FINAL_PRODUCTION_SUBJECT",
  "representative_subject_sha256": "sha256:ACTUAL_FINAL_V10_SUBJECT"
}
```
````

Reject rejection/hash-mention comments, other authors, duplicate/malformed/unknown
fields, multiple fences/prose and wrong subjects. Both saved and freshly fetched
GitHub comment authors must match exactly. This template is not an approval.

Parent owns the transaction. Before writing any live metadata, persist the actual
four-field production approval, full API comment, exact V10/registry/ledger preimages
and attempt status under `coverage/jpeg188-application-v1/`. Validate pending state,
authenticate the strong actual decision, then apply the exact three-file template
transaction, deriving only V10's manifest-file hash after adding real approval.
Run approved candidate/registry/strict-owner/inventory/ownership checks and the
affected binding tests. Mark `applied` only after success, naming both subjects.

On failure restore and byte-check all three preimages; retain partial receipts,
decision and error with `failed_rolled_back` or `rollback_failed`. Retry only after
verifying restored preimages and archiving the full failed attempt under a unique
name. Incomplete/failed rollback requires manual recovery; never erase evidence.
The checker intentionally has no live apply command. `workflow.py verify-applied`
requires persisted real approval/comment/status and authenticates them **before**
running current evidence and exact migration validation. V10 approval alone is
insufficient to authorize the ICC source/owner changes.

No dependency/spec/doc/reference changes are included. Native baseline/#178,
global ledger approval/closure, #187 sampling and pre-existing fixed-size CMS
allocation/resource work remain separate. No commit is part of preparation.
