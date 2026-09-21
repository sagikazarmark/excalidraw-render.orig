#!/usr/bin/env python3
"""Independent PDF 1.7 controls for the pinned PDFium bitmap representation."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'target/pdf-harness/pdfium'))
spec = importlib.util.spec_from_file_location('pdfium_adapter', Path(__file__).with_name('render.py'))
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


def pdf(objects):
    """Hand-authored objects and xref; no production PDF writer or renderer."""
    data = bytearray(b'%PDF-1.7\n')
    offsets = []
    for i, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f'{i} 0 obj\n'.encode() + obj + b'\nendobj\n')
    start = len(data)
    data.extend(f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode())
    for offset in offsets:
        data.extend(f'{offset:010} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode())
    return bytes(data)


def stream(data, dictionary=b''):
    return b'<< ' + dictionary + f' /Length {len(data)} >>\nstream\n'.encode() + data + b'\nendstream'


def control(kind):
    # Four disjoint 12x12 device-pixel cells. Cell zero stays transparent;
    # cells one/two/three use alpha 1/128/255 and exact RGB 64,128,192.
    if kind == 'vector':
        resources = b'/ExtGState << /A << /ca 0.003921569 >> /B << /ca 0.501960785 >> >>'
        content = b'0.250980392 0.501960784 0.752941176 rg q /A gs 9 0 9 9 re f Q q /B gs 18 0 9 9 re f Q 27 0 9 9 re f'
        extra = []
    elif kind == 'soft-mask':
        resources = b'/XObject << /I 5 0 R >>'
        content = b'q 27 0 0 9 9 0 cm /I Do Q'
        extra = [stream(bytes([64, 128, 192])*3, b'/Type /XObject /Subtype /Image /Width 3 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate false /SMask 6 0 R'),
                 stream(bytes([1, 128, 255]), b'/Type /XObject /Subtype /Image /Width 3 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Interpolate false')]
    else:
        raise ValueError(kind)
    return pdf([b'<< /Type /Catalog /Pages 2 0 R >>',
                b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
                b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 36 9] /Resources << '+resources+b' >> /Contents 4 0 R >>',
                stream(content), *extra])


class AlphaControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        adapter.configure_pdfium()

    def test_vector_and_soft_mask_preserve_straight_samples(self):
        for kind in ['vector', 'soft-mask']:
            with self.subTest(kind=kind):
                with adapter.pypdfium2.PdfDocument(control(kind)) as document:
                    page = document[0]
                    bitmap = adapter.render_at_exact_scale(page, 48, 12)
                    try:
                        output = adapter.canonicalize(bitmap)
                        expected_row = b''.join((bytes([64, 128, 192, alpha]) if alpha else bytes(4))*12
                                                for alpha in [0, 1, 128, 255])
                        self.assertEqual(output, expected_row*12, kind)
                    finally:
                        bitmap.close()
                        page.close()

    def test_canonicalization_preserves_every_alpha_and_clears_hidden_color(self):
        # Adapter-only representation contract: native rendering above independently
        # establishes that these channels are already straight, including alpha 1.
        class Bitmap:
            mode = 'RGBA'
            width = 256
            stride = width*4
            buffer = bytes(v for a in range(256) for v in [1, 127, 254, a])
        expected = bytes(4) + Bitmap.buffer[4:]
        self.assertEqual(adapter.canonicalize(Bitmap()), expected)

    def test_adapter_process_repeats_without_color_requantization(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'target', prefix='pdfium-alpha-') as directory:
            directory = Path(directory)
            source = directory/'control.pdf'
            source.write_bytes(control('soft-mask'))
            outputs = []
            for i in range(2):
                destination = directory/f'{i}.rgba'
                result = subprocess.run(['bash', 'tools/pdf-harness/run-pdfium.sh', 'render',
                                         str(source), str(destination), '48', '12'],
                                        cwd=ROOT, capture_output=True, check=True)
                self.assertFalse(result.stderr)
                outputs.append((destination.read_bytes(), result.stdout))
            self.assertEqual(outputs[0], outputs[1])
            self.assertEqual(outputs[0][0][(6*48+18)*4:(6*48+18)*4+4], bytes([64, 128, 192, 1]))


if __name__ == '__main__':
    unittest.main()
