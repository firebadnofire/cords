"""Assertions over real SSH-launched cords-dev processes in separate VMs."""
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import queue
import secrets
import subprocess
import threading
import time
import uuid
import urllib.error
import urllib.request

from host import ROOT, command, wait


class Client:
    def __init__(self, vm, directory, label, password=True, ca=True):
        self.vm, self.directory, self.label = vm, Path(directory), label
        self.password = secrets.token_urlsafe(24) if password else None
        self.ca = ca
        self.counter = 0
        self.start()

    def start(self, crash_at=None):
        self.responses = queue.Queue()
        invocation = f'exec ~/e2e/cords-dev --state ~/e2e/state-{self.label} --migrations ~/e2e/migrations'
        if self.ca:
            invocation += ' --ca ~/e2e/ca.crt'
        if self.password:
            invocation += ' --passphrase-stdin'
        if crash_at is not None:
            if crash_at not in ('outbox_persisted.application', 'response_received.application'):
                raise ValueError('Unsupported native send crash boundary')
            invocation = invocation.replace('exec ', f'exec env CORDS_TEST_CRASH_AT={crash_at} ', 1)
        self.error_log = (self.directory / (self.label + '-stderr.log')).open('a')
        self.process = subprocess.Popen(self.vm.ssh_argv(invocation), stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.error_log, text=True, bufsize=1)

        def read():
            for line in self.process.stdout:
                try:
                    self.responses.put(json.loads(line))
                except json.JSONDecodeError:
                    self.responses.put({'invalid_json': line[:200]})
            self.responses.put({'event': 'process-ended'})

        threading.Thread(target=read, daemon=True).start()
        if self.password:
            self.process.stdin.write(self.password + '\n')
            self.process.stdin.flush()
        ready = self.responses.get(timeout=90)
        assert ready.get('event') == 'ready', f'{self.label}: client initialization failed; see stderr'
        self.initial = ready['status']
        self.fault_injection = ready.get('fault_injection', False)

    def op(self, operation, denied=False, crash_exit=None, **fields):
        self.counter += 1
        identifier = self.counter
        self.process.stdin.write(json.dumps(dict(id=identifier, op=operation, **fields)) + '\n')
        self.process.stdin.flush()
        end = time.monotonic() + 90
        while time.monotonic() < end:
            response = self.responses.get(timeout=max(0.1, end - time.monotonic()))
            if response.get('event') == 'process-ended' and crash_exit is not None:
                actual = self.process.wait(timeout=10)
                assert actual == crash_exit, f'{self.label} {operation}: expected crash exit {crash_exit}, received {actual}'
                return
            assert response.get('event') != 'process-ended', f'{self.label}: process ended during {operation}'
            assert 'invalid_json' not in response, f'{self.label}: invalid response'
            if response.get('id') != identifier:
                continue
            assert crash_exit is None, f'{self.label} {operation}: expected injected termination, received a response'
            with (self.directory / (self.label + '-commands.jsonl')).open('a') as log:
                log.write(json.dumps(dict(operation=operation, ok=response.get('ok'), error=response.get('error'))) + '\n')
            assert response.get('ok') is not denied, f'{self.label} {operation}: expected {"denial" if denied else "success"}; {response.get("error")}'
            return response.get('error') if denied else response.get('result')
        raise AssertionError('operation timed out: ' + operation)

    def close(self, crash=False):
        if self.process.poll() is None:
            if crash:
                self.vm.ssh(f'pkill -KILL -f "^/home/testuser/e2e/cords-dev --state /home/testuser/e2e/state-{self.label} "')
            else:
                self.op('exit')
            self.process.wait(timeout=20)
        self.error_log.close()


def stage(vm, directory, binary, source, ca):
    vm.ssh('mkdir -p ~/e2e')
    ssh = vm.ssh_argv('')
    command(['scp', *ssh[1:-2], binary, 'testuser@' + vm.ip + ':e2e/cords-dev'])
    command(['scp', *ssh[1:-2], ca, 'testuser@' + vm.ip + ':e2e/ca.crt'])
    command(['scp', *ssh[1:-2], '-r', Path(source) / 'migrations/sqlite', 'testuser@' + vm.ip + ':e2e/migrations'])
    expected = hashlib.file_digest(Path(binary).open('rb'), 'sha256').hexdigest()
    vm.ssh(f'test "$(sha256sum ~/e2e/cords-dev | cut -d" " -f1)" = {expected}')
    vm.ssh('~/e2e/cords-dev --help > ~/e2e/client-help.txt')


