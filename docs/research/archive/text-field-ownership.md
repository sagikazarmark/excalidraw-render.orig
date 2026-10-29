# Text field ownership (#85)

Base: `076604a1c72e88dd94123c36819555c0a852e689`.

## Completed assignments

Four public-facade selectors in `fast/text_field_contract.rs` cover the nine
supported text fields in SVG, PNG and PDF: fontSize, fontFamily, textAlign,
verticalAlign, containerId (15 obligations), text/originalText (6), lineHeight
(3), and legacy font (3). The ledger binds exact selectors, source descriptors
and reviewed rationales. Totals: **673 assigned / 255 gaps** (104 surface and
151 other).

The matrices assert defaults, invalid JSON and field-domain recovery, complete
ordered diagnostic fields, preserved original scene bytes, fresh sessions,
selected/unselected frame scope, standalone/lazy equality and deleted silence.
Valid font families and sizes, horizontal/vertical alignment, absent/unresolved
containers and reciprocal/nonreciprocal container participation are exercised.

Positive layout oracles include Cascadia's 1200/2048-em advance: two A glyphs
at 20px occupy 23.4375px inside the authored 60px box. SVG anchors, PDF glyph
matrices and PNG ink bounds distinguish left/center/right. Reciprocal shape
placement and the existing arrow-midpoint semantic test supplement the domain
matrices. SVG WOFF2 subsets are decoded and their cmap checked: extra Z in
originalText is demanded alongside displayed A/B/C/D, while unrelated Q is absent.
PNG/PDF do not consume originalText for painting; embedded originals retain it.

Line-height tests cover all eight source families, zero/positive/negative height,
LF/CRLF/CR and blank wrapped lines. Explicit worked defaults match the pinned
operation order (40 / 3 / 20 is 0.6666666666666667, not a reassociated 2 / 3).
Legacy font tests demonstrate that valid strings override modern fields and
malformed strings fall back to them, including unknown/multiword family names
and exponent size syntax.

## Demonstrated fixes

Invalid containerId and originalText fields were silently recovered without
their required warnings. Two failing facade tests demonstrated the omissions.
Text normalization now emits each field's specified substituted diagnostic;
containerId is read through the private typed source view. Source preservation
and effective recovered rendering are unchanged.

## Review and approval

Two-axis review found no hard Standards violations; the misleading legacy-font
test name was corrected. The Spec review requested direct subset-demand and
placement oracles; both were added and passed. Its final review supports all
27 assignments with no remaining essential findings in scope.

The maintainer's continuing “approve all” / “do it” authorization covers these
assignments and fixes. Global ledger approval remains pending and #85 closure
is incomplete. Targeted tests, workspace/all-target typechecking, and all ten
canonical Dagger conformance/lint checks pass. The final full workspace suite
(`dagger check excalidraw-render:test`) passed fast, render/parity and doctest
tiers in 17m35s.
