# CSS tokenization compatibility (#181)

`cssparser 0.37.0` owns ordinary color-mix function/comma boundaries and static
nested-SVG stylesheet rule/declaration boundaries. Source slices, rather than
reserialized tokens, cross into color evaluation and the pinned `simplecss`
selector/cascade interpreter used by usvg. PDF resolves those selectors on the
original DOM before inserting private effect IDs (ADR-0009).

## Preserved policy

- A mix has exactly three source slices, held in a fixed array. Its stops still
  recurse at most 128 levels. Nested functions must actually close: cssparser's
  EOF recovery is not an admission decision. A bounded component walk allows an
  additional ordinary color function at the deepest stop.
- Weight conversion reads the original spelling into `f64`, never cssparser's
  `f32` percentage value. Half-byte-boundary tests distinguish 49.9999999% from
  50.0000001%. Historical Rust float spellings such as `25.%` remain admitted.
- Stylesheet at-rules, escapes, comment markers, unsafe resources, selector
  colons (including quoted colons), nested rule braces, empty values, and
  disallowed properties retain their existing admission policy.
- Tokenization does not validate property values or add selectors. Invalid
  values that were admitted still reach simplecss/usvg to interpret or ignore.
  Rule ordinals remain source-rule ordinals; selector lists do not increment
  them. Declaration attribution remains that of the downstream interpreter.
- No semantic resource charges change. SVG byte admission precedes this work.
  The stylesheet component walker has a 128-level stack bound and no component
  lists. On its first incompatible rule it switches the remaining sheet to a
  flat path, avoiding repeated whole-suffix CSS recovery and quadratic work.
  Deeper uninterpreted values retain admission via that flat path.
- Before entering nested-block APIs, a shared allocation-free token preflight
  checks cssparser's 16-entry inline cleanup stack. It uses a fresh parser per
  token so the library cannot automatically skip a nested block. Deeper input,
  escapes and NUL replacement use the compatibility path: library cleanup must
  not spill into an infallibly allocated vector, nor may the preflight allocate
  decoded token strings. Thus the library's error path is bounded as well as
  our recursive descent. This guard does not lower the 128-level color contract
  or impose a new stylesheet admission limit.

## Necessary residual scanning

Replacing every raw scan would change the contract. These compatibility paths
are intentional, not additional supported CSS:

1. **Comment-bearing color mixes.** The old delimiter grammar saw punctuation
   inside comments. `currentcolor/*(,)*/` and `currentcolor/*(*/ /*)*/` could be
   stops, whereas `currentcolor/*,*/` could not. A fixed-size, checked-depth
   compatibility scan preserves those boundaries and whitespace positions;
   normal comment-free mixes within the inline-stack guard use the tokenizer.
   The same fallback handles deeper mixes and escaped/NUL spellings that the
   guard cannot pass to allocation-free library cleanup. Ordinary color parsing retains
   its existing comment/escape handling. These examples were verified against
   the starting implementation at `e08fc52`.
2. **Opaque or malformed stylesheet boundaries.** The old admission rules accept
   `rect {fill:rgb(1,2,3}` and an unclosed quoted value before `}`, but reject
   `font-family:"a;b"`. They even admit `font-family:"a;fill:red"`. A tokenizer
   alone would respectively consume a different rule or silently expand the
   subset. `svg/stylesheet.rs` confines the old raw rule/declaration boundaries
   to a compatibility fallback when library boundaries cannot reproduce them.
   Admission and PDF lowering share the same rule iterator. It retains leading
   Unicode characters verbatim: for example, EM SPACE belongs to a selector
   identifier and must not be trimmed as if it were CSS whitespace.
3. **Security and existing inline-style policy.** Raw checks for at-rules,
   escapes, comment markers and nonlocal URLs deliberately remain conservative,
   including inside strings. Inline-style no-op filter handling and URL-target
   extraction are outside the targeted stylesheet-boundary change. They retain
   their existing behavior; the PDF cascade still uses simplecss declarations.

