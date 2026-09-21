# Bounded GIF control-state and later-frame validation evidence

Independent **9×7 GIF89a** bytes and index/palette expectations, collected against
official Excalidraw **v0.18.1 / Chromium 149.0.7827.55**.

```sh
bash tools/reference-oracle/run-gif-control.sh generate # create-only wx
bash tools/reference-oracle/run-gif-control.sh verify   # recollect read-only
```

## Closed inventory

- **24 static cells × 2 patterns = 48 vectors:** global/local two-entry image
  palette × 12 signals: no GCE, transparent, opaque, transparent→comment,
  transparent→application, transparent→Plain Text, transparent→Plain Text→opaque
  GCE, duplicate transparent→opaque, duplicate opaque→transparent, duplicate
  transparent index 0→index 1, user-input flag with long delay, trailing GCE.
- **8 two-frame cells × 2 patterns = 16 vectors:** global/local palette × first
  opaque/later no GCE, first opaque/later transparent, first opaque/later opaque,
  first transparent/later no GCE. First-frame delay is **65535cs = 655.35s**,
  disposal 1, no input flag or loop extension. The later no-GCE hypotheses reset
  transparency rather than inheriting it from frame 1.
- **20 uncollected malformed controls:** early/late pairs for bad palette index,
  undefined LZW code, truncated GCE, out-of-bounds descriptor, truncated
  sub-blocks, invalid block/end marker, minimum code size 1, early EOI, missing
  EOI, truncated local palette. Each late control preserves a complete valid
  first-frame prefix from the positive two-frame local/opaque/pattern-1 source.
- **84 distinct GIF SHA-256 bindings:** 64 collected, 20 malformed controls.
  Of the collected probes, **48 are spec-valid**, **12 duplicate-GCE probes**
  violate the one-GCE-per-graphic rule, and **4 trailing-GCE probes** have orphan
  scope. “Well-framed” does not imply spec-conforming control scope.

## Authoring and hypotheses

