#!/usr/bin/env python3
"""Exercise an installed client with the disposable desktop-role TLS fixture.

Never point this at personal accounts. Passwords below belong only to the
repository's synthetic desktop_role_acceptance_fixture.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--fixture', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--settle-ms', type=int, default=5000)
    args = parser.parse_args()
    if not 2500 <= args.settle_ms <= 120000:
        parser.error('--settle-ms must be between 2500 and 120000')
    executable = args.executable.resolve(strict=True)
    fixture = args.fixture.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    ready = json.loads((fixture / 'ready.json').read_text(encoding='utf-8'))
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('QT_', 'QML_', 'CORDS_CLIENT_'))}
    if os.name == 'nt':
        env['PATH'] = str(Path(os.environ['SYSTEMROOT']) / 'System32')
    else:
        env['PATH'] = '/usr/bin:/bin'
    env['CORDS_CLIENT_STATE'] = ready['state']
    env['QT_FORCE_STDERR_LOGGING'] = '1'
    env['QML_DISK_CACHE_PATH'] = str(output / 'qml-cache')
    env['CORDS_CLIENT_CA'] = ready['ca']
    listing = subprocess.run([str(executable), '--list-accounts'], env=env,
                             cwd=fixture, capture_output=True, text=True,
                             timeout=15, check=True)
    accounts = json.loads(listing.stdout)
    account = next(row['account_id'] for row in accounts
                   if row['label'] == 'Role acceptance')
    for size, scale, label in [('1440x900', '1', 'wide'),
                               ('1024x768', '1', 'standard'),
                               ('680x520', '1', 'narrow'),
                               ('900x650', '1.5', 'scaled')]:
        env['QT_SCALE_FACTOR'] = scale
        capture = output / f'installed-{label}.png'
        result = subprocess.run(
            [str(executable), '--account', account, '--password-stdin',
             '--window-size', size, '--capture', str(capture),
             '--capture-after', str(max(1800, args.settle_ms - max(2000, args.settle_ms // 10))),
             '--quit-after', str(args.settle_ms)],
            input='Test-only Role Vault 9321!\n', env=env, cwd=fixture,
            capture_output=True, text=True, timeout=args.settle_ms / 1000 + 40)
        capture.with_suffix('.log').write_text(result.stdout + result.stderr,
                                               encoding='utf-8')
        if result.returncode or not capture.is_file() or result.stderr.strip():
            raise RuntimeError(f'{label} startup failed (exit={result.returncode}, capture={capture.is_file()}); inspect {capture.with_suffix(".log")}')
        print(f'{label}: clean exit, captured {capture}', flush=True)
    print('Inspect the captures. This host test is not clean-machine acceptance.')


if __name__ == '__main__':
    main()
