#!/usr/bin/env python3
"""Real .54 libvirt execution using the pinned cloud-init-automation scripts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import time
import uuid
import xml.etree.ElementTree as ET

from framework import FRAMEWORK, ROOT, prepare

UBUNTU_SHA256 = '04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94'

def command(argv, **kw):
    return subprocess.run(list(map(str, argv)), check=True, text=True, **kw)


def output(*argv):
    return command(argv, stdout=subprocess.PIPE).stdout.strip()


def wait(predicate, seconds, description):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        result = predicate()
        if result:
            return result
        time.sleep(2)
    raise RuntimeError('Timed out: ' + description)


class Results:
    def __init__(self, directory):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.cases = []

    def record(self, name, state, detail=''):
        item = dict(name=name, state=state, detail=str(detail), timestamp=time.time())
        self.cases.append(item)
        with (self.directory / 'events.jsonl').open('a') as log:
            log.write(json.dumps(item) + '\n')
        print(json.dumps(item), flush=True)
        self.save()

    def save(self):
        (self.directory / 'results.json').write_text(json.dumps(self.cases, indent=2) + '\n')
        suite = ET.Element('testsuite', name='cords-e2e', tests=str(len(self.cases)),
                           failures=str(sum(c['state'] == 'fail' for c in self.cases)),
                           errors=str(sum(c['state'] == 'error' for c in self.cases)),
                           skipped=str(sum(c['state'] == 'unavailable' for c in self.cases)))
        for case in self.cases:
            node = ET.SubElement(suite, 'testcase', name=case['name'])
            kind = {'fail': 'failure', 'error': 'error', 'unavailable': 'skipped'}.get(case['state'])
            if kind:
                ET.SubElement(node, kind, message=case['detail']).text = case['detail']
        ET.ElementTree(suite).write(self.directory / 'junit.xml', encoding='utf-8', xml_declaration=True)


def base_image(ubuntu=False):
    name = 'ubuntu-24.04-server-cloudimg-amd64.img' if ubuntu else 'debian-13-genericcloud-amd64.qcow2'
    destination = ROOT / 'bases' / name
    digest_file = destination.with_suffix(destination.suffix + '.sha256')
    if not destination.exists():
        temporary = destination.with_suffix('.download')
        if ubuntu:
            url = 'https://cloud-images.ubuntu.com/releases/noble/release/'
            sums = output('curl', '-fLSs', '--retry', '2', url + 'SHA256SUMS')
            expected = next(line.split()[0] for line in sums.splitlines() if line.split()[-1].lstrip('*') == name)
            if expected != UBUNTU_SHA256:
                raise RuntimeError('Ubuntu release moved beyond the inspected image pin; restore the pinned cached image or review an explicit pin update')
            command(['curl', '-fL', '--retry', '2', '-o', temporary, url + name])
            actual = hashlib.file_digest(temporary.open('rb'), 'sha256').hexdigest()
            if actual != expected:
                raise RuntimeError('Ubuntu image checksum mismatch')
        else:
            source = Path('/var/lib/libvirt/ro-images') / name
            command(['cp', '--reflink=auto', source, temporary])
        temporary.rename(destination)
        digest_file.write_text(hashlib.file_digest(destination.open('rb'), 'sha256').hexdigest() + '\n')
        destination.chmod(0o444)
        command(['sudo', '-n', 'restorecon', str(destination)])
    if not digest_file.is_file() or hashlib.file_digest(destination.open('rb'), 'sha256').hexdigest() != digest_file.read_text().strip():
        raise RuntimeError('base-image integrity failure: ' + str(destination))
    if ubuntu and digest_file.read_text().strip() != UBUNTU_SHA256:
        raise RuntimeError('Ubuntu base differs from the inspected pin')
    return destination


class VM:
    def __init__(self, run, suffix, ubuntu=True, script=None, builder=False):
        self.run = Path(run)
        self.name = self.run.name + '-' + suffix
        if not re.fullmatch(r'cords-e2e-[a-z0-9-]+', self.name):
            raise ValueError('unsafe VM name')
        self.directory = self.run / ('framework-' + suffix)
        self.directory.mkdir()
        self.base = base_image(ubuntu)
        for name in ('bring-up.sh', 'bring-down.sh', 'build.sh', 'copy-log.sh'):
            shutil.copy2(FRAMEWORK / name, self.directory / name)
        (self.directory / 'image-info.conf').write_text(
            f'IMAGE={shlex.quote(self.base.name)}\nVM_MEMORY={6144 if builder else 1536}\n'
            f'VM_VCPUS={2 if builder else 1}\nVM_OS_VARIANT={"ubuntu24.04" if ubuntu else "debian13"}\n'
            f'VM_DISK_ADD={45 if builder else 4}\nVM_NET_MODE=libvirt\nVM_NETWORK=default\n')
        self.disk = ROOT / 'disks' / (self.name + '-run.qcow2')
        self.env = os.environ | {'CORDS_VM_BASE_DIR': str(ROOT / 'bases'), 'CORDS_VM_DISK_DIR': str(ROOT / 'disks')}
        self.key = Path.home() / '.ssh/cgpt/cgpt'
        public = self.key.with_suffix('.pub').read_text().strip()
        test = self.directory / self.name
        (test / 'include').mkdir(parents=True)
        (test / 'meta-data').write_text(f'instance-id: {self.name}\nlocal-hostname: {self.name}\n')
        (test / 'include/bootstrap.sh').write_text(script or '#!/bin/bash\nset -euo pipefail\nprintf \'{"passed":true}\\n\' > /var/log/cords-e2e-result.json\n')
        (test / 'include/bootstrap.sh').chmod(0o755)
        data = f'''#cloud-config
package_update: false
package_upgrade: false
users:
  - name: testuser
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {public}
write_files:
  - path: /etc/apt/apt.conf.d/01cords-cache
    content: 'Acquire::http::Proxy "http://192.168.122.1:3142";'
runcmd:
  - [mount, --mkdir, -o, ro, -L, cidata, /mnt/cidata]
  - [bash, -e, /mnt/cidata/bootstrap.sh]
'''
        (test / 'user-data').write_text(data)
        self.manifest = self.run / (self.name + '.json')
        self.manifest.write_text(json.dumps(dict(name=self.name, disk=str(self.disk), base=str(self.base),
                                                framework=str(self.directory), created=False), indent=2))

    def start(self):
        if subprocess.run(['sudo', '-n', 'virsh', 'dominfo', self.name], capture_output=True).returncode == 0 or self.disk.exists():
            raise RuntimeError('VM/disk collision: ' + self.name)
        record = json.loads(self.manifest.read_text())
        record['created'] = True  # Own partial resources even when bring-up fails.
        self.manifest.write_text(json.dumps(record, indent=2))
        with (self.run / (self.name + '-boot.log')).open('w') as log:
            command(['bash', self.directory / 'bring-up.sh', '--no-console', self.name], env=self.env, stdout=log, stderr=subprocess.STDOUT)
        domain = ET.fromstring(output('sudo', '-n', 'virsh', 'dumpxml', self.name))
        disk = domain.find("./devices/disk[@device='disk']")
        if Path(disk.find('source').get('file')) != self.disk or Path(disk.find('backingStore/source').get('file')) != self.base:
            raise RuntimeError('unexpected disk backing chain')
        self.ip = wait(self.address, 180, self.name + ' DHCP address')
        wait(lambda: subprocess.run(self.ssh_argv('true'), capture_output=True).returncode == 0, 180, self.name + ' SSH')
        self.ssh('cloud-init status --wait --long', timeout=1200)

    def address(self):
        text = output('sudo', '-n', 'virsh', 'domifaddr', self.name)
        match = re.search(r'ipv4\s+(\d+\.\d+\.\d+\.\d+)/', text)
        return match[1] if match else None

    @classmethod
    def recover(cls, manifest):
        record = json.loads(Path(manifest).read_text())
        vm = cls.__new__(cls)
        vm.manifest = Path(manifest)
        vm.run = vm.manifest.parent
        vm.name, vm.disk, vm.base = record['name'], Path(record['disk']), Path(record['base'])
        if not re.fullmatch(r'cords-e2e-[a-z0-9-]+', vm.name) or vm.disk != ROOT / 'disks' / (vm.name + '-run.qcow2'):
            raise RuntimeError('invalid recovery ownership manifest')
        vm.directory = Path(record['framework'])
        if not vm.directory.is_relative_to(vm.run):
            raise RuntimeError('framework outside owned run')
        vm.env = os.environ | {'CORDS_VM_BASE_DIR': str(ROOT / 'bases'), 'CORDS_VM_DISK_DIR': str(ROOT / 'disks')}
        vm.key = Path.home() / '.ssh/cgpt/cgpt'
        vm.ip = None if record.get('cleaned') else (vm.address() if output('sudo', '-n', 'virsh', 'domstate', vm.name) == 'running' else None)
        return vm

    def ssh_argv(self, text):
        return ['ssh', '-i', str(self.key), '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=5',
                '-o', 'StrictHostKeyChecking=accept-new', '-o', f'UserKnownHostsFile={self.run}/known_hosts',
                'testuser@' + self.ip, text]

    def ssh(self, text, **kw):
        return command(self.ssh_argv(text), **kw)

    def stop_collect(self):
        record = json.loads(self.manifest.read_text())
        if not record['created'] or record.get('cleaned'):
            return
        # No offline image reads until libvirt confirms the guest is shut off.
        state = lambda: output('sudo', '-n', 'virsh', 'domstate', self.name).strip()
        if state() != 'shut off':
            command(['sudo', '-n', 'virsh', 'shutdown', self.name])
            wait(lambda: state() == 'shut off', 120, self.name + ' safe shutdown')
        for path in ('/var/log/cloud-init.log', '/var/log/cloud-init-output.log', '/var/log/cords-e2e-result.json'):
            command(['bash', self.directory / 'copy-log.sh', self.name, path], env=self.env)
            extracted = self.directory / 'logs' / self.name / Path(path).name
            if not extracted.is_file() or extracted.stat().st_size == 0:
                raise RuntimeError('required guest artifact missing; preserving disk: ' + path)
        command(['bash', self.directory / 'bring-down.sh', self.name], env=self.env)
        if self.disk.exists() or subprocess.run(['sudo', '-n', 'virsh', 'dominfo', self.name], capture_output=True).returncode == 0:
            raise RuntimeError('cleanup incomplete for ' + self.name)
        record['cleaned'] = True
        self.manifest.write_text(json.dumps(record, indent=2))

    def relocate_legacy_nvram(self):
        if output('sudo', '-n', 'virsh', 'domstate', self.name) != 'shut off':
            raise RuntimeError('NVRAM relocation requires stopped owned guest')
        domain = ET.fromstring(output('sudo', '-n', 'virsh', 'dumpxml', self.name))
        nvram = domain.find('./os/nvram')
        if nvram is None:
            return
        old = Path(nvram.text)
        destination = ROOT / 'disks' / (self.name + '-VARS.qcow2')
        if old == destination:
            return
        if old.parent != Path('/var/lib/libvirt/qemu/nvram') or not old.name.startswith(self.name + '_VARS') or destination.exists():
            raise RuntimeError('unexpected NVRAM ownership/path')
        backup = self.run / (self.name + '-before-nvram-relocation.xml')
        ET.ElementTree(domain).write(backup)
        command(['sudo', '-n', 'mv', '--', old, destination])
        nvram.text = str(destination)
        xml = self.run / (self.name + '-relocated.xml')
        ET.ElementTree(domain).write(xml)
        try:
            command(['sudo', '-n', 'virsh', 'define', xml])
        except Exception:
            command(['sudo', '-n', 'mv', '--', destination, old])
            command(['sudo', '-n', 'virsh', 'define', backup])
            raise


def new_run():
    run = ROOT / 'runs' / ('cords-e2e-' + time.strftime('%Y%m%d-%H%M%S') + '-' + uuid.uuid4().hex[:6])
    run.mkdir()
    return run


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=('infrastructure', 'build-guest'))
    parser.add_argument('--fixture', choices=('normal', 'missing-result', 'provisioning-failure'), default='normal')
    args = parser.parse_args()
    prepare()
    run = new_run()
    results = Results(run)
    print('RUN=' + str(run), flush=True)
    script = {'normal': None, 'missing-result': '#!/bin/bash\nset -euo pipefail\nprintf "Intentionally omitted result\\n"\n',
              'provisioning-failure': '#!/bin/bash\nprintf "Deliberate provisioning failure\\n" >&2\nexit 17\n'}[args.fixture]
    vm = VM(run, 'ubuntu' if args.action == 'build-guest' else 'probe', builder=args.action == 'build-guest', script=script)
    try:
        vm.start()
        results.record('guest-provisioning', 'pass', vm.name)
        if args.action == 'build-guest':
            (run / 'build-guest.json').write_text(json.dumps(dict(name=vm.name, ip=vm.ip, directory=str(vm.directory)), indent=2))
            results.record('build-guest-ready', 'pass', vm.ip)
        else:
            label = output('ls', '-Z', str(vm.disk))
            prepare()
            if output('ls', '-Z', str(vm.disk)) != label:
                raise RuntimeError('repeat setup changed a live disk SELinux label')
            vm.ssh("python3 -c 'import json; assert json.load(open(\"/var/log/cords-e2e-result.json\"))[\"passed\"] is True'")
            results.record('guest-assertion', 'pass')
            vm.stop_collect()
            results.record('shutdown-extraction-cleanup', 'pass')
    except Exception as exc:
        results.record(args.action, 'error', exc)
        try:
            vm.stop_collect()
        except Exception as collection:
            results.record('preserved-failure-disk', 'error', collection)
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
