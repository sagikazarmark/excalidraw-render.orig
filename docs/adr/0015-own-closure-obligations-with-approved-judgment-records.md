---
status: accepted; owner verification superseded by ADR-0017, approval binding by ADR-0019
---

# Own closure obligations with approved judgment records

The ownership ledger (ADR-0011) gives every obligation one primary owner: a
test, a registered case, or the representative. Four obligations are closure
statements about the corpus as a whole, and none of those owners can carry
them:

- `closure:all-five-layers`;
- `closure:atomic-profile-inventory`;
- `closure:pdf-judgments`;
- `closure:process-target-equality`.

A test can assert a closure statement, but its approval covers only the test's
source, not the judgment the statement rests on. `ownership.rs` requires all
four rows, so they cannot be dropped. They stay gaps even when their evidence
exists. `pdf-judgments.json`, approved on #85, is the first such evidence, and
the row it answers is still a gap. The global ledger approval refuses while any
gap remains, so #85 cannot close.

## Decision

Add a fourth owner kind, `record`, for closure obligations only:

```json
{"kind": "record",
 "record": {"path": "tests/conformance/coverage/pdf-judgments.json", "sha256": "…"},
 "approval_subject_sha256": "sha256:…",
 "verifier": "pdf-judgments"}
```

A record owner is valid only when every check below holds. `verify-ownership`
enforces them, and any failure is stale or unbound ownership:

- **Only closure rows.** The row id starts with `closure:`. No other obligation
  may use a record owner, and a closure row may not use any other kind.
- **The record answers the row.** The record names the obligation it closes
  (`"obligation": "closure:pdf-judgments"`), its approving issue is #85, and
  its bytes match the bound digest.
- **The approval is bound.** The record carries an approval on a #85 comment
  that binds exactly `approval_subject_sha256`, and the record's own subject
  computation reproduces that value.
- **The verifier is compiled in.** `verifier` names a verifier registered in
  xtask by the record's schema, never a shell command. It is run in-process
  with approval required. For `pdf-judgments` it is today's
  `verify-pdf-judgments --require-approval`, which rederives the reports,
  checks every bound digest and the completeness rule, and compares against
  the approved native baseline.

Rebinding a record owner is like rebinding a representative: a changed record
is a reviewed change, and the new binding goes into the ledger through an
applied proposal. Nothing refreshes it automatically.

(The approval binding and the `record-ownership-approval` recorder this
decision introduced are retired by ADR-0019; the ledger's `approval` member
and the records' `approval_subject_sha256` fields are historical metadata.)

## Consequences

- `closure:pdf-judgments` can close on the approved record by adding a
  `record` owner to the ledger.
- Each remaining closure row needs its own approved record with a compiled-in
  verifier:
  - **`all-five-layers`:** a layer-integration record. Every registered
    vector family, fuzz risk, regression corpus and case, in every layer,
    maps to a ledger row.
  - **`atomic-profile-inventory`:** an axis-to-row matrix, which also needs
    #187 and #190.
  - **`process-target-equality`:** the native and WASM gate results, which
    need #178.
- A record owner is exactly as strong as its verifier. A new verifier is
  reviewed as code, and each record's approval covers what it binds.
- `verify-closure` stays an unconditional failure. Its milestones in
  `requirements.json` span #82, #83, #84 and #86 as well as #85. Replacing
  that failure is a separate decision, to take once the milestones have
  evidence.
