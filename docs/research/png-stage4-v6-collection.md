# Stage 4 representative v6 — historical dev-profile collection

**Superseded by Spec2 review:** the successful measurements below were collected
with the dev native profile. §14.4 requires release qualification. Preserve v6
as historical evidence; do not request approval of its subject. The new release
and global-owner adapters are described in
[the release/owner checkpoint](archive/png-stage4-release-owner-adapter.md). A successor
capture awaits the parent's final production/spec hashes.

**Approval subject (canonical subject digest):**

```text
sha256:ed8d3128b2cc9f964055af6df971599c0b916d54568962f680d46e9b216e979b
```

Candidate: `tests/conformance/candidates/stage4-frames-text-assets-v6/`.
Case ID: `representative.stage4-frames-text-assets.v6`.
The subject includes exactly three proposed PDF exceptions and their diagnosis,
inputs, profile identities and producer chains. **No maintainer approval has
been recorded.** The manifest has `approval: null`; approved execution is blocked.

## Preserved authoring and failed run

V6 uses the same scene, request, reference inputs and crop plan as v4. The gate
checks all copied authoring/reference files byte-for-byte against v4. Scene and
crop-plan SHA-256 values are unchanged:

- Scene: `f9b7d01d634a222effe360a172ceec9183de72390a43d5fd73d9a39872094dc4`
- Plan: `73b8e2333b62ea403b05542b4ab8d363638cc6665928b6814e2fc0457d82b97f`

V5 was a fresh attempt. Its native capture, derived resources and text audit
succeeded; the collector retained both repeated Chromium pairs, then stopped
with `ModuleNotFoundError: No module named 'pypdfium2'`. The installation under
`target/pdf-harness/pdfium` was absent. V5 is marked retired/incomplete and its
observations were preserved without overwriting.

The already available PDFium wheel was verified against
`tools/pdf-harness/pdfium.lock.json`, SHA-256
`e10cbf41b21233ec5e20adfc170cf60edd77abead86a97dc708fff55a8a886c7`, then extracted
using the existing installer protocol. Installed versions were checked:
pypdfium2 5.12.1 / PDFium 152.0.7947.0, Python 3.12.3. All collection was restarted
in v6. No viewer implementation/profile or production renderer was changed.

## Fresh process and provenance gates

- Rust 1.97.1, Cargo 1.97.1, Linux x86-64, debug0, incremental disabled.
- SVG, PNG and PDF each rendered twice in fresh native processes; artifact bytes
  and full CLI reports repeat exactly. All reports match the independently
  authored silent compatibility-report contract.
- `native-capture.json` records 161 producer/dependency descriptors and the
  compiler/executable identity. Later inspection, materialization and subject
  verification each rechecked the bindings and fresh native replay.
- Resources were derived from the original SVG exports. Read-only verification
  reconstructed the resource plan, embedded image/WOFF2 bytes, SFNT metadata and
  bytes, font coverage and normalized pure-Rust SVGs successfully.
- The scoped Chromium text-audit qualification passed the positive custom-font
  run and rejected ambient-font and invalid-format controls.
- Six viewer/input pairs each repeated byte-for-byte across two fresh processes:
  official/native Chromium SVG, official/native pure-Rust SVG, native PDFium and
  native PDF.js. The collector checked producers/profiles/resources/inputs before
  and after collection. The gate checked the mandatory producer sets and existing
  approved viewer/comparator qualifications.

## Visual results

Whole image: 200×120. Typography crop: `(10,12,124,41)`.

| Comparison | Whole changed pixels | Whole SSIM | Typography changed pixels | Typography SSIM | Judgment |
| --- | ---: | ---: | ---: | ---: | --- |
| Chromium SVG | 0 | 1 | 0 | 1 | Canonical pass |
| Pure-Rust SVG | 0 | 1 | 0 | 1 | Canonical pass |
| Direct PNG / official canvas | 2 | 0.9995282750642132 | 2 | 0.9973446254064746 | Canonical pass |
| PDFium / official Chromium SVG | 118 | 0.9898474730435529 | 118 | 0.9413564483014669 | Exact proposed limits pass |
| PDF.js / official Chromium SVG | 16 | 0.998520923299333 | 16 | 0.9920016961872306 | Proposed whole limit; canonical typography pass |

**Every raster and nested-SVG crop is exact:** zero changed pixels, SSIM 1, for
all five comparisons. No global budget or crop was changed. The complete
canonical failures remain visible alongside the effective proposed-limit results
in `judgments.json`; no stored pass boolean substitutes for recomputation.

### Exactly three proposed exceptions

| Viewer / scope | Exact changed-pixel ceiling | Exact SSIM floor bits |
| --- | ---: | --- |
| PDFium / whole `(0,0,200,120)` | 118 | `3fefacd49b980278` |
| PDFium / typography `(10,12,124,41)` | 118 | `3fee1f978eeaac28` |
| PDF.js / whole `(0,0,200,120)` | 16 | `3feff3e226be27cf` |

