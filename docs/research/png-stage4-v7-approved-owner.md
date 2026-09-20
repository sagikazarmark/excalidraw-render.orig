# Final v7 approval and representative owner — 2026-09-20

## Genuine scoped decision

The user explicitly selected **“Approve exact subject (Recommended)”** for
`representative.stage4-frames-text-assets.v7` and its exact three PDF exceptions.
The parent reported independent subject recomputation and 493 checked descriptors
with no stale bindings. A duplicate search found no existing final-subject
comment. The user-granted decision was recorded by authenticated
`sagikazarmark` on 2026-09-20:

**https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5749841192**

This records the user's approval, not an autonomous coding-agent decision.
The actual GitHub API comment is archived outside the approved candidate in
`tests/conformance/coverage/stage4-v7-approval-comment.json` (SHA-256
`2b1313a6f98a5728825810926e801b4634e5555a17cf3e84d3b6959c3d9db16c`).

The canonical subject is unchanged before approval, after approval, and after
owner recording:

```text
sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05
```

Only `manifest.json.approval` changed within the approval-bearing manifest:

```json
{
  "approved_by": "sagikazarmark",
  "approved_at": "2026-09-20",
  "approval_url": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5749841192",
  "approval_subject_sha256": "sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05"
}
```

The approved manifest file SHA-256 is
`0a4b5339c4301cca8a98a4ce832d574dad24259f1017c9a3429e11f8784d4f4b`.
The serialized approval-subject file remains
`e7ec13c4a21c41e532cdd9964acc059c5a0a9249ce3f15974e3fef9239d6ec86`.

Approved scopes: PDFium whole 118 / `3fefacd49b980278`, PDFium typography
118 / `3fee1f978eeaac28`, PDF.js whole 16 / `3feff3e226be27cf`, with the exact
case/variant/viewer/bounds/reasons/proofs in the subject. No other exception or
global budget changed. The proposal's original preparation-status text is
preserved as evidence; the case's approval authorizes the bound scopes.

## Approved receipts and strict ownership

Both commands passed with `--release --locked`:

```sh
cargo run --release --locked -p xtask -- conformance candidate-verify \
  tests/conformance/candidates/stage4-frames-text-assets-v7
cargo run --release --locked -p xtask -- conformance candidate-executions
```

Each returned exactly these **three approved execution receipts**, for the same
subject, with two fresh native release processes per format and exact full-report
matching. The registry's one case record equals the direct consumer result.

| Execution suffix | Native artifact SHA-256 |
| --- | --- |
| `:default:svg` | `6d27f04f4dad351ca4330895d90a6ab68a5e05e2d6e39f2e21cccffc72171743` |
| `:default:png` | `e6c206a45e03e4f1f634275d7537f096f53f2b22d990e39a820e47dd90757d93` |
| `:default:pdf` | `310ca74edc3ea7e69fc549739c45e05d90d7522704be1d94bb9f553084658265` |

All keys have prefix `representative.stage4-frames-text-assets.v7`. Each receipt
binds canonical judgment digest
`d27b34b512b10d74cdac574c78bf9f61761f268fc67e3073dea6c90184c602e8`
and semantic digest
`7a2903a28b6d7f94e9da36a2e532f84b20a59cb51aab29f984038706e5056701`.
These are canonical digests, distinct from the newline-terminated file hashes.

Strict `verify-representative-owner` passed. The existing
`record-representative-owner` command then recorded only
`interaction:frames-text-assets`, with the exact approval URL/subject and this
audit in its rationale. It binds the approved manifest and unchanged registry
(`2590fa1fb437444b603353fe9589aacc10827a5fe5d37e9fb61155d5c9028a2e`)
and the exact release consumer. The registry's captured pending-status annotation
is retained; manifest approval and validated receipts determine eligibility.

## Ledger transition and preservation

`tests/conformance/coverage/stage4-representative-owner-transition.json` records
the original gap, approved owner, exact before/after ledger bindings, approval,
registry and complete direct/registry receipts. Its SHA-256:
`05f1a180a56e213004504dcc4977276a1129884f45a36833e1f0110732032658`.

Ledger SHA-256 changed from
`088c3dfd5fb1c864363da31b2efc1317a00737b294dccce60ef0400ce4e69c3b`
to `a48720e66bab027d02ffccb21d482a5371f7344088935d20dd1f2c4df2d68262`.
Restoring that one assignment reconstructs the exact prior ledger bytes. Earlier
binding reconciliation records remain immutable. The unbound preservation audit
consumes this explicit transition, checks the actual approved subject/comment/
owner/receipts, then reconstructs the previous ledger for its historical checks.

Current totals:

| Measure | Count |
| --- | ---: |
| Assignments | 1,876 |
| Primary owners | 1,800 |
| Approved exclusions | 3 |
| Assigned/excluded | **1,803** |
| Explicit gaps | **73** |
| New Stage-4 owners | **60** (59 test owners + representative) |
| Original inventory executions | **168**, unchanged |
| Supplemental approved execution keys | **3** |

Catalog digest remains
`944fb1bb349def48ebff5bd570c34ee492aa54893842ba7d5a8b8a5c43c099fd`.
Inventory/ownership validation, live locator/source audit and the transition-aware
preservation audit pass. No stale bindings were found. Global ledger approval
remains null. Production, adapters, captured evidence, v4/v6 history, comparator
budgets, spec and protected PDF pins remain frozen.

## Focused checks and next full suite

Passed: approved consumer/registry/strict-owner verification and owner recording;
release subject consistency; inventory and ownership validation; preservation
audit; five coverage tests, one approved registry integration test and four
ownership/authority CLI tests; live binding audit and `git diff --check`.
Logs: `target/stage4-v7-approved-logs/`.

The default full-workspace suite is for the parent next; it was **not run here**:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0
cargo test --locked --workspace --no-fail-fast
```

Use the pinned Rust 1.97.1 Linux x86_64 environment, retaining the repository's
optimized test profile. The now-approved registry test executes real native
release replay. Keep the pinned viewer/bridge runtime files available, including
the qualified qpdf/PDFium installations under `target/pdf-harness/` and PDF.js
dependencies; their descriptors are verified. Do not waive the registry test or
replace the release producer with dev output. Default Cargo testing leaves
ignored harness and release-qualification gates separate; use their documented
CI tiers for that broader qualification, not a claim that the default suite ran
them. Further producer/adapter changes invalidate this approved subject.

No commit was made. Native baseline refresh/#178, global ledger approval/closure
and the pre-existing fixed-size moxcms allocation/resource gap remain explicit
and are not covered by this scoped approval.
