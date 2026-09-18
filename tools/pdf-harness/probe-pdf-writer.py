#!/usr/bin/env python3
"""Reproduce #186's bounded API probe with pinned qpdf/PDFium/PDF.js.

Run from any directory after tools/pdf-harness/install.sh. Writes disposable
artifacts/report under target/pdf-writer-probe. Uses the caller's Rust toolchain.
This independent five-object control is not reference-renderer evidence.
"""

import base64
import hashlib
import json
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "target/pdf-writer-probe"
QPDF = ROOT / "target/pdf-harness/qpdf/bin/qpdf"
WIDTH, HEIGHT = 192, 144
CONTENT = b"""q
1 1 1 rg
0 0 144 108 re
f
1 0 0 1 0.30000000000000004 -0.0000000000000001 cm
0.2 0.4 0.8 rg
12.123456789012345 18.000000000000004 96.00000000000001 54.123456789012344 re
f
0 0 0 RG
1.125 w
12 84 m
36 12 72 108 132 24 c
S
Q"""
BOX = b"[0 0 144.00000000000003 108.00000000000001]"


def run(*args):
    try:
        return subprocess.run(
            [str(arg) for arg in args], cwd=ROOT, check=True, capture_output=True
        ).stdout
    except subprocess.CalledProcessError as error:
        sys.stderr.buffer.write(error.stdout + error.stderr)
        raise


def digest(data):
    return hashlib.sha256(data).hexdigest()


def xref_boundary(pdf):
    """Strictly validate only this probe's classic, contiguous five-object graph."""
    end = re.search(rb"\nstartxref\n(\d+)\n%%EOF\n?\Z", pdf)
    assert end, "missing final startxref"
    offset = int(end[1])
    header = b"xref\n0 6\n"
    assert pdf[offset:].startswith(header)
    rows_start = offset + len(header)
    rows = pdf[rows_start : rows_start + 6 * 20]
    assert len(rows) == 120
    assert rows[:20] == b"0000000000 65535 f\r\n"
    for object_id in range(1, 6):
        row = rows[object_id * 20 : (object_id + 1) * 20]
        assert re.fullmatch(rb"\d{10} 00000 n\r\n", row)
        object_offset = int(row[:10])
        assert pdf[object_offset:].startswith(f"{object_id} 0 obj\n".encode())
    boundary = rows_start + len(rows)
    assert pdf[boundary:].startswith(b"trailer\n")
    return offset, boundary


def finish(prefix, xref):
    # Deliberately retained handwritten finalization: no such hook in 0.12.0.
    identifier = digest(prefix)[:32].upper()
    return prefix + (
        f"trailer\n<< /Size 6 /Root 1 0 R /ID [<{identifier}> <{identifier}>] >>"
        f"\nstartxref\n{xref}\n%%EOF\n"
    ).encode()


def adapt_candidate(raw):
    xref, boundary = xref_boundary(raw)
    # No first-occurrence search (XMP deliberately contains 'trailer\n'). No
    # general PDF rewrite: fail if the generated probe's known suffix changes.
    assert raw[boundary:] == (
        f"trailer\n<<\n  /Size 6\n  /Root 1 0 R\n>>\nstartxref\n{xref}\n%%EOF"
    ).encode()
    return finish(raw[:boundary], xref)


def control_pdf(xmp):
    compressed = zlib.compress(CONTENT, level=6)

    def stream(entries, data):
        return (
            b"<< " + entries + f" /Length {len(data)} >>\nstream\n".encode()
            + data + b"\nendstream"
        )

    objects = [
        b"<< /Type /Catalog /Pages 2 0 R /Metadata 5 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << >> /MediaBox "
        + BOX + b" >>",
        stream(b"/Filter /FlateDecode", compressed),
        stream(b"/Type /Metadata /Subtype /XML", xmp),
    ]
    pdf = bytearray(b"%PDF-1.7\n%\x80\x80\x80\x80\n")
    offsets = []
    for object_id, body in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf.extend(f"{object_id} 0 obj\n".encode() + body + b"\nendobj\n")
    xref = len(pdf)
    pdf.extend(b"xref\n0 6\n0000000000 65535 f\r\n")
    for offset in offsets:
        pdf.extend(f"{offset:010} 00000 n\r\n".encode())
    return finish(bytes(pdf), xref)


