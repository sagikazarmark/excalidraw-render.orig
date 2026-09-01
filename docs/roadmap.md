# Roadmap

Aspirational protocols that are **not** part of the compatibility revision 1
contract. Nothing here gates a release today; nothing here is normative.

These two protocols were specified in full before implementation existed, as
sections 15 and 16 of the
[Excalidraw Static Renderer Specification](renderer-spec-v1.md) at document
version `1.1.0`. They are moved here unchanged rather than deleted: the wording
is settled, and the footprint budget in particular is the only standing
pushback against the 12.4 MiB of embedded fonts the renderer carries. When
either protocol is activated, it moves back into the specification (or into its
own approved closed artifact) and becomes normative at that point.

Until then, treat both as design intent. The hard gates that do bind revision 1
are conformance, safety, and artifact byte stability, all specified in
[renderer-spec-v1.md](renderer-spec-v1.md).

## Performance Protocol

Performance is a parity-first regression guard, not a latency SLA. Conformance,
safety, and artifact byte stability are prerequisites.

The immutable workload set contains:

| Workload | Required role |
| --- | --- |
| `empty` | Format/request/operation-matched fixed-cost control |
| `common-32` | 32-element common scene with image, frames, bindings, text, and styles |
| `mixed-512` | Sixteen varied common tiles with raster and nested-SVG slots |
| `geometry-{128,512,2048,8192}` | Prefix-stable geometry ladder |
| `text-{64,256,1024,4096}` | Prefix-stable official/fallback/CJK/emoji/missing/bidi text ladder |
| `decoded-image-{1,4,16}mp` | Exact selected decoded-pixel ladder |
| `png-surface-{1,4,16}mp` | Exact PNG output-pixel ladder |
| `frame-binding-2048` | Fixed 64-frame, depth-8, 512-binding stress scene |
| `codec-1mp` | Each accepted raster family and sanitized nested SVG |
| `diagnostic-512` | Exactly 512 ordered recoverable diagnostics |

`common-32` contains exactly 6 rectangles, 4 ellipses, 3 diamonds, 4
lines, 4 arrows, 3 freedraw elements, 5 text elements, one local 256x256 image,
and 2 frames. Its checked schedule covers bound/unbound linear elements, bound
text, nested frame clipping, common rough/fill/stroke combinations, official and
fallback text, overlap, and frame placement without changing the total.

`mixed-512` is 16 uniquely positioned/content-varied common tiles. Its image
slots alternate 8 raster and 8 sanitized nested-SVG assets. The checked schedule
covers common roughness, fill, stroke, text, binding, frame, safe-link, and
expected-diagnostic classes; content remains unique where deduplication could
erase representative work.

The geometry base motif is exactly 128 elements: 24 rectangles, 20 ellipses, 20
diamonds, 24 lines, 24 arrows, and 16 freedraw elements. Exactly 16 arrows bind
at both endpoints. The ladder repeats the motif 1, 4, 16, and 64 times with
prefix stability and fixed style/point/binding schedules.

The text base motif is exactly 64 runs: 24 official-family Latin, 8 Liberation
Sans fallback, 8 Xiaolai/CJK, 8 Noto Emoji, 4 missing-glyph, and 12
bidirectional/wrapped/bound/frame-label runs. Catalog order and fixed alignment,
direction, wrapping, size, and binding schedules apply. Larger points use unique
strings and placements.

Decoded-image points contain exactly 1,048,576, 4,194,304, and 16,777,216 unique
selected decoded pixels with stable-entropy patterns and identities while other
costs stay fixed. PNG-surface points produce exactly those output-pixel counts
while scene semantics and decoded-image pixels stay fixed.

`frame-binding-2048` has 64 frames as eight depth-8 chains, 640 rectangles, 256
ellipses, 256 diamonds, 256 arrows, 384 text elements, 128 lines, and 64
freedraw elements. Its 512 bindings are 384 arrow-endpoint and 128
container-text bindings. Non-frame elements split equally among inside,
crossing, clipped, and overlapping placement classes.

