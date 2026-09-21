#!/usr/bin/env python3
"""Prepare exact focused rectangle profile rebinding; approval stays null."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[4]
P=Path(__file__).resolve().parent
PATH='tests/conformance/cases/focused/rectangle-outline/pdf-renderer-expectations.json'
PROFILE='tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json'


def write(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as f:f.write(data)


old=(ROOT/PATH).read_bytes()
write(P/'before'/PATH,old)
value=json.loads(old)
prior_approval=value.pop('approval')
changes=[]
for d in value['files']:
    digest=hashlib.sha256((ROOT/d['path']).read_bytes()).hexdigest()
    if d['sha256']!=digest:
        assert d['path']==PROFILE
        changes.append({'path':PROFILE,'before':d['sha256'],'after':digest})
        d['sha256']=digest
assert len(changes)==1
canonical=lambda v:json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()+b'\n'
subject='sha256:'+hashlib.sha256(canonical(value)).hexdigest()
value['approval']=None
write(P/'pending/focused-pdf-expectations.json',canonical(value))
write(P/'focused-plan.json',canonical({'approval':None,'subject_sha256':subject,'target':PATH,'changes':changes,
    'prior_approval':prior_approval,'scope':'Only PDFium dependency hash and renewed approval; same case, criteria, regions, exception and exact limits. Approval on issue44 required.'}))
print(subject)
