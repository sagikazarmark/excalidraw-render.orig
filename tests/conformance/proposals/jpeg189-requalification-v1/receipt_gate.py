#!/usr/bin/env python3
"""Strict receipt-producing apply/read-only verification for actual #189 approval."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')
from pathlib import Path
import types

path=Path(__file__).with_name('workflow.py')
module=types.ModuleType('jpeg189_workflow');module.__file__=str(path)
exec(compile(path.read_bytes(),str(path),'exec',optimize=0),module.__dict__)
if __name__=='__main__':
    args=sys.argv[1:];gate=module.receipt_gate()
    if args==['verify']:gate.verify(False)
    elif args==['verify-applied']:gate.verify(True)
    elif len(args)==2 and args[0]=='apply':gate.apply(args[1])
    else:raise SystemExit('verify|verify-applied|apply ACTUAL_APPROVAL_JSON')
