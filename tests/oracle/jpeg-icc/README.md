# JPEG ICC reference vectors

Project-authored ICC v2/v4 RGB matrix/TRC and grayscale TRC profiles with gamma
1 and 2.2, carried by baseline/progressive RGB and gray JPEG. Each profile uses
one APP2 segment or three header-splitting segments in forward/reverse order.
Invalid profiles, mismatched models and malformed multipart sequences are
separate controls. These 160 vectors are bounded evidence, not full ICC coverage.

Collection: `bash tools/reference-oracle/run-jpeg-icc.sh generate` (new file only).
Verification: `bash tools/reference-oracle/run-jpeg-icc.sh verify` (read-only).
Every collection runs two fresh pinned browser processes in opposite orders.
