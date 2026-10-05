#!/usr/bin/env python3
"""Real CLI peer for the interactive Tauri smoke test; no protocol implementation."""
import argparse
import json
from pathlib import Path
import runpy
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("--client", type=Path, required=True)
parser.add_argument("--ca", type=Path, required=True)
parser.add_argument("--device", required=True)
parser.add_argument("--origin", default="https://192.168.86.54:5848")
parser.add_argument("--migrations", type=Path, default=Path("migrations/sqlite").resolve())
parser.add_argument("--report", type=Path, default=Path("target/desktop-smoke.json"))
args = parser.parse_args()
acceptance = runpy.run_path(str(Path(__file__).with_name("encrypted-acceptance.py")))
client = acceptance["Installation"](args, Path(tempfile.mkdtemp(prefix="cords-desktop-peer-")))
try:
    client.command("trust", origin=args.origin)
    client.command("authenticate")
    route = client.command("create", name="desktop-smoke")
    client.command("add", route=route, device=args.device)
    client.command("connect")
    print(json.dumps({"route": route, "peer_device": client.initial["device_id"], "ready": True}), flush=True)
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        client.command("sync")
        messages = client.command("history", route=route)
        sent = [m for m in messages if m["sender_device_id"] == args.device and m["body"] == "desktop encrypted smoke"]
        if sent:
            assert len(sent) == 1
            reply = client.command("send", route=route, body="CLI reply to desktop")
            report = {"origin": args.origin, "server_id": pin, "route": route, "desktop_device": args.device,
                      "desktop_message_id": sent[0]["message_id"], "peer_reply_id": reply, "desktop_message_decrypted_once": True}
            args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(report), flush=True)
            break
        time.sleep(1)
    else:
        raise AssertionError("desktop did not send the smoke message")
finally:
    client.kill()
