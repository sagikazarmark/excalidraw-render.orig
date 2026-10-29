# Compatibility recovery ownership (#85)

Base: `fc62016`, following completion of nested SVG/link ownership.

## Malformed elements

The selected owner is
`compatibility_recovery_contract::malformed_element_contract_covers_recovery_and_semantic_loss`.
It compares complete ordered machine reports for all three formats, covers
absent/empty/non-string discriminators and non-object entries, suppresses
descendant classification, and distinguishes bounded placeholders from omissions.

A safe common link is recoverable without a valid kind discriminator. A bounded
placeholder keeps it; without bounds, SVG/PDF lose that semantic channel. The
initial regression failed because malformed-element diagnostics always carried
only `visual`. The producer now adds `semantic` for that loss. PNG does not claim
loss of a navigation channel it does not carry. Unsafe links have their own
diagnostic and are not mistaken for recoverable navigation.
The same effect reconciliation applies when a coordinate-rejected parent frame
removes a previously bounded malformed placeholder. A regression first reproduced
the missing semantic effect, then passed with safe/null/unsafe links in all formats.

Independent unknown-type shell/empty-scene oracles, explicit label/path checks,
navigation checks, canonical original embedding, deletion silence, source order,
frame batches and fatal diagnostic prefixes complement report repeatability.
An authored SVG drawing specifies a quarter-turned, half-opacity box clipped by
an inner frame, independently of any renderer recovery path. SVG and direct PNG
are compared across every pixel channel within 3/255 for separately quantized
antialiasing, with exact flat-fill and clipped/outside probes. PDF assertions
establish the enclosing frame clip, numeric quarter-turn matrix, applied opacity
state, source-bounds clip, dashed outline and diagonal paths. The larger-label
case separately checks the prescribed text and fallback-font report.

## Unsupported property values

The selected owner is
`compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family`.
Its exact selector executes the existing independently authored field matrices
through explicit component entry points in the identity, geometry/style, text,
image, point/stroke, relationship, frame and metadata modules. Those matrices
already compare full reports, independently repaired scene artifacts, original
source preservation, per-kind participation, frame replay, and silent controls.
The composed owner intentionally runs the full matrices rather than treating
their source anchors as execution evidence.

A separately authored field inventory is compared to the current manifest's
missing/invalid bindings for this code. New fields or value-rule branches make
the closure check fail until executable coverage is reviewed. The current
bindings use **repaired, substituted, placeholder and omitted** recovery.
`ignored` exists in the registry vocabulary but no current field binds it to
this code; inapplicable fields are silent and unknown properties have a different
diagnostic.

Mixed-cause cases prove precise pointers and source ordering, narrower point
cause suppression, malformed-record descendant suppression, metadata effects,
and the distinction between the surface-classification failure prefix and the
final source-sorted report. A new zero-area point regression found a placeholder
claim despite there being no box to paint. Normalization now retains the precise
point diagnostic but selects omission/null recipe before retention.
Review also exposed an invalid binding's field-level omission suppressing the
entire bounded point placeholder. The regression failed on empty SVG output;
source-replacement matching now distinguishes field omission from whole-element
recovery. Element-ID, gap and focus binding failures retain the required box in
SVG/PNG/PDF. The aggregate selector explicitly includes the metadata module's
line-arrowhead matrix as well as its persisted request-control matrix.
The follow-up proof review found that allocating image-payload pointer matching
treated allocation refusal as non-applicability. Matching now compares RFC 6901
bytes without allocation, so a transient refusal cannot enable generic recovery.
Exact payload reports/artifacts cover slash, tilde, Unicode and suffix-like IDs.

## Generic known-element fallback

