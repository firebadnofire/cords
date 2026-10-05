#!/usr/bin/env python3
"""Real-process acceptance. Protocol operations are exclusively cords-client-core CLI calls."""
import argparse
import base64
import json
import os
from pathlib import Path
import queue
import secrets
import ssl
import subprocess
import tempfile
import threading
import time
import urllib.request

MARKERS = [
    "persistent message from client A",
    "persistent reply from client B",
    "message while client B is offline",
    "live message after client restart",
    "persistent message before server restart",
    "message after server restart",
    "message after PostgreSQL restart",
]
CRASH_MARKERS = ["message recovered from durable outbox", "message recovered after upload acknowledgement"]


class Installation:
    def __init__(self, args, path):
        self.args, self.path = args, path
        self.passphrase = secrets.token_urlsafe(32)
        self.counter = 0
        self.events = []
        self.start()

    def start(self, fault=None):
        self.output = queue.Queue()
        environment = os.environ.copy()
        environment.pop("CORDS_TEST_CRASH_AT", None)
        if fault:
            environment["CORDS_TEST_CRASH_AT"] = fault
        self.process = subprocess.Popen(
            [str(self.args.client), "--state", str(self.path), "--migrations",
             str(self.args.migrations), "--ca", str(self.args.ca), "--passphrase-stdin"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8", bufsize=1, env=environment,
        )
        def read_output():
            for line in self.process.stdout:
                try:
                    self.output.put(json.loads(line))
                except json.JSONDecodeError:
                    self.output.put({"event": "invalid-output"})
            self.output.put({"event": "process-ended"})
        threading.Thread(target=read_output, daemon=True).start()
        self.process.stdin.write(self.passphrase + "\n")
        self.process.stdin.flush()
        ready = self.output.get(timeout=60)
        assert ready.get("event") == "ready", "client failed initialization"
        self.initial = ready["status"]
        if fault:
            assert ready.get("fault_injection"), "runner requires CLI built with --features fault-injection"

    def crash_command(self, operation, **fields):
        self.counter += 1
        self.process.stdin.write(json.dumps({"id": self.counter, "op": operation, **fields}) + "\n")
        self.process.stdin.flush()
        assert self.process.wait(timeout=60) == 73, "expected durable-boundary process exit"

    def command(self, operation, expect_failure=False, **fields):
        self.counter += 1
        self.process.stdin.write(json.dumps({"id": self.counter, "op": operation, **fields}) + "\n")
        self.process.stdin.flush()
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            item = self.output.get(timeout=max(0.1, deadline - time.monotonic()))
            if item.get("id") == self.counter:
                if expect_failure:
                    assert not item.get("ok"), f"{operation} unexpectedly succeeded"
                    return item.get("error")
                assert item.get("ok"), f"{operation} failed: {item.get('error')}"
                return item.get("result")
            assert item.get("event") != "process-ended", "client terminated unexpectedly"
            self.events.append(item)
        raise AssertionError(f"timeout during {operation}")

    def realtime(self, message_id):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            for item in self.events:
                if item.get("event") == "realtime" and any(
                    m["message_id"] == message_id for m in item["result"]["messages"]
                ):
                    return
            self.events.append(self.output.get(timeout=max(.1, deadline - time.monotonic())))
        raise AssertionError("message was not received through a WebSocket-triggered sync")

    def kill(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=15)


def remote(args, command):
    base = "cd /home/william/cords-milestone && docker compose --env-file .env -f deploy/compose.milestone.yaml "
    return subprocess.run(["ssh", "-o", "BatchMode=yes", args.host, base + command],
                          check=True, capture_output=True).stdout


def fingerprint(args):
    context = ssl.create_default_context(cafile=str(args.ca))
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    with urllib.request.urlopen(args.origin + "/.well-known/cords/server", context=context, timeout=10) as response:
        return json.load(response)["server_id"]


def wait_server(args, expected):
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        try:
            observed = fingerprint(args)
        except (OSError, urllib.error.URLError):
            time.sleep(1)
            continue
        assert observed == expected, "server fingerprint changed across restart"
        return observed
    raise AssertionError("server failed to recover")


def marker_forms(marker):
    raw = marker.encode("utf-8")
    forms = {raw, raw.hex().encode(), raw.hex().upper().encode(),
             base64.b64encode(raw), base64.urlsafe_b64encode(raw),
             base64.b64encode(raw).rstrip(b"="), base64.urlsafe_b64encode(raw).rstrip(b"="),
             json.dumps(marker, ensure_ascii=True)[1:-1].encode(),
             json.dumps(marker, ensure_ascii=False)[1:-1].encode(),
             "".join(f"\\u{ord(c):04x}" for c in marker).encode(),
             "".join(f"\\{b:03o}" for b in raw).encode(),
             "".join(f"\\x{b:02x}" for b in raw).encode(),
             marker.replace("\\", "\\\\").replace("'", "''").encode()}
    return forms | {value.replace(b"\\", b"\\\\") for value in forms}


def run(args):
    root = Path(tempfile.mkdtemp(prefix="cords-acceptance-", dir=args.state_parent))
    report = {"origin": args.origin, "state_root": str(root), "checks": []}
    a = b = None
    try:
        before = fingerprint(args)
        report["fingerprint_before"] = before
        a, b = Installation(args, root / "a"), Installation(args, root / "b")
        assert a.initial["account_id"] != b.initial["account_id"]
        assert a.initial["device_id"] != b.initial["device_id"]
        report["client_a"] = a.initial
        report["client_b"] = b.initial
        for client in (a, b):
            client.command("trust", origin=args.origin)
            client.command("authenticate")
            client.command("publish")
        route = a.command("create", name="acceptance-" + secrets.token_hex(4))
        report["route"] = route
        if args.crash_boundaries:
            a.kill(); a.start("response_received.commit")
            a.crash_command("add", route=route, device=b.initial["device_id"])
            a.start(); a.command("sync")
            report["checks"].append("pending_mls_commit_recovered_after_server_ack")
        else:
            a.command("add", route=route, device=b.initial["device_id"])
        b.command("join", route=route)
        for client in (a, b):
            client.command("connect")
        message = a.command("send", route=route, body=MARKERS[0])
        b.realtime(message)
        first_retry = a.command("retry")
        second_retry = a.command("retry")
        assert first_retry == second_retry, "event retry changed durable result"
        message = b.command("send", route=route, body=MARKERS[1])
        a.realtime(message)
        report["checks"].append("bidirectional_realtime_and_idempotency")

        b.command("disconnect")
        a.command("send", route=route, body=MARKERS[2])
        catchup = b.command("connect")
        assert catchup["fetched"] >= 1
        assert sum(m["body"] == MARKERS[2] for m in catchup["messages"]) == 1
        assert b.command("sync")["fetched"] == 0
        report["offline_http_fetched"] = catchup["fetched"]
        report["checks"].append("websocket_interruption_http_exactly_once")

        saved_a, saved_b = a.command("status"), b.command("status")
        a.kill(); b.kill()
        a.start(); b.start()
        assert a.initial == saved_a and b.initial == saved_b, "client state changed on restart"
        for client in (a, b):
            client.command("authenticate")
            client.command("connect")
            history = client.command("history", route=route)
            assert all(sum(m["body"] == marker for m in history) == 1 for marker in MARKERS[:3])
        message = a.command("send", route=route, body=MARKERS[3]); b.realtime(message)
        report["checks"].append("client_process_kill_restart_identity_pin_cursor_cache_mls")
        a.command("send", route=route, body=MARKERS[4])
        remote(args, "kill cords-server")
        remote(args, "up -d --wait cords-server")
        report["fingerprint_after_server_restart"] = wait_server(args, before)
        for client in (a, b):
            client.command("connect")
        message = b.command("send", route=route, body=MARKERS[5]); a.realtime(message)
        report["checks"].append("server_kill_restart_same_fingerprint_history_realtime")

        remote(args, "restart postgres")
        remote(args, "up -d --wait")
        report["fingerprint_after_postgres_restart"] = wait_server(args, before)
        for client in (a, b):
            client.command("connect")
        message = a.command("send", route=route, body=MARKERS[6]); b.realtime(message)
        markers = list(MARKERS)
        if args.crash_boundaries:
            for stage, marker in zip(("outbox_persisted.application", "response_received.application"), CRASH_MARKERS):
                a.command("disconnect"); a.kill(); a.start(stage)
                a.crash_command("send", route=route, body=marker)
                a.start(); a.command("connect"); b.command("sync")
                first = a.command("retry"); repeated = a.command("retry")
                assert first == repeated, "recovered outbox recreated event or ciphertext"
                report["checks"].append("crash_recovered_" + stage)
                report.setdefault("recovered_events", []).append({"event_id": first["envelope"]["event_id"], "sequence": first["server_sequence"]})
            markers.extend(CRASH_MARKERS)
        for client in (a, b):
            client.command("sync")
            history = client.command("history", route=route)
            assert len(history) == len(markers), "history duplicated or omitted messages"
            assert all(sum(m["body"] == marker for m in history) == 1 for marker in markers)
        report["checks"].append("postgres_restart_complete_history_and_continued_mls")
        dump = remote(args, "exec -T postgres pg_dump -U cords -d cords --no-owner --no-privileges")
        counts = [sum(dump.count(form) for form in marker_forms(marker)) for marker in markers]
        assert counts == [0] * len(markers), "plaintext marker found in server database dump"
        report["database_marker_match_counts"] = counts
        report["checks"].append("database_raw_utf8_base64_hex_json_sql_bytea_marker_scan")
        report["final_a"] = a.command("status")
        report["final_b"] = b.command("status")
        report["passed"] = True
    finally:
        for client in (a, b):
            if client:
                client.kill()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--client", type=Path, required=True)
    parser.add_argument("--crash-boundaries", action="store_true")
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--migrations", type=Path, default=Path("migrations/sqlite").resolve())
    parser.add_argument("--origin", default="https://192.168.86.54:5848")
    parser.add_argument("--host", default="192.168.86.54")
    parser.add_argument("--state-parent", type=Path, default=Path(tempfile.gettempdir()))
    parser.add_argument("--report", type=Path, default=Path("target/encrypted-acceptance.json"))
    run(parser.parse_args())
