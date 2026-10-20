#!/usr/bin/env python3
"""Stage the exact snapshot and pinned SDKs into an owned Ubuntu build VM."""
from pathlib import Path
import argparse
import hashlib
import json
import shlex
import subprocess
from host import ROOT, VM, command, wait
import sys
import fcntl


def save_dependency_cache(vm):
    """Export only dependencies; compiler aliases and credentials are not portable."""
    vm.ssh('mkdir -p ~/artifacts && tar --exclude=.cache/cargo-xwin/clang-cl '
           '-cf ~/artifacts/rust-dependencies.tar -C /home/testuser '
           '.cargo/registry .cache/cargo-xwin', timeout=300)
    cache = ROOT / 'cache/guest-transfer/rust-dependencies.tar'
    partial = cache.with_suffix('.partial')
    command(['scp', *vm.ssh_argv('')[1:-2],
             'testuser@' + vm.ip + ':artifacts/rust-dependencies.tar', partial])
    with partial.open('rb') as incoming:
        digest = hashlib.file_digest(incoming, 'sha256').hexdigest()
    partial.replace(cache)
    cache.with_suffix('.sha256').write_text(digest + '\n')
    print('Saved portable Rust dependency cache: ' + digest, flush=True)


def restore_dependency_cache(vm, scp):
    cache = ROOT / 'cache/guest-transfer/rust-dependencies.tar'
    if not cache.exists():
        print('No declared Rust dependency cache yet; using pinned toolchain downloads', flush=True)
        return
    expected = cache.with_suffix('.sha256').read_text().strip()
    with cache.open('rb') as incoming:
        digest = hashlib.file_digest(incoming, 'sha256').hexdigest()
    if digest != expected:
        raise RuntimeError('Rust dependency cache checksum mismatch')
    command([*scp, cache, 'testuser@' + vm.ip + ':tools/'])
    vm.ssh(f'test "$(sha256sum ~/tools/rust-dependencies.tar | cut -d" " -f1)" = {digest}')
    program = ('import tarfile; from pathlib import Path; '
               'a=tarfile.open(Path.home()/"tools/rust-dependencies.tar"); '
               'members=a.getmembers(); '
               'assert all(any(m.name == p or m.name.startswith(p+"/") for p in (".cargo/registry", ".cache/cargo-xwin")) for m in members); '
               'a.extractall(Path.home(), members=members, filter="data")')
    vm.ssh('python3 -c ' + shlex.quote(program))
    print('Restored verified Rust registry and Windows dependency cache: ' + digest, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('guest_manifest', type=Path)
    parser.add_argument('--archive', required=True, type=Path)
    parser.add_argument('--manifest', required=True, type=Path)
    parser.add_argument('--platform', choices=('windows', 'linux'), default='windows')
    args = parser.parse_args()
    vm = VM.recover(args.guest_manifest)
    lock = (vm.run / 'builder.lock').open('a')
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as exc:
        raise RuntimeError('Another builder owns this Ubuntu guest; wait before retrying') from exc
    if vm.ip is None:
        command(['sudo', '-n', 'virsh', 'start', vm.name])
        vm.ip = wait(vm.address, 180, 'Ubuntu build guest DHCP')
    wait(lambda: subprocess.run(vm.ssh_argv('true'), capture_output=True).returncode == 0,
         180, 'Ubuntu build guest SSH')
    vm.ssh('mkdir -p ~/tools ~/Qt ~/source')
    ssh_args = vm.ssh_argv('')
    # scp shares SSH identity, host-key policy and known-hosts file with the guest session.
    scp = ['scp', *ssh_args[1:-2]]
    script = 'build-ubuntu.sh' if args.platform == 'windows' else 'build-native-gui.sh'
    files = [Path(__file__).with_name(name) for name in (script, 'verify_guest.py', 'provision-builder.sh', 'run-linux-ui-tests.sh')]
    command([*scp, *files, 'testuser@' + vm.ip + ':tools/'])
    restore_dependency_cache(vm, scp)
    vm.ssh('bash ~/tools/provision-builder.sh', timeout=1200)
    command([*scp, args.archive, 'testuser@' + vm.ip + ':tools/input.tar.gz'])
    command([*scp, args.manifest, 'testuser@' + vm.ip + ':tools/input.json'])
    sdk_receipt = {}
    pinned_sdks = {'qt-linux.tar': 'ef2a2fea4ca717ac6df320e54ec843c07e6aa0f50c041b293045df3b25df51db',
                   'qt-windows.tar': '381a4001c8fb3aa99b7631d7e8aebfabc596b7c1868309911a71a9cfef31db65'}
    for name in ('qt-linux.tar', 'qt-windows.tar'):
        sdk = ROOT / 'cache/guest-transfer' / name
        digest = hashlib.file_digest(sdk.open('rb'), 'sha256').hexdigest()
        if digest != pinned_sdks[name]:
            raise RuntimeError('SDK differs from inspected Qt 6.10.3 pin: ' + name)
        sdk_receipt[name] = digest
        command([*scp, sdk, 'testuser@' + vm.ip + ':tools/'])
        vm.ssh(f'test "$(sha256sum ~/tools/{name} | cut -d" " -f1)" = {digest}')
        vm.ssh(f'tar -xf ~/tools/{name} -C ~/Qt')
    source = '/home/testuser/sources/' + json.loads(args.manifest.read_text())['archive_sha256']
    vm.ssh('mkdir -p ~/sources')
    vm.ssh('python3 ~/tools/verify_guest.py ' + shlex.quote(source))
    (vm.run / 'sdk-manifest.json').write_text(json.dumps(sdk_receipt, indent=2))
    vm.ssh('CORDS_SOURCE_DIR=' + shlex.quote(source) + ' bash ~/tools/' + script, timeout=5400)
    artifacts = vm.run / ('artifacts-' + args.platform) / Path(source).name
    artifacts.mkdir(parents=True, exist_ok=True)
    command([*scp, '-r', 'testuser@' + vm.ip + ':artifacts/' + args.platform + '/.', artifacts])
    if args.platform == 'linux':
        native = artifacts / 'native-tests'
        native.mkdir(exist_ok=True)
        command([*scp, '-r', 'testuser@' + vm.ip + ':artifacts/linux-xvfb/.', native])
        from aggregate import qt_reports
        qt_reports(native, artifacts / 'native-results')
    command([*scp, 'testuser@' + vm.ip + ':artifacts/ubuntu-packages.txt', vm.run])
    hashes = {p.relative_to(artifacts).as_posix(): hashlib.file_digest(p.open('rb'), 'sha256').hexdigest() for p in artifacts.rglob('*')
              if p.is_file() and p.name not in ('artifact-hashes.json', 'build-receipt.json')}
    if not any(n.endswith(('.zip', '.tar.gz')) for n in hashes):
        raise RuntimeError('Builder returned no package')
    (artifacts / 'artifact-hashes.json').write_text(json.dumps(hashes, indent=2))
    (artifacts / 'build-receipt.json').write_text(json.dumps(dict(source_sha256=Path(source).name,
                                                               sdks=sdk_receipt, artifacts=hashes,
                                                               automation={p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                                                                           for p in files}), indent=2))


if __name__ == '__main__':
    main()
