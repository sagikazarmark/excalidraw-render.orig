# Executable ownership validation (#85)

Base: `50f3255963afe69d35712c41649f18ce47165a48`.

## Defect and correction

The ownership consistency gate accepted dead owners because it located module
declarations and `#[test]` functions by line/string matching. A block comment
containing the expected function passed, as did a commented-out module. A
`#[path]` module redirect separated from its declaration by a comment or blank
line could point execution away from the digest-bound source. Conditional or
ignored attributes placed after another item on the same line escaped the
line-prefix check.

Three independent red/green regressions reproduced these failures. The adapter
now parses the binary and owner source with exact-pinned `syn 2.0.119` (already in
the workspace dependency lock; a development-only xtask dependency). It selects
exactly one top-level external module and one top-level test function, rejects
unknown attributes at those boundaries and non-documentation file attributes,
and validates the complete source digest and exact Cargo selector as before.
Comments, string contents and nested scopes cannot satisfy the selector. Valid
Rust formatting, documentation and unrelated tests' attributes are accepted.

The two-axis review found two further holes: imports can shadow the built-in
`test` attribute, and `syn` includes inner function attributes in its attribute
list. A further red/green regression closes both. The adapter requires an outer
marker and rejects potentially shadowing imports, globs, item macro invocations
and macro-use imports instead of attempting macro expansion/name resolution.

This verifies source selection, not execution or assertion adequacy. The
workspace test tier remains responsible for execution; reviewed assignments
remain responsible for the claimed criterion. The producer-only approval policy
continues to exclude consuming-test source hashes from the decision subject.

## Closure status

The ledger still has 928 obligations: 166 assigned and 762 explicit gaps.
`ownership-approval-subject` refuses to approve a completed ledger with gaps,
and `verify-closure` continues to reject the draft. On inspection, prerequisites
#83 (complete-render fuzzing) and #84 (representative corpus) remain open; #178
also blocks the ledger's process/target equality criterion. Global closure needs
the missing evidence and mappings, not an approval-only change.

The user's authorization in this session was “#85 and approve all”. It covers
the implemented verifier correction; no completed-ledger subject can currently
be generated. Existing case, PDF and native baseline subjects are unaffected.

## Verification

- Workspace/all-target typechecking passes.
- All ten ownership adapter tests and the three coverage CLI tests pass.
- All ten canonical Dagger conformance/lint checks pass.
- Full workspace suite passed through `dagger check excalidraw-render:test`
  (fast, render/parity and doctests). The subsequent review-only verifier fixes
  were rechecked with the focused tests and typechecking.
- `/code-review`: Standards found no issues. Spec identified incomplete #85
  delivery and the two attribute-validation defects corrected above.
