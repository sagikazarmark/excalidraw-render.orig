# Bounded GIF LZW, palette, version, and extension evidence

Independently authored GIF bytes and palette/index RGBA expectations, collected
against official Excalidraw v0.18.1 / Chromium **149.0.7827.55**.

```sh
bash tools/reference-oracle/run-gif-lzw.sh generate # create-only wx
bash tools/reference-oracle/run-gif-lzw.sh verify   # recollect read-only
```

The digest-pinned linux/amd64 container disables networking, builds in tmpfs,
and verifies the immutable revision-1 profile. Collection uses two fresh browser
processes, forward/reverse order, and a fresh context per vector per pass. Exact
complete observations must match across passes. Author, collector, runner, scene
helper, profile/wrapper dependency closure, lockfile and profile inputs are hashed
before collection and checked again afterwards. Verification checks source hashes
before launching browsers and compares the entire regenerated record byte-for-byte.

## Closed inventory

- **36 matrix cells × 2 patterns = 72 vectors:** palettes **2 / 16 / 256** ×
  headers **GIF87a / GIF89a** × programs **growth-to-full / midstream-reset /
  kwkwk** × image sub-block sizes **1 / 255**.
- **8 additional extension cells/vectors**, GIF89a, 16-entry palette,
  growth-to-full, 255-byte image sub-blocks: comment before, comment after,
  byte-split comment before, byte-split comment after, unknown application with
  byte-split payload, NETSCAPE infinite loop, NETSCAPE finite loop (2), plain text
  before the image. Comment payloads exceed 255 bytes even in the unsplit variant.
- **12 uncollected authored malformed controls:** minimum code size 1 and 9,
  undefined initial code, undefined code after a literal, missing EOI, truncated
  sub-block, missing image-data terminator, early EOI, excess pixels, missing
  trailer, GIF87a with comment, GIF87a with application extension.
- Total **92 vectors / 56 cell identifiers**; **80 collected vectors**.

All images use a global palette, opaque sequential full-screen pixels, and no GCE.
Two-color screens are **257×193 = 49,601 pixels**; 16/256-color screens are
**129×97 = 12,513 pixels**. The larger binary screen supplies enough entropy to
fill the dictionary: a random 12,513-bit binary image is too small for that claim.
NETSCAPE cases still contain exactly one table image. The plain-text case has no
GCE, uses a valid 8×8 text grid, and precedes a full-screen opaque image which
overwrites it; there is no font raster or intermediate text-frame expectation.

GIF87a extension controls express strict version-policy intent, not a browser
rejection claim. All controls need independent parent-policy binding. They are
never submitted to Chromium and have no fabricated browser results.

## Independent author and stream proof

Primary specification: [CompuServe GIF89a, 31 July 1990](https://www.w3.org/Graphics/GIF/spec-gif89a.txt),
sections 15–17, 22, 24–26, Appendix F, and the deferred-clear cover sheet.
NETSCAPE2.0 looping is conventional application payload, not GIF89a-defined
looping semantics. GIF is a service mark of CompuServe Incorporated.

The encoder implements longest-prefix LZW with a real dictionary; it does not
clear before literals. A separate reader-lag counter determines serialized code
width, including the width of EOI after the final phrase. The growth program fills
4096 entries and continues at 12 bits without clearing. Reset inserts an explicit
clear at the halfway pixel, after flushing the preceding phrase. The KwKwK program
uses variable-length repeated-symbol runs to force **code == next dictionary code**.

The two fixed xorshift32 seeds are `0x13579bdf` / `0x2468ace1`; entropy takes the
upper 16 bits modulo palette size. KwKwK runs and both explicit RGB palettes vary
by pattern. `authored_indices_base64` stores display-order indices; expectations
expand those directly through the recorded palette, never through a GIF decoder.

Before browser collection, a separately structured JS string-table decoder audits
the packed LZW bytes against authored indices, validates EOI/padding, and asserts
program invariants. A container walker checks the finished header, palette,
extension framing, descriptor, reconstructed sub-block payload, and trailer.
Each stream records widths actually read, dictionary references,
clear/KwKwK counts, full-dictionary usage, and bit/pixel positions of width/clear
events. This audit is an author sanity check; it does not substitute for the
parent's independent Rust checker. Neither author nor collector imports a
production GIF codec.

## Evidence layout

`cases` retains original GIF bytes/hashes, indices/palette, expected RGBA,
stream statistics/events, scene document, exact official PNG/SVG, SVG-viewer PNG,
and intrinsic/PNG/SVG-viewer RGBA. `comparison` reports dimensions and changed
pixels on each surface. `differences` retains all author/browser discrepancies
without rewriting expectations. `controls` contains bytes, reason, and authored
strict/recovery intent only.

Generation refuses an existing record; verification mounts the repository
read-only. Preserve source-bound scripts after collection. This collection covers
the listed static images and does not establish arbitrary animation timing,
later-frame compositing, or production qualification.

## Collected result

Generation and a separate read-only verification passed under the pinned profile.
**80/80** vectors were admitted; all **240** observed surfaces matched authored
dimensions and RGBA exactly. **Zero discrepancies** were observed; `differences`
is empty. Both commands compared complete forward/reverse observations. The
record binds **18** source/profile files and occupies **55,363,520 bytes**.

| Audited stream statistic | Range across collected vectors |
| --- | --- |
| Actual code widths | 3 through 12 bits |
| Code count (including clear and EOI) | 681–12,072 |
| Dictionary references | 444–5,651 |
| Clear codes (including initial clear) | 1–2 |
| KwKwK events | 0–1,310 |
| Largest next-code value | 696–4,096 |
| Data codes read with a full dictionary | 0–8,126 |

All 24 dedicated KwKwK matrix vectors have **129–1,310** KwKwK events. All 24
growth-to-full matrix vectors reach a full dictionary and continue using it;
the 8 extension vectors use the same growth program. All 24 reset vectors reach
12-bit codes and emit one midstream clear in addition to the initial clear.
Binary growth/reset streams actually read every width **3→12**. Widths for
16-entry palettes start at 5; widths for 256-entry palettes start at 9.

`reference.json` SHA-256:
`3c05784f1152a10abcc731638ed46a185c16871ecb050da56f5caba7de30f707`

The pre-collection container audit caught an incorrect application-identifier
length; it was corrected before any browser collection or record creation.
Source-bound scripts have not been edited since successful generation.
