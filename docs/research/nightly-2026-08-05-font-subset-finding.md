# The 2026-08-05 font subset nightly finding

Nightly run `30974954216` (2026-08-05) failed
`render::font_subset_fuzz::nightly_font_subset_fuzz_is_bounded_and_deterministic`
on `d31399a5` at `font_subset_fuzz.rs:318`, the public-facade route's
"render failed outside font subsetting: None", for seed `6a09e667f3bcc908`
case 3482:

```
08248c5f6d1f5d1d19553b2131698fac5d254b53131321574d6995a2d730f431e108068c4bdddefbda9ba4931240aa3389889dc0c9120ab6250df1929756deab0ab6adcca70232e8f16e42bc5773f77c9cddee9f0e00
```

It failed every night until the scheduled runs stopped on 2026-08-14, and no
commit cites it. Recorded during the follow-up to the
[2026-11-01 testing review](review-2026-11-01/fuzz-nightly-ci.md) (ticket D3).

## Reproduction and cause

The input still comes out of the fixed-seed generator as case 3482, and it
reproduces at `d31399a5` on any host (it was re-run on aarch64 macOS). Route 8
decodes the payload into a 22-scalar Helvetica (`fontFamily: 2`) text element
whose last scalar is U+0000. Liberation Sans, Helvetica's sealed substitute,
maps U+0000 to its `.null` glyph, which has metrics and no outline. The text
shaper treated any non-`.notdef`, non-whitespace glyph without an outline as a
corrupt sealed font (`LabelError::InvalidFont`), which surfaced as "the sealed
fallback font could not be resolved" with no error details, so the fuzz
harness, correctly, refused it as an untyped failure. The minimal reproducer
is the text `"A\u0000B"` (a lone `"\u0000"` fails the same way).

## Resolution

Bisecting `d31399a5..bcf754de` with the reproducer finds the first passing
commit `cfa25a2f` ("draw Cascadia calt ligatures through their empty spacer
glyphs", 2026-09-07). It fixed the same rule for a different reason: a glyph
the font defines without an outline now draws nothing and keeps its advance,
and only a glyph with no metrics at all is a sealed-font failure. The parent,
`a1be429f`, still fails. On `main` the case renders, deterministically, with a
qualified subset.

The minimal reproducer is now the public-facade regression case
`nightly-2026-08-05-nul-outlineless-glyph` in
`crates/excalidraw-render/tests/qualification/font-subset-fuzz-regressions.json`,
which every fast-tier run replays twice and requires to render.
