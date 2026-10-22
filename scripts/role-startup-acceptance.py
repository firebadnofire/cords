#!/usr/bin/env python3
"""Real server executable startup gate, run only on the Linux acceptance host.

Uses an explicitly labeled disposable PostgreSQL container and unique schemas.
Database credentials stay in the inherited environment; private server logs and
identity files stay outside the source tree. This checks the intended loopback
HTTP backend, independently of the TLS/MLS acceptance transport.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--container", required=True)
    parser.add_argument("--expected-label", required=True)
    args = parser.parse_args()
    if os.name != "posix" or not args.container.startswith("cords-e2e-"):
        raise RuntimeError("Requires Linux and an explicitly named cords-e2e container")
    label = subprocess.check_output(
        ["docker", "inspect", "--format", '{{index .Config.Labels "cords.acceptance"}}', args.container],
        text=True,
    ).strip()
    if not args.expected_label or label != args.expected_label:
        raise RuntimeError("Disposable database label does not match; no state was changed")
    database = urllib.parse.urlsplit(os.environ["CORDS_TEST_DATABASE_URL"])
    # The inherited URL must address the selected container's published loopback port.
    port = subprocess.check_output(["docker", "port", args.container, "5432/tcp"], text=True).strip()
    if database.hostname != "127.0.0.1" or port != f"127.0.0.1:{database.port}":
        raise RuntimeError("Database URL does not match the disposable loopback container")
    binary, source, output = (path.resolve(strict=True) for path in (args.binary, args.source, args.output))
    if source in output.parents or output == source:
        raise RuntimeError("Private acceptance output must be outside the source tree")
    os.umask(0o077)
    work = output / ("startup-" + uuid.uuid4().hex)
    work.mkdir(mode=0o700)
    schema = "cords_e2e_startup_" + uuid.uuid4().hex
    if not re.fullmatch(r"cords_e2e_startup_[a-f0-9]{32}", schema):
        raise RuntimeError("Invalid owned schema")
    child = None
    checks = []
    report = {"checks": checks, "transport": "loopback HTTP backend", "passed": False}

    def sql(statement):
        try:
            return subprocess.check_output(
                ["docker", "exec", args.container, "sh", "-c",
                 'exec psql -U "${POSTGRES_USER:-postgres}" -d "${POSTGRES_DB:-postgres}" -v ON_ERROR_STOP=1 -At -c "$1"',
                 "sh", statement], text=True, stderr=subprocess.PIPE,
            ).strip()
        except subprocess.CalledProcessError as error:
            raise RuntimeError("Acceptance SQL failed: " + error.stderr.strip()) from None

    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        listen_port = probe.getsockname()[1]
    config = work / "server.toml"
    config.write_text(
        '[server]\nlisten="127.0.0.1:1"\npublic_origin="https://localhost:1"\n'
        'name="Cords startup acceptance"\njoin_policy="moderator_approval"\ndata_dir="unused"\n'
        '[database]\nurl="postgres://invalid/unused"\nmigrations_dir="unused"\n', encoding="utf-8",
    )
    query = urllib.parse.parse_qsl(database.query, keep_blank_values=True)
    if any(key == "options" for key, _ in query):
        raise RuntimeError("Acceptance database URL must not supply search-path options")
    query.append(("options", "-csearch_path=" + schema))
    env = os.environ.copy()
    env["CORDS_DATABASE__URL"] = urllib.parse.urlunsplit(database._replace(query=urllib.parse.urlencode(query)))
    env["RUST_LOG"] = "error"
    command = [str(binary), "--config", str(config), "--listen", f"127.0.0.1:{listen_port}",
               "--public-origin", f"https://localhost:{listen_port}", "--data-dir", str(work / "data"),
               "--migrations-dir", str(source / "migrations/postgres"), "serve"]
    endpoint = f"http://127.0.0.1:{listen_port}/health/ready"

    def stop():
        nonlocal child
        if child is None:
            return
        if child.poll() is None:
            child.send_signal(signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
                raise RuntimeError("Owned server failed graceful shutdown and required termination")
        child = None

    def start(name, flag, ready):
        nonlocal child
        with (work / (name + ".log")).open("wb") as log:
            child = subprocess.Popen(command + [flag], env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        if not ready:
            try:
                code = child.wait(timeout=20)
            except subprocess.TimeoutExpired as error:
                raise RuntimeError("Invalid policy did not stop server startup") from error
            if code == 0:
                raise RuntimeError("Invalid policy unexpectedly succeeded")
            try:
                urllib.request.urlopen(endpoint, timeout=1).close()
            except (OSError, urllib.error.URLError):
                return
            raise RuntimeError("Invalid policy opened a listener")
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if child.poll() is not None:
                raise RuntimeError("Server exited before readiness; inspect the private startup log")
            try:
                with urllib.request.urlopen(endpoint, timeout=1) as response:
                    if response.status == 204:
                        return
            except (OSError, urllib.error.URLError):
                pass
            time.sleep(0.1)
        raise RuntimeError("Server did not become healthy within 30 seconds")

    try:
        sql(f"CREATE SCHEMA {schema}")
        start("fresh", "--migrate", True)
        flags = sql(f"SELECT p.active,m.completed,s.version,(SELECT count(*) FROM {schema}.server_roles) FROM {schema}.role_policy p CROSS JOIN {schema}.rbac_migration m CROSS JOIN {schema}.cords_schema_metadata s")
        if flags != "t|t|22|3":
            raise RuntimeError("Fresh startup did not enforce a fully verified current policy")
        checks.append("fresh normal migrations verify and activate before readiness")
        key_hash = hashlib.sha256((work / "data/server-signing.key").read_bytes()).digest()
        stop()
        start("restart", "--require-current-schema", True)
        if hashlib.sha256((work / "data/server-signing.key").read_bytes()).digest() != key_hash:
            raise RuntimeError("Restart replaced the server identity")
        audit = sql(f"SELECT count(*) FROM {schema}.role_policy_changes")
        if audit != "0":
            raise RuntimeError("Startup invented role policy audit entries")
        checks.append("restart preserves identity and policy without new audit entries")
        stop()
        sql(f"UPDATE {schema}.role_policy SET active=FALSE; UPDATE {schema}.server_roles SET definition='{{}}' WHERE id='builtin:default'")
        start("invalid-policy", "--require-current-schema", False)
        if sql(f"SELECT active FROM {schema}.role_policy") != "f":
            raise RuntimeError("Invalid policy activated despite failed verification")
        checks.append("invalid stored policy exits before opening listener and remains inactive")
        report["passed"] = True
    finally:
        stop()
        sql(f"DROP SCHEMA IF EXISTS {schema} CASCADE")
        temporary = work / "result.json"
        temporary.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        os.replace(temporary, output / "startup-binary.json")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
