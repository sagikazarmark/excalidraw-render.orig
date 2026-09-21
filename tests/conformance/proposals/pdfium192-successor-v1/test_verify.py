import importlib.util
import json
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('workflow',Path(__file__).with_name('verify.py'))
w=importlib.util.module_from_spec(spec);spec.loader.exec_module(w)


class DecisionAndScope(unittest.TestCase):
    def test_exact_decision_and_rejections(self):
        decision={'decision':'approve','scope':'pdfium192-successor',
                  'production_subject_sha256':'sha256:'+'a'*64,'representative_subject_sha256':w.REP}
        body=lambda d:'```json\n'+json.dumps(d)+'\n```'
        w.affirmative(body(decision),decision['production_subject_sha256'],w.REP)
        for field,value in [('decision','reject'),('scope','pdfium192-alpha-profile'),
                            ('production_subject_sha256','sha256:'+'b'*64),('extra',True)]:
            bad=dict(decision);bad[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):
                w.affirmative(body(bad),decision['production_subject_sha256'],w.REP)
        for bad in ['request '+body(decision),body(decision)+' accepted',
                    '```json\n{"decision":"reject",'+json.dumps(decision)[1:]+'\n```']:
            with self.assertRaises(ValueError):w.affirmative(bad,decision['production_subject_sha256'],w.REP)

    def test_pending_scope_and_receipt_inventory(self):
        w.verify_migration(False)
        self.assertEqual([r['count'] for r in w.gate().test_inventory()],[103,5,1])
        commands=w.gate().COMMANDS
        self.assertEqual(len(commands),6)
        self.assertEqual(commands[1][1][-1],w.V14)

    def test_wrong_ownership_counts_fail(self):
        with self.assertRaises(ValueError):
            w.gate().outcome('ownership',b'verified 2418 ownership assignments: 2333 assigned/excluded, 85 explicit gaps; ledger approval pending\n',b'')


if __name__=='__main__':unittest.main()