class Scenarios:
    def __init__(self, vms, deployment, source, results, expectations=None):
        self.vms, self.deployment, self.source, self.results = vms, deployment, Path(source), results
        self.expectations = expectations or {}
        self.clients = []

    def setup(self):
        for index, vm in enumerate(self.vms):
            stage(vm, self.results.directory, ROOT / 'cache/linux-target/release/cords-dev',
                  self.source, self.deployment.ca)
            client = Client(vm, self.results.directory, chr(97 + index))
            self.clients.append(client)
        self.a, self.b, self.c = self.clients
        assert len({c.initial['account_id'] for c in self.clients}) == 3
        assert len({c.initial['device_id'] for c in self.clients}) == 3
        pins = [c.op('trust', origin=self.deployment.origin)['server_id'] for c in self.clients]
        assert len(set(pins)) == 1
        self.pin = pins[0]
        self.a.op('claim-ownership', code=self.deployment.claim_code())
        self.a.op('authenticate')
        for client in (self.b, self.c):
            pending = client.op('request-admission')
            assert pending['admission_state'] == 'pending', pending
            client.op('create', name='must-be-denied', denied=True)
        requests = self.a.op('membership-requests')
        assert {r['device_id'] for r in requests} == {self.b.initial['device_id'], self.c.initial['device_id']}
        self.a.op('reject', device=self.c.initial['device_id'])
        rejected = self.c.op('request-admission')
        assert rejected['admission_state'] in ('rejected', 'pending'), rejected
        self.c.op('create', name='rejection-grants-nothing', denied=True)
        if rejected['admission_state'] == 'rejected':
            assert self.c.op('request-admission')['admission_state'] == 'pending'
        for client in (self.b, self.c):
            self.a.op('approve', device=client.initial['device_id'])
            client.op('authenticate')
        for client in self.clients:
            client.op('publish')
        self.route = self.a.op('create', name='e2e-encrypted')
        self.a.op('add', route=self.route, device=self.b.initial['device_id'])
        self.b.op('join', route=self.route)
        self.a.op('add', route=self.route, device=self.c.initial['device_id'])
        self.c.op('join', route=self.route)
        self.b.op('sync')
        self.results.record('identity-admission-independent-vms', 'pass', {'server_id': self.pin, 'route': self.route})

    def messaging(self):
        marker = 'Cords E2E ' + secrets.token_hex(8)
        self.a.op('connect')
        self.b.op('connect')
        self.a.op('send', route=self.route, body=marker)
        self.b.op('sync')
        received = self.b.op('history', route=self.route)
        assert received[-1]['body'] == self.expectations.get('first_body', marker), 'peer message content mismatch'
        self.b.op('send', route=self.route, body='reply ' + marker)
        self.a.op('sync')
        assert self.a.op('history', route=self.route)[-1]['body'] == 'reply ' + marker
        self.a.op('retry')
        self.a.op('retry')
        self.b.op('disconnect')
        for index in range(20):
            self.a.op('send', route=self.route, body=f'burst-{index}-{marker}')
        self.b.op('connect')
        history = self.b.op('history', route=self.route)
        burst = [m['body'] for m in history if m['body'].startswith('burst-')]
        assert burst == [f'burst-{i}-{marker}' for i in range(20)], 'burst ordering/duplicate mismatch'
        assert len(history) == 22, 'history duplicated or omitted a message'
        def submit(client):
            for index in range(5):
                client.op('send', route=self.route, body=f'concurrent-{client.label}-{index}-{marker}')
        with ThreadPoolExecutor(max_workers=2) as pool:
            for future in [pool.submit(submit, client) for client in (self.b, self.c)]:
                future.result()
        for client in self.clients:
            client.op('sync')
            values = client.op('history', route=self.route)
            assert len(values) == 32 and len({m['message_id'] for m in values}) == 32
            for label in ('b', 'c'):
                assert [m['body'] for m in values if m['body'].startswith(f'concurrent-{label}-')] == [f'concurrent-{label}-{i}-{marker}' for i in range(5)]
        self.results.record('bidirectional-ordering-burst-idempotency-offline', 'pass')

    def public(self):
        route = self.a.op('create-with-mode', name='e2e-public', mode='public')
        for client in (self.b, self.c):
            client.op('join', route=route)
        self.a.op('send', route=route, body='public message from owner')
        self.b.op('sync')
        assert self.b.op('history', route=route)[-1]['body'] == 'public message from owner'
        self.b.op('send', route=route, body='public message from peer')
        self.a.op('sync')
        assert self.a.op('history', route=route)[-1]['body'] == 'public message from peer'
        self.results.record('public-device-signed-bidirectional', 'pass')

    def permissions(self):
        self.b.op('create', name='unauthorized', denied=not self.expectations.get('unauthorized_create_allowed', False))
        catalog = self.a.op('role-catalog')
        self.b.op('role-catalog', denied=True)
        if catalog['revision'] == 0:
            self.a.op('change-role', request=dict(expected_revision=catalog['revision'], request_id=secrets.token_hex(16),
                                                change=dict(operation='initialize')))
            catalog = self.a.op('role-catalog')
        role = dict(id=str(uuid.uuid4()), name='E2E', color='#2277aa', description='Disposable test role',
                    priority=10, permissions=['channel.read'], mentionable=False)
        def change(payload):
            current = self.a.op('role-catalog')
            return self.a.op('change-role', request=dict(expected_revision=current['revision'],
                                                        request_id=secrets.token_hex(16), change=payload))
        change(dict(operation='put_role', role=role))
        role['name'] = 'E2E modified'
        change(dict(operation='put_role', role=role))
        account = self.b.initial['account_id']
        change(dict(operation='assign_role', account_id=account, role_id=role['id']))
        assert role['id'] in self.a.op('account-roles', account=account)['role_ids']
        change(dict(operation='revoke_role', account_id=account, role_id=role['id']))
        assert role['id'] not in self.a.op('account-roles', account=account)['role_ids']
        change(dict(operation='delete_role', role_id=role['id']))
        assert not any(r['id'] == role['id'] for r in self.a.op('role-catalog')['roles'])
        self.results.record('owner-denial-role-catalog-crud-assignment-revocation', 'pass')
        if not catalog['active']:
            self.results.record('active-role-channel-permission-enforcement', 'unavailable',
                                'Inspected client explicitly supports preparatory inactive role policy only')

    def profiles(self):
        account = self.a.initial['account_id']
        current = self.a.op('profile', account=account)
        image = (self.source / 'crates/cords-avatar-codec/tests/fixtures/looping-a.jxl').read_bytes()
        descriptor = dict(sha256=base64.urlsafe_b64encode(hashlib.sha256(image).digest()).decode().rstrip('='),
                          bytes=len(image), width=64, height=64, shape='circle')
        # Fixture dimensions are taken from its checked-in test contract, then validated by the real codec.
        profile = dict(display_name='E2E profile', avatar=descriptor)
        self.a.op('save-profile', profile=profile, image=base64.b64encode(image).decode(), expected_heads=current['state']['heads'])
        observed = self.b.op('profile', account=account)
        assert observed['state']['agreed'] == profile, 'cross-client profile mismatch'
        encoded = self.b.op('profile-image', account=account, sha256=descriptor['sha256'])
        assert base64.b64decode(encoded.split(',')[-1]) == image
        self.a.op('save-profile', profile=profile, image=base64.b64encode(b'invalid image').decode(),
                  expected_heads=observed['state']['heads'], denied=True)
        self.a.op('save-profile', profile=dict(display_name='x' * 10000, avatar=None),
                  expected_heads=observed['state']['heads'], denied=True)
        replacement = dict(display_name='E2E replacement', avatar=descriptor | dict(shape='square'))
        self.a.op('save-profile', profile=replacement, image=base64.b64encode(image).decode(),
                  expected_heads=observed['state']['heads'])
        assert self.c.op('profile', account=account)['state']['agreed'] == replacement
        self.a.op('save-profile', profile=replacement, image='A' * 6_666_672,
                  expected_heads=self.c.op('profile', account=account)['state']['heads'], denied=True)
        assert self.b.op('profile', account=account)['state']['agreed'] == replacement, 'Denied oversized avatar changed profile'
        refused = self.b.op('test-publish-foreign-profile', account=account, denied=True)
        assert 'CORDS_PERMISSION_DENIED' in refused, 'Foreign profile did not reach server authorization: ' + refused
        assert self.c.op('profile', account=account)['state']['agreed'] == replacement, 'Foreign profile publication changed victim state'
        self.saved_profile = replacement
        self.results.record('signed-profile-avatar-cross-client', 'pass')

    def resilience(self):
        saved = self.b.op('status')
        self.b.close(crash=True)
        self.b.start()
        assert self.b.initial['account_id'] == saved['account_id']
        self.b.op('authenticate')
        self.b.op('connect')
        for boundary in ('outbox_persisted.application', 'response_received.application'):
            self.b.close()
            self.b.start(crash_at=boundary)
            assert self.b.fault_injection, 'Build lacks the explicitly required development crash hooks'
            marker = 'interrupted native send at ' + boundary
            self.b.op('send', route=self.route, body=marker, crash_exit=73)
            self.b.close()
            self.b.start()
            self.b.op('connect')
            self.b.op('retry')
            self.b.op('retry')
            for client in (self.a, self.b, self.c):
                client.op('sync')
                matching = [message for message in client.op('history', route=self.route) if message['body'] == marker]
                assert len(matching) == 1, f'{client.label}: interrupted send duplicated or lost at {boundary}'
            self.results.record('interrupted-send-' + boundary, 'pass')
        before = self.b.op('history', route=self.route)
        # Schedule recovery inside this owned guest before cutting its network.
        interface = self.b.vm.ssh("ip -o route show default | awk '{print $5; exit}'", stdout=subprocess.PIPE).stdout.strip()
        assert interface.isalnum(), 'unexpected guest network interface'
        timeline = self.results.directory / 'network-faults.jsonl'
        with timeline.open('a') as log:
            log.write(json.dumps(dict(guest=self.b.vm.name, fault='link-down', seconds=6, timestamp=time.time())) + '\n')
        self.b.vm.ssh(f"sudo systemd-run --unit=cords-e2e-link-fault --on-active=1 /bin/bash -c 'ip link set dev {interface} down; sleep 6; ip link set dev {interface} up'")
        time.sleep(2)
        self.a.op('send', route=self.route, body='during owned guest network outage')
        time.sleep(7)
        wait(lambda: subprocess.run(self.b.vm.ssh_argv('true'), capture_output=True).returncode == 0, 30, 'guest network recovery')
        self.b.op('connect')
        assert self.b.op('history', route=self.route)[-1]['body'] == 'during owned guest network outage'
        # Only this run's disposable guest is affected. Schedule fail-safe removal
        # before adding netem, so loss of the controlling SSH session recovers too.
        self.b.vm.ssh(f"sudo systemd-run --unit=cords-e2e-netem-recovery --on-active=60 /usr/sbin/tc qdisc del dev {interface} root")
        with timeline.open('a') as log:
            log.write(json.dumps(dict(guest=self.b.vm.name, fault='netem', delay_ms=100,
                                      loss_percent=10, timestamp=time.time())) + '\n')
        try:
            self.b.vm.ssh(f'sudo tc qdisc add dev {interface} root netem delay 100ms loss 10%')
            self.a.op('send', route=self.route, body='delivery through owned guest latency and loss')
            self.b.op('sync')
            assert self.b.op('history', route=self.route)[-1]['body'] == 'delivery through owned guest latency and loss'
        finally:
            self.b.vm.ssh(f'sudo tc qdisc del dev {interface} root')
            self.b.vm.ssh('sudo systemctl stop cords-e2e-netem-recovery.timer')
            with timeline.open('a') as log:
                log.write(json.dumps(dict(guest=self.b.vm.name, fault='netem-removed', timestamp=time.time())) + '\n')
        self.results.record('owned-guest-latency-packet-loss-delivery', 'pass')
        before = self.b.op('history', route=self.route)
        self.deployment.lifecycle('restart')
        for client in (self.a, self.b):
            assert client.op('trust', origin=self.deployment.origin)['server_id'] == self.pin
            client.op('connect')
        assert self.b.op('history', route=self.route) == before
        self.a.op('send', route=self.route, body='after durable server restart')
        self.b.op('sync')
        assert self.b.op('history', route=self.route)[-1]['body'] == 'after durable server restart'
        if hasattr(self, 'saved_profile'):
            assert self.b.op('profile', account=self.a.initial['account_id'])['state']['agreed'] == self.saved_profile
        self.results.record('client-crash-server-database-restart-persistence', 'pass')

    def security(self):
        from deployment import Deployment
        # Substitute a real independently signed server behind the same valid
        # TLS endpoint. Rust must reject the changed persistent signing key.
        alternate_run = ROOT / 'runs' / (self.results.directory.name + '-substitution')
        alternate_run.mkdir()
        (self.results.directory / 'substitution-resource.json').write_text(json.dumps(dict(directory=str(alternate_run))))
        alternate = Deployment(alternate_run, self.source)
        try:
            alternate.start()
            self.deployment.start_proxy(alternate.http_port)
            wait(self.deployment.ready, 30, 'substitution TLS endpoint')
            denial = self.a.op('trust', origin=self.deployment.origin, denied=True)
            assert 'CORDS_SERVER_KEY_CHANGED' in denial, 'Substitution did not reach the native signing-key pin check: ' + denial
        finally:
            self.deployment.start_proxy()
            wait(self.deployment.ready, 30, 'restored original TLS endpoint')
            alternate.close()
        assert self.a.op('trust', origin=self.deployment.origin)['server_id'] == self.pin
        self.results.record('valid-tls-server-signing-key-substitution-denied', 'pass')
        self.wire_security()
        untrusted = Client(self.vms[2], self.results.directory, 'untrusted', ca=False)
        try:
            untrusted.op('trust', origin=self.deployment.origin, denied=True)
        finally:
            untrusted.close()
        self.a.op('send', route=self.route, body='')
        self.c.op('sync')
        assert self.c.op('history', route=self.route)[-1]['body'] == '', 'empty body boundary changed'
        self.a.op('send', route=self.route, body='x' * 100000, denied=True)
        self.b.op('create', name='still denied after reconnect', denied=True)
        self.a.op('remove', route=self.route, device=self.b.initial['device_id'])
        before = self.a.op('history', route=self.route)
        self.b.op('sync')
        assert not any(c['channel_id'] == self.route for c in self.b.op('channels'))
        self.b.op('send', route=self.route, body='must not be accepted', denied=True)
        self.b.close()
        self.b.start()
        self.b.op('send', route=self.route, body='still excluded after restart', denied=True)
        self.a.op('sync')
        assert self.a.op('history', route=self.route) == before, 'Denied sends had an unauthorized effect'
        self.results.record('invalid-tls-boundaries-mls-removal-restart-denials', 'pass')

    def wire_security(self):
        """Negative wire probes only: no client authentication/signature logic in Python."""
        before = self.a.op('channels')
        probes = [
            ('GET', '/api/v1/channels', None, None, 403),
            ('GET', '/api/v1/channels', 'invalid-test-session', None, 403),
            ('GET', '/api/v1/channels', 'x' * 129, None, 403),
            ('POST', '/api/v1/auth/session', None, b'{broken', 400),
            ('POST', '/api/v1/auth/challenge', None, b'[]', 422),
        ]
        for method, path, token, body, expected in probes:
            headers = {'Content-Type': 'application/json'}
            if token:
                headers['Authorization'] = 'Bearer ' + token
            request = urllib.request.Request(self.deployment.origin + path, data=body, headers=headers, method=method)
            try:
                with urllib.request.urlopen(request, context=self.deployment.context, timeout=15) as response:
                    status = response.status
            except urllib.error.HTTPError as response:
                status = response.code
                response.close()
            assert status == expected, f'{method} {path}: expected HTTP {expected}, received {status}'
            with (self.results.directory / 'wire-negatives.jsonl').open('a') as log:
                log.write(json.dumps(dict(method=method, path=path, token_kind='invalid' if token else 'absent',
                                          status=status, timestamp=time.time())) + '\n')
        assert self.a.op('channels') == before, 'Negative wire probes changed authorized channel state'
        self.results.record('invalid-sessions-malformed-json-no-effect', 'pass')

    def close(self):
        errors = []
        for client in self.clients:
            try:
                client.close()
            except Exception as exc:
                errors.append(str(exc))
        if errors:
            raise RuntimeError('client cleanup: ' + '; '.join(errors))
