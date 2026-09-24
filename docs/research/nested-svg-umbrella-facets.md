# Nested SVG umbrella facets (#85)

This closes the three nested-SVG umbrella facets: sanitization, references
and pdf-fallback.

## Reference behavior

Excalidraw draws an SVG image through an `<img>`. Chromium 149 renders it as
an SVG image, which runs no scripts and loads nothing external. The PNG export
draws it onto the canvas.

A 54-variant probe compared that drawing with the renderer. Scripts, event
handlers, `javascript:` links, stylesheet processing instructions, external
entities, `use` and `xml:base` never took effect in either. Several other
cases showed differences:

- **Bugs, now fixed:**
  - A `use` cycle failed the whole image. The reference draws the rest, and
    spec section 13 already required the cycle to be cut.
  - Removing `@import` or `@font-face` dropped the whole stylesheet.
  - An external `url()` paint lost its fallback color, and drew black where
    the reference draws none.
- **Policy differences:** the renderer removed internal entities without
  expanding them, and did not admit percent-encoded data images.
- **Crashes in cycle handling:** the rewritten cycle pass fixed a gradient
  `href` 3-cycle that hung the renderer. It also fixed three-element mask,
  clip and pattern cycles, and filters that reference each other through
  `feImage`, all of which overflowed the stack. Cycles closed through
  `<style>` rules, including `!important` ones and values inherited from a
  rule-styled ancestor, now join the same graph. As a backstop, the sanitized
  output is checked again before the viewer library parses it, and any cycle
  still left rejects the image as decode-failed instead of recursing.

## Policy (chosen by the user on #85)

Fix the bugs, and match the reference wherever the result stays static and
local:

- Internal entities expand, within a documented amplification bound. An
  undefined entity or an expansion bomb rejects the image, as it does in the
  reference.
- A non-fragment `url()` resolves as a missing fragment.
- A removed at-rule leaves the stylesheet's other rules in place.
- A reference cycle loses its closing edge in source order.
- Percent-encoded data images are admitted under the same bounds as base64
  ones.

Three differences are declared divergences:

- the animation state at the time the reference draws;
- `foreignObject` content;
- the reference's own placeholder for an external image.

The renderer draws the static document, removes `foreignObject`, and paints
its forbidden-resource placeholder. Spec section 8.2 states these rules.
`render/asset/svg/entities.rs` and `render/asset/svg/references.rs` implement
them, and their module docs give the bounds.

## Evidence

`tests/oracle/nested-svg/reference.json` holds 54 rows. It is collected by
`tools/reference-oracle/run-nested-svg.sh` in two fresh processes, forward
and reverse, with a fresh context per case and no network. Each row is a
24×24 SVG drawn at its natural size and declares one of three expectations:

- **`match`:** the PNG passes the canonical comparison against the reference.
  Gradients and nested images differ from Skia by at most 5 channel units,
  within the budget. SVG and PDF must report exactly what PNG reports, except
  PDF's raster fallback for `feImage`.
- **`rejected`:** every format recovers exactly as a missing asset.
- **`divergent`:** PNG and PDF must equal a static twin row exactly. The test
  also checks that the reference still draws the divergent row differently
  from its twin. The external-image row pins its placeholder.

Text rows are left out, because text through nested fonts is outside this
tranche.

## pdf-fallback

The vector-first PDF rules have no reference; spec section 8.6 is the only
authority. `pdf_raster_fallback_contract` already held exact reports,
isolated artifacts, forbid failures and silence in SVG and PNG for ten effect
families. It now also covers drop shadow, offset, flood, composite, merge and
tile. The native matrix (`tests/conformance/pdf-backend-qualification/nested-native-operations.excalidraw`)
already covers spread-mode gradients, patterns, masks, clips, markers and all
sixteen native blend modes under `Forbid`.

## Owners

| Facet | Owner |
| --- | --- |
| sanitization | `nested_svg_reference_contract::nested_svg_sanitization_matches_the_reference_where_it_stays_static` |
| references | `nested_svg_reference_contract::nested_svg_references_match_the_reference` |
| pdf-fallback | `pdf_raster_fallback_contract::raster_fallback_contract_covers_families_occurrences_geometry_and_failures` |
