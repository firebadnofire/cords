"""Use the real independent member client; never implement client signing in Python."""
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import uuid
sys.path.insert(0, '/mnt/1tb-disk/cords-e2e/tools')
from host import ROOT, Results
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('run', type=Path)
parser.add_argument('revision', type=int)
args = parser.parse_args()
run, revision = args.run.resolve(), args.revision
if not run.is_relative_to(ROOT / 'runs') or revision < 0:
    parser.error('Use an owned .54 run and a nonnegative policy revision')
directory = run / ('member-denial-' + str(revision))
try:
    directory.mkdir(exist_ok=False)
except OSError as exc:
    parser.error('Cannot create unique member observation: ' + str(exc))
results = Results(directory)
try:
    source = Path(json.loads((run / 'source.json').read_text())['source'])
    binary = ROOT / 'cache/linux-target/release/cords-dev'
    db = sqlite3.connect('file:' + str(run / 'desktop/vault/accounts.db') + '?mode=ro', uri=True)
    local_id, account = db.execute("SELECT local_id,account_id FROM local_accounts WHERE nickname='Member acceptance'").fetchone()
    db.close()
    requests = [dict(id='authenticate', op='authenticate'), dict(id='catalog', op='role-catalog'),
                dict(id='unauthorized-role', op='change-role', request=dict(expected_revision=revision, request_id=str(uuid.uuid4()),
                     change=dict(operation='put_role', role=dict(id=str(uuid.uuid4()), name='Unauthorized role', color='#ffffff', description='', priority=1, permissions=[], mentionable=False)))),
                dict(id='exit', op='exit')]
    completed = subprocess.run([binary, '--state', run / 'desktop/vault/accounts' / local_id, '--migrations', source / 'migrations/sqlite', '--ca', run / 'desktop/fixture-ca.pem', '--passphrase-stdin'],
                               input='Test-only Member Vault 5682!\n' + '\n'.join(json.dumps(r) for r in requests) + '\n', capture_output=True, text=True, timeout=60)
    (directory / 'client.log').write_text(completed.stdout)
    (directory / 'stderr.log').write_text(completed.stderr)
    assert completed.returncode == 0, 'Native member client exited unsuccessfully'
    replies = {r['id']: r for line in completed.stdout.splitlines() if (r := json.loads(line)).get('id')}
    assert replies['authenticate']['ok'] is True, 'Independent member authentication failed'
    for operation in ('catalog', 'unauthorized-role'):
        reply = replies[operation]
        assert reply['ok'] is False and '403' in reply.get('error', ''), f'{operation} did not receive an authenticated 403 denial: {reply}'
    (directory / 'client.json').write_text(json.dumps(dict(account=account, binary=str(binary), binary_sha256=hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest()), indent=2))
    results.record('independent-member-no-administrative-grant', 'pass', 'Native member authenticated; preparatory catalog read and role write denied with HTTP 403. Server checkpoint must separately prove no mutation.')
except AssertionError as exc:
    results.record('independent-member-no-administrative-grant', 'fail', str(exc))
except Exception as exc:
    results.record('independent-member-no-administrative-grant', 'error', str(exc))
sys.exit(2 if any(c['state'] == 'error' for c in results.cases) else 1 if any(c['state'] == 'fail' for c in results.cases) else 0)
