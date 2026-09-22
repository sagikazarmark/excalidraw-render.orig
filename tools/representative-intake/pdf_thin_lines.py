"""Measure how the pinned PDF viewers rasterize sub-pixel strokes (#84).

Hand-authored single-stroke PDFs, independent of the renderer: one horizontal
black line on a 75x75 pt page (100x100 px at 96 DPI) per requested device width.
Reports the alpha coverage each viewer paints across the line at column 50.
"""
import json
from pathlib import Path
import subprocess
import sys

import intake

ROOT = Path(__file__).resolve().parents[2]
WIDTHS_PX = [0.125, 0.25, 0.5, 0.75, 1, 1.5, 2]


def document(width_pt):
    content = f"0 0 0 RG {width_pt:g} w 7.5 37.3 m 67.5 37.3 l S".encode()
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 75 75] /Contents 4 0 R /Resources << >> >>",
               b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream"]
    out, offsets = b"%PDF-1.7\n", []
    for number, body in enumerate(objects, 1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % number + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1) + b"".join(b"%010d 00000 n \n" % o for o in offsets)
    return out + b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)


def main():
    output = Path(sys.argv[1])
    intake.require(len(sys.argv) == 2, "usage: pdf_thin_lines.py <new-directory-under-repository>")
    output.resolve().relative_to(ROOT)
    output.mkdir()
    rows = []
    for width in WIDTHS_PX:
        name = f"stroke-{width:g}px".replace(".", "_")
        pdf = output / f"{name}.pdf"
        pdf.write_bytes(document(width * 0.75))
        row = {"device_width_px": width, "pdf": {"file": pdf.name, **intake.descriptor(pdf.read_bytes())}, "viewers": {}}
        for viewer in ("pdfium", "pdfjs"):
            raster = output / f"{name}-{viewer}.rgba"
            subprocess.run(["bash", f"tools/pdf-harness/run-{viewer}.sh", "render", str(pdf.relative_to(ROOT) if pdf.is_absolute() else pdf),
                            str(raster), "100", "100"], cwd=ROOT, check=True, capture_output=True, timeout=300)
            data = raster.read_bytes()
            raster.unlink()
            column = [(y, data[(y * 100 + 50) * 4 + 3]) for y in range(100) if data[(y * 100 + 50) * 4 + 3]]
            row["viewers"][viewer] = {"alpha_rows": column, "coverage_px": round(sum(a for _, a in column) / 255, 4),
                                      "raster": intake.descriptor(data)}
        rows.append(row)
    record = {"schema": "excalidraw-render/pdf-thin-line-controls/v1", "approval": None, "rows": rows}
    (output / "thin-lines.json").write_bytes(intake.encode(record))
    for row in rows:
        print(row["device_width_px"], {v: r["coverage_px"] for v, r in row["viewers"].items()})


if __name__ == "__main__":
    main()
