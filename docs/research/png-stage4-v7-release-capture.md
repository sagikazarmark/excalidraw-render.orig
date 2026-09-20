# Final release representative capture v7 — 2026-09-20

Subsequent status: the user approved this exact subject and its three exception
scopes; the approved release consumers passed and the strict owner was recorded.
See the [approval/owner audit](png-stage4-v7-approved-owner.md). The sections below
preserve the original pending-capture checkpoint and post-decision procedure.

## Review identity and status

- Case: `representative.stage4-frames-text-assets.v7`.
- Directory: `tests/conformance/candidates/stage4-frames-text-assets-v7/`.
- Status: ready for independent parent review and final user approval.
- `manifest.json` approval remains **null**; eligible executions remain blocked.
- Final **canonical approval-subject** digest:

```text
sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05
```

The serialized `approval-subject.json` file includes a trailing newline; its file
SHA-256 is `e7ec13c4a21c41e532cdd9964acc059c5a0a9249ce3f15974e3fef9239d6ec86`.
Use the **canonical subject above**, not the file hash, when approving.
Pending manifest file SHA-256:
`0d815849fe5937ab1a455e25244d12009ead2d5cff23b19d93804c6f4131e734`.

## Release producer and repeat proof

The actual recorded build command is:

```sh
cargo build --release --locked -p excalidraw-render-cli --message-format=json-render-diagnostics
```

Rust `1.97.1 (8bab26f4f 2026-07-14)`, Cargo `1.97.1 (c980f4866 2026-06-30)`,
host `x86_64-unknown-linux-gnu`, LLVM 22.1.6. Actual Cargo profile: opt-level 3,
debuginfo 0, debug assertions false, overflow checks false, test false. Bound
settings: LTO true, one codegen unit, panic abort, incremental false, no Rust flags.
CLI executable SHA-256:
`d4252b1a73c41382f28392bfeff0ee3d37b549662099bf32aba404eeedc57869`.

`native-capture.json` SHA-256:
`2375894a71caf6a278cefe04a79a3818effac056ed855084ceccffe74f24ed5e`.
Capture and verification run each format in two fresh processes, comparing exact
artifact bytes and complete structured CLI reports. All reports indicate success
with empty diagnostics and the fixed revision/profile contract.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| SVG | 15,314 | `6d27f04f4dad351ca4330895d90a6ab68a5e05e2d6e39f2e21cccffc72171743` |
| PNG | 5,598 | `e6c206a45e03e4f1f634275d7537f096f53f2b22d990e39a820e47dd90757d93` |
| PDF | 42,939 | `310ca74edc3ea7e69fc549739c45e05d90d7522704be1d94bb9f553084658265` |

## Fixed inputs, resources and viewers

Scene SHA-256 remains `f9b7d01d634a222effe360a172ceec9183de72390a43d5fd73d9a39872094dc4`.
Judgment-plan SHA-256 remains `73b8e2333b62ea403b05542b4ab8d363638cc6665928b6814e2fc0457d82b97f`.
All frozen v4 authoring/reference/report-expectation bytes are checked for exact
equality. The native request is unchanged:

```text
--compatibility-revision 1 --padding 0 --no-background --no-frame-names
--no-frame-outlines --embed-scene --embed-provenance --embed-diagnostics
```

Whole image is 200×120. Fixed crop bounds `[x,y,width,height]`:
typography `[10,12,124,41]`, raster `[178,38,22,44]`, nested SVG `[38,53,36,28]`.
Global comparator profiles and budgets are unchanged.

Read-only resource verification reconstructs the original WOFF2/images, SFNT
derivatives, coverage declarations and pure-Rust SVG inputs. Chromium's font
audit confirms custom fonts and rejects ambient-font and invalid-format controls.
All six pairs were freshly collected twice; every pair repeats byte-for-byte:

| Capture | RGBA SHA-256 |
| --- | --- |
| Official and native Chromium (each two runs) | `100d99704264561daea15731b1809b5f1a3ea52b700235957faa2c22d7bb6f27` |
| Official and native pure-Rust SVG (each two runs) | `538f1d3a06da630ee3b47d9be4cea4cb0e869259b6950fee2376528b39c529e6` |
| Native PDFium (two runs) | `e871292ec41aea3d358e3896fbed30421708e0ff1f677136a29f3d18c49c3685` |
| Native PDF.js (two runs) | `2f4f36ed90ef5ac440b6713a57ef7cb7b0d4fdc8ee7149fa9e00376912b71845` |

