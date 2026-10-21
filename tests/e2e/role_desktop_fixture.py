"""Owned .54 real-service fixture for packaged Windows role acceptance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import tarfile

sys.path.insert(0, '/mnt/1tb-disk/cords-e2e/tools')
from host import ROOT, Results, command, new_run, output, wait

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('selection', type=Path, help='Native role test selection with the verified binary hash')
parser.add_argument('source', type=Path, help='Exact source directory recorded beside the native selection')
args = parser.parse_args()
try:
    selection = json.loads(args.selection.read_text())
    source = args.source.resolve()
    binary = Path(selection['binary']).resolve()
    receipt = json.loads(args.selection.with_name('source.json').read_text())
    if not source.is_relative_to(ROOT / 'runs') or source != Path(receipt['source']).resolve():
        parser.error('Source differs from the owned native build receipt')
    if not binary.is_relative_to(ROOT / 'cache') or hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest() != selection['binary_sha256']:
        parser.error('Native binary ownership or checksum mismatch')
except (OSError, ValueError, KeyError) as exc:
    parser.error('Cannot verify native fixture inputs: ' + str(exc))
run = new_run()
run.chmod(0o700)
results = Results(run)
print('RUN=' + str(run), flush=True)
name = run.name + '-gui-roles-postgres'
image = 'postgres@sha256:9b18b78397054fce88a9552e9d5a3ad5bb7fd258c5b3cc1c5028e46373d6ea8f'
owner = dict(name=name, image=image, data=str(run / 'postgres'), created=False, cleaned=False)
manifest = run / 'gui-database.json'
manifest.write_text(json.dumps(owner, indent=2))
child = None
try:
    assert subprocess.run(['docker', 'inspect', name], capture_output=True).returncode != 0, 'Name collision'
    (run / 'postgres').mkdir()
    (run / 'tmp').mkdir()
    password = secrets.token_hex(32)
    command(['docker', 'run', '-d', '--name', name, '--label', 'chat.cords.instance=' + run.name,
             '--memory', '1g', '--cpus', '2', '--env-file', '/dev/stdin', '-e', 'POSTGRES_USER=cords',
             '-e', 'POSTGRES_DB=cords', '-p', '127.0.0.1::5432', '-v', str(run / 'postgres') + ':/var/lib/postgresql/data:Z', image],
            input='POSTGRES_PASSWORD=' + password + '\n', stdout=subprocess.DEVNULL)
    owner['created'] = True
    manifest.write_text(json.dumps(owner, indent=2))
    wait(lambda: subprocess.run(['docker', 'exec', name, 'pg_isready', '-h', '127.0.0.1', '-U', 'cords'], capture_output=True).returncode == 0, 60, 'Dedicated PostgreSQL readiness')
    binding = output('docker', 'port', name, '5432/tcp')
    assert re.fullmatch(r'127\.0\.0\.1:\d+', binding)
    env = os.environ | dict(CORDS_TEST_DATABASE_URL='postgres://cords:' + password + '@' + binding + '/cords?sslmode=disable',
                            CORDS_ROLE_DESKTOP_FIXTURE=str(run / 'desktop'), TMPDIR=str(run / 'tmp'))
    (run / 'source.json').write_text(json.dumps(dict(source=str(source), binary=str(binary), binary_sha256=selection['binary_sha256']), indent=2))
    with (run / 'fixture.log').open('w') as log:
        child = subprocess.Popen([binary, 'messaging::tests::desktop_role_acceptance_fixture', '--exact', '--ignored', '--nocapture'], cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
        def ready():
            if child.poll() is not None:
                raise RuntimeError('Fixture exited before readiness; see fixture.log')
            return (run / 'desktop/ready.json').is_file()
        wait(ready, 90, 'Real native role service and test accounts')
        with tarfile.open(run / 'desktop-input.tar.gz', 'w:gz') as archive:
            for part in ('vault', 'fixture-ca.pem', 'ready.json'):
                archive.add(run / 'desktop' / part, arcname=part)
        results.record('live-role-fixture-ready', 'pass', json.loads((run / 'desktop/ready.json').read_text()))
        code = child.wait(timeout=1900)
    transcript = (run / 'fixture.log').read_text()
    defaults_only = os.environ.get('CORDS_ROLE_DESKTOP_DEFAULTS_ONLY') == '1'
    results.record('unchanged-default-role-policy' if defaults_only else 'gui-role-persistence-and-assignment',
                   'pass' if code == 0 and '1 passed; 0 failed' in transcript else 'fail',
                   'Native fixture validates untouched defaults' if defaults_only else 'Native fixture requires exactly one GUI role and one assignment; see fixture.log')
except Exception as exc:
    results.record('desktop-role-infrastructure', 'error', str(exc))
finally:
    if child is not None and child.poll() is None:
        child.terminate()
        child.wait(timeout=20)
    if owner['created']:
        try:
            with (run / 'database.sql').open('w') as dump:
                command(['docker', 'exec', name, 'pg_dump', '-U', 'cords', '-d', 'cords', '--no-owner', '--no-privileges'], stdout=dump)
            with (run / 'postgres.log').open('w') as log:
                command(['docker', 'logs', name], stdout=log, stderr=subprocess.STDOUT)
            command(['docker', 'rm', '-f', name], stdout=subprocess.DEVNULL)
            owner['cleaned'] = True
            manifest.write_text(json.dumps(owner, indent=2))
            results.record('owned-fixture-cleanup', 'pass', 'Retained database dump, logs, profiles and assertions on dedicated storage')
        except Exception as exc:
            results.record('owned-fixture-cleanup', 'error', str(exc))
    results.save()
sys.exit(2 if any(c['state'] == 'error' for c in results.cases) else 1 if any(c['state'] == 'fail' for c in results.cases) else 0)
