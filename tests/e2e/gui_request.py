"""Send one atomic request to an active .54 GUI bridge; never implements Cords logic."""
import argparse
import json
from pathlib import Path
import shlex
import subprocess
import time
import uuid


def validate_request(value):
    if value.get('action') not in ('client', 'assert-peer-message', 'finish'):
        raise ValueError('Expected action client, assert-peer-message, or finish')
    if value.get('client', 'a') not in ('a', 'b', 'c'):
        raise ValueError('Expected independent Linux client a, b, or c')
    if value['action'] == 'client':
        if not isinstance(value.get('operation'), str):
            raise ValueError('Client request needs operation')
        fields = value.get('fields', {})
        if not isinstance(fields, dict):
            raise ValueError('Client fields must be an object')
        if value['operation'] == 'send':
            if not isinstance(fields.get('route'), str) or not isinstance(fields.get('body'), str):
                raise ValueError('send requires fields.route and fields.body (not text)')
    if value['action'] == 'assert-peer-message':
        if not isinstance(value.get('route'), str) or not isinstance(value.get('body'), str):
            raise ValueError('Message assertion requires route and body')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--context', required=True, type=Path)
    parser.add_argument('--request', required=True, type=Path)
    args = parser.parse_args()
    context = json.loads(args.context.read_text())
    directory = context['directory']
    if not directory.startswith('/mnt/1tb-disk/cords-e2e/runs/cords-e2e-') or not directory.endswith('/gui'):
        raise RuntimeError('Invalid owned GUI bridge directory')
    validate_request(json.loads(args.request.read_text()))
    name = 'request-' + uuid.uuid4().hex + '.json'
    destination = directory + '/' + name
    subprocess.run(['scp', str(args.request), '192.168.86.54:' + destination + '.pending'], check=True)
    subprocess.run(['ssh', '192.168.86.54', shlex.join(['mv', '--', destination + '.pending', destination])], check=True)
    response = destination.replace('/request-', '/response-')
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        result = subprocess.run(['ssh', '192.168.86.54', shlex.join(['cat', response])], capture_output=True, text=True)
        if result.returncode == 0:
            parsed = json.loads(result.stdout)
            print(json.dumps(parsed, indent=2))
            return 0 if parsed['ok'] else 1
        time.sleep(1)
    raise RuntimeError('GUI bridge response timed out: ' + response)


if __name__ == '__main__':
    raise SystemExit(main())
