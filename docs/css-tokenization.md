# CSS tokenization compatibility (#181)

`cssparser` owns ordinary color-mix function/comma boundaries and static
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
forbidden-fallback errors. The focused tests also run in the browser WASM
worker (`just qualification wasm32-unknown-unknown`), and the packaged
snapshot replays on both targets (`just snapshot-replay`,
`just wasm-snapshot-replay`) showed no artifact or report delta from the
change.
