# Stage 4 bounded tooling checkpoint: release qualification and ownership

No candidate capture, production edit, ownership assignment, baseline refresh,
GitHub approval, or commit is performed by this unit. V6 is preserved as correct
historical **dev-profile** evidence; its previous readiness claim is superseded
by the §14.4 release requirement. Final capture waits for the parent's confirmed
production and spec hashes.

## Release native qualification

`conformance_candidate_native` now constructs exactly:

```text
cargo build --release --locked -p excalidraw-render-cli --message-format=json-render-diagnostics
```

It fixes the repository release settings (optimization 3, debug0, no incremental
compilation, no debug assertions/overflow checks, one codegen unit, LTO, abort
panic strategy, no extra rustflags/wrappers), checks Rust 1.97.1 Linux x86-64, and
records Cargo's actual compiler-artifact profile with the executable identity
and exact build command. Replay validates this record **before building**; dev
captures cannot be relabeled or reused. Profile/command/flag mutations fail.

A tiny isolated Cargo probe confirmed that pinned Cargo emits the expected
release profile object: `opt_level: "3"`, `debuginfo: 0`,
`debug_assertions: false`, `overflow_checks: false`, `test: false`. This probe
was not a renderer or representative capture.

Both native producer bindings and the candidate's adapter-source bundle now
include `docs/renderer-spec-v1.md`. All xtask source modules, including the new
owner and authority-migration modules, are included before the next subject is
frozen. Further production/spec/adapter edits invalidate the corresponding
capture or approval subject.

## Narrow global ownership adapter

The new `Owner::Representative { binding }` variant is accepted only for
`interaction:frames-text-assets`, category `interaction`. It binds:

- The exact registered representative case and canonical candidate directory.
- The complete supplemental registry descriptor and approved manifest descriptor.
- The canonical case approval-subject digest.
- The exact command
  `cargo run --release --locked -p xtask -- conformance candidate-verify DIRECTORY`.

The registry must declare its precise SVG/PNG/PDF execution keys, coverage,
exception records and approval requirement. The adapter verifies the real case
approval before dispatching directly to the checked candidate consumer; it never
executes an arbitrary command from the ledger. That consumer checks current
producers, native release capture/replay, reports, independent semantics, viewer
judgments and exception/proof bindings. Three complete, subject-bound execution
receipts are required. Missing/duplicate/unknown receipts and dead consumers fail.

Seven critical adapter source files must be present and current in the case's
source bundle. Unit checks also verify that the normal bundle builder includes
them. Ordinary case-label and renderer fast/lib adapters retain their original
scope and executable restrictions.

New commands:

```sh
cargo run --release --locked -p xtask -- conformance verify-representative-owner CASE_ID
# Only after genuine case approval and reviewed catalog/ledger reconciliation:
cargo run --release --locked -p xtask -- conformance record-representative-owner CASE_ID "reviewed rationale"
```

Recording permits only that one still-open interaction obligation, validates the
approved consumer and the complete ledger, and refuses to mutate an approved
ledger. The same adapter is used during ownership and closure validation.
**The interaction remains a gap.** No assignment was made because approval is
still null. The pending registry retains v6 with an explicit historical-dev
status and a release-qualified consumer command; it grants no eligibility.

## Explicit authority-binding migration proposal

The approved draft §8.5 amendment changes the spec's byte identity. It does not
change the ownership policy's criteria, IDs, pointers, candidates or dispositions.
The new read-only planner is:

```sh
cargo run --locked -p xtask -- conformance ownership-authority-migration \
  629930b CONFIRMED_NEW_SPEC_SHA256 \
  docs/research/stage4-png-device-phase-assessment.md
```

It resolves the explicit old commit, confines the spec change to the reviewed
PNG text-placement paragraph, reconstructs the old catalog using the stored
inventory, and requires its digest to equal the ledger's existing catalog
binding. Normalizing **only** the spec authority descriptors must make the new
catalog exactly equal to the old one. The exact assignment set and all existing
exclusion proofs/approval subjects are preserved. Unexpected criteria/source
changes, unreviewed spec hashes, a changed ledger/review note, or an already
approved ledger fail. There is no apply/digest-override operation.

