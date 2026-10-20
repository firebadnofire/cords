"""Harness integrity tests. Execute on .54; these do not stand in for application acceptance."""
import json
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
import xml.etree.ElementTree as ET

from gui_request import validate_request
from host import ROOT, Results
from snapshot import snapshot
from suite import extract


class HarnessTests(unittest.TestCase):
    def test_snapshot_is_deterministic_and_detects_tampering(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'cache') as temporary:
            directory = Path(temporary)
            repository = directory / 'repository'
            repository.mkdir()
            subprocess.run(['git', 'init', '-q', repository], check=True)
            source = repository / 'input.txt'
            source.write_text('committed source')
            subprocess.run(['git', '-C', repository, 'add', '.'], check=True)
            subprocess.run(['git', '-C', repository, '-c', 'user.name=E2E', '-c', 'user.email=e2e@invalid',
                            'commit', '-qm', 'fixture'], check=True)
            source.write_text('working-tree source')
            (repository / 'tests/e2e').mkdir(parents=True)
            (repository / 'tests/e2e/definition.json').write_text('{}')
            (repository / 'private-untracked.txt').write_text('excluded')
            first, second = directory / 'first.tar.gz', directory / 'second.tar.gz'
            a, b = snapshot(repository, first), snapshot(repository, second)
            self.assertEqual(a['archive_sha256'], b['archive_sha256'])
            self.assertEqual(a['untracked_included'], ['tests/e2e/definition.json'])
            self.assertNotIn('private-untracked.txt', a['files'])
            extract(first, directory / 'extracted', a)
            self.assertEqual((directory / 'extracted/input.txt').read_text(), 'working-tree source')
            self.assertGreater((directory / 'extracted/input.txt').stat().st_mtime, time.time() - 10,
                               'Deterministic archive timestamps must not bypass Cargo rebuilds')
            first.write_bytes(first.read_bytes() + b'corruption')
            with self.assertRaisesRegex(RuntimeError, 'checksum mismatch'):
                extract(first, directory / 'corrupt', a)

    def test_results_keep_failure_error_and_unavailable_distinct(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'cache') as temporary:
            result = Results(temporary)
            for state in ('pass', 'fail', 'error', 'unavailable'):
                result.record(state, state, state)
            xml = ET.parse(Path(temporary) / 'junit.xml').getroot()
            self.assertEqual((xml.get('failures'), xml.get('errors'), xml.get('skipped')), ('1', '1', '1'))
            self.assertEqual(len(json.loads((Path(temporary) / 'results.json').read_text())), 4)

    def test_gui_send_schema_rejects_text_before_transmission(self):
        with self.assertRaisesRegex(ValueError, 'fields.body'):
            validate_request(dict(action='client', operation='send', fields=dict(route='route', text='wrong')))
        validate_request(dict(action='client', operation='send', fields=dict(route='route', body='right')))
        with self.assertRaises(ValueError):
            validate_request(dict(action='assert-peer-message', route='route'))


if __name__ == '__main__':
    unittest.main()
