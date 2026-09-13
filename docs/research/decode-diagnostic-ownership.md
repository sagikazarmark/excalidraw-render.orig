# Decode-failure diagnostic ownership

Base: `b6025db`. Proposed owner for all three `image.decode-failed` format obligations:
`diagnostic_contract::decode_failures_cover_every_codec_and_authority`.

## Exact matrix

- PNG, JPEG, GIF, WebP, BMP, ICO and SVG each have a valid payload, a signature-recognized invalid payload and deeper corruption. PNG uses invalid compressed data behind a recomputed valid CRC; other formats truncate otherwise valid content.
- Every format checks the complete ordered report: warning/visual, asset details, exact validated MIME despite conflicting `image/png` claims, authoritative escaped file pointer, first-referring element owner, and format/status-specific recipe/recovery.
- File-source order opposes reference and lexical order. Repeated references use distinct IDs and mixed saved/error statuses, proving one file-owned failure with the first reference's recipe.
- Supplemental failures use root pointers and lexical file-ID order, independently of source-reference order and optional media claims.
- Valid scene/supplemental payloads, deleted references and unreferenced supplements are silent. Frame exports exclude an outside opposite-status reference and agree with fresh standalone rendering and batch replay.
- Every codec is also exercised as a failed nested image; nested malformed base64 and shared outer-reference multiplicity are covered. A two-level nested failure asserts the complete node/attribute path and compares its PNG artifact with an explicitly repaired document to establish that surrounding accepted text survives.

The exact selector runs the whole matrix and local helpers. Every report assertion compares full arrays and repeated artifact/report bytes, rather than finding a single expected diagnostic.

## Semantic effects review

Top-level decoder failures occur before a failed raster or SVG can contribute accepted semantic output. Nested raster failures remove only the failed href; accepted surrounding SVG text/links remain, with unsafe-content removal diagnosed separately. The recursive preservation test establishes that boundary. The registry's conditional semantic clause remains authoritative if a future path removes admitted semantic content; these malformed-payload paths do not justify inventing semantic effects for unadmitted content.

This ownership covers the diagnostic contract across decoder families. It does not claim completion of the separate codec-profile matrix, fuzzing or image resource-limit obligations.

Counts: **158 proposed owners / 770 gaps**, including **39 diagnostic owners / 83 diagnostic gaps**. The new three assignments await scoped review; global ownership approval remains pending. Production code and approved baseline/inventory bytes are unchanged.

Verification: the exact combined decode selector passes; `verify-ownership` validates 928 assignments with 158 assigned/excluded and 770 gaps; approved native baseline verification passes; all ten Dagger conformance/lint checks and the Dagger fast tier pass. `git diff --check` passes.
