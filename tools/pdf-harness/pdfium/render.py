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
                bitmap = page.render(
                    scale=96 / 72,
                    rotation=0,
                    crop=(0, 0, 0, 0),
                    may_draw_forms=False,
                    fill_color=(0, 0, 0, 0),
                    grayscale=False,
                    optimize_mode=None,
                    draw_annots=False,
                    no_smoothtext=False,
                    no_smoothimage=False,
                    no_smoothpath=False,
                    force_halftone=False,
                    limit_image_cache=False,
                    rev_byteorder=True,
                    force_bitmap_format=pdfium_c.FPDFBitmap_BGRA,
                )
                try:
                    rgba = canonicalize(bitmap)
                    width, height = bitmap.width, bitmap.height
                finally:
                    bitmap.close()
            finally:
                page.close()
        if caught:
            raise RuntimeError(f"PDFium emitted warning: {caught[0].message}")
    if (width, height) != (expected_width, expected_height):
        raise RuntimeError(
            f"PDFium produced {width}x{height}, expected {expected_width}x{expected_height}"
        )

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


def canonicalize(bitmap) -> bytes:
    if bitmap.mode != "RGBA" or bitmap.stride != bitmap.width * 4:
        raise RuntimeError("PDFium did not produce packed top-left RGBA")
    output = bytearray(bitmap.buffer)
    for index in range(0, len(output), 4):
        alpha = output[index + 3]
        if alpha == 0:
            output[index : index + 3] = b"\0\0\0"
        elif alpha != 255:
            for channel in range(3):
                output[index + channel] = min(
                    255, (output[index + channel] * 255 + alpha // 2) // alpha
                )
    return bytes(output)


if __name__ == "__main__":
    raise SystemExit(main())
