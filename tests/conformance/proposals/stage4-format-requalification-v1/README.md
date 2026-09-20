# Pending formatting/source requalification — exact review subjects

No new approval or live ownership migration has been recorded.

**Formatting/migration subject:**

```text
sha256:4befa87e77a521aae20223b78fa6da20bb3a38d8555800f844e9de5c4d3f4e52
```

**Representative:** `representative.stage4-frames-text-assets.v8`

**V8 subject:**

```text
sha256:aaa772430f342a60807ef97255bf2296041cbcf23dff4c2aa4284052deb92454
```

V8 directory: `tests/conformance/candidates/stage4-frames-text-assets-v8/`.
Its manifest approval remains null. The formatting subject binds the completed
V8 subject, exact source deltas, equality evidence and pending metadata migration.
Python and the existing Rust/serde_json canonical-hash checker agree on the
formatting subject. This README is excluded explanatory metadata.

## Completed checks

- Six exact pre-format sources archived and authenticated; all six applied files
  equal pinned rustfmt stdout byte-for-byte.
- External typography-test preimage reconstructed and authenticated as
  `93c4d040619c9f9b042caee5b7be3de5ecf7910791877b2a5549beb22cb5dc86`.
  Its parent-applied pinned-libm fix is separately classified as test-only.
- Workspace `cargo fmt --all --check` and
  `cargo clippy --locked --workspace --all-targets -- -D warnings` passed before
  the source/tooling freeze.
- Bounded tests: PNG Stage-4 25, interactions 4, dark-mode/frame/image 10,
  external typography 6, protected PDF 1, candidate binding/approval negatives 18
  — **64 passed**. No full-workspace test suite ran.
- Strict bridge v2, native release capture/replay, resource derivation, font audit,
  all six fresh viewer pairs, exact exception preparation, inspection and
  materialization passed.
- **16 artifact/report equality comparisons, 32 fresh release processes**:
  representative SVG/PNG/PDF, protected PDF, six-case SVG/PNG all exactly equal
  their approved pre-format captures. Protected qpdf and both repeated PDF
  viewers pass; V8's viewer rasters exactly equal V7's.
- All six normal focused-case approvals and the inventory remain valid.
- Historical preservation audit passed in authenticated before-source context.
- Current v8 approved execution correctly rejects null approval. Current v7,
  protected-PDF historical producer verification and the live ledger correctly
  reject stale source bindings; those expected failures are retained in `checks/`.

## Release proof and unchanged artifacts

Actual build: `cargo build --release --locked -p excalidraw-render-cli --message-format=json-render-diagnostics`.
Rust 1.97.1 Linux x86_64; opt-level 3, no debuginfo/assertions/overflow checks,
LTO, one codegen unit, panic abort, no incremental compilation or Rust flags.
The CLI executable itself also remains byte-identical:
`d4252b1a73c41382f28392bfeff0ee3d37b549662099bf32aba404eeedc57869`.

| Artifact | Unchanged SHA-256 |
| --- | --- |
| Representative SVG | `6d27f04f4dad351ca4330895d90a6ab68a5e05e2d6e39f2e21cccffc72171743` |
| Representative PNG | `e6c206a45e03e4f1f634275d7537f096f53f2b22d990e39a820e47dd90757d93` |
| Representative PDF | `310ca74edc3ea7e69fc549739c45e05d90d7522704be1d94bb9f553084658265` |
| Protected PDF | `50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc` |

All five V8 effective comparison gates and semantic checks pass. SVG comparisons
and both nontext crops remain exact; PNG whole/typography remains two changed
pixels. The three proposed PDF scopes remain exactly 118 / `3fefacd49b980278`,
118 / `3fee1f978eeaac28` and 16 / `3feff3e226be27cf`. PDF.js typography retains its
canonical pass without an exception. The fixed scene/request/crops are unchanged.

## Exact change inventory

Existing files edited in this session:

1. `crates/excalidraw-render/src/lib.rs`
2. `crates/excalidraw-render/src/render/asset/color.rs`
3. `crates/excalidraw-render/src/render/png.rs`
4. `crates/excalidraw-render/tests/fast/dark_mode_frame_and_image_render.rs`
5. `crates/excalidraw-render/tests/fast/png_stage4_contract.rs`
6. `crates/excalidraw-render/tests/fast/stage4_interaction_contract.rs`
7. `xtask/src/conformance_pdf_bridge.rs` — only the v1→v2 directory constant.

The parent's already-applied `tests/stage4_typography.rs` change was archived,
authenticated and tested, not edited again here.

New artifact roots:

- This packet. `manifest.json.files` is its exact file inventory (plus the
  manifest, approval-subject envelope and this excluded README).
- `tests/conformance/candidates/stage4-frames-text-assets-v8/`.
- `tests/conformance/candidates/stage4-pdf-colorspace-bridge-v2/`.

`new-evidence-bindings.json` enumerates every captured V8/bridge-v2 file; README
prose is excluded. V7, bridge v1, protected PDF and six-case approved packets,
all six case approvals, the live registry and ledger remain unchanged.

## Recheck and approval/application

```sh
P=tests/conformance/proposals/stage4-format-requalification-v1
python3 "$P/requalify.py" verify
```

The prospective migration proves exactly 57 source-binding refreshes and one
representative replacement. It preserves all criteria, other dispositions,
three exclusions, catalog and all 168 full inventory keys. Expected counts stay
1,874 assignments / 1,803 assigned-excluded / 71 gaps. Only the final approved
V8 manifest-file digest is deferred until the actual approval metadata exists.

After independent review, obtain explicit user approval of **both subjects** and
record that genuine decision on #85, naming both complete `sha256:` values.
Then follow [PLAN.md](PLAN.md#actual-approval-and-later-application): provide an
actual four-field formatting-subject approval record outside this packet and run
the guarded metadata-only `apply` command. It checks the real comment, records
V8's approval, derives the final manifest binding and validates real approved
execution/ownership, with rollback on failure. No new source edits or capture
are needed for that transaction.

Until then the live ledger is honestly stale and V8 emits no eligible receipts.
Baseline/#178, global closure and fixed-size CMS allocation/resource work remain
separate. No approval comment, commit or broad suite was performed here.
