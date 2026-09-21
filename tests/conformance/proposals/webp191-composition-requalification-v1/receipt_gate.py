#!/usr/bin/env python3
"""Strict actual #191 decision, exact three-file migration and successful receipts."""
import sys
if not __debug__ or sys.flags.optimize:raise SystemExit('Optimized Python forbidden; no workflow started.')
from pathlib import Path
import types
path=Path(__file__).with_name('workflow.py');q=types.ModuleType('webp191_workflow');q.__file__=str(path)
exec(compile(path.read_bytes(),str(path),'exec',optimize=0),q.__dict__)
if __name__=='__main__':
    args=sys.argv[1:];gate=q.receipt_gate()
    if args==['verify']:gate.verify(False)
    elif args==['verify-applied']:gate.verify(True)
    elif len(args)==2 and args[0]=='apply':gate.apply(args[1])
    else:raise SystemExit('verify|verify-applied|apply ACTUAL_APPROVAL_JSON')
