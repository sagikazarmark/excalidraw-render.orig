# GIF graphic-control scope and later-frame validation (#82 / #85)

This bounded tranche starts at `5780a00`, with no production, dependency or
earlier approval/reference change. It adds **52 owners**: 32 collected first-display
cells (64 vectors) and 20 uncollected early/later malformed controls.

## Evidence and scope

The independent author builds 9×7 GIF89a streams from literal LZW and explicit
global/local palettes. Static signals cover no/opaque/transparent GCE, intervening
comments/applications/empty Plain Text, duplicate GCE replacement, long-delay
user-input metadata and trailing GCE. Two-frame probes retain distinguishable
later-display hypotheses while capturing only frame zero under the verified
30-second bound against a 655.35-second first-frame delay.

The [GIF89a specification](https://www.w3.org/Graphics/GIF/spec-gif89a.txt) defines
GCE scope through the next graphic-rendering block. The authored hypothesis treats
Plain Text as consuming control state. The actual pinned Chromium profile skips
empty Plain Text while retaining transparency: four vectors differ by 32 or 42
pixels on all three observed surfaces. Those **12 discrepancy facts remain in
the reference**, and the Rust gate independently confirms that the browser result
equals the transparent-GCE sibling. Native output follows the actual browser;
the authored hypothesis is never overwritten to manufacture agreement.

Of the 64 collected probes, 48 are spec-valid, 12 duplicate-GCE probes violate
the one-GCE-per-graphic rule, and four trailing-GCE probes have orphan scope.
Their recorded behavior is non-normative characterization, not new format
validity. The source is preserved in direct SVG. Later display, text rasterization
and generalized animation timing are not qualified by these first-display checks.

Official Excalidraw v0.18.1 / Chromium149 capture uses the unchanged pinned profile,
network-disabled OCI, two fresh opposite-order processes and per-vector contexts.
All 192 observed surfaces have correct dimensions. 180 match the original authored
hypothesis, and the other 12 match the explicitly distinguished transparent
sibling. Generation and separate read-only recollection agree byte-for-byte.

Reference SHA-256:
`46e948db20e9bf75abeae48990fb0e4e7eece1e14d51f1aa24bc34436957c387`.
Eighteen author/collector/wrapper/profile files are bound.

## Native and malformed gates

Both exact owner selectors run independent source/GCE/palette/LZW reconstruction,
closed-matrix and negative/discrimination gates. Every native SVG/PNG/PDF artifact
checks exact source octets where applicable, actual extents, bound image samples,
alpha, full ordered machine report and fresh replay. All 64 collected probes
match actual browser first-display pixels. Flattening is reported only for the
actual two-image PNG/PDF occurrences; static and SVG remain silent.

Ten fault types are paired across early and later images. Late controls retain a
complete byte-identical first-frame prefix and must still reject the entire GIF
before SVG passthrough or PNG/PDF projection. Faults include palette/LZW errors,
truncated GCE/palette/subblocks, invalid descriptor/end marker/code size and
early/missing EOI. Exact decode-failed recovery equals an independently authored
missing-asset control with separately checked dimensions/format invariants.
Additional exact-pixel EOI pairs isolate EOI enforcement from pixel underflow.

Results: **192 valid format checks, 60 malformed checks, 12 paired-EOI checks and
48 valid two-frame witness checks** pass, all fresh-replayed. No native defect
was found. Uncollected malformed controls make no browser rejection claim.

## Append-only ownership

The predecessor catalog is authenticated against the `5780a00` ledger. All 2,318
prior dispositions and criteria, three exclusions, 168 inventory keys and approved
V12 remain unchanged. The ledger is still an incomplete, unapproved global proposal.

| Measure | Count |
| --- | ---: |
| Obligations | 2,370 |
| Primary owners | 2,282 |
| Approved exclusions | 3 |
| Assigned/excluded | **2,285** |
| Explicit gaps | **85** |
| New bounded owners | 52 |

Broader GIF animation/extension interactions, resource/fuzz closure and codec
umbrella facets remain open. JPEG #187/#190 and #178 remain deferred/separate.

## Reproduction

```sh
bash tools/reference-oracle/run-gif-control.sh verify
cargo test --locked -p excalidraw-render --test fast gif_control_contract::
node tools/reference-oracle/scripts/gif-control-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 with incremental/debug profiles disabled. Ordinary Rust tests
need no browser or Node. Generation is create-only; verification is read-only.

## Final verification and review

- Generation and separate read-only browser verification reproduced the exact
  record, including all 12 authored-hypothesis/reference discrepancy facts.
- Full workspace: **1,257 passed, zero failures, 19 existing ignored tests**,
  including all 621 fast tests and registered-case parity gates.
- Both exact owner selectors, formatting and Clippy with warnings denied pass.
  Ownership/preservation and approved V12 verification pass.
- Independent Standards review found no hard violations; more explicit helper
  naming and typed signal/palette enums were low-priority maintainability
  suggestions. Independent Spec review found no actionable findings.
- No production correction, reference refresh, tolerance change or new approval
  was needed. First-display scope remains distinct from later animation behavior.
