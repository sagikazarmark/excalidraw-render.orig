# JPEG EXIF precedence vectors

164 project-authored vectors / 82 cells over frozen asymmetric RGB JPEG sources:
baseline/progressive, TIFF byte orders, duplicate APP1 and IFD orientation tags,
invalid/absent-tag controls, markers before EOI and between progressive scans.
The controls distinguish identity, 180°, 90° and 270° orientations.

Generate only a new record with `bash tools/reference-oracle/run-jpeg-exif.sh generate`.
Read-only recollection uses `verify`. Both modes collect in two fresh pinned
Chromium processes, opposite order, fresh contexts, with network disabled.