Primary source: [CompuServe GIF89a, 31 July 1990](https://www.w3.org/Graphics/GIF/spec-gif89a.txt),
sections 12, 15–16, 19–27 and Appendix F. GIF is a service mark of CompuServe
Incorporated. Sections 12 and 23 delimit GCE scope at the next graphic-rendering
block; comments and applications do not consume it. Plain Text is a
graphic-rendering block even when its data is empty. Section 23 permits at most
one GCE before a graphic; complete replacement by the last GCE is explicitly a
**recovery hypothesis**, not normative behavior for duplicate GCEs.

The independent author uses literal-only LZW: clear=4 before every literal,
EOI=5, fixed three-bit codes, minimum size 2, zero final padding. Two explicit
binary index patterns and RGB palettes produce expected RGBA directly, without
decoding. A GCT exists in every source; local-image cases use a distinct LCT so
incorrect palette selection is visible. The Plain Text header has an in-bounds
9×7 grid, 1×1 cells, valid global foreground/background indices, and empty text
data. It has no glyphs or text-rendering oracle. All static delays are zero except
the single-image user-input case, which has an explicit nonzero 65535cs delay.

The separately structured bounded JS audit walks the finished container through
the trailer and checks **every** image's palette indices, bounds, literal stream,
EOI, pixel count and padding. It records block offsets, frame counts and GCE
scope, and verifies indices against the independently authored arrays. All 64
positive/probe containers complete this audit. All 20 controls fail at their
declared error; early failures have zero completed images, late failures exactly
one. This is author sanity checking, not independent production qualification.

Each control binds its own bytes/hash, positive base ID/hash, target frame,
mutation, failure offset, and preserved-prefix length/hash. Strict rejection of
the entire GIF and image-local recovery are authored policy intent only.
Malformed controls are never passed to the reference browser.

## Timing and evidence boundaries

Four opaque-first timer groups bind identical first-frame source prefixes across
later no-GCE/transparent/opaque variants. Their hypothesized later displays
differ, and every two-frame expected later display differs from its first display.
Collection asserts a **30-second maximum** from first input decode through all
three captures, well below 655.35s. No sleeps or animation-timer sampling are used.
The exact bound assertion is recorded; variable durations are not serialized.

Intrinsic image, official PNG, and official SVG-viewer RGBA are compared to the
authored **first** display. Later patches and disposal-1 display hypotheses are
retained solely as discriminators. Native first-frame observations cannot prove
later display, inherited-state handling on later images, or whole-file rejection.
Frame-count and validation claims here refer to the bounded author audit.

## Reproducibility and layout

The digest-pinned linux/amd64 container has no network, mounts the source tree
read-only, builds in tmpfs and verifies the frozen revision-1 renderer profile.
Generation adds a writable mount for this evidence directory and writes the
record with `wx`; verification is entirely read-only. Forward/reverse passes
use fresh browser processes and a fresh context per vector. Complete observations
must match exactly, including PNG, SVG, SVG-derived PNG and RGBA.

The record binds **18** sources: author, collector, runner, frozen scene helper,
profile/wrapper dependency closure, lockfile and profile inputs. Sources are
hashed before collection and checked afterwards; verification checks bindings
before browser launch and compares the entire regenerated JSON byte-for-byte.
The README is deliberately outside the source binding so results can be recorded
after the write-once collection.

`cases` contains original GIF bytes, audit, indices, palettes, first/later
hypotheses, scene, complete observations and per-surface comparisons.
`differences` preserves every author/reference mismatch. Expectations remain
fixed even when Chromium skips Plain Text without consuming GCE, or retains a
previous transparency index across an opaque duplicate GCE.

Schema: `excalidraw-render/gif-control-reference/v1`.

## Collected result

Write-once generation and a separate read-only verification both passed the
pinned profile and exact forward/reverse equality checks. **64/64** probes were
admitted. All **192** surfaces have the authored dimensions; **180/192** match
the authored first-display RGBA. The **4 discrepant vectors / 12 surfaces** are
exactly `transparent-plain-text`, across both palette scopes and patterns:

| Signal | Authored hypothesis | Chromium observation | Changed pixels per surface |
| --- | --- | --- | --- |
| transparent→empty Plain Text→image, pattern 1 | Plain Text consumes GCE; image opaque | Image retains index-0 transparency | 32 of 63 |
| transparent→empty Plain Text→image, pattern 2 | Plain Text consumes GCE; image opaque | Image retains index-0 transparency | 42 of 63 |

Each discrepant intrinsic/PNG/SVG-viewer sample exactly equals the corresponding
direct transparent-GCE sibling's authored pixels. This supports retained GCE
transparency across the empty Plain Text block in this Chromium profile; it does
not establish text rendering or broader Plain Text support. Adding an opaque GCE
after Plain Text produces the authored opaque image.

Comment/application preservation, all three duplicate-GCE replacement probes,
user-input/long-delay first display, and trailing-GCE non-retroactivity match
their stated hypotheses. In particular, transparent→opaque duplicate GCE clears
transparency in this measured profile. **16/16 two-frame cases** match the first
display on all three surfaces, and none matches its discriminating later-display
hypothesis. Later-display correctness remains unmeasured.

The record occupies **667,351 bytes** and binds **18 source/profile files**.
All **84 source GIF hashes** are distinct. Each of the **20 controls** has a
distinct GIF hash and an explicit positive-source binding; 10 are early failures
and 10 preserve the complete first frame before failing later validation.
For a compact additional inventory check, SHA-256 of UTF-8
`JSON.stringify(controls.map(r => ({id:r.id,sha256:r.gif_sha256})))` is:

`7b1e23c2fc2801e001f43a70ca3cfa7d85d6fe812441100f3aaf2ce43a8b3c93`

`reference.json` SHA-256:

`46e948db20e9bf75abeae48990fb0e4e7eece1e14d51f1aa24bc34436957c387`

Source-bound files and authored expectations have not changed after successful
generation. No malformed browser observations or production-decoder results are
represented by this record.
