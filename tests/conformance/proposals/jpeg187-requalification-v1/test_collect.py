#!/usr/bin/env python3
"""Negative tests over isolated copies; no production or captured bytes edited."""
import contextlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import types
import unittest

HERE=Path(__file__).resolve().parent
c=types.ModuleType('test_collector')
c.__file__=str(HERE/'collect.py')
exec(compile((HERE/'collect.py').read_bytes(),c.__file__,'exec'),c.__dict__)

class SnapshotChecks(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir='/tmp/opencode',prefix='jpeg187-check-')
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.snapshots=self.root/'snapshots'
        shutil.copytree(HERE/'old/snapshots',self.snapshots)
    def write(self,path,value): path.write_text(json.dumps(value))
    def check(self):
        with contextlib.redirect_stdout(io.StringIO()): c.verify_snapshots(self.snapshots)
    def test_actual_old_capture_passes(self): self.check()
    def test_changed_blob_rejected(self):
        blob=next((self.snapshots/'blobs').iterdir()); blob.write_bytes(blob.read_bytes()+b'x')
        with self.assertRaisesRegex(ValueError,'blob mismatch'): self.check()
    def test_wrong_input_identity_rejected(self):
        path=self.snapshots/'snapshots.json'; value=json.loads(path.read_bytes()); value['inventory']['sha256']='0'*64; self.write(path,value)
        with self.assertRaisesRegex(ValueError,'stale snapshot input'): self.check()
    def test_missing_key_in_manifest_and_worker_rejected(self):
        path=self.snapshots/'snapshots.json'; value=json.loads(path.read_bytes()); value['runs'][0]['results'].pop(); self.write(path,value); self.write(self.snapshots/'forward.json',value['runs'][0])
        with self.assertRaisesRegex(ValueError,'missing/changed keys'): self.check()
    def test_reused_process_rejected(self):
        path=self.snapshots/'snapshots.json'; value=json.loads(path.read_bytes()); value['runs'][1]['process_id']=value['runs'][0]['process_id']; self.write(path,value)
        with self.assertRaisesRegex(ValueError,'fresh processes'): self.check()
    def test_package_archive_mutation_rejected(self):
        package=next((self.snapshots/'packages').iterdir()); package.write_bytes(package.read_bytes()+b'x')
        with self.assertRaisesRegex(ValueError,'package archive mismatch'): self.check()
    def test_final_capture_requires_explicit_complete_freeze_before_packaging(self):
        path=self.root/'freeze.json'; self.write(path,{'final_current_capture_authorized':False,'producer_files':{}})
        with self.assertRaisesRegex(ValueError,'explicit parent'): c.final_capture(self.root/'final',path)
        self.write(path,{'final_current_capture_authorized':True,'producer_files':{}})
        with self.assertRaisesRegex(ValueError,'freeze omits'): c.final_capture(self.root/'final',path)
        self.assertFalse((self.root/'final').exists())
    def test_optimized_execution_refuses_before_workflow(self):
        result=subprocess.run([sys.executable,'-O',str(HERE/'collect.py'),'old'],capture_output=True,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1'))
        self.assertNotEqual(result.returncode,0); self.assertIn(b'Optimized Python is forbidden',result.stderr)

if __name__=='__main__': unittest.main(verbosity=2)
