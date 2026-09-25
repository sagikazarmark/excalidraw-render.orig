---
status: accepted
---

# Own coverage with independently scheduled test groups

The unsupported-property-value owner was an umbrella test that serially called
33 routines, 31 already registered as tests. Measuring it in isolation took
13m30s locally; it repeated coverage that the ordinary suite already executed.
Represent that one primary owner as a digest-bound **test group**, whose members
are unconditional, independently scheduled tests. Register the two formerly
private routines as tests and remove the umbrella and its dispatch helpers.

The group manifest fixes the package, integration-test binary and exact member selectors.
`verify-ownership` checks its digest, nonempty unique membership, and every
member's actual Rust test declaration through the existing executable resolver.
The normal nextest tiers execute the members once; group validation does not run
them again. An exact nextest filter is available for focused execution.

This extends executable ownership, not ADR-0015's approved judgment records:
test groups cannot own closure or tolerance-exception obligations. The existing
gap/exclusion rules and rendering expectations are unchanged. Moving or changing
membership is a reviewable manifest/owner edit; removing, ignoring or disabling
a member fails validation.
