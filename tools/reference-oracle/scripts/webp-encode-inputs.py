#!/usr/bin/env python3
"""Offline fixture-only libwebp ABI bridge; JSON stdin/stdout, no file writes."""
import base64
import ctypes as c
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys

PINS = {
    "/lib/x86_64-linux-gnu/libwebp.so.7": "9664db51ec65209078abbb8bc7b699888a43ecc2f3c74ce8dd4e497175aa5bf0",
    "/lib/x86_64-linux-gnu/libsharpyuv.so.0": "b2e62799e5dbd783a0671d53be99f7c6489567a5332a1e8f471e6bb250f294dc",
    "/usr/bin/python3": "e1efa562c2cc2e35521a5c9c9b9939921001ff8ca9708a13ef15ace68cc2ccd7",
}
for path, digest in PINS.items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest, path
assert platform.python_version() == "3.12.3"
lib = c.CDLL("/lib/x86_64-linux-gnu/libwebp.so.7")
assert lib.WebPGetEncoderVersion() == 0x010302
assert lib.WebPGetDecoderVersion() == 0x010302
ptr = c.POINTER(c.c_uint8)
common = [ptr, c.c_int, c.c_int, c.c_int]
lib.WebPEncodeRGBA.argtypes = common + [c.c_float, c.POINTER(ptr)]
lib.WebPEncodeLosslessRGBA.argtypes = common + [c.POINTER(ptr)]
lib.WebPEncodeRGBA.restype = lib.WebPEncodeLosslessRGBA.restype = c.c_size_t
lib.WebPDecodeRGBA.argtypes = [ptr, c.c_size_t, c.POINTER(c.c_int), c.POINTER(c.c_int)]
lib.WebPDecodeRGBA.restype = ptr
lib.WebPFree.argtypes = [c.c_void_p]


def packed(data):
    return {"base64": base64.b64encode(data).decode(), "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


frames = []
for row in json.load(sys.stdin):
    rgba = base64.b64decode(row["rgba_base64"], validate=True)
    width, height = row["width"], row["height"]
    assert len(rgba) == width * height * 4
    source = (c.c_uint8 * len(rgba)).from_buffer_copy(rgba)
    output = ptr()
    args = [source, width, height, width * 4]
    if row["codec"] == "lossless":
        size = lib.WebPEncodeLosslessRGBA(*args, c.byref(output))
    else:
        assert row["codec"] == "lossy"
        size = lib.WebPEncodeRGBA(*args, c.c_float(75), c.byref(output))
    assert size and output
    try:
        encoded = c.string_at(output, size)
        w, h = c.c_int(), c.c_int()
        decoded = lib.WebPDecodeRGBA(output, size, c.byref(w), c.byref(h))
        assert decoded and (w.value, h.value) == (width, height)
        try:
            native = c.string_at(decoded, width * height * 4)
        finally:
            lib.WebPFree(decoded)
    finally:
        lib.WebPFree(output)
    assert native[3::4] == rgba[3::4], row["id"]
    if row["codec"] == "lossless":
        assert all(native[i:i+3] == rgba[i:i+3] for i in range(0, len(rgba), 4) if rgba[i+3])
    frames.append({**row, "encoded": packed(encoded), "native_decoded_rgba": packed(native)})

print(json.dumps({"encoder": {
    "implementation": "libwebp", "version": "1.3.2",
    "distribution_package": subprocess.check_output(["dpkg-query", "-W", "-f=${Version}", "libwebp7:amd64"], text=True),
    "python": "3.12.3", "binaries": PINS,
    "invocation": "/usr/bin/python3 scripts/webp-encode-inputs.py (JSON stdin/stdout)",
    "lossless": "WebPEncodeLosslessRGBA(rgba,width,height,stride=width*4,out); simple API defaults; invisible RGB may be discarded",
    "lossy": "WebPEncodeRGBA(rgba,width,height,stride=width*4,quality_factor=75.0f,out); simple API defaults; exact alpha",
    "native_diagnostic": "WebPDecodeRGBA; same external libwebp, not an independent codec cross-check",
    "independence": "External C libwebp encoder; no production Rust image-webp encoder/decoder or renderer invoked",
    "build_provenance": "Prebuilt Ubuntu package in digest-pinned OCI; no local compiler or network preparation; OCI pins full runtime closure"
}, "frames": frames}))