Broadening comment syntax, accepting quoted semicolons generally, rejecting
previously admitted malformed values, or changing downstream URL/selector
interpretation requires separate work with explicit compatibility decisions.

## Verification

The agreed seams are `parse_css` and public rendering (SVG/PNG/PDF artifacts,
reports, failures and resource admission). Tests cover nested functions,
percentage precision, quoted delimiters, malformed/unclosed blocks, rejected
constructs, deep uninterpreted values, exact stylesheet effect causes and
forbidden-fallback errors. The focused tests are also executable in the browser
WASM worker; qualification results are recorded after execution below.

### Completed qualification

Starting commit: `e08fc5214345ed74375ede135b5a10fc2e037fcb`.

- Regular `cargo check --locked --workspace --all-targets` and focused native
  color, nested-SVG and PDF effect tests passed.
- The workspace suite was run once with `cargo test --locked --workspace`:
  385 renderer unit tests, all 535 fast integration tests, parity, resource
  ceilings, CLI and comparator tests passed. Two ownership checks caught the
  placement of the browser configuration macro and an edit to a hash-bound PDF
  test file. The configuration now lives in its own module; the new effect test
  lives in the unbound nested-SVG test module. Ownership checks and the complete
  xtask suite passed on targeted follow-up, including the remaining integration
  and documentation checks. No ownership digest was refreshed.
- The manifest-pinned Rust **1.97.1**, Chrome/ChromeDriver **149.0.7827.55** and
  wasm-bindgen runner **0.2.126** executed the two color tests and three
  stylesheet tests in the dedicated browser worker. These cover precision,
  recursion, admitted/rejected syntax, source attribution, Forbid failures,
  selector behavior, PNG appearance and asset-work admission. The focused
  browser build used unoptimized tests to avoid the cost of whole-library fat
  LTO; release artifact collection below uses the normal release profile.
- `dagger check excalidraw-render:lint:clippy` passed with the pinned toolchain.
  Formatting and diff whitespace checks passed.
- Parallel Standards and Spec reviews against the starting commit found and
  drove fixes for vertical-tab admission, Unicode selector prefixes, library
  cleanup allocation and repeated trailing-whitespace scans. Rechecks closed
  all implementation findings.

The focused browser commands inside the pinned browser container were:

```sh
CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_OPT_LEVEL=0 \
  cargo --config 'profile.dev.package."*".opt-level=0' test --locked \
  --target wasm32-unknown-unknown -p excalidraw-render --lib color::tests
CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_OPT_LEVEL=0 \
  cargo --config 'profile.dev.package."*".opt-level=0' test --locked \
  --target wasm32-unknown-unknown -p excalidraw-render --test fast stylesheet_
```

Final release-profile packaged collections:

```sh
dagger api call packaging snapshots --output target/issue-181-checked-native-snapshots
dagger api call wasm browser-snapshot-logs --output target/issue-181-checked-wasm-snapshots
```

Both collections cover **168 keys**, two renders per key in each of two fresh
forward/reverse processes (**672 renders per target**). Every ordered native
key/sample array equals the saved `target/issue-180-results/native-snapshots`
collection. Browser `runs` and `differences` arrays likewise equal the saved
`target/issue-180-results/wasm-snapshots/replay.json`. No artifact or report delta
was introduced on either target. Temporary Dagger hooks allowed working-tree
packaging and focused browser execution; they were removed afterward, and the
metadata checks binding `dagger.dang` passed. These are working-tree
qualification results, not clean-commit release attestations.

**The approved-baseline equality gate is still not green.** Direct comparison
between the current targets retains exactly #178's four PNG keys:
`freedraw-loop-fills`, `legacy-stroke-sharpness`, `rotated-shape-bounds`, and
`rough-fillers-dashed-zigzag-line`. All 168 reports agree across targets. The
three additional stale approved expectations already recorded under #85 are
unchanged on both targets: image-codecs PNG/PDF and transform-opacity-groups
no-background PNG. See the
[pre-existing measurements](research/ecmascript-number-formatting.md#packaged-artifactreport-results).
No approved expectation or tolerance was repinned.