def inspect(path, xmp):
    pdf = path.read_bytes()
    _, boundary = xref_boundary(pdf)
    identifier = digest(pdf[:boundary])[:32].upper()
    assert f"/ID [<{identifier}> <{identifier}>]".encode() in pdf[boundary:]
    assert b"/MediaBox " + BOX in pdf, "dictionary precision lost"
    check = run(QPDF, "--check", path).decode().replace(str(path), path.name)
    # qpdf exit 3 (warnings) is a failure because run uses check=True.
    document = json.loads(run(QPDF, "--json", "--json-stream-data=inline", path))
    (OUT / f"{path.stem}-qpdf.json").write_text(json.dumps(document, indent=2) + "\n")
    objects = document["qpdf"][1]
    assert set(objects) == {f"obj:{i} 0 R" for i in range(1, 6)} | {"trailer"}
    assert objects["obj:1 0 R"]["value"] == {
        "/Type": "/Catalog", "/Pages": "2 0 R", "/Metadata": "5 0 R"
    }
    assert objects["obj:2 0 R"]["value"] == {
        "/Type": "/Pages", "/Kids": ["3 0 R"], "/Count": 1
    }
    page = objects["obj:3 0 R"]["value"]
    assert page["/Parent"] == "2 0 R" and page["/Contents"] == "4 0 R"
    assert page["/Type"] == "/Page" and page["/Resources"] == {}
    assert len(document["pages"]) == 1
    assert base64.b64decode(objects["obj:4 0 R"]["stream"]["data"]) == CONTENT
    metadata = objects["obj:5 0 R"]["stream"]
    assert base64.b64decode(metadata["data"]) == xmp
    assert metadata["dict"]["/Type"] == "/Metadata"
    assert metadata["dict"]["/Subtype"] == "/XML"
    # Decode independently of qpdf too; it otherwise removes /Filter and /Length
    # from its decoded JSON. Read encoded lengths, never scan binary delimiters.
    for object_id, expected, flate in [(4, CONTENT, True), (5, xmp, False)]:
        header = re.search(
            f"{object_id} 0 obj\n".encode() + rb"(<<.*?>>)\nstream\n", pdf, re.S
        )
        assert header
        dictionary = header[1]
        length = int(re.search(rb"/Length\s+(\d+)", dictionary)[1])
        encoded = pdf[header.end() : header.end() + length]
        assert pdf[header.end() + length :].startswith(b"\nendstream")
        assert (b"/Filter /FlateDecode" in dictionary) == flate
        assert (b"/Filter" in dictionary) == flate
        assert (zlib.decompress(encoded) if flate else encoded) == expected
    assert "/Info" not in objects["trailer"]["value"]
    return {"bytes": len(pdf), "sha256": digest(pdf), "file_id": identifier,
            "qpdf_check": check.strip(), "object_count": 5}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    command = ["cargo", "run", "--locked", "--offline", "-p", "excalidraw-render",
               "--example", "pdf_writer_probe", "--", OUT]
    run(*command)
    raw = (OUT / "candidate-raw.pdf").read_bytes()
    run(*command)  # A second fresh process, not just two calls in one process.
    assert raw == (OUT / "candidate-raw.pdf").read_bytes()
    assert (OUT / "content.bin").read_bytes() == CONTENT
    xmp = (OUT / "metadata.xmp").read_bytes()
    assert b"trailer\n" in xmp
    assert b"pdfaid" not in xmp
    # Parse the stored XML independently and assert the probe's metadata value.
    packet = ET.fromstring(xmp)
    assert packet.find(".//{http://www.w3.org/1999/02/22-rdf-syntax-ns#}li").text == (
        "pdf-writer #186 probe"
    )
    (OUT / "candidate.pdf").write_bytes(adapt_candidate(raw))
    (OUT / "control.pdf").write_bytes(control_pdf(xmp))
    report = {
        "purpose": "bounded serializer API probe, not production qualification",
        "source_revision": run("git", "rev-parse", "HEAD").decode().strip(),
        "cargo_lock_sha256": digest((ROOT / "Cargo.lock").read_bytes()),
        "rustc": run("rustc", "--version").decode().strip(),
        "qpdf": run(QPDF, "--version").decode().splitlines()[0],
        "control_zlib": zlib.ZLIB_RUNTIME_VERSION,
        "dimensions": [WIDTH, HEIGHT],
        "fresh_process_bytes_equal": True,
        "content_sha256": digest(CONTENT), "xmp_sha256": digest(xmp),
        "artifacts": {}, "viewers": {},
    }
    for name in ["control", "candidate"]:
        report["artifacts"][name] = inspect(OUT / f"{name}.pdf", xmp)
    for viewer in ["pdfium", "pdfjs"]:
        buffers = []
        for name, suffix in [("control", "control"), ("candidate", "candidate"),
                             ("candidate", "repeat")]:
            output = OUT / f"{viewer}-{suffix}.rgba"
            run("bash", ROOT / f"tools/pdf-harness/run-{viewer}.sh", "render",
                OUT / f"{name}.pdf", output, WIDTH, HEIGHT)
            buffers.append(output.read_bytes())
        control, candidate, repeat = buffers
        assert len(control) == len(candidate) == len(repeat) == WIDTH * HEIGHT * 4
        assert candidate == repeat, "fresh viewer process mismatch"
        changed = sum(control[i:i+4] != candidate[i:i+4]
                      for i in range(0, len(control), 4))
        assert changed == 0, f"{viewer}: {changed} changed pixels"
        assert len(set(candidate[i:i+4] for i in range(0, len(candidate), 4))) > 2
        report["viewers"][viewer] = {
            "changed_pixels": changed, "rgba_sha256": digest(candidate),
            "fresh_process_bytes_equal": True,
        }
    (OUT / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