`codec-1mp` independently gates PNG, JPEG/JFIF, GIF, WebP, BMP, ICO, AVIF, and
sanitized nested SVG. Every raster variant supplies 1,048,576 selected pixels
with equivalent stable-entropy appearance; animation-capable formats include a
contrasting non-selected frame. If one legal source cannot supply the count, use
the minimum unique sources summing to it. Nested SVG uses a 1024x1024 viewBox.

`diagnostic-512` yields an artifact and exactly 128 ordered occurrences in each
of repairs, substitutions, unsupported behavior, and resource fallbacks.
Minimal independent triggers distribute round-robin over canonically sorted
applicable codes, with earlier codes receiving any remainder. The expected
report proves no coalescing, omission, or fatal conversion.

Fixtures, assets, recipes, generators, driver, schemas, statistics, runner
profiles, and policy records share one versioned protocol digest. Activation
freezes exact bytes, options, identities, and hashes. Runtime pseudorandom
generation is forbidden. A validator recomputes all declared composition,
ladder, pixel, diagnostic, applicability, and digest facts. Maximum and
representative fixtures pass default resource limits.

All generated families are deterministic, versioned, prefix-stable, and checked
by output hash. Larger points use unique IDs/content/placement where needed and
hold non-target cost drivers fixed.

Targeted variants are only dark-mode `common-32`, embedded-scene `common-32`,
scale-2 PNG `common-32`, and default `diagnostic-512` where applicable. No
arbitrary option cross-product is allowed.

### Collection and Statistics

Public timing boundaries are parse, renderer construction, parse plus first
render, first render of a pre-parsed scene with new renderer, repeated render,
and native CLI end to end. Cold operations use fresh processes. Warm operations
reuse only immutable scene and renderer while creating a fresh session and
fully consuming/dropping each outcome.

Candidate versus merge-base and candidate versus checkpoint each collect exactly
30 valid adjacent timing pairs in 15 quartets alternating `A/B/B/A` and
`B/A/A/B`. Their quartets are interleaved in one job but estimates and bootstrap
streams remain separate. If reference commit and protocol digest are identical,
one series serves both comparisons; otherwise each gets 30 pairs.

Before collection, calibrate both references for the exact result by doubling an
integer batch count until three consecutive trials all exceed the greater of 50
ms and 10,000 measured monotonic-clock quanta. The larger reference requirement
applies to candidate, merge-base, and checkpoint. Calibration, excluded setup,
and declared warm-up are not samples. Batch count cannot change after observing
candidate results. Record integer nanoseconds and normalize with the common
count.

There is no outcome-dependent early stop. A sample is invalid only for a
predeclared mechanical failure: wrong build/protocol/environment/fixture,
nonconformant output, missing invocation, clock or supervisor failure, or
whole-runner-health failure. Any invalid member reruns its complete quartet.
Slow runs, page faults, context switches, isolated load spikes, and watchdog
timeouts are not removable outliers; timeout is measured failure.

The runner pins CPU/microcode, affinity, governor/turbo, memory, OS image,
browser/tools, clock, process allowlist, and numeric memory/disk/thermal limits.
Identity/topology change, clock fault, throttle increase, OOM/machine check,
unexpected competitor, or threshold breach invalidates the whole job. One clean
retry is allowed; two consecutive invalid jobs are infrastructure failure.

For pair `i`, compute `ratio_i = c_i / r_i` and `delta_i = c_i - r_i`. Sort
numerically. An odd median is the center; an even median is the arithmetic mean
of its two centers. No trimming, winsorization, or manual exclusion is allowed.

The confidence bound uses exactly 100,000 paired percentile-bootstrap
replicates, resampling all complete rows and recomputing median ratio. Seed is
the first 64 big-endian bits of SHA-256 over protocol digest and canonical
result identity. SplitMix64 uses wrapping `u64` exactly:

```text
state += 0x9E3779B97F4A7C15
z = state
z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9
z = (z ^ (z >> 27)) * 0x94D049BB133111EB
output = z ^ (z >> 31)
```

