# #187 isolated sampling-patch hardening

Status: diagnostic candidate, not integrated into production. The production
dependency graph, failure pins, owners and six sampling gaps remain unchanged.

Archived on 2026-09-25: the Python orchestration runner was retired. The command
below is a historical transcript; Git history retains the runner. The patch,
independent mixed-scan fixtures/probe sources and raw results remain research
inputs, including the unresolved reverse-order browser discrepancy. Ordinary
sampling and truncated-entropy regressions are in `jpeg_color_contract.rs` and
`jpeg_release_limitation.rs` under `crates/excalidraw-render/tests/` (the former
under `fast/`).

## Changes tested

The expanded stable-0.5.15 experiment adds explicit accounting for real entropy
bits when lookahead encounters a marker or physical EOF. Synthetic lookahead
padding remains available to the Huffman lookup, but strict baseline decoding
rejects a block that consumes it. A valid final block can use buffered real bits
without consuming the next scan prematurely.

Buffered baseline scans now distinguish singleton data-unit rows from legal
multi-component MCU rows. Multi-component subsets retain each component's
horizontal/vertical block traversal and layout. Scan-entry restart/DC reset is
retained. Strict checks cover restart position/sequence, missing/duplicate
components, terminal padding, trailing restart markers, and missing EOI.

The original review found physical-EOF synthesis and the ordinary all-components-
first path were not covered by the initial boundary checks. Both were addressed
and exercised with new controls, including a hand-authored one-bit DC/EOB
grayscale stream where fabricated zeroes could otherwise look correct.

## Historical execution

```sh
python3 tools/jpeg187-diagnosis/hardening/run.py /tmp/opencode/jpeg187-hardening-new
```

This creates an isolated cached-decoder copy, applies the retained complete patch,
and runs locked offline Rust 1.97.1 probes with zune-core 0.5.1. It then checks
the independently authored controls in pinned Chromium 149.0.7827.55, in two
fresh browser processes and fresh contexts, network disabled and read-only mounts.
Raw logs, source and fixture hashes are retained in `hardening/results/`.

Passed decoder-side controls:

| Check | Result |
| --- | --- |
| Neutral size/sampling/restart/baseline-progressive matrix | 2,160 pass |
| Original 76 vectors × two frozen browser authorities | 152 canonical comparisons pass |
| Independent nonuniform DC-only mixed scans | 225 exact authored-pixel matches |
| Missing/altered restart markers | 132 strict rejections |
| Truncated entropy with subsequent marker retained | 24 strict rejections |
| Missing/duplicate/extra-entropy scans | 9 strict rejections; permissive calls do not panic |
| Physical EOF, terminal marker, trailing RST, and bad padding | 32 strict rejections |
| Inter-scan COM/APP/DHT/DQT/DRI controls | 3 unchanged decodes |
| Minimal valid grayscale DC/EOB control | exact pass |

The independent encoder directly writes quantization/Huffman tables, per-block
DC differences and entropy padding; expected spatial gray values are authored
from block coordinates, not derived from either decoder. The matrix spans
singleton scans, `[Y],[Cb,Cr]`, `[Y,Cb],[Cr]`, reversed `[Cr],[Cb,Y]`, and all-
component scans. These tests do not establish AC/refinement/fuzz completeness.

## Browser cross-check: unresolved reverse-order discrepancy

**180/225 controls match Chromium exactly; 45/225 do not.** Both fresh browser
processes reproduce the same partition (360 exact and 90 discrepant observations).
All discrepancies are the reverse-order `[Cr],[Cb,Y]` group: browser image decode
resolves, but canvas readback is transparent, while the patched decoder emits the
authored opaque image. The original six #187 source cells do not use this order.

The cause of the browser behavior has not been established. These 45 controls
remain decoder-layout diagnostics and cannot support browser-parity ownership.
Neither ignoring them nor assuming that authored JPEG validity alone establishes
the project's browser contract is justified.

## Review and limits

Standards re-review confirms the specifically reported final-boundary and EOF
accounting gaps are addressed and found no newly established panic/overflow.
Progressive decoding shares the bitstream bookkeeping but does not acquire the
same strict entropy validation. Existing allocation and IDCT write-span behavior
still needs production/resource qualification. Strict rejection differs from the
old decoder for eighteen of the original twenty-four truncated inputs; recovery
compatibility must be explicitly qualified rather than silently refreshing pins.

No production claim follows from a temporary decoder patch and an external
diagnostic dependency graph. Native/WASM and packaged renderer regression checks
are still required after a chosen integration. No upstream issue/PR was opened
and no vendor dependency was added by this investigation.

## Next integration decision

The sampling fix and baseline hardening now have a reproducible candidate.
Recommended next step: prepare an upstream-quality patch against stable zune-jpeg
with these controls, while investigating the reverse-order browser discrepancy.
If the project needs the correction before an upstream release, adopting a
pinned vendored patch is an explicit maintenance decision. That choice also
requires malformed-recovery, license/package, resource and native/WASM
qualification and fresh source-bound approvals. The six #187 gaps remain open
until the actual renderer passes those checks.
