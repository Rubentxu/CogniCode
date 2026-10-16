import tempfile
import unittest
from pathlib import Path
from acceptance_receipt import verify

class ReceiptTests(unittest.TestCase):
    def data(self,root):
        file=root/'tests'/'contract.rs'
        file.parent.mkdir()
        file.write_text('test')
        return {'block':'B1','source_sha':'a'*40,'cases':[
            dict(id=f'B1-{kind}-01',status='PASS',exit_code=0,
                 command='cargo test -p cognicode-core',duration_ms=5,
                 scenario_sha256='b'*64,artifact_sha256='c'*64,
                 permanent_contract='tests/contract.rs',
                 negative_proof='mutation rc=101' if kind=='AAT' else '',
                 ephemeral_path=f'test-harness/.cycles/B1/{kind}.rs')
            for kind in ('UAT','E2E','AAT') ]}
    def test_structural_valid_receipt(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            self.assertEqual(verify(self.data(root),root),[])
    def test_missing_negative_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);data=self.data(root)
            data['cases'][2]['negative_proof']=''
            self.assertTrue(any('negative_proof' in p for p in verify(data,root)))
    def test_missing_scenario_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);data=self.data(root)
            data['cases'].pop()
            self.assertTrue(any('exactly one' in p for p in verify(data,root)))
    def test_undeleted_cycle_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);data=self.data(root)
            cycle=root/'test-harness/.cycles/B1/UAT.rs'
            cycle.parent.mkdir(parents=True)
            cycle.write_text('temporary')
            self.assertTrue(any('still present' in p for p in verify(data,root)))

if __name__=='__main__':unittest.main()
