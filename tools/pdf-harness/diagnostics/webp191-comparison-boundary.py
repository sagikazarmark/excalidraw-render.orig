#!/usr/bin/env python3
"""Independent straight-sample PDF controls for the #191 decision; no gate edits."""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT/'tests/oracle/webp-sample/approved-profile-viewer-diagnosis'
spec = importlib.util.spec_from_file_location('alpha_controls', ROOT/'tools/pdf-harness/pdfium/test_alpha.py')
controls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(controls)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(data)


def project(raw, floor=False):
    return bytes(v for i in range(0,len(raw),4)
                 for v in [*((c*raw[i+3]+(0 if floor else 127))//255 for c in raw[i:i+3]),255])


def main():
    destination = ROOT/'tests/oracle/webp-sample/comparison-boundary-controls'
    destination.mkdir(exist_ok=False)
    reference_bytes = (ROOT/'tests/oracle/webp-sample/reference.json').read_bytes()
    inputs_bytes = (ROOT/'tests/oracle/webp-sample/inputs.json').read_bytes()
    assert sha(reference_bytes)=='b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6'
    assert sha(inputs_bytes)=='21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4'
    reference=json.loads(reference_bytes)
    frames={r['id']:r for r in json.loads(inputs_bytes)['frames']}
    rows=[]
    for row in reference['cases']:
        frame=base64.b64decode(frames[row['first_frame_id']]['native_decoded_rgba']['base64'])
        sub=row['config']['shape']=='subrect'
        width,height,x,y=(5,3,2,2) if sub else (9,7,0,0)
        straight=bytearray(252)
        for j in range(height):
            for i in range(width):
                p=frame[(j*width+i)*4:(j*width+i+1)*4]
                if p[3]==0:p=bytes(4)
                offset=((j+y)*9+i+x)*4
                straight[offset:offset+4]=p
        rgb=bytes(c for i in range(0,252,4) for c in straight[i:i+3])
        alpha=bytes(straight[3::4])
        content=b'q 6.75 0 0 5.25 0 0 cm /I Do Q'
        # The independent library decoder supplies samples, while PDF objects,
        # placement, RGB and mask encoding are authored here without production.
        pdf=controls.pdf([b'<< /Type /Catalog /Pages 2 0 R >>',
            b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
            b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 6.75 5.25] /Resources << /XObject << /I 5 0 R >> >> /Contents 4 0 R >>',
            controls.stream(content),
            controls.stream(rgb,b'/Type /XObject /Subtype /Image /Width 9 /Height 7 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate false /SMask 6 0 R'),
            controls.stream(alpha,b'/Type /XObject /Subtype /Image /Width 9 /Height 7 /ColorSpace /DeviceGray /BitsPerComponent 8 /Interpolate false')])
        folder=destination/row['id']
        source=folder/'independent.pdf'
        put(source,pdf)
        put(folder/'external-straight.rgba',straight)
        observations={}
        for viewer in ['pdfium','pdfjs']:
            outputs=[]
            for repeat in range(2):
                output=folder/f'{viewer}-{repeat}.rgba'
                result=subprocess.run(['bash',f'tools/pdf-harness/run-{viewer}.sh','render',str(source),str(output),'9','7'],cwd=ROOT,capture_output=True,check=True)
                assert not result.stderr
                put(folder/f'{viewer}-{repeat}.json',result.stdout)
                outputs.append(output.read_bytes())
            assert outputs[0]==outputs[1]
            actual=(EVIDENCE/row['id']/f'{viewer}-0.rgba').read_bytes()
            assert outputs[0]==actual, (row['id'],viewer,'independent PDF differs from production')
            if viewer=='pdfium':assert actual==straight
            browser=base64.b64decode(row['reference']['samples']['intrinsic']['rgba_base64'])
            opaque=base64.b64decode(row['reference']['samples']['intrinsic']['composites']['black']['rgba_base64'])
            assert project(straight,True)==opaque
            projected=project(actual)
            canonical_browser=project(browser)
            assert opaque==canonical_browser
            differences=[i//4 for i in range(0,252,4) if projected[i:i+4]!=opaque[i:i+4]]
            delta=max(abs(a-b) for a,b in zip(projected,opaque))
            assert delta<=1
            assert bool(differences)==(row['alpha']=='partial')
            observations[viewer]={'independent_equals_production':True,'actual_sha256':sha(actual),
                'alpha_equal':actual[3::4]==alpha,'black_differing_pixels':len(differences),
                'black_max_channel_delta':delta,'small_artifact_exact_views_pass':not differences}
        rows.append({'id':row['id'],'cell':row['cell'],'alpha':row['alpha'],
                     'independent_pdf_sha256':sha(pdf),'straight_sha256':sha(straight),'viewers':observations})
    bound=['tools/pdf-harness/pdfium/render.py','tools/pdf-harness/pdfium/test_alpha.py',
        'tools/pdf-harness/pdfjs/render.mjs','tools/pdf-harness/run-pdfium.sh','tools/pdf-harness/run-pdfjs.sh',
        'tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json','tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json',
        'tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.json',
        'tests/oracle/webp-sample/approved-profile-viewer-diagnosis/measurements.json',
        'tools/pdf-harness/diagnostics/webp191-comparison-boundary.py']
    record={'schema':'webp191-comparison-boundary-controls/v1','approval':None,
        'inputs_sha256':sha(inputs_bytes),'reference_sha256':sha(reference_bytes),
        'sources':[{'path':p,'sha256':sha((ROOT/p).read_bytes())} for p in bound],
        'files':{str(p.relative_to(destination)):sha(p.read_bytes()) for p in sorted(destination.rglob('*')) if p.is_file()},
        'cases':rows,'scope':'Independent DeviceRGB/SMask PDFs from external straight decoder samples, exact same-grid comparison with production through each approved viewer. No color fitting, resizing, metric exception or replacement reference.'}
    put(destination/'measurements.json',(json.dumps(record,indent=2,sort_keys=True)+'\n').encode())
    print('72 independent PDFs match production in both viewers; 24 partial cases per viewer fail exact views by at most one composite channel unit.')


if __name__=='__main__':main()
