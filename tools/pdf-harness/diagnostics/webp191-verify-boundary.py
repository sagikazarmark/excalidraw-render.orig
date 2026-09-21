#!/usr/bin/env python3
"""Read-only evidence and negative checks for the unapproved #191 decision."""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[3]
BASE=ROOT/'tests/oracle/webp-sample'
CONTROL=BASE/'comparison-boundary-controls'
CURRENT=BASE/'approved-profile-viewer-diagnosis'


def sha(data):return hashlib.sha256(data).hexdigest()


def require(value,message):
    if not value:raise ValueError(message)


def views(raw):
    require(len(raw)==252,'9x7 RGBA expected')
    alpha=bytes(v for a in raw[3::4] for v in [a,a,a,255])
    black=bytes(v for i in range(0,252,4) for v in [*((c*raw[i+3]+127)//255 for c in raw[i:i+3]),255])
    white=bytes(v for i in range(0,252,4) for v in [*((c*raw[i+3]+255*(255-raw[i+3])+127)//255 for c in raw[i:i+3]),255])
    return alpha,black,white


def independently_expected(source):
    inputs=json.loads((BASE/'inputs.json').read_bytes())
    frame=next(r for r in inputs['frames'] if r['id']==source['first_frame_id'])
    decoded=base64.b64decode(frame['native_decoded_rgba']['base64'])
    sub=source['config']['shape']=='subrect'
    width,height,x,y=(5,3,2,2) if sub else (9,7,0,0)
    out=bytearray(252)
    for j in range(height):
        for i in range(width):
            p=decoded[(j*width+i)*4:(j*width+i+1)*4]
            start=((y+j)*9+x+i)*4
            out[start:start+4]=p if p[3] else bytes(4)
    return bytes(out)


def gate(candidate,independent,source,viewer,measurement):
    require(candidate==independent,'candidate must equal independent PDF viewer output exactly')
    require(all(candidate[i+3] or candidate[i:i+3]==bytes(3) for i in range(0,len(candidate),4)),'invisible RGB must be zero')
    straight=independently_expected(source)
    require(candidate[3::4]==straight[3::4],'alpha must equal independently authored alpha')
    if viewer=='pdfium':require(candidate==straight,'PDFium must preserve external straight samples')
    browser=base64.b64decode(source['reference']['samples']['intrinsic']['rgba_base64'])
    a,b,w=views(candidate)
    ra,rb,rw=views(browser)
    require(a==ra,'alpha view mismatch')
    require((b==rb and w==rw)==(source['alpha']!='partial'),'canonical outcome changed')
    if source['alpha']=='partial':
        # Both viewers produce the same rounded black/white views in this bounded
        # matrix; their raw straight readbacks are deliberately not equated.
        require((b,w)==views(straight)[1:],'unexplained quantization mechanism')
        require(b!=rb and w!=rw,'residual must remain an actual failure')
    require(sha(candidate)==measurement['actual_sha256'],'observed viewer hash moved')


def main():
    record=json.loads((CONTROL/'measurements.json').read_bytes())
    require(sha((CONTROL/'measurements.json').read_bytes())=='36dc6cff0076b013b6086be1fbe6290392440f6a6c9676e73e9067ae4b89e079','control record changed')
    current=json.loads((CURRENT/'measurements.json').read_bytes())
    require(sha((CURRENT/'measurements.json').read_bytes())=='6d47db1399e8c857b8f402d7604d21bd0ed8474f4422fa2128bcab6a40833879','viewer record changed')
    require(sha((ROOT/'tools/pdf-harness/diagnostics/webp191-partial-viewers.py').read_bytes())==current['script_sha256'],'collector source changed')
    require(sha((ROOT/'target/release/excalidraw-render').read_bytes())==current['cli_sha256'],'native CLI identity changed')
    env=dict(os.environ,PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'],CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0')
    subprocess.run(['cargo','run','--locked','-p','xtask','--','conformance','verify-pdf-viewers'],cwd=ROOT,env=env,check=True,capture_output=True)
    for directory,manifest in [(CONTROL,record),(CURRENT,current)]:
        for path,digest in manifest['files'].items():require(sha((directory/path).read_bytes())==digest,path)
    for d in record['sources']:require(sha((ROOT/d['path']).read_bytes())==d['sha256'],d['path'])
    for path,key in [('reference.json','reference_sha256'),('inputs.json','inputs_sha256')]:
        require(sha((BASE/path).read_bytes())==record[key],path)
    reference=json.loads((BASE/'reference.json').read_bytes())
    sources={r['id']:r for r in reference['cases']}
    require(len(record['cases'])==len(sources)==72,'vector inventory')
    rejected=0
    paired=0
    for row in record['cases']:
        source=sources[row['id']]
        for viewer in ['pdfium','pdfjs']:
            candidate=(CURRENT/row['id']/f'{viewer}-0.rgba').read_bytes()
            independent=(CONTROL/row['id']/f'{viewer}-0.rgba').read_bytes()
            observation=row['viewers'][viewer]
            gate(candidate,independent,source,viewer,observation)
            # Reject color drift, alpha drift and replacing real output with the
            # browser's values merely to make a failing comparison pass.
            color=bytearray(candidate);color[0]^=1
            alpha=bytearray(candidate);alpha[3]^=1
            bads=[bytes(color),bytes(alpha)]
            if source['alpha']=='partial':
                bads.append(base64.b64decode(source['reference']['samples']['intrinsic']['rgba_base64']))
            for bad in bads:
                try:gate(bad,independent,source,viewer,observation)
                except ValueError:rejected+=1
                else:raise ValueError('mutation passed')
            # Change both sides to bypass the first equality guard deliberately.
            # Rebinding the observation hash cannot bypass semantic constraints.
            for bad in bads:
                observed=dict(observation,actual_sha256=sha(bad))
                try:gate(bad,bad,source,viewer,observed)
                except ValueError:paired+=1
                else:raise ValueError('paired semantic mutation passed')
    print(f'144 independent viewer comparisons verified; {rejected} independent-buffer and {paired} paired semantic mutations rejected; 48 partial viewer judgments remain canonical failures.')


if __name__=='__main__':main()
