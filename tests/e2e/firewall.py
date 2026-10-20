"""Ephemeral firewall access to one manifest-owned TLS port from the SSH workstation only."""
import argparse
import ipaddress
import json
import os
import subprocess
from pathlib import Path
from host import ROOT, command


def configure(run, remove=False):
    run = Path(run).resolve()
    if run.parent != ROOT / 'runs' or not run.name.startswith('cords-e2e-'):
        raise RuntimeError('Firewall target must be an owned run')
    record = run / 'firewall.json'
    if remove:
        if record.exists():
            saved = json.loads(record.read_text())
            if saved.get('removed'):
                return
            query = subprocess.run(['sudo', '-n', 'firewall-cmd', '--zone=public', '--query-rich-rule=' + saved['rule']],
                                   capture_output=True, text=True)
            if query.returncode == 0:
                command(['sudo', '-n', 'firewall-cmd', '--zone=public', '--remove-rich-rule=' + saved['rule']])
            elif query.returncode != 1:
                raise RuntimeError('Cannot inspect owned firewall rule: ' + query.stderr)
            saved['removed'] = True
            record.write_text(json.dumps(saved, indent=2))
        return
    source = str(ipaddress.IPv4Address(os.environ['SSH_CONNECTION'].split()[0]))
    deployment = json.loads((run / 'deployment.json').read_text())
    if deployment['instance'] != run.name or not 1024 <= deployment['port'] <= 65535:
        raise RuntimeError('Invalid owned deployment port')
    rule = f'rule family="ipv4" source address="{source}/32" port port="{deployment["port"]}" protocol="tcp" accept'
    command(['sudo', '-n', 'firewall-cmd', '--zone=public', '--timeout=1200', '--add-rich-rule=' + rule])
    record.write_text(json.dumps(dict(rule=rule, removed=False, expires_seconds=1200), indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run')
    parser.add_argument('--remove', action='store_true')
    args = parser.parse_args()
    configure(args.run, args.remove)
