#!/usr/bin/env python3
"""Prepare the pinned upstream harness on .54; never operates on other checkouts."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path('/mnt/1tb-disk/cords-e2e')
FRAMEWORK = Path.home() / 'codex-cloud-init-automation'
REVISION = '86a4c86557d8588453d3589c2aa394e28c4c0172'


def run(*args):
    return subprocess.run(args, check=True, text=True, capture_output=True).stdout.strip()


def prepare():
    if run('hostname', '-I').split().count('192.168.86.54') != 1:
        raise RuntimeError('framework setup must execute on 192.168.86.54')
    run('sudo', '-n', 'install', '-d', '-o', str(__import__('os').getuid()), '-g',
        str(__import__('os').getgid()), '-m', '0755', str(ROOT))
    for name in ('bases', 'disks', 'runs', 'cache', 'sources', 'tools'):
        (ROOT / name).mkdir(exist_ok=True)
    if not FRAMEWORK.exists():
        run('git', 'clone', 'https://pubcode.archuser.org/firebadnofire/cloud-init-automation', str(FRAMEWORK))
        run('git', '-C', str(FRAMEWORK), 'checkout', '--detach', REVISION)
        (FRAMEWORK / '.cords-e2e-owner').write_text(REVISION + '\n')
    if not (FRAMEWORK / '.cords-e2e-owner').is_file():
        raise RuntimeError(f'{FRAMEWORK} already exists without our marker; preserve and inspect it')
    if run('git', '-C', str(FRAMEWORK), 'rev-parse', 'HEAD') != REVISION:
        raise RuntimeError('framework revision differs from pin')
    # Review authoritative files before applying the deliberately small local patch.
    reviewed = {}
    for name in ('README.md', 'AGENTS.md', 'ACCESS.md', 'image-info.conf',
                 'bring-up.sh', 'bring-down.sh', 'build.sh', 'copy-log.sh',
                 'test/user-data', 'test/meta-data', 'test/include/test.sh'):
        reviewed[name] = hashlib.sha256(run('git', '-C', str(FRAMEWORK), 'show',
                                           f'{REVISION}:{name}').encode()).hexdigest()
    for name in ('bring-up.sh', 'bring-down.sh', 'copy-log.sh'):
        path = FRAMEWORK / name
        original = run('git', '-C', str(FRAMEWORK), 'show', f'{REVISION}:{name}') + '\n'
        patched = original.replace('RO_DIR="/var/lib/libvirt/ro-images"',
                                   'RO_DIR="${CORDS_VM_BASE_DIR:?set CORDS_VM_BASE_DIR}"')
        patched = patched.replace('RUN_DIR="/var/lib/libvirt/images"',
                                  'RUN_DIR="${CORDS_VM_DISK_DIR:?set CORDS_VM_DISK_DIR}"')
        prior_patch = patched
        if name == 'bring-up.sh':
            patched = patched.replace('--boot uefi', '--boot "uefi,nvram=$RUN_DIR/${NAME}-VARS.qcow2"')
        if path.read_text() not in (original, prior_patch, patched):
            raise RuntimeError(f'unexpected edits in {path}; refusing overwrite')
        path.write_text(patched)
    # Label ONLY the owned disk subtree. Keep enforcing mode and other host policy intact.
    expression = str(ROOT) + '/(bases|disks)(/.*)?'
    policies = run('sudo', '-n', 'semanage', 'fcontext', '-l')
    if expression not in policies:
        run('sudo', '-n', 'semanage', 'fcontext', '-a', '-t', 'virt_image_t', expression)
    # Never recursively relabel live disk files: libvirt owns their dynamic MCS labels.
    run('sudo', '-n', 'restorecon', '-F', str(ROOT / 'bases'), str(ROOT / 'disks'))
    patch = run('git', '-C', str(FRAMEWORK), 'diff') + '\n'
    (ROOT / 'framework.patch').write_text(patch)
    record = {'upstream': REVISION, 'reviewed': reviewed,
              'patch_sha256': hashlib.sha256(patch.encode()).hexdigest(),
              'storage': str(ROOT), 'framework': str(FRAMEWORK)}
    (ROOT / 'framework.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record, indent=2))


if __name__ == '__main__':
    prepare()