## All visual and semantic gates

Each table cell is **changed pixels / SSIM**. Both nontext crops are exact in
every comparison. All five effective comparison gates pass.

| Comparison | Whole | Typography | Raster | Nested SVG | Canonical result |
| --- | --- | --- | --- | --- | --- |
| SVG / Chromium | 0 / 1 | 0 / 1 | 0 / 1 | 0 / 1 | Pass |
| SVG / pure Rust | 0 / 1 | 0 / 1 | 0 / 1 | 0 / 1 | Pass |
| PDF / PDFium | 118 / 0.9898474730435529 | 118 / 0.9413564483014669 | 0 / 1 | 0 / 1 | Whole + typography need proposed exceptions |
| PDF / PDF.js | 16 / 0.9985209232993330 | 16 / 0.9920016961872306 | 0 / 1 | 0 / 1 | Whole needs proposed exception |
| PNG / canvas | 2 / 0.9995282750642132 | 2 / 0.9973446254064746 | 0 / 1 | 0 / 1 | Pass |

The proposal contains exactly three no-margin scopes, authorized for preparation
but **not yet approved**:

| Viewer/region | Maximum changed pixels | Minimum SSIM bits |
| --- | ---: | --- |
| PDFium whole | 118 | `3fefacd49b980278` |
| PDFium typography | 118 | `3fee1f978eeaac28` |
| PDF.js whole | 16 | `3feff3e226be27cf` |

PDF.js typography canonically passes (`3fefbe7a57566bff`), with no exception.
PNG whole/typography bits are `3feffc22b89a7fb2` / `3fefea3f469eca2c`.
`exception-proposal.json` file SHA-256:
`573edb679a54790786f5c80ea027ab4d75d6245874b363eb84a4ead782de4e1e`.

The proposal binds current PDF `310ca74e…` to the original diagnosed PDF
`95c399f0…` through the exact validated colour-space bridge, file SHA-256
`9d97c210dc63a0a5ffda4431b9bd7887243f53dde0befa528aaa37384ecab286`.
All raw streams, font programs, text coordinates and other objects are identical;
only image object 18's direct ICC reference and derived IDs/xref offsets differ.
Both viewer rasters and all scoped measurements match the diagnosis exactly.

Semantic checks pass:

- SVG: both links bound, frame clip used, source raster octets preserved.
- PDF: one page, two image objects, nested vector form, embedded scene preserved,
  bound OutputIntent/direct ICC image association, text `Stage four`, 1,600
  attached alpha samples.
- PDF raster link rectangle: `[135,30,150,60]`.
- PDF text link rectangle:
  `[8.182279349193683,50.712783867389795,99.81772065080632,80.53721613261021]`.

Judgments file SHA-256: `30d21663ffc3f8ee90912b97bc4599439c96aa83cb728221d723a70a66600233`.
Semantics file SHA-256: `433cd0350b184331946ef0e74588f58955f97a22b587178334f09d03337b3a21`.

## Frozen bindings and approval boundary

The manifest binds 51 evidence files and 31 adapter sources. Native capture binds
162 production/spec/assets sources; collection binds all 19 viewer producers;
the text profile binds all eight font-audit producers. Materialization and final
subject verification recompute descriptors, resource derivation, visual metrics,
semantics, the bridge and native release replay; no stale descriptors were found.

A before/after SHA-256 audit checked all 2,097 pre-existing repository files.
The only changed pre-existing file is the supplemental registry. Production,
adapters, approved PDF transition, spec, v4/v6 records, profiles, original 168
executions and ownership ledger are unchanged. The registry now declares:

```text
representative.stage4-frames-text-assets.v7:default:svg
representative.stage4-frames-text-assets.v7:default:png
representative.stage4-frames-text-assets.v7:default:pdf
```

