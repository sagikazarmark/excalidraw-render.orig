#!/usr/bin/env python3

import hashlib
import json
import sys
import warnings
import ctypes
from pathlib import Path

import pypdfium2
import pypdfium2._library_scope as library_scope
import pypdfium2.internal as pdfium_i
import pypdfium2.raw as pdfium_c
from pypdfium2.version import PDFIUM_INFO, PYPDFIUM_INFO

_FONT_PATH = None
_FONT_PATHS = None
_LIBRARY_CONFIG = None
SCALE = 96 / 72


def main() -> int:
    if len(sys.argv) != 5:
        print("usage: render.py <input.pdf> <output.rgba> <width> <height>", file=sys.stderr)
        return 2
    input_path = Path(sys.argv[1]).resolve(strict=True)
    output_path = Path(sys.argv[2]).resolve()
    expected_width = int(sys.argv[3])
    expected_height = int(sys.argv[4])
    if output_path.exists() and output_path.is_symlink():
        raise RuntimeError("output path must not be a symlink")

    configure_pdfium()
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("error")
        with pypdfium2.PdfDocument(input_path) as document:
            if len(document) != 1:
                raise RuntimeError("PDFium profile accepts exactly one page")
            if document.get_version() != 17:
                raise RuntimeError("PDFium profile accepts PDF 1.7 only")
            page = document[0]
            try:
                if page.get_rotation() != 0:
                    raise RuntimeError("PDFium profile rejects page rotation")
                bitmap = render_at_exact_scale(page, expected_width, expected_height)
                try:
                    rgba = canonicalize(bitmap)
                    width, height = bitmap.width, bitmap.height
                finally:
                    bitmap.close()
            finally:
                page.close()
        if caught:
            raise RuntimeError(f"PDFium emitted warning: {caught[0].message}")

    output_path.write_bytes(rgba)
    print(
        json.dumps(
            {
                "byte_length": len(rgba),
                "height": height,
                "layout": "top-left-row-major",
                "pdfium": str(PDFIUM_INFO),
                "pixel_format": "straight-alpha-srgb-rgba8",
                "pypdfium2": str(PYPDFIUM_INFO),
                "sha256": hashlib.sha256(rgba).hexdigest(),
                "width": width,
            },
            separators=(",", ":"),
            sort_keys=True,
        )
    )
    return 0


def configure_pdfium() -> None:
    global _FONT_PATH, _FONT_PATHS, _LIBRARY_CONFIG
    font_directory = Path(__file__).with_name("empty-fonts").resolve(strict=True)
    entries = sorted(path.name for path in font_directory.iterdir())
    if entries != [".keep"]:
        raise RuntimeError("PDFium font directory contains undeclared entries")
    keep_digest = hashlib.sha256((font_directory / ".keep").read_bytes()).hexdigest()
    if keep_digest != "214f294fd6c8df5b4b53d84fd437aee8c30edbee74b929863e40ec2e6b3cf621":
        raise RuntimeError("PDFium empty font directory marker is stale")
    library_scope.destroy_lib()
    _FONT_PATH = ctypes.create_string_buffer(str(font_directory).encode())
    _FONT_PATHS = (ctypes.POINTER(ctypes.c_char) * 2)()
    _FONT_PATHS[0] = ctypes.cast(_FONT_PATH, ctypes.POINTER(ctypes.c_char))
    _FONT_PATHS[1] = ctypes.POINTER(ctypes.c_char)()
    _LIBRARY_CONFIG = pdfium_c.FPDF_LIBRARY_CONFIG(
        version=2,
        m_pUserFontPaths=_FONT_PATHS,
        m_pIsolate=None,
        m_v8EmbedderSlot=0,
    )
    pdfium_c.FPDF_InitLibraryWithConfig(_LIBRARY_CONFIG)
    pdfium_i.LIBRARY_AVAILABLE.value = True


def render_at_exact_scale(page, expected_width: int, expected_height: int):
    """Render the page at exactly 96/72 scale onto a bitmap of the supplied size.

    The page origin sits at the bitmap origin; the bitmap must lie within one
    device pixel of the page's 96 DPI size. Device pixels beyond the bitmap are
    discarded and bitmap pixels the page does not cover stay transparent. The
    page is never scaled, stretched, or offset to fit the bitmap, which rules
    out ``PdfPage.render``: it sizes its bitmap with ``ceil`` and lets
    ``FPDF_RenderPageBitmap`` stretch a fractional page onto it.
    """
    if expected_width < 1 or expected_height < 1:
        raise RuntimeError("expected dimensions must be positive integers")
    device_width = page.get_width() * SCALE
    device_height = page.get_height() * SCALE
    if abs(device_width - expected_width) >= 1 or abs(device_height - expected_height) >= 1:
        raise RuntimeError(
            f"PDFium page is {device_size(device_width)}x{device_size(device_height)} px "
            f"at 96 DPI, expected {expected_width}x{expected_height}"
        )
    bitmap = pypdfium2.PdfBitmap.new_native(
        expected_width,
        expected_height,
        format=pdfium_c.FPDFBitmap_BGRA,
        rev_byteorder=True,
    )
    bitmap.fill_rect((0, 0, 0, 0), 0, 0, expected_width, expected_height)
    matrix = pdfium_c.FS_MATRIX(SCALE, 0, 0, SCALE, 0, 0)
    clip = pdfium_c.FS_RECTF(0, 0, expected_width, expected_height)
    pdfium_c.FPDF_RenderPageBitmapWithMatrix(
        bitmap, page, matrix, clip, pdfium_c.FPDF_REVERSE_BYTE_ORDER
    )
    return bitmap


def device_size(value: float) -> str:
    """Format a 96 DPI dimension like the PDF.js adapter's number formatting:
    whole pixels without a fraction, fractional pixels at full precision. The
    value itself comes from PDFium's binary32 page size."""
    return str(int(value)) if value.is_integer() else repr(value)


def canonicalize(bitmap) -> bytes:
    if bitmap.mode != "RGBA" or bitmap.stride != bitmap.width * 4:
        raise RuntimeError("PDFium did not produce packed top-left RGBA")
    output = bytearray(bitmap.buffer)
    for index in range(0, len(output), 4):
        alpha = output[index + 3]
        if alpha == 0:
            output[index : index + 3] = b"\0\0\0"
        # This pinned PDFium backend writes straight BGRA (swizzled to RGBA
        # by FPDF_REVERSE_BYTE_ORDER), not premultiplied canvas samples.
        # Unpremultiplying again corrupts translucent colors. Independent
        # vector-opacity and image-soft-mask controls pin this representation.
    return bytes(output)


if __name__ == "__main__":
    raise SystemExit(main())
