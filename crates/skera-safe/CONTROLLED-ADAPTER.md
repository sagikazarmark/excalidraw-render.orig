# Controlled skera adapter

This crate preserves the frozen font subset profile's exact `skera 0.5.1`
implementation and invocation. For profile-admitted fonts it delegates directly
to `skera::subset_font` and does not alter plans or output bytes.

Issue #100 found that `skera 0.5.1` panics when a parsed `name` table declares a
record array beyond the table bytes. The adapter checks that structural
precondition and returns skera's existing `SubsetTableError(name)` instead. This
is an input-safety guard for malformed fonts, not a new subset profile setting;
the complete 230-source qualification corpus proves output bytes and resource
traces remain unchanged.
