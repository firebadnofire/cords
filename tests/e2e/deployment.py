"""Dedicated Cords containers managed through the real cords-up lifecycle."""
import json
import os
from pathlib import Path
import re
import shutil
import socket
import ssl
import subprocess
import sys
import urllib.request

from host import ROOT, command, output, wait


def unused_port():
    with socket.socket() as sock:
        sock.bind(('0.0.0.0', 0))
        return sock.getsockname()[1]


class Deployment:
    def __init__(self, run, source):
        self.run, self.source = Path(run), Path(source)
        self.name = self.run.name
        self.port, self.http_port = unused_port(), unused_port()
        if self.port == self.http_port:
            raise RuntimeError('port reservation collision; retry with a fresh run')
        self.origin = f'https://192.168.86.54:{self.port}'
        self.ca = self.run / 'tls/ca.crt'
        self.env = os.environ | dict(CORDS_INSTANCE=self.name, CORDS_CONF_DIR=str(self.run / 'config'),
                                     CORDS_IMAGE_NAME=self.name + '-server', CORDS_IMAGE_TAG='e2e',
                                     CORDS_PREBUILT_IMAGE=self.name + '-server:e2e',
                                     CORDS_PUBLIC_ORIGIN=self.origin, CORDS_JOIN_POLICY='moderator_approval',
                                     CORDS_PUBLISH=f'127.0.0.1:{self.http_port}:4848')
        self.created = False
        self.proxy = None

    def lifecycle(self, action, *extra):
        command(['bash', self.source / 'cords-up.sh', action, *extra], env=self.env)

    def start(self):
        tls = self.ca.parent
        tls.mkdir(mode=0o700)
        command(['openssl', 'req', '-x509', '-newkey', 'rsa:3072', '-nodes', '-days', '2',
                 '-subj', '/CN=Cords disposable E2E CA', '-addext', 'basicConstraints=critical,CA:TRUE',
                 '-addext', 'keyUsage=critical,keyCertSign,cRLSign', '-keyout', tls / 'ca.key', '-out', self.ca],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        command(['openssl', 'req', '-new', '-newkey', 'rsa:3072', '-nodes', '-subj', '/CN=192.168.86.54',
                 '-keyout', tls / 'server.key', '-out', tls / 'server.csr'],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        (tls / 'extensions').write_text('subjectAltName=IP:192.168.86.54,IP:192.168.122.1\nextendedKeyUsage=serverAuth\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\n')
        command(['openssl', 'x509', '-req', '-in', tls / 'server.csr', '-CA', self.ca, '-CAkey', tls / 'ca.key',
                 '-CAcreateserial', '-days', '2', '-extfile', tls / 'extensions', '-out', tls / 'server.crt'],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for key in tls.glob('*.key'):
            key.chmod(0o600)
        image = self.run / 'image'
        image.mkdir()
        shutil.copy2(ROOT / 'cache/linux-target/release/cords-server', image / 'cords-server')
        shutil.copytree(self.source / 'migrations/postgres', image / 'migrations')
        shutil.copy2(self.source / 'deploy/server.toml', image / 'server.toml')
        (image / 'Dockerfile').write_text('''FROM ubuntu:24.04
RUN groupadd -g 4848 cords && useradd -u 4848 -g 4848 -M cords && mkdir -p /var/lib/cords && chown 4848:4848 /var/lib/cords
COPY --chmod=755 cords-server /usr/local/bin/cords-server
COPY migrations /opt/cords/migrations/postgres
COPY server.toml /opt/cords/server.toml
USER 4848:4848
ENTRYPOINT ["cords-server"]
''')
        command(['docker', 'build', '-t', self.env['CORDS_PREBUILT_IMAGE'], image])
        command(['docker', 'run', '--rm', self.env['CORDS_PREBUILT_IMAGE'], '--help'], stdout=subprocess.DEVNULL)
        self.created = True
        self.lifecycle('up')
        self.start_proxy()
        self.context = ssl.create_default_context(cafile=str(self.ca))
        wait(self.ready, 90, 'TLS server readiness')
        record = dict(instance=self.name, origin=self.origin, port=self.port, http_port=self.http_port,
                      configuration=self.env['CORDS_CONF_DIR'], image=self.env['CORDS_PREBUILT_IMAGE'])
        (self.run / 'deployment.json').write_text(json.dumps(record, indent=2))

    def start_proxy(self, target=None):
        """Replace only this deployment's test-owned proxy, retaining its valid TLS certificate."""
        if self.proxy:
            self.proxy.terminate()
            self.proxy.wait(timeout=15)
            self.proxy = None
        proxy_log = (self.run / 'tls-proxy.log').open('a')
        self.proxy = subprocess.Popen([sys.executable, str(Path(__file__).with_name('tls_proxy.py')), str(self.port),
                                       str(target or self.http_port), str(self.ca.parent / 'server.crt'),
                                       str(self.ca.parent / 'server.key')],
                                      stdout=proxy_log, stderr=subprocess.STDOUT)
        (self.run / 'proxy-process.json').write_text(json.dumps(dict(pid=self.proxy.pid, argv=self.proxy.args)))
        proxy_log.close()

    def ready(self):
        try:
            with urllib.request.urlopen(self.origin + '/health/ready', context=self.context, timeout=3) as response:
                return response.status == 204
        except (OSError, urllib.error.URLError) as exc:
            if str(exc) != getattr(self, 'readiness_error', None):
                print('Readiness pending:', exc, flush=True)
                self.readiness_error = str(exc)
            return False

    def claim_code(self):
        logs = output('docker', 'logs', self.name + '-server')
        for line in logs.splitlines():
            try:
                value = json.loads(line).get('fields', {}).get('claim_code')
                if value and re.fullmatch(r'[A-Za-z0-9_-]{43}', value):
                    return value
            except json.JSONDecodeError:
                continue  # Non-JSON build/runtime lines are handled below.
        logs = re.sub(r'\x1b\[[0-9;]*m', '', logs)
        code = re.search(r'claim_code=([A-Za-z0-9_-]{43})', logs)
        if not code:
            raise RuntimeError('owner bootstrap code unavailable in owned server log')
        return code[1]

    def collect(self):
        for suffix in ('server', 'postgres'):
            result = subprocess.run(['docker', 'logs', self.name + '-' + suffix], text=True, capture_output=True)
            logs = result.stdout + result.stderr
            logs = re.sub(r'("claim_code"\s*:\s*")[^"]+', r'\1[redacted]', logs)
            logs = re.sub(r'claim_code=[A-Za-z0-9_-]{43}', 'claim_code=[redacted]', logs)
            (self.run / (suffix + '.log')).write_text(logs)
            if result.returncode:
                raise RuntimeError('cannot collect ' + suffix + ' logs')

    def close(self):
        from firewall import configure
        configure(self.run, remove=True)
        if self.created:
            self.collect()
            self.lifecycle('down', '--yes')
            self.created = False
        if self.proxy:
            self.proxy.terminate()
            self.proxy.wait(timeout=15)
            self.proxy = None
        record = self.run / 'deployment.json'
        if record.exists():
            value = json.loads(record.read_text())
            value['cleaned'] = True
            record.write_text(json.dumps(value, indent=2))
