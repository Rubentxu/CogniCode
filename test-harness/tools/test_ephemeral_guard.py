import tempfile
import unittest
from pathlib import Path
from ephemeral_guard import pending_files

class GuardTests(unittest.TestCase):
    def test_clean_repo_without_temp(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'test-harness').mkdir()
            self.assertEqual(pending_files(root), [])

    def test_cargo_cycle_file_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'test-harness').mkdir()
            folder = root / 'crates' / 'core' / 'tests'
            folder.mkdir(parents=True)
            (folder / '__vdh_cycle_B1_truth.rs').write_text('fn main() {}')
            self.assertEqual(pending_files(root), ['crates/core/tests/__vdh_cycle_B1_truth.rs'])

    def test_cycle_directory_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cyc = root / 'test-harness' / '.cycles' / 'B3-abc'
            cyc.mkdir(parents=True)
            (cyc / 'README.txt').write_text('temp')
            self.assertTrue(pending_files(root))

    def test_only_regular_non_cycle_test_allowed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'test-harness').mkdir()
            tests = root / 'apps' / 'explorer-ui' / 'e2e'
            tests.mkdir(parents=True)
            (tests / 'graph.spec.ts').write_text('test')
            self.assertEqual(pending_files(root), [])

if __name__ == '__main__':
    unittest.main()
