#!/usr/bin/env python3
"""Create-only archival of the explicitly external #187 hardening experiment."""
import difflib
from pathlib import Path

HERE=Path(__file__).resolve().parent
destination=HERE/'hardening'
destination.mkdir(exist_ok=False)
original=next((Path.home()/'.cargo/registry/src').glob('*/zune-jpeg-0.5.15'))
patched=Path('/tmp/opencode/jpeg187-zune-probe')
probe=Path('/tmp/opencode/jpeg187-probe')
patch=''
for name in ['mcu.rs','bitstream.rs']:
    patch+=''.join(difflib.unified_diff((original/'src'/name).read_text().splitlines(True),
        (patched/'src'/name).read_text().splitlines(True),fromfile='a/src/'+name,tofile='b/src/'+name))
with (destination/'decoder.patch').open('x') as f:f.write(patch)
(destination/'src').mkdir()
for name in ['main.rs','mixed.rs']:
    data=(probe/'src'/name).read_text()
    data=data.replace('"/home/laborant/excalidraw-render.orig/tests/oracle/jpeg-color/reference.json"',
        'format!("{}/tests/oracle/jpeg-color/reference.json",std::env::var("JPEG187_REPOSITORY").expect("JPEG187_REPOSITORY"))')
    with (destination/'src'/name).open('x') as f:f.write(data)
for name in ['Cargo.toml','Cargo.lock']:
    with (destination/name).open('xb') as f:f.write((probe/name).read_bytes())
with (destination/'browser.cjs').open('xb') as f:
    f.write(Path('/tmp/opencode/jpeg187-mixed-reference/browser.cjs').read_bytes())
