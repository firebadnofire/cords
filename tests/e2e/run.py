#!/usr/bin/env python3
"""Windows/Linux orchestration entry point; application tests execute on .54."""
import argparse
from pathlib import Path
import shlex
import subprocess
import sys
import time

from snapshot import snapshot

HOST = '192.168.86.54'
REMOTE = '/mnt/1tb-disk/cords-e2e'


def checked(argv, **kwargs):
    return subprocess.run(argv, check=True, **kwargs)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=('smoke', 'full', 'scenario', 'resilience', 'gui', 'infrastructure'))
    p.add_argument('scenario', nargs='?')
    p.add_argument('--expectations', type=Path, help='Temporary assertion overrides for harness failure validation')
    p.add_argument('--skip-build', action='store_true')
    p.add_argument('--fixture', choices=('normal', 'missing-result', 'provisioning-failure'), default='normal')
    a = p.parse_args()
    root = Path(__file__).resolve().parents[2]
    local = root / 'target/e2e' / time.strftime('%Y%m%d-%H%M%S')
    local.mkdir(parents=True)
    archive = local / 'source.tar.gz'
    manifest = snapshot(root, archive)
    # Bootstrap only the framework script through /tmp; all subsequent payloads live on the dedicated disk.
    checked(['scp', str(Path(__file__).with_name('framework.py')), HOST + ':/tmp/cords-e2e-framework.py'])
    checked(['ssh', '-o', 'BatchMode=yes', HOST, 'python3 /tmp/cords-e2e-framework.py'])
    payload = [str(path) for path in Path(__file__).parent.iterdir() if path.suffix in ('.py', '.sh')]
    checked(['scp', *payload, HOST + ':' + REMOTE + '/tools/'])
    identity = manifest['archive_sha256']
    remote_archive = REMOTE + '/sources/' + identity + '.tar.gz'
    remote_manifest = REMOTE + '/sources/' + identity + '.json'
    checked(['scp', str(archive), HOST + ':' + remote_archive])
    checked(['scp', str(archive.with_suffix('.json')), HOST + ':' + remote_manifest])
    if a.mode == 'infrastructure':
        args = ['python3', REMOTE + '/tools/host.py', 'infrastructure', '--fixture', a.fixture]
    else:
        args = ['python3', REMOTE + '/tools/suite.py', a.mode]
        if a.scenario:
            args += [a.scenario]
        args += ['--archive', remote_archive, '--manifest', remote_manifest]
        if a.skip_build:
            args += ['--skip-build']
        if a.expectations:
            remote_expectations = REMOTE + '/sources/' + identity + '-expectations.json'
            checked(['scp', str(a.expectations), HOST + ':' + remote_expectations])
            args += ['--expectations', remote_expectations]
    run = None
    with (local / 'orchestrator.log').open('w', encoding='utf-8') as log:
        process = subprocess.Popen(['ssh', '-o', 'BatchMode=yes', HOST, shlex.join(args)],
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding='utf-8')
        for line in process.stdout:
            print(line, end='', flush=True)
            log.write(line)
            log.flush()
            if line.startswith('RUN='):
                run = line.strip()[4:]
            if line.startswith('GUI='):
                context_path = line.strip()[4:]
                if not run or context_path != run + '/gui/context.json':
                    raise RuntimeError('GUI context outside owned run')
                checked(['scp', HOST + ':' + context_path, str(local / 'gui-context.json')])
                checked(['scp', HOST + ':' + run + '/tls/ca.crt', str(local / 'gui-ca.crt')])
                print('Windows GUI context:', local / 'gui-context.json', flush=True)
        status = process.wait()
    if run:
        if not run.startswith(REMOTE + '/runs/cords-e2e-') or any(c in run for c in (' ', '..', '\n')):
            raise RuntimeError('invalid remote artifact path')
        (local / 'remote-run.txt').write_text(run + '\n')
        checked(['scp', HOST + ':' + run + '/results.json', HOST + ':' + run + '/junit.xml',
                 HOST + ':' + run + '/events.jsonl', str(local)])
    print('Local evidence:', local)
    return status


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, subprocess.CalledProcessError, RuntimeError) as exc:
        print('Infrastructure error:', exc, file=sys.stderr)
        sys.exit(2)
