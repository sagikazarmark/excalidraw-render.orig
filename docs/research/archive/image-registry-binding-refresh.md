# Image registry downstream binding refresh

Base and clean tested commit: `3836a21f4fe222327c07bb6a8b23ab0be4c4c6f1`.
Candidates committed as `b30412c`. Status: **both approvals recorded; PDF binding and packaged replay passing**.

## #44: rectangle PDF expectation subject

`sha256:0179d1a610e46190e48abba5289b46d3203acf32685e29565ecb77e4db5f08de`

`rectangle-outline/pdf-renderer-expectations.json` changes only:

- case approval subject to `sha256:a0911188e9a091595e3ef636aed992c247b5c52e05c096abe5792b4153c30cba`;
- the case manifest byte digest to `e471038d87bca7f06c081283daa99df53e1a99ffb27d31416b27b76576d960ad`;
- clears the previous approval before requesting the new one.

All 13 dependency digests verify. Structural comparison with the base proves
that coverage, regions, crops and every other nonbinding field are unchanged.
The expected structure, visual tolerance, reference artifacts and reports are
byte-identical. Subject construction is the existing fast verifier's compact
sorted-key JSON without `approval`, followed by one LF, then SHA-256.

The previous #44 subject was `sha256:ad76a6acecc493ef8cd3fb7580be1489c939134c0ecde36fd3547e9398c0a339`,
approved at https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5653717088.
The prior nine-test canonical PDF harness pass remains recorded in
[the preceding binding refresh](rectangle-pdf-binding-refresh.md); it was not
rerun for this metadata-only candidate. The fresh native collection below
independently confirms that its rectangle PDF artifact/report is unchanged.

## #85: native snapshot baseline subject

`sha256:56aa7e51d3990375b31bea1198f2c262871c5a7dbfd59344b2254cdf35c52721`

`dagger api call packaging snapshots --output target/image-registry-native-snapshots`
passed in 3m44s. The verifier checked 168 keys with two sessions in each of fresh
forward/reverse processes: **672 renders**, clean source and packaged provenance.

Comparing by `(case, variant, export target, format)`, all **168 artifact/report
pairs are byte-identical** to the approved baseline. Every input-key digest is
refreshed by the approved image registry/catalog/case bindings, with no missing
or extra case/variant/target/format tuple. All **187 blobs**, totaling
**6,146,996 bytes**, compare identical by name and actual bytes.

`stage-native-baseline` verified archive/source provenance and Git ancestry and
created the new baseline candidate. Only its JSON metadata and digest sidecar
were installed; existing output blobs were untouched. Approval is null rather
than transferred from the preceding subject
`sha256:f6e6a9e6a538b60b088038d19586c943b92166a18f5a9dee8a1b68c37984c957`.
The repository `native-baseline-approval-subject` validator confirms the new
subject above.

## Recorded approvals and verification

The maintainer explicitly approved both exact subjects in the coding session
on 2026-09-13. Approval records:

- [#44 PDF expectations](https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5654932918)
- [#85 native baseline](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5654932917)

The unchanged PDF approval/dependency binding test passes, and
`verify-native-baseline` validates the recorded approval. The canonical
`dagger check excalidraw-render:packaging:snapshot-replay` passed in 5m9s,
freshly matching all 168 approved artifact/report expectations on the working
tree based on `b30412c`. Case reapproval and these downstream approvals are
separate scoped records.

`git diff --check` passes. Global ownership remains 149 proposed owners / 779
gaps; this refresh does not waive #178 or the remaining diagnostic branches.