Planning against the authenticated stored inventory is diagnostic only. Its
live validation status is explicit in the output. Ordinary catalog, assignment
and closure paths continue to require live inventory validation; this planner
does not weaken them or refresh stale evidence.

### Observed preview, not a parent-finalized migration

The initially supplied spec hash `e687578d…` stopped matching while the typography
agent clarified the font-cache axis-classification rule. The planner correctly
refused that obsolete expected digest. A read-only preview of the subsequent
observed bytes derived:

| Binding | Digest |
| --- | --- |
| Old spec, commit `629930b` | `fb1ef02d6cd07385d94b82d97f56d40a8a8ac9f94f98b0969ac3f10a7539830f` |
| Observed new spec, **awaiting parent confirmation** | `628acb6d7cf2e2756b1d0bacbab3aab9ee73b548df5252057747f7ea17270243` |
| Existing catalog | `fb90d519e1ddb2a21b40357df48ab47f02efdce02403705f8890e9e1e7ef5001` |
| Spec-only proposed catalog | `1f6cc7cd79145cb47b928283356d2a941a9e36efde5f26d68fb8e299cbd6b7d8` |
| Unmodified ledger file | `6882c37db5a89ebcccce0ee0b0a5bf399cbcc891cb20a10e19f3bfea1e0d7788` |
| Unmodified assignment records | `96fdc9ef7240d42fb6ea3a5181dc4062de81f191af85f90498eeb19769b9cf2d` |

Exactly 1,007 obligation authority descriptors change. All 1,876 assignment
records and all three approved exclusion subjects remain unchanged. The output
is `review-required-not-applied`, with approval null. It also reports the real
live blocker: **`stale evidence anchor for fuzz.codecs`**. That requirement binds
`crates/excalidraw-render/tests/fast/static_png_asset_render.rs`; the earlier PDF
matrix assertion correction changed its source digest. No evidence pin was
silently updated here.

These preview hashes are not instructions to overwrite `catalog_sha256`. If
requirements/inventory bindings are reviewed and updated, their additional
authority changes need a separately reviewed migration and a newly authenticated
before/after derivation. Preserve the original execution semantics, exclusions
and existing baseline approvals throughout; stale baseline/#178 work remains
separate.

## Focused verification and next steps

Use pinned Rust 1.97.1 and debug0 for these tooling tests:

```sh
cargo test --locked -p xtask --bin xtask release_qualification -- --nocapture
cargo test --locked -p xtask --bin xtask representative_owner -- --nocapture
cargo test --locked -p xtask --bin xtask authority_migration -- --nocapture
cargo test --locked -p xtask --test representative_ownership -- --nocapture
cargo clippy --locked -p xtask --all-targets -- -D warnings
```

All focused checks passed: one release-identity test, three representative-owner
tests, two authority-migration tests, four CLI tests, and two existing ordinary
adapter scope/execution guards (12 tests total). Clippy and `git diff --check`
passed. The ledger still hashes to
`6882c37db5a89ebcccce0ee0b0a5bf399cbcc891cb20a10e19f3bfea1e0d7788`;
`interaction:frames-text-assets` was not assigned. No full suite or representative
capture was run.

The final capture/rebind order is:

1. Wait for the parent's final production/spec hashes and review the authority
   proposal against that exact revision. Resolve stale live bindings through
   explicit reviewed steps; do not use a digest override.
2. Freeze the adapter sources, then create a fresh successor (normally v7) with
   unchanged v4 authoring/request/crops. Run native release capture, resource/text
   qualification, repeated viewers, exact exception proposal checks and fresh
   replay. Unexpected artifact/proof/measurement changes require review.
3. Materialize a new subject. V6's historical subject is not reusable. Parent
   independently reviews and requests actual maintainer approval of the successor.
4. Only after that approval and valid ledger/catalog bindings, verify and record
   the narrow representative owner. Closure still checks all remaining gaps and
   global approval requirements.

The pre-existing fixed-size moxcms execution-table allocation/resource gap remains
explicit. Owned parser/LUT variable-allocation checks do not imply end-to-end CMS
OOM closure, and this interaction owner cannot claim any codec/resource obligation.
