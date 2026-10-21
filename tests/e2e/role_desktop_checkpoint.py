"""Assert GUI effects against the owned fixture's authoritative PostgreSQL state."""
import argparse
import json
from pathlib import Path
import re
import sys
sys.path.insert(0, '/mnt/1tb-disk/cords-e2e/tools')
from host import ROOT, Results, output
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('run', type=Path)
parser.add_argument('expectation', type=Path)
args = parser.parse_args()
run = args.run.resolve()
if not run.is_relative_to(ROOT / 'runs'):
    parser.error('Run must reside under the owned .54 runs directory')
try:
    expectation = json.loads(args.expectation.read_text())
except (OSError, ValueError) as exc:
    parser.error('Cannot read expected result artifact: ' + str(exc))
if not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,79}', str(expectation.get('name', ''))):
    parser.error('Checkpoint name must use lowercase letters, digits and hyphens')
directory = run / 'checkpoints' / expectation['name']
try:
    directory.mkdir(parents=True, exist_ok=False)
except OSError as exc:
    parser.error('Cannot create unique checkpoint: ' + str(exc))
results = Results(directory)
try:
    manifest = json.loads((run / 'gui-database.json').read_text())
    assert manifest['created'] and not manifest['cleaned']
    name = manifest['name']
    assert name == run.name + '-gui-roles-postgres'
    def sql(query):
        return output('docker', 'exec', name, 'psql', '-XAt', '-v', 'ON_ERROR_STOP=1', '-U', 'cords', '-d', 'cords', '-c', query)
    schema = sql("SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE 'cords_test_%'")
    assert re.fullmatch(r'cords_test_[0-9a-f]+', schema), 'Expected exactly one fixture schema'
    snapshot = json.loads(sql(f"SET search_path TO {schema}; SELECT json_build_object('policy',(SELECT row_to_json(p) FROM role_policy p),'roles',(SELECT json_agg(definition::json) FROM server_roles),'assignments',(SELECT coalesce(json_agg(a),'[]'::json) FROM account_roles a),'audit',(SELECT coalesce(json_agg(c ORDER BY revision),'[]'::json) FROM role_policy_changes c))").splitlines()[-1])
    (directory / 'actual.json').write_text(json.dumps(snapshot, indent=2))
    (directory / 'expected.json').write_text(json.dumps(expectation, indent=2))
    assert snapshot['policy']['revision'] == expectation['revision'], 'Unexpected policy revision'
    assert snapshot['policy']['active'] is False, 'Preparatory policy activation changed'
    custom = {r['name']: r for r in snapshot['roles'] if not r['id'].startswith('builtin:')}
    assert set(custom) == set(expectation['roles']), 'Custom role names differ'
    for role_name, fields in expectation['roles'].items():
        for key, value in fields.items():
            assert custom[role_name][key] == value, f'{role_name}: unexpected {key}'
    assert len(snapshot['assignments']) == expectation['assignments'], 'Unexpected assignment count'
    if 'assigned_accounts' in expectation:
        assert sorted(a['account_id'] for a in snapshot['assignments']) == sorted(expectation['assigned_accounts']), 'Assignment targets differ'
    if 'assigned_roles' in expectation:
        names = {r['id']: r['name'] for r in snapshot['roles']}
        assert sorted(names[a['role_id']] for a in snapshot['assignments']) == sorted(expectation['assigned_roles']), 'Assigned roles differ'
    if 'audit' in expectation:
        assert [c['operation'] for c in snapshot['audit']] == expectation['audit'], 'Unexpected audit operations'
    results.record(expectation['name'], 'pass', 'Exact expected revision, role definitions, assignment count and optional audit verified against dedicated PostgreSQL')
except AssertionError as exc:
    results.record(expectation['name'], 'fail', str(exc))
except Exception as exc:
    results.record(expectation['name'], 'error', type(exc).__name__ + ': ' + str(exc))
sys.exit(2 if any(c['state'] == 'error' for c in results.cases) else 1 if any(c['state'] == 'fail' for c in results.cases) else 0)
