# Rectangle PDF expectation binding refresh

Base: `3ec1a4d`. Status: candidate verified; **explicit #44 reapproval pending**.

## Requested approval subject

`sha256:ad76a6acecc493ef8cd3fb7580be1489c939134c0ecde36fd3547e9398c0a339`

Subject construction matches `fast/pdf_renderer_expectations.rs`: remove the
`approval` field, serialize sorted-key compact JSON, append one LF, then SHA-256.
The candidate's approval is null. The prior approval was
`sha256:6719fc34c9831a4ef3e7af42d5852bba07056d44a7050b28048a9ec0d81499b9`,
recorded at https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5618040464.

## Binding-only change

In `tests/conformance/cases/focused/rectangle-outline/pdf-renderer-expectations.json`:

- Case subject changes from `sha256:9744b816d8689d53db597fddc383b784b64dafe5006ab6374642a3328d2cb6ef`
  to the reapproved `sha256:96638d660453deb84fa5f4d1844fc758c2b40c6c1b01450b0a7929455d816dcc`.
- The manifest dependency digest changes from
  `a7946661383309b971cf1eeb9b48c9aa1cbe1fc747d11ac49c1d27345b7d012d`
  to `3a50d8acb7aaadb8b0d7e70cf677b3ebd8b0916aea992002d9ba2d5ddbaab047`.
- The old approval is cleared rather than reused for the changed subject.

All 13 current dependency hashes verify. A structural before/after comparison
confirms these are the only non-approval changes. Coverage, regions, crops,
expected structure, reference artifacts and reports, viewer/comparator profiles,
and the PDFium exception remain unchanged. The exception still permits zero
changed pixels at its recorded minimum SSIM bits `3fefec69adc2369e`.

## Verification

`dagger api call evidence pdf` passed all nine canonical PDF harness tests,
including rectangle PDFium/PDF.js visual checks and structural inspection, in
18m26s. No rendered artifacts or tolerance records were updated.

The initial concurrent run failed because nested Docker could not create viewer
containers (`no space left on device`). The harness profile now runs one test at
a time. The repeated run retained all nine tests, pinned tools, network isolation,
and comparison rules and passed without retries inside the harness.

The fast approval-binding test remains intentionally blocked by the null approval.
After explicit approval, record the four approval fields with a #44 comment URL,
recompute the subject independently and run the unchanged fast binding test.
This approval is separate from the rectangle SVG/PNG case approval and does not
approve global ownership or waive any remaining release gates.