The current generic diagnostic is unreachable by the argument and executable
guards in [known-element-replacement-reachability.md](../known-element-replacement-reachability.md).
Its three entries received explicit, digest-bound [#85 exclusion approval](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5700122166)
on 2026-09-16. The approval binds the control-flow proof and exact per-format
subjects below; negative tests alone are not substituted for an exclusion.

This assessment concerns diagnostic ownership. The metadata failure-injection,
PDF fallback, codec/profile, interaction, and global release-closure work remains
separately tracked.

## Ownership checkpoint

At this implementation checkpoint, six diagnostic/format entries gained proposed
test owners: **821 assigned, 107 gaps** globally; **97 of 122** diagnostic/format
entries assigned. The three generic-fallback entries were then gaps pending
approval. Their later approval is recorded below.

## Verification

Using Rust 1.97.1 explicitly on the working tree based on `fc62016`:

- Both reviewed runtime defects first failed their public-facade regressions,
  then passed after correction.
- `cargo test --locked --workspace`: **1,054 passed, 18 ignored**, including the
  expanded unsupported-value owner and all registered SVG/PNG parity cases.
  The renderer fast tier passed all 496 tests in 921.56 seconds. The ignored
  tests require separate harnesses or explicit qualification/materialization runs.
- After the final allocation-free payload matcher and its additional regression,
  the nine focused compatibility tests (excluding the already-executed aggregate)
  and all five image-field contract tests passed. The new payload regression is
  also called by the aggregate owner; the full workspace suite was not repeated
  after this narrow change.
- Workspace/all-target Clippy with `-D warnings`, library rustdoc with
  `RUSTDOCFLAGS="-D warnings"`, and the renderer's
  `wasm32-unknown-unknown` check passed. An initial all-workspace doc build also
  passed but printed Cargo's library/CLI output-name collision warning; the
  library-only documentation check was clean.
- Catalog verification, all **37** registered conformance cases, draft execution
  inventory verification and ownership verification passed. The inventory still
  reports eight unresolved release milestones; ownership remains **821/107**.
- Standards review: the missing line-arrowhead composition is corrected, with no
  remaining findings. Spec review: the three initial recovery/artifact findings
  and the subsequent allocation-matching proof finding are resolved; final narrow
  review found no remaining blocker in that proof.
- `git diff --check` passed. No native/WASM execution equality is claimed for
  these runtime changes; the separate #178 evidence remains the prior checkpoint.

<a id="pending-exclusion-approval-subjects"></a>

## Exclusion approval subjects

**Approved by sagikazarmark on 2026-09-16**, following explicit authorization in
the implementation session. The scoped decision is recorded in
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5700122166).
All three current subjects were recomputed and matched these previously recorded
proposals before approval was applied. The approved proof descriptor is:

```json
{
  "source": {
    "path": "docs/research/known-element-replacement-reachability.md",
    "sha256": "ff81adaf36f6f2f09ce298ba33307e8d82ad1a93c812096ebb1e0e96044a8226"
  },
  "anchor": "## Revision-1 reachability conclusion"
}
```

Exact approved reason for each format:

> The current revision-1 supported-kind dispatch cannot emit this generic diagnostic; unavailable geometry and image failures retain narrower replacement causes. See the reviewed control-flow proof and its invalidation conditions.

| Obligation | Approval subject SHA-256 |
| --- | --- |
| `diagnostic:compatibility.known-element-replaced:pdf` | `sha256:09f7fa37c60a486122398bd73fc125bf7d1d90701802b4223ef33375d35877d1` |
| `diagnostic:compatibility.known-element-replaced:png` | `sha256:2a2ec2b1b8f34d58a024436b0bc1071e5e9261928b8f58b1fe8c2e0accbd5f97` |
| `diagnostic:compatibility.known-element-replaced:svg` | `sha256:707ec4c5b195ba59dc1771f799a8f84e75c8ac7b6ad3f20b58a4e38e0a7f9b3b` |

The subjects use `xtask`'s exclusion formula: canonical JSON of schema
`excalidraw-render/coverage-exclusion/v1`, the **complete obligation row** from
`conformance ownership-catalog`, the exact reason, and the proof descriptor.
The current rows bind scene-surface manifest SHA-256
`93029f2dab63c203dee69da4497ea4c2988a72d73f2a0017e62c529a4caac202` at
`/registryMappings/9/formatRules/{format}`. Recompute subjects if any bound input
changes. The three dispositions are now approved exclusions. Following the later
PDF and metadata ownership work, current totals are **843 owned plus 3 approved
exclusions: 846 assigned/excluded and 82 gaps**. All 122 current diagnostic/format
entries have an owner or approved exclusion. Global ledger approval and release
closure remain pending.