These measurements match the diagnosed values exactly. PDF.js typography has
no exception. The PDF is byte-identical to the diagnosed v4 PDF. The proposal
binds the diagnosis package, its repository producer chain and recorded upstream
source identities, the case/default variant, PDF/viewer/profile, comparator,
region coordinates, exact no-margin limits, documented non-defect cause and
approving issue #85. Proposal digest:
`89b1e298c565eb29e4787854f34c889868d48178521d8602308085ffd6876f99`.

## Semantic assertions passed

`semantics.json` records:

- SVG: both URI links bind their actual content; frame clipping is used; all
  source raster octets are preserved.
- PDF: one page; exactly two image objects (source RGB and attached alpha mask);
  all 1,600 attached alpha samples and corresponding RGB samples match the
  authored source oracle; used nested vector form; selectable `Stage four` text;
  used color space bound to the output intent; original embedded scene values
  preserved.
- Both PDF links bind independently derived rectangles. Raster rectangle is
  `[135,30,150,60]`; rotated-text rectangle is
  `[8.182279349193683,50.712783867389795,99.81772065080632,80.53721613261021]`.

## Artifacts and evidence identities

| File | SHA-256 of stored file bytes |
| --- | --- |
| `native.svg` | `6d27f04f4dad351ca4330895d90a6ab68a5e05e2d6e39f2e21cccffc72171743` |
| `native.png` | `e6c206a45e03e4f1f634275d7537f096f53f2b22d990e39a820e47dd90757d93` |
| `native.pdf` | `95c399f0eaaee5bd0b2ae13a50a1a68171c0d5abaeab0b71bdf8816abda5817b` |
| `native-capture.json` | `d0eb1c201d590bec5946bac774715772b29e003a64d8fe021cc7df3c10c323b1` |
| `viewer-evidence/collection.json` | `24337a5a328c2064deaa384619d663eab8477b6a0929274c157b2b2505177c3c` |
| `chromium-text-profile.json` | `905d13199c660efceedbe2b3a3dc6f439e84511882ef25fc15e645264e992771` |
| `judgments.json` | `28a6f5e9b1963e6f2a650866d7c5d3c9281a563439f27214881bdcc5d0c2cc67` |
| `semantics.json` | `433cd0350b184331946ef0e74588f58955f97a22b587178334f09d03337b3a21` |

The approval subject digest is computed over canonical subject JSON, not the
stored file's trailing newline. Use the subject command below to verify it.

## Pending registry and refusal proof

`tests/conformance/candidates/registry.json` selects v6 with status
`pending-maintainer-approval`, `approval_required: true`, the exact three scope
records above, and these keys:

```text
representative.stage4-frames-text-assets.v6:default:svg
representative.stage4-frames-text-assets.v6:default:png
representative.stage4-frames-text-assets.v6:default:pdf
```

The registered consumer is `candidate-verify` for v6. Running
`candidate-executions` reached the approval check and exited nonzero with
`error: approval must be an object`, because the approval remains null. It emitted
no successful execution receipts. No case or exception approval was fabricated.
The original execution registry and baseline approvals were not edited.

## Parent independent review commands

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
D=tests/conformance/candidates/stage4-frames-text-assets-v6

# Fresh native replay, source/profile/derivation/semantic/visual checks:
cargo run --locked -p xtask -- conformance candidate-inspect "$D"
cargo run --locked -p xtask -- conformance candidate-approval-subject "$D"

# Independent re-observation of font controls and every viewer pair:
node tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs "$D" verify
node tools/reference-oracle/scripts/stage4-candidate-views.mjs verify "$D"

# Must reject until genuine bound maintainer approval is recorded:
cargo run --locked -p xtask -- conformance candidate-executions
```

Do not rerun `collect`, `candidate-native-capture`, or materialization into v6.
Further producer/proof/input changes invalidate this source-bound subject and
require a new revision/requalification. This unit ran collection, inspection,
materialization, subject verification, refusal checking and `git diff --check`;
it did not run the full workspace or record approval/commit anything.

## Explicitly separate remaining global gaps

This is representative-case readiness, not global Stage 4 closure. In particular:

- Owned ICC parsers/LUT variable allocations and their typed failure tests have
  been verified separately. **Pre-existing fixed-size moxcms execution-table
  allocations remain an explicit global PNG resource/limits gap. This audit
  does not claim end-to-end CMS OOM closure.**
- Existing inventory-bound baseline/approval staleness and protected PDF backend
  digest requalification remain separate. No goldens or reference approvals were
  repinned in this collection unit.
- Parent independent review and the parent's full-workspace run remain required
  before the maintainer is asked to approve this exact subject.
