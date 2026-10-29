# Bounded GIF first-frame qualification (#82 / #85)

Starting at `4de0863`, this tranche adds **120 matrix cells / 240 valid vectors**
and **16 authored malformed/resource controls**, without production, dependency,
old reference or approval changes.

## Independent authoring and timed reference capture

The GIF89a author imports no GIF encoder. Its fixed three-bit LZW stream clears
before each literal, explicitly packs subblocks and interlace pass order, and
derives expected RGBA separately from display-order indices. A closed Rust
matrix checker reconstructs bytes, fields and expected pixels independently.

The 9×7 logical-screen matrix crosses global/local/local-override four-entry
palettes, full screen or a 5×5 image at (2,1), opaque or transparent index zero,
sequential/interlaced rows, and static or first-frame disposal 0–3 in two-frame
animations. Each cell has alternating and asymmetric patterns. Background index
three deliberately differs from transparent zero; space outside a subrectangle
is authored transparent black. This is first-frame disposal metadata coverage,
not later-frame disposal/compositing qualification.

Official Excalidraw v0.18.1 / Chromium 149.0.7827.55 collection uses the existing
pinned profile/OCI with network disabled. Two fresh processes collect opposite
orders, each case in a fresh context. Animated first-frame delay is 655.35 seconds;
host/browser clocks require the complete capture within 30 seconds from initial
load. The three observed surfaces must match the independently authored first
frame and not the distinguishable later frame. No timer/profile override freezes
arbitrary animation.

All 240 vectors were admitted. Intrinsic GIF, official direct PNG and rasterized
official SVG produced **720 exact first-frame matches, zero differences**.
The immutable record binds 18 source/profile files:

`sha256:732e65695419a9f30d105b2d54cd3da1a392b9f7d2e51c89905abd116e825ae8`.

## Executable gates

Every exact owner selector executes evidence/source/helper binding, closed-matrix
reconstruction, discrimination and negative-oracle gates. Valid vectors compare
native PNG and bound PDF RGBA exactly to authored/browser pixels, including
transparent RGB zero, palettes, offsets and interlace. SVG retains exact source
octets and live animation. Artifact/page/image extents and full ordered reports
are checked independently. Multiframe PNG/PDF emit one flattening diagnostic per
rendered occurrence; static and SVG remain silent. Repeated fresh renderers must
produce identical artifacts and complete reports.

Twelve uncollected malformed controls exercise invalid palette/LZW data (including
a later frame), trailer/GCE/descriptor faults, missing palette and out-of-screen
frames. An additional isolated palette-index fault distinguishes bad indices
from bad background-index framing. These require exact decode-failed recovery,
independent output extents and equality to authored missing-asset controls.

Finite resource tests cover byte, logical-pixel, dimension and prepaid validation
work equality/first crossing, direct-SVG pixel exemption and complete recovery.
The additional-frame validation charge is independently calculated as four units
per later-frame pixel. Two huge-header controls establish early work-cap rejection
before unsafe decode/allocation only. They do **not** qualify huge decode or
pixel-limit precedence, aggregate resource closure or physical OOM behavior.

## Coverage and remaining scope

The normal append-only extension authenticates the 2,126-obligation predecessor
catalog against the `4de0863` ledger, preserving every prior criterion/disposition,
three exclusions, all 168 inventory keys and approved V12.

| Measure | Count |
| --- | ---: |
| Obligations | 2,262 |
| Primary owners | 2,174 |
| Approved exclusions | 3 |
| Assigned/excluded | **2,177** |
| Explicit gaps | **85** |
| New bounded owners | 136 |

No GIF umbrella facet is closed. GIF87a, dictionary growth, larger palettes,
looping/short-delay animation, arbitrary capture offsets, later-frame composition
and broader resource/fuzz matrices remain to qualify. Deferred JPEG work (#187,
#190) and #178/native-baseline work remain separate. The global ledger remains
unapproved.

## Reproduction

```sh
bash tools/reference-oracle/run-gif-first-frame.sh verify
cargo test --locked -p excalidraw-render --test fast gif_first_frame_contract::
node tools/reference-oracle/scripts/gif-first-frame-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 with incremental/debug profiles disabled. The ordinary Rust tests
consume frozen evidence without Node/browser/container access. Generation refuses
existing output; verification mounts the repository read-only and builds in tmpfs.

## Final verification and review

- Generation and separate read-only pinned-browser verification passed with the
  same complete reference digest and all 720 exact observed surfaces.
- Full `cargo test --locked --workspace --no-fail-fast`: **1,253 passed, zero
  failures, 19 existing ignored tests**, including all 617 fast tests.
- The four new exact owner selectors, formatting and Clippy with warnings denied
  passed. Normal ownership and approved V12 verification passed, as did the
  independent append-only preservation audit.
- Independent Standards review found zero hard violations or actionable
  heuristic findings. Independent Spec review found zero scoped findings.
- No production fix, tolerance change, evidence refresh or new approval was
  needed for this bounded tranche. GIF umbrella requirements remain open.