Row selection rejects outputs at or above `floor(2^64 / n) * n`, then uses
`output mod n`. The one-sided 95% lower bound is sorted zero-based index 4,999.

Memory is measured separately from timing in three layers: Linux fresh-process
high-water RSS including materialized outcome; renderer-attributed native heap
high-water and post-drop live bytes through a counting allocator; and browser
WASM linear-memory high-water and repeated-call growth. Browser RSS is telemetry.
Each layer uses exactly 10 valid adjacent pairs, five balanced quartets, per
reference comparison, with median ratio/delta, no bootstrap, and no deletion.

Retention orders every active tuple by case identity, variant identity, then
format. It warms each tuple once through shared immutable scenes/renderers,
performs exactly 100 fresh-session renders cycling that list and consuming and
dropping each outcome, then repeats the sequence three times in fresh processes
or WASM instances. Any run over cap fails. Artifact size is measured only after
byte stability.

### Gates

A timing comparison fails only when all three strict conditions in its row hold;
equality passes. Failure against merge-base or checkpoint fails.

| Operation/platform | Median ratio | Median delta | Bootstrap lower bound |
| --- | ---: | ---: | ---: |
| Linux batched parse/construction/warm API | `> 1.10` | `> 0.25 ms/op` | `> 1.05` |
| Linux first render/parse-first/CLI | `> 1.10` | `> 5 ms` | `> 1.05` |
| WASM batched API | `> 1.15` | `> 1 ms/op` | `> 1.08` |
| WASM first render/parse-first | `> 1.15` | `> 10 ms` | `> 1.08` |

Linux RSS fails only above both 10% and 16 MiB. Native heap and WASM linear
memory fail only above both 10% and 4 MiB. After warm-up and 100 renders,
additional native live heap is at most 64 KiB and WASM growth at most two 64-KiB
pages. SVG/PDF size fails above both 10% and 4 KiB; PNG above both 10% and 64
KiB. Savings elsewhere do not offset growth.

In addition, focused Latin, multi-chunk, and complete-CJK direct SVG fixtures
containing subsets MUST each be more than 20% and 4 KiB smaller than a
same-revision control embedding the selected complete chunks. A full-intersection fixture, whose family demand intersects
every declared range, is exempt from this reduction threshold but not
correctness or ordinary size gates. No representative-corpus direct SVG may grow
against its same-revision complete-chunk control.

Scaling subtracts matched empty median. A nonpositive or sub-quantum net value
clamps only to monotonic-clock quantum divided by batch count for time, one byte
for native heap/artifact size, or one 64-KiB page for WASM linear memory. Every
clamp is reported. Unweighted natural-log OLS over all points must have elapsed
exponent at most 1.2, memory exponent at most 1.1, and SVG/PDF byte exponent at
most 1.1. No adjacent 4x step exceeds 6x elapsed or 5x memory. If fewer than
three points or the maximum are measurable, exponent remains explicit
non-blocking telemetry while the adjacent-step gate still applies.

Sample watchdog is `max(5 seconds, 10 * approved median)`, capped at 120 seconds
native and 180 seconds WASM. Before approval, use the cap.

Every pull request without path exemptions runs the complete active Linux
matrix across operation boundaries, formats, ladder points, targeted variants,
memory layers, retention, artifact size, and scaling. It also runs a blocking
Chromium/WASM subset with `common-32`, `mixed-512`, every applicable maximum
ladder/risk endpoint and format, and retention. Nightly and release run every
active WASM ladder and variant. macOS and Windows performance is telemetry;
correctness and bytes remain hard gates. Release repeats performance from the
protected tag and packaged sources.

Before activation a case is telemetry. Every mandatory case activates before
first release. The first checkpoint can be the first fully conformant
implementation after evidence approval, byte stability, default-limit,
retention, and resolvable scaling gates pass.