Registry SHA-256: `2590fa1fb437444b603353fe9589aacc10827a5fe5d37e9fb61155d5c9028a2e`.
Its exact consumer is the release `candidate-verify` command below. Direct
candidate execution, registry execution and strict owner verification each fail
with **`approval must be an object`**, emit no stdout/execution receipts and leave
`interaction:frames-text-assets` a gap. Inspection returns
`ready_for_approval_check`, `eligible_executions: []`, and no inspection approval
subject; materialization separately produces the final subject above.

No source/tooling changes are needed before approval. Keep this bundle and its
producer/adapter inputs frozen. The manifest's `approval` field is intentionally
excluded from the canonical approval subject. Subsequent ownership-ledger
metadata is outside the candidate subject; the owner adapter binds the approved
manifest and current registry when recording ownership. README prose is excluded
from candidate evidence. Do not add a new approval file inside the candidate or
edit exception status, profiles, evidence or source bindings to record approval.

## Focused checks

Passed: 18 candidate unit tests (including source/build/report/resource/semantic,
exception-proof and approval mutations), one registry integration test and four
ownership/authority CLI tests. Also passed: release subject verification,
inventory/ownership validation, Stage-4 preservation audit, live locator/source
audit and `git diff --check`. No stale live owner sources or requirement locators.
Capture/inspection logs are in `target/stage4-v7-capture-logs/`.

No full-workspace suite or commit ran. The remaining candidate blocker is final
independent review and genuine user approval. Native baseline refresh, global
ledger approval/closure, 74 ownership gaps and the pre-existing fixed-size moxcms
resource/OOM gap remain separate from this candidate decision.

## Parent review and post-decision recording

Use the pinned toolchain for all commands:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0
D=tests/conformance/candidates/stage4-frames-text-assets-v7
cargo run --release --locked -p xtask -- conformance candidate-inspect "$D"
cargo run --release --locked -p xtask -- conformance candidate-approval-subject "$D"
```

After independent review and the user's actual final approval, record that
decision in a genuine issue #85 comment naming this exact case, canonical subject
and all three exception scopes. The earlier protected-PDF approval is not this
approval. Set **only** `manifest.json.approval` to an object with exactly:

```json
{
  "approved_by": "<actual approver GitHub login>",
  "approved_at": "<actual YYYY-MM-DD>",
  "approval_url": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-<actual numeric ID>",
  "approval_subject_sha256": "sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05"
}
```

There is no dedicated supplemental-candidate approval-writing CLI. The following
recording command is for the actual approved comment, **not to run while pending**:

```sh
APPROVAL_COMMENT_ID=<actual-numeric-comment-id> python3 - <<'PY'
import json, os, subprocess
from pathlib import Path
subject = 'c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05'
comment_id = os.environ['APPROVAL_COMMENT_ID']
assert comment_id.isdecimal()
comment = json.loads(subprocess.check_output([
    'gh', 'api', f'repos/sagikazarmark/excalidraw-render.orig/issues/comments/{comment_id}'
]))
assert comment['html_url'] == f'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-{comment_id}'
assert subject in comment['body']
path = Path('tests/conformance/candidates/stage4-frames-text-assets-v7/manifest.json')
manifest = json.loads(path.read_bytes())
assert manifest['approval'] is None
manifest['approval'] = {
    'approved_by': comment['user']['login'],
    'approved_at': comment['created_at'][:10],
    'approval_url': comment['html_url'],
    'approval_subject_sha256': f'sha256:{subject}',
}
path.write_text(json.dumps(manifest, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n')
PY
cargo run --release --locked -p xtask -- conformance candidate-approval-subject "$D"
cargo run --release --locked -p xtask -- conformance candidate-verify "$D"
cargo run --release --locked -p xtask -- conformance candidate-executions
cargo run --release --locked -p xtask -- conformance verify-representative-owner representative.stage4-frames-text-assets.v7
```

The canonical subject must still be `c314cff4…`. The last three commands should
then verify approved three-format receipts. If the parent is also authorized to
record this interaction owner, the existing strict command is:

```sh
cargo run --release --locked -p xtask -- conformance record-representative-owner \
  representative.stage4-frames-text-assets.v7 \
  'User-approved release representative and exact three scoped PDF exceptions; see the actual final issue #85 approval comment and docs/research/png-stage4-v7-release-capture.md.'
```

This does not approve the global ledger or refresh the native baseline.
