"""Bounded, run-owned file protocol connecting Windows UIA to independent Linux clients."""
import json
from pathlib import Path
import time
from gui_request import validate_request


def coordinate(scenarios, seconds=1200):
    from firewall import configure
    configure(scenarios.results.directory)
    directory = scenarios.results.directory / 'gui'
    directory.mkdir()
    context = dict(origin=scenarios.deployment.origin, ca=str(scenarios.deployment.ca),
                   route=scenarios.route, server_id=scenarios.pin, directory=str(directory))
    (directory / 'context.json').write_text(json.dumps(context, indent=2))
    print('GUI=' + str(directory / 'context.json'), flush=True)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        for request in sorted(directory.glob('request-*.json')):
            response = request.with_name(request.name.replace('request-', 'response-'))
            if response.exists():
                continue
            try:
                value = validate_request(json.loads(request.read_text()))
                if value['action'] == 'finish':
                    response.write_text(json.dumps(dict(ok=True)))
                    return
                client = dict(a=scenarios.a, b=scenarios.b, c=scenarios.c)[value.get('client', 'a')]
                if value['action'] == 'client':
                    result = client.op(value['operation'], **value.get('fields', {}))
                elif value['action'] == 'assert-peer-message':
                    delivery_deadline = time.monotonic() + 45
                    while True:
                        client.op('sync')
                        history = client.op('history', route=value['route'])
                        if any(m['body'] == value['body'] for m in history):
                            break
                        assert time.monotonic() < delivery_deadline, 'GUI message absent on independent Linux client after 45 seconds'
                        time.sleep(1)
                    result = True
                    scenarios.results.record('windows-gui-to-linux-message', 'pass', value['body'])
                else:
                    raise ValueError('Unknown GUI bridge action')
                response.write_text(json.dumps(dict(ok=True, result=result)))
            except Exception as exc:
                response.write_text(json.dumps(dict(ok=False, error=str(exc))))
                scenarios.results.record('gui-bridge-request', 'fail' if isinstance(exc, AssertionError) else 'error', exc)
                return
        time.sleep(0.25)
    scenarios.results.record('gui-driver', 'unavailable', 'Windows driver did not finish within the bounded session')