Checkpoint and protocol transitions use immutable records with commits, old/new
digests, overlap evidence, reproduction commands, and maintainer approval. A
performance exception is machine-readable, narrowly scoped, approved, and
expires no later than the next release. It cannot waive conformance, safety,
artifact byte stability, resource isolation, or default-limit admission.

Exceptions cannot use wildcards, overlap, stack, rely on mutable labels, or be
renewed in place. Continued need requires a new evidence-backed record. Expired
exceptions fail closed. An active exception cannot be absorbed into or coexist
with a checkpoint transition; a checkpoint move must pass ordinary gates against
the old checkpoint without exception.

Every result report includes all candidate/merge-base/checkpoint comparisons,
raw samples, confidence data, memory high-water and retention values, artifact
sizes, scaling fits, environment identities, threshold decisions, active
exceptions, and reproduction commands.

## Canonical WASM Footprint

The first release has no absolute WASM size SLA. It gates canonical WASM
footprint: raw-module and deterministic Brotli lengths for a release-shaped,
non-published harness containing every output path and the complete asset and
license catalog. JavaScript glue, browser/test code, fixtures, expected outputs,
and measurement tooling remain outside the measured module.

The dedicated footprint harness is separate from the `wasm-bindgen-test` smoke.
It depends on the packaged `excalidraw-render` crate and invokes only supported
public Rust API through a fixed adapter making SVG, PNG, PDF, report
serialization, and `license_bundle()` reachable. It calls no internals and
creates no public browser interface. Its checked fixtures and coverage manifest
remain external to the module, are supplied through the adapter, and validate
coverage identities against the compatibility asset and scene-surface manifests.

The harness exercises SVG, PNG, PDF, every font/fallback/deprecated family,
every decoder and nested-SVG path, fallback geometry/notices, report
serialization, and `license_bundle()`. Missing coverage, unexpected diagnostics,
nonconformant artifacts, default-limit failure, or unreachable catalog content
invalidates evidence.

The canonical profile uses `opt-level = "z"`, fat LTO, one codegen unit, aborting
panics, no debug/name metadata, exact-pinned `wasm-bindgen --target web`,
exact-pinned `wasm-opt -Oz`, and canonical stripping. Two isolated builds produce
byte-identical modules and record SHA-256.

Independent gates are raw bytes and deterministic generic Brotli quality 11,
window 22. The protocol digest binds every measurement-affecting tool, graph,
profile, harness, fixture, manifest, compatibility revision, browser,
compressor, and schema. Different protocol digests require a reviewed
dual-protocol transition.

Each metric compares independently against merge-base and approved checkpoint.
It fails only when growth is greater than both 1% and 131,072 raw bytes, or both
1% and 65,536 Brotli bytes. Equality passes; metrics cannot offset each other.
Every increase is reported even when it passes.

Every pull request without path exemptions builds the canonical module twice,
validates harness coverage/conformance/default limits, and performs raw and
Brotli comparisons against merge-base and the approved checkpoint. Nightly and
release repeat from clean immutable environments; release uses protected-tag and
packaged sources.

A binding checkpoint is required before release and records complete coverage,
conformance, default-limit success, reproducible module hashes/sizes, protocol,
commands, and maintainer approval. Upward transitions require approval.
Temporary exceptions are candidate- and metric-specific with narrow ceiling and
next-release expiry. They cannot waive correctness, safety, determinism,
catalog completeness, or coverage.

Footprint exceptions cannot use wildcards, overlap, stack, rely on mutable
labels, or renew in place; expiry fails closed. Active exceptions cannot be
absorbed into or coexist with a checkpoint transition. Upward checkpoint moves
are permitted only for parity, safety, newly supported contract behavior, or the
fixed complete catalog, with ordinary gate evidence.

Deterministic gzip size and WASM section/code/data/asset/dependency attribution
are recorded as non-gating telemetry. Every result reports raw/Brotli bytes,
deltas against both references, threshold decisions, protocol/checkpoint,
exceptions, module digest, and reproduction command. Release notes publish raw
and Brotli sizes, deltas, checkpoint identity, and protocol digest.
