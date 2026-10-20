#!/usr/bin/env python3
"""Read-only ownership/storage audit on .54; never deletes discovered resources."""
import argparse
import json
from pathlib import Path
import xml.etree.ElementTree as ET

from host import ROOT, output


def audit():
    manifests = {}
    for path in (ROOT / 'runs').glob('*/cords-e2e-*.json'):
        value = json.loads(path.read_text())
        if {'name', 'disk', 'base', 'framework'} <= value.keys():
            if value['name'] in manifests:
                raise RuntimeError('Duplicate VM ownership: ' + value['name'])
            manifests[value['name']] = value
    guests, unrelated, referenced = [], [], set()
    for name in output('sudo', '-n', 'virsh', 'list', '--all', '--name').splitlines():
        state = output('sudo', '-n', 'virsh', 'domstate', name)
        if not name.startswith('cords-e2e-'):
            unrelated.append(dict(name=name, state=state))
            continue
        if name not in manifests or manifests[name].get('cleaned'):
            raise RuntimeError('VM has no active ownership manifest: ' + name)
        domain = ET.fromstring(output('sudo', '-n', 'virsh', 'dumpxml', name))
        disks = [Path(n.attrib['file']) for n in domain.findall('./devices/disk[@device="disk"]/source')]
        nvram = domain.findtext('./os/nvram')
        for path in disks + ([Path(nvram)] if nvram else []):
            if not path.resolve().is_relative_to(ROOT.resolve()):
                raise RuntimeError('Owned VM disk outside dedicated storage: ' + str(path))
            referenced.add(path.resolve())
        base = Path(manifests[name]['base'])
        if not base.resolve().is_relative_to((ROOT / 'bases').resolve()) or base.stat().st_mode & 0o222:
            raise RuntimeError('Backing base is outside owned storage or writable: ' + str(base))
        chain = None
        if state == 'shut off':
            chain = json.loads(output('qemu-img', 'info', '--backing-chain', '--output=json', manifests[name]['disk']))
            for image in chain:
                path = Path(image['filename'])
                if not path.is_absolute() or not path.resolve().is_relative_to(ROOT.resolve()):
                    raise RuntimeError('Backing chain escapes dedicated storage: ' + str(path))
        guests.append(dict(name=name, state=state, disks=list(map(str, disks)), nvram=nvram, chain=chain))
    extra = [str(path) for path in (ROOT / 'disks').iterdir() if path.is_file() and path.resolve() not in referenced]
    if extra:
        raise RuntimeError('Disk files have no live domain ownership reference; preserve and inspect: ' + ', '.join(extra))
    docker = {}
    for kind in ('container', 'volume', 'network'):
        names = output('docker', kind, 'ls', *(['--all'] if kind == 'container' else []), '--filter',
                       'label=chat.cords.instance', '--format', '{{.Name}}' if kind != 'container' else '{{.Names}}')
        docker[kind] = [name for name in names.splitlines() if name.startswith('cords-e2e-')]
    proxies = []
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            argv = (proc / 'cmdline').read_bytes().split(b'\0')
        except (FileNotFoundError, PermissionError):
            continue
        if any(value.endswith(b'/tls_proxy.py') for value in argv) and any(str(ROOT).encode() in value for value in argv):
            proxies.append(dict(pid=int(proc.name), argv=[v.decode() for v in argv if v]))
    return dict(guests=guests, unrelated_guests=unrelated, docker=docker, proxies=proxies,
                selinux=output('getenforce'))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = audit()
    args.output.write_text(json.dumps(result, indent=2))
    print(json.dumps(dict(report=str(args.output), owned_guests=[g['name'] for g in result['guests']],
                          docker=result['docker'], proxies=result['proxies'], selinux=result['selinux']), indent=2))
