#!/usr/bin/env python3

import hashlib
import struct
import sys
from pathlib import Path


SVG_PATH = [
    "0 0 m 60.36 0.51 117.93 -1.29 180 0 c",
    "0 0 m 46.23 0.2 91.7 -0.37 180 0 c",
    "180 0 m 180.82 21.75 178.03 45.56 180 100 c",
    "180 0 m 178.91 21.23 180.01 41.46 180 100 c",
    "180 100 m 116.02 98.39 52.73 98.26 0 100 c",
    "180 100 m 124.78 101.1 71.18 100.93 0 100 c",
    "0 100 m 0.8 76.02 2.55 50.08 0 0 c",
    "0 100 m -0.56 64.79 -0.37 31.41 0 0 c",
]


def fixed(value: float) -> bytes:
    return struct.pack(">i", round(value * 65536))


def xyz(x: float, y: float, z: float) -> bytes:
    return b"XYZ \0\0\0\0" + fixed(x) + fixed(y) + fixed(z)


def curve(gamma: float) -> bytes:
    return b"curv\0\0\0\0" + struct.pack(">IH", 1, round(gamma * 256)) + b"\0\0"


def icc_profile() -> bytes:
    tags = [
        (b"rXYZ", xyz(0.4360747, 0.2225045, 0.0139322)),
        (b"gXYZ", xyz(0.3850649, 0.7168786, 0.0971045)),
        (b"bXYZ", xyz(0.1430804, 0.0606169, 0.7141733)),
        (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
        (b"rTRC", curve(2.2)),
        (b"gTRC", curve(2.2)),
        (b"bTRC", curve(2.2)),
    ]
    data_offset = 128 + 4 + len(tags) * 12
    table = bytearray(struct.pack(">I", len(tags)))
    payload = bytearray()
    for signature, data in tags:
        while (data_offset + len(payload)) % 4:
            payload.append(0)
        table.extend(signature)
        table.extend(struct.pack(">II", data_offset + len(payload), len(data)))
        payload.extend(data)

    size = 128 + len(table) + len(payload)
    header = bytearray(128)
    header[0:4] = struct.pack(">I", size)
    header[4:8] = b"ERND"
    header[8:12] = b"\x02\x10\0\0"
    header[12:16] = b"mntr"
    header[16:20] = b"RGB "
    header[20:24] = b"XYZ "
    header[24:36] = struct.pack(">6H", 2000, 1, 1, 0, 0, 0)
    header[36:40] = b"acsp"
    header[48:52] = b"ERND"
    header[52:56] = b"sRGB"
    header[68:80] = fixed(0.9642) + fixed(1.0) + fixed(0.8249)
    header[80:84] = b"ERND"
    return bytes(header + table + payload)


def stream(dictionary: bytes, data: bytes) -> bytes:
    return (
        b"<< "
        + dictionary
        + f" /Length {len(data)} >>\nstream\n".encode()
        + data
        + b"\nendstream"
    )


def reference_raster() -> bytes:
    path = (
        Path(__file__).resolve().parents[2]
        / "tests/conformance/svg-viewer-qualification/rectangle-outline/evidence/chromium-reference.rgba"
    )
    data = path.read_bytes()
    if len(data) != 200 * 120 * 4:
        raise RuntimeError("canonical Chromium rectangle raster has invalid length")
    if any(data[index + 3] != 255 for index in range(0, len(data), 4)):
        raise RuntimeError("calibration control requires an opaque reference raster")
    return data


def raster_runs(data: bytes) -> list[str]:
    operations = ["1 1 1 rg", "0 0 200 120 re", "f"]
    for y in range(120):
        x = 0
        while x < 200:
            offset = (y * 200 + x) * 4
            color = data[offset : offset + 3]
            end = x + 1
            while end < 200:
                other = (y * 200 + end) * 4
                if data[other : other + 3] != color:
                    break
                end += 1
            if color != b"\xff\xff\xff":
                red, green, blue = (value / 255 for value in color)
                operations.extend(
                    [
                        f"{red:.12f} {green:.12f} {blue:.12f} rg",
                        f"{x} {y} {end - x} 1 re",
                        "f",
                    ]
                )
            x = end
    return operations


def content(*, visual_failure: bool) -> bytes:
    operations = [
        "q",
        "0.75 0 0 -0.75 0 90 cm",
        "1 1 1 rg",
        "0 0 200 120 re",
        "f",
        "1 0 0 1 10 10 cm",
        "/GS1 gs",
        "2 w",
        "1 J",
        "0.117647059 0.117647059 0.117647059 RG",
        *SVG_PATH,
        "S",
        "1 0 0 1 -10 -10 cm",
        *raster_runs(reference_raster()),
    ]
    if visual_failure:
        operations.extend(["0.8 0 0 rg", "40 30 40 40 re", "f"])
    operations.extend(
        [
        "Q",
        "",
        ]
    )
    return "\n".join(operations).encode()


def build_pdf(*, visual_failure=False, image=False, active_action=False) -> bytes:
    profile = icc_profile()
    resources = b"/ExtGState << /GS1 5 0 R >>"
    if image:
        resources += b" /XObject << /UndeclaredForm 8 0 R >>"
    catalog = b"<< /Type /Catalog /Pages 2 0 R /OutputIntents [7 0 R]"
    if active_action:
        catalog += b" /OpenAction 8 0 R"
    catalog += b" >>"
    objects = [
        catalog,
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 150 90] /CropBox [0 0 150 90] /Resources << "
        + resources
        + b" >> /Contents 4 0 R >>",
        stream(b"", content(visual_failure=visual_failure)),
        b"<< /Type /ExtGState /CA 1 /ca 1 /BM /Normal >>",
        stream(b"/N 3 /Alternate /DeviceRGB", profile),
        b"<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /Info (excalidraw-render project sRGB) /DestOutputProfile 6 0 R >>",
    ]
    if image:
        objects.append(
            stream(
                b"/Type /XObject /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject << /UndeclaredImage 9 0 R >> >>",
                b"",
            )
        )
        objects.append(
            stream(
                b"/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8",
                b"\xff\0\0",
            )
        )
    elif active_action:
        objects.append(b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>")

    return serialize_pdf(objects)


def build_structure_coverage_pdf() -> bytes:
    profile = icc_profile()
    xmp = (
        b'<x:xmpmeta xmlns:x="adobe:ns:meta/">'
        b'<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">'
        b'<rdf:Description xmlns:er="https://excalidraw-render.invalid/manifest/1" '
        b'er:artifact_manifest="{&quot;schema_version&quot;:1}" er:notices="coverage"/>'
        b"</rdf:RDF></x:xmpmeta>"
    )
    to_unicode = b"""/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (ERND) /Ordering (Coverage) /Supplement 0 >> def
/CMapName /Coverage def
/CMapType 2 def
1 begincodespacerange
<0000> <ffff>
endcodespacerange
1 beginbfchar
<0001> <0041>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"""
    page_content = b"q\n0 0 150 90 re\nW\nn\n/MaskGS gs\n/CoverageForm Do\nQ\n"
    form_content = b"/Span << /ActualText (A) >> BDC\nBT\n/F1 12 Tf\n0 0 Td\n<0001> Tj\nET\nEMC\n/SourceImage Do\n"
    mask_content = b"0 g\n0 0 10 10 re\nf\n"
    attachment = b'{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}'
    fake_sfnt = b"\0\x01\0\0\0\x01\0\x10\0\0\0\0ERND\0\0\0\0\0\0\0\x0c\0\0\0\x04test"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R /OutputIntents [6 0 R] /Metadata 11 0 R /Names << /EmbeddedFiles 15 0 R >> >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 150 90] /CropBox [0 0 150 90] /Resources << /ExtGState << /MaskGS 10 0 R >> /XObject << /CoverageForm 7 0 R >> >> /Contents 4 0 R /Annots [14 0 R] >>",
        stream(b"", page_content),
        stream(b"/N 3 /Alternate /DeviceRGB", profile),
        b"<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /Info (excalidraw-render project sRGB) /DestOutputProfile 5 0 R >>",
        stream(
            b"/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Group << /S /Transparency /I true /K false /CS /DeviceRGB >> /Resources << /Font << /F1 8 0 R >> /XObject << /SourceImage 13 0 R >> >>",
            form_content,
        ),
        b"<< /Type /Font /Subtype /Type0 /BaseFont /Coverage /Encoding /Identity-H /DescendantFonts [9 0 R] /ToUnicode 12 0 R >>",
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Coverage /CIDSystemInfo << /Registry (ERND) /Ordering (Coverage) /Supplement 0 >> /FontDescriptor 16 0 R /CIDToGIDMap /Identity >>",
        b"<< /Type /ExtGState /CA 0.75 /ca 0.5 /BM /Multiply /SMask << /S /Alpha /G 17 0 R >> >>",
        stream(b"/Type /Metadata /Subtype /XML", xmp),
        stream(b"", to_unicode),
        stream(
            b"/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask 21 0 R",
            b"\0\x80\xff",
        ),
        b"<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] /Border [0 0 0] /A << /S /URI /URI (https://example.com/) >> >>",
        b"<< /Names [(scene.excalidraw) 18 0 R] >>",
        b"<< /Type /FontDescriptor /FontName /Coverage /Flags 4 /FontBBox [0 -200 1000 800] /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 20 0 R >>",
        stream(
            b"/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Group << /S /Transparency /I true /K false /CS /DeviceGray >> /Resources << >>",
            mask_content,
        ),
        b"<< /Type /Filespec /F (scene.excalidraw) /UF (scene.excalidraw) /Desc (Excalidraw scene) /EF << /F 19 0 R /UF 19 0 R >> >>",
        stream(
            f"/Type /EmbeddedFile /Subtype /application#2Fvnd.excalidraw+json /Params << /Size {len(attachment)} >>".encode(),
            attachment,
        ),
        stream(b"/Length1 32", fake_sfnt),
        stream(
            b"/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            b"\x80",
        ),
    ]
    return serialize_pdf(objects)


def serialize_pdf(objects: list[bytes]) -> bytes:
    output = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for number, obj in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{number} 0 obj\n".encode())
        output.extend(obj)
        output.extend(b"\nendobj\n")
    xref = len(output)
    output.extend(f"xref\n0 {len(objects) + 1}\n".encode())
    output.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode())
    output.extend(
        f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    )
    return bytes(output)


def write(path: Path, data: bytes) -> None:
    path.write_bytes(data)
    print(f"{path.name} {len(data)} {hashlib.sha256(data).hexdigest()}")


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: generate-fixtures.py <output-directory>", file=sys.stderr)
        return 2
    output = Path(sys.argv[1]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    valid = build_pdf()
    write(output / "candidate-control.pdf", valid)
    write(output / "visual-fail-structure-pass.pdf", build_pdf(visual_failure=True))
    write(output / "visual-pass-structure-fail.pdf", build_pdf(image=True))
    write(output / "structure-coverage.pdf", build_structure_coverage_pdf())
    write(output / "active-action.pdf", build_pdf(active_action=True))
    marker = b"startxref\n"
    start = valid.rfind(marker) + len(marker)
    end = valid.find(b"\n", start)
    write(output / "parser-warning.pdf", valid[:start] + b"1" + valid[end:])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
