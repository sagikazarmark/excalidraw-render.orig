# JPEG MPF / XMP gain-map vectors

Project-authored two-image containers with a real MP Index IFD: version 0100,
image count 2, exact MPEntry lengths and TIFF-relative secondary offset. Primary
XMP carries a Container Directory; secondary XMP carries Adobe hdrgm 1.0 metadata.
Independent tests must verify those structures, not merely decoder admission.

Wire-format facts were checked against libultrahdr commit
`f3e36226914289cfefcf0d81a6471cf6266c79fb`, files `lib/src/multipictureformat.cpp`,
`lib/include/ultrahdr/multipictureformat.h`, and `lib/src/jpegrutils.cpp`.
Fixture code is independently authored; no upstream source code is copied.

104 vectors cross baseline/progressive, both MPF byte orders, two base patterns,
identity/nonidentity SDR and HDR-base signaling and eight malformed/absent controls.
This is Adobe XMP/MPF evidence, not ISO 21496-1 or Apple gain-map qualification.

Use `bash tools/reference-oracle/run-jpeg-gain-map.sh generate` for a new file,
or `verify` for read-only, two-process pinned-browser recollection.
