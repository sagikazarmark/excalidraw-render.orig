# Protected PDF scoped approval applied — 2026-09-20

## Decision and applied scope

The user's explicit **“Approve scoped update”** decision was recorded by
`sagikazarmark` on 2026-09-20 in
[GitHub issue #85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5748855374).
The comment records the user's session decision, not an autonomous agent
approval. The authenticated API response is archived as `user-approval-comment.json`.

Approved subject, unchanged after application:

```text
sha256:c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389
```

Evidence directory: `tests/conformance/candidates/stage4-pdf-backend-requalification-v2/`.
The applied changes are exactly:

| Pin | Before | After |
| --- | --- | --- |
| Native protected PDF | `b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43` | `50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc` |
| Operation sequence | `678bb667b05d6ec652516f202d4e9b17e7960fb0f52b8f030b93de62e201fb6e` | `ebf933b7a43e6d53730578e44562484e2aa04eb4cc0d47af7c876625e054de16` |
| Structural inventory | `e33789145c274e9363302378e4d76b2bc3aed2643b1c456a758abf6389ad6b64` | `a993befecf6da05bbf204d240bc3242b045a9eefd7460348811e1dfe3e83159b` |
| Expectation-file binding | `b6cbc6049556ae87780a83e9c252f64193594bf37c9d6a2f18a80434b31cb7b9` | `442f0cb2580a4066d64bcb924eaa6e070252faca2fa2b37fa4f34c81ff600b1b` |

The structural expectation has only the two approved digest substitutions.
`crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs` has only the
native PDF and expectation-file digest substitutions; its assertions and test
logic are otherwise byte-identical to the archived/base source.

The same comment separately records approval of draft PNG §8.5 and its assessment.
It grants no representative exception, native baseline refresh or global
ownership/closure approval.

## Immutable history and current verifier

`applied-transition.json` SHA-256:
`d38af8a6c14d20cf1149d0d1f6f157f8b64020b586003b9b5fae40e26abc6266`.
It binds the archived expectation, test and verifier, their current counterparts,
and the actual approval/comment snapshot. Original captured evidence and
`approval-subject.json` remain unchanged. The subject binds the original verifier;
the transition record separately binds the current history-aware implementation.

| Source | Current SHA-256 |
| --- | --- |
| `xtask/src/conformance_pdf_requalification.rs` | `c7ff711cef85fce00336a90270f5fbace6c0c06ac32d55659e7dd163d30bd93b` |
| `crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs` | `cf5483a364d58d1e060111741d06f4fbec4630b4446b8c5782ef27ea4833d726` |
| `tools/pdf-harness/diagnostics/stage4-pdf-approved-transition.py` | `c9f3d8d28826dde7171706f0bb05e4e54825e425610e0adc4a7338f4da98e78d` |
| `tools/reference-oracle/scripts/verify-png-stage4.mjs` | `fa931ebf8af46378efa609b401396a171c76dc94714bf18834bfe7c276a879d1` |

The read-only preservation audit now permits the protected expectation change
only after checking the exact approved subject, pinned comment snapshot,
before/after descriptors, base bytes, exact two structural substitutions and
exact two test substitutions. All other prior protected paths remain subject to
the base-diff check. Transition tests reject unrelated expectation/test changes,
missing/wrong-subject approval and a different recorded approver.

## Focused verification

Passed with Rust 1.97.1 and debug information/incremental compilation disabled:

```sh
cargo test --locked -p xtask --bin xtask approved_pin_history -- --nocapture
cargo test --locked --release -p excalidraw-render --test fast \
  pdf_backend_qualification::complete_project_operation_matrix_is_deterministic_and_vector_first -- --exact
cargo clippy --locked -p xtask --all-targets -- -D warnings
cargo fmt -p xtask -- --check
cargo run --locked -p xtask -- conformance pdf-requalification-subject \
  tests/conformance/candidates/stage4-pdf-backend-requalification-v2
cargo run --locked -p xtask -- conformance verify-inventory
cargo run --locked -p xtask -- conformance verify-ownership
node tools/reference-oracle/scripts/verify-png-stage4.mjs
git diff --check
```

Subject verification replayed the release native PDF and complete CLI report
twice, and returned the original approved `c15e6bab…` subject.

Fresh pinned qpdf 12.3.2 checks in
`target/stage4-approved-pdf-transition-verification/` established:

- Original PDF + archived original expectation: pass.
- Current PDF + applied expectation: pass.
- Current PDF + archived original expectation: expected failure at exactly
  `operation-sequence-sha256` and `structural-inventory-sha256`.

Both original and current PDFs were rendered twice in fresh processes by each
pinned viewer at 520×200. All eight outputs exactly equal the corresponding
retained old/current viewer rasters (zero changed pixels):

| Viewer | RGBA SHA-256 |
| --- | --- |
| PDFium | `8b6b02bc587434fc22775bd5749efc2bc73e6777ece8b4ebd69916ea357847bb` |
| PDF.js | `99c1915e82eb51b925990396c0aedbaff22347a4901429a93223bfb9a780c488` |

The local replay `checks.json` SHA-256 is
`e738a82c5a9e148a1df37101562c68c3b318ec88a6d2bdc999b025286de5a1be`.
Original retained captures were not overwritten.

Live binding audit: no stale requirement locators or owner sources. Inventory
retains all 168 execution records and 320 coverage rows. Ownership retains
1,876 assignments, 1,802 assigned/excluded and 74 gaps; all three exclusions and
the frozen PNG oracle records are preserved. No further binding refresh was
needed for these two protected files.

## Source checkpoint for the next release capture

The production/spec identities were checked after application:

| File | SHA-256 |
| --- | --- |
| `docs/renderer-spec-v1.md` | `628acb6d7cf2e2756b1d0bacbab3aab9ee73b548df5252057747f7ea17270243` |
| `docs/research/stage4-png-device-phase-assessment.md` | `286a8aaf93d0050608baac9cb86c074fbd513bf20fdc616dc034a5db1cd24e40` |
| `crates/excalidraw-render/src/render/pdf.rs` | `139518855e22412fdfef206134d147a55e6b27f84a7fb3fa8df09d9c14d100eb` |
| `crates/excalidraw-render/src/render/png.rs` | `a089e88fb04f58f40ab186ead55a973048e5099995ad13a653be15006d0d2ade` |
| `crates/excalidraw-render/src/render/png/paint.rs` | `c7d105d9dad483742df6e586fcd8fbee718d51fc95bce0144b669e8a8b9689ec` |
| `crates/excalidraw-render/src/render/text.rs` | `f8f36b8377a9ce437c1734099e6118941e33081998463d1035361e26addc24f7` |

The bounded approval/application unit is complete. These producer/consumer
sources are ready for the next fresh **release** representative capture. V6
remains historical dev-profile evidence; `interaction:frames-text-assets` remains
a gap until a new candidate and its exact exception scopes receive genuine
approval. The existing colour-space bridge and typography diagnosis retain
their original identities and limits.

Native baseline approval is still stale at the inventory source binding; global
ledger approval and closure remain pending. The pre-existing fixed-size moxcms
execution-table allocation/resource gap remains explicit. No full-workspace
suite or commit was performed in this unit.
