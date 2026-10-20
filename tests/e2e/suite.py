#!/usr/bin/env python3
""".54 suite controller. No simulated provisioning or application behavior."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import fcntl
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tarfile

from framework import prepare
from host import ROOT, Results, VM, command, new_run
from deployment import Deployment
from scenarios import Scenarios


def extract(archive, destination, manifest):
    with Path(archive).open('rb') as incoming:
        digest = hashlib.file_digest(incoming, 'sha256').hexdigest()
    if digest != manifest['archive_sha256']:
        raise RuntimeError('source archive checksum mismatch')
    destination.mkdir()
    with tarfile.open(archive) as bundle:
        bundle.extractall(destination, filter='data')
    for name, digest in manifest['files'].items():
        with (destination / name).open('rb') as incoming:
            actual = hashlib.file_digest(incoming, 'sha256').hexdigest()
        if actual != digest:
            raise RuntimeError('source input mismatch: ' + name)
        # Archives deliberately use deterministic timestamps. Cargo uses mtimes
        # for workspace inputs, so restore fresh extraction times after hashing.
        os.utime(destination / name, None)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('mode', choices=('smoke', 'full', 'scenario', 'resilience', 'gui'))
    p.add_argument('scenario', nargs='?')
    p.add_argument('--archive', required=True)
    p.add_argument('--manifest', required=True)
    p.add_argument('--expectations', type=Path)
    p.add_argument('--skip-build', action='store_true', help='Require matching previously built source receipt')
    a = p.parse_args()
    run = new_run()
    result = Results(run)
    print('RUN=' + str(run), flush=True)
    source = run / 'source'
    vms, deployment, scenarios = [], None, None
    try:
        lock = (ROOT / 'suite.lock').open('a')
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise RuntimeError('Another suite owns the resource lock; wait for its cleanup before retrying') from exc
        prepare()
        manifest = json.loads(Path(a.manifest).read_text())
        (run / 'source-manifest.json').write_text(json.dumps(manifest, indent=2))
        extract(a.archive, source, manifest)
        result.record('exact-source-snapshot', 'pass', manifest['archive_sha256'])
        receipt = ROOT / 'cache/linux-build.json'
        # Only application/compiler inputs affect the binary receipt; test definitions can vary.
        inputs = {k: v for k, v in manifest['files'].items() if k.startswith(('crates/', 'bins/', 'assets/')) or k in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')}
        if a.skip_build:
            saved = json.loads(receipt.read_text()) if receipt.is_file() else {}
            if saved.get('version') != 2 or saved.get('inputs') != inputs:
                raise RuntimeError('--skip-build requires a matching source build receipt')
            for name, digest in saved.get('artifacts', {}).items():
                binary = ROOT / 'cache/linux-target/release' / name
                if hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest() != digest:
                    raise RuntimeError('Cached build artifact checksum changed: ' + name)
            if not saved.get('artifacts'):
                raise RuntimeError('Legacy build receipt lacks artifact hashes; rebuild once without --skip-build')
        else:
            env = __import__('os').environ | dict(PATH=str(Path.home() / '.cargo/bin') + ':' + __import__('os').environ['PATH'],
                                                RUSTUP_TOOLCHAIN='1.97.1',
                                                CARGO_TARGET_DIR=str(ROOT / 'cache/linux-target'),
                                                CORDS_DIST_DIR=str(run / 'artifacts/linux'), CARGO_BUILD_JOBS='2',
                                                TMPDIR=str(ROOT / 'cache/tmp'))
            Path(env['TMPDIR']).mkdir(exist_ok=True)
            with (run / 'linux-build.log').open('w') as log:
                command(['bash', source / 'build-scripts/linux-server-tar.sh'], cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
                command(['cargo', 'build', '--locked', '--release', '--manifest-path', source / 'Cargo.toml',
                         '--bin', 'cords-dev', '--features', 'fault-injection'], cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
            artifacts = {name: hashlib.file_digest((ROOT / 'cache/linux-target/release' / name).open('rb'), 'sha256').hexdigest()
                         for name in ('cords-server', 'cords-dev')}
            receipt.write_text(json.dumps(dict(version=2, inputs=inputs, artifacts=artifacts), indent=2))
        shutil.copy2(receipt, run / 'build-receipt.json')
        result.record('native-build-gate', 'pass')
        memory_kib = int(next(line.split()[1] for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:')))
        if memory_kib < 6 * 1024 * 1024 or shutil.disk_usage(ROOT).free < 20 * 1024**3:
            raise RuntimeError('Three-client run needs at least 6 GiB available RAM and 20 GiB free owned storage')
        environment = dict(os_release=Path('/etc/os-release').read_text(), memory_available_kib=memory_kib,
                           disk_free_bytes=shutil.disk_usage(ROOT).free,
                           framework=json.loads((ROOT / 'framework.json').read_text()))
        (run / 'environment.json').write_text(json.dumps(environment, indent=2))
        deployment = Deployment(run, source)
        with (run / 'deployment.log').open('w') as log:
            # Lifecycle stdout contains only ephemeral test credentials, never real-user data.
            deployment.start()
        result.record('isolated-server-tls-readiness', 'pass', deployment.origin)
        vms = [VM(run, label) for label in ('a', 'b', 'c')]
        with ThreadPoolExecutor(max_workers=3) as pool:
            futures = [pool.submit(vm.start) for vm in vms]
            for future in futures:
                future.result()
        result.record('three-independent-client-vms', 'pass', [vm.name for vm in vms])
        scenarios = Scenarios(vms, deployment, source, result,
                              json.loads(a.expectations.read_text()) if a.expectations else None)
        scenarios.setup()
        if a.mode == 'gui':
            from gui_bridge import coordinate
            coordinate(scenarios)
        selected = {'smoke': ['messaging'], 'resilience': ['messaging', 'resilience', 'security'],
                    'full': ['messaging', 'public', 'permissions', 'profiles', 'resilience', 'security'],
                    'gui': []}.get(a.mode, [a.scenario])
        aliases = {'role-revocation': 'permissions', 'avatars': 'profiles', 'identity': None,
                   'wire-security': 'wire_security'}
        for name in selected:
            name = aliases.get(name, name)
            if name is None:
                continue
            if name not in ('messaging', 'public', 'permissions', 'profiles', 'resilience', 'security', 'wire_security'):
                raise ValueError('Unknown scenario: ' + str(name))
            getattr(scenarios, name)()
        if a.mode in ('gui', 'full'):
            result.record('windows-gui', 'unavailable', 'Full GUI acceptance requires bidirectional independent-client messages, permission denials, avatars, account switching and restart evidence; inspect run/gui for executed partial checks')
        if a.mode == 'full':
            inventory = json.loads((source / 'tests/e2e/coverage.json').read_text())
            for item in inventory['unavailable']:
                if item['feature'] not in ('active-role-channel-permission-enforcement', 'integrated-windows-gui-suite'):
                    result.record(item['feature'], 'unavailable', item['reason'])
    except AssertionError as exc:
        result.record('application-assertion', 'fail', exc)
    except Exception as exc:
        result.record('infrastructure', 'error', f'{type(exc).__name__}: {exc}')
    finally:
        if scenarios:
            try:
                scenarios.close()
            except Exception as exc:
                result.record('client-cleanup', 'error', exc)
        for vm in vms:
            try:
                vm.stop_collect()
            except Exception as exc:
                result.record('guest-collection-' + vm.name, 'error', exc)
        if deployment:
            try:
                deployment.close()
            except Exception as exc:
                result.record('server-cleanup', 'error', exc)
        result.save()
    if any(c['state'] == 'error' for c in result.cases):
        return 2
    if any(c['state'] == 'fail' for c in result.cases):
        return 1
    if any(c['state'] == 'unavailable' for c in result.cases):
        return 3
    return 0


if __name__ == '__main__':
    sys.exit(main())
