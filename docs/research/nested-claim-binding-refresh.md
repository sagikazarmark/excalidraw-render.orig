# Nested-claim downstream binding refresh

Clean tested commit: `73a1560c83eafb68914a9ab548532d1099202baa`.
Candidates committed as `7612116`. Status: **both approvals recorded; PDF binding and packaged replay passing**.

## #44 PDF expectation subject

`sha256:5ee606e25112a4cc9e95c0640bc37731d97190ab9590fad8d34a5b762cec9665`

The rectangle PDF expectations change only their case approval subject to
`sha256:260be471b7371534b6579b6f9fa2c177ef446469968fdfa1d12ba2d6007508e6`
and manifest dependency digest to
`0eb6b9e75c5cb8c5685d17134c66f81d293075e258735342ac3113d9ef4b8aa4`;
the old approval is cleared. All 13 dependency hashes verify. Structural
comparison proves that all other non-approval fields are unchanged, including
coverage, regions, crops, structure expectations and tolerance bindings.

The subject uses the existing PDF verifier's compact sorted-key JSON without
`approval`, followed by one LF. Previous subject:
`sha256:0179d1a610e46190e48abba5289b46d3203acf32685e29565ecb77e4db5f08de`,
approved at https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5654932918.
The prior canonical PDF harness pass remains recorded in the earlier refresh;
this metadata-only candidate does not alter its expectations. Fresh packaged
collection confirms the rectangle PDF artifact/report is unchanged.

## #85 native snapshot subject

`sha256:3296d5a63885e443cd191fcacf6f0924656e81fb434640564a3e753b49b9f890`

`dagger api call packaging snapshots --output target/nested-claim-native-snapshots`
passed in 4m5s. The verifier checked 168 keys, each rendered twice in separate
forward/reverse processes: **672 renders** with clean source and package provenance.

Comparison by `(case, variant, export target, format)` finds no missing or extra
tuple, and **all 168 artifact/report samples match the prior approved baseline**.
All 168 input digests change with the registry/catalog/case bindings. All **187
blobs**, totaling **6,146,996 bytes**, match by both name and actual bytes.

`stage-native-baseline` validated source archives and ancestry and produced the
candidate. Only baseline metadata and its sidecar were installed; output blobs
are untouched and approval is null. `native-baseline-approval-subject` validates
the staged subject. Previous subject:
`sha256:56aa7e51d3990375b31bea1198f2c262871c5a7dbfd59344b2254cdf35c52721`.

## Recorded approvals and checks

The maintainer explicitly approved both exact subjects on 2026-09-14:

- [#44 PDF approval](https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5662705238)
- [#85 baseline approval](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5662705380)

The unchanged PDF binding test and `verify-native-baseline` pass after recording.
`dagger check excalidraw-render:packaging:snapshot-replay` passed in 5m21s on the
working tree based on `7612116`, freshly matching all 168 expectations. These
approvals are separate from the case approvals. `git diff --check` passes.

Global ownership remains 166 proposed owners / 762 gaps. #178 and the outstanding
nested SVG diagnostic coverage are not waived by this binding-only refresh.
