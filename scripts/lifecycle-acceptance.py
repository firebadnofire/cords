#!/usr/bin/env python3
"""Real-core three-client catch-up, removal and root-revocation acceptance."""
import argparse
import json
from pathlib import Path
import runpy
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--client", type=Path, required=True)
parser.add_argument("--ca", type=Path, required=True)
parser.add_argument("--origin", default="https://192.168.86.54:5848")
parser.add_argument("--host", default="192.168.86.54")
parser.add_argument("--migrations", type=Path, default=Path("migrations/sqlite").resolve())
parser.add_argument("--report", type=Path, default=Path("target/lifecycle-acceptance.json"))
args = parser.parse_args()
helpers = runpy.run_path(str(Path(__file__).with_name("encrypted-acceptance.py")))
root = Path(tempfile.mkdtemp(prefix="cords-lifecycle-"))
clients = []
report = {"origin": args.origin, "state_root": str(root), "passed": False}
markers = ["before additional member", "after additional member", "after member removal", "after device revocation"]
try:
    for name in ("a", "b", "c", "d"):
        client = helpers["Installation"](args, root / name)
        clients.append(client)
        client.command("trust", origin=args.origin)
        client.command("authenticate")
        client.command("publish")
    a, b, c, d = clients
    route = a.command("create", name="lifecycle-acceptance")
    a.command("add", route=route, device=b.initial["device_id"])
    b.command("join", route=route)
    b.command("disconnect")
    a.command("send", route=route, body=markers[0])
    a.command("add", route=route, device=c.initial["device_id"])
    c.command("join", route=route)
    a.command("send", route=route, body=markers[1])
    a.command("remove", route=route, device=c.initial["device_id"])
    a.command("send", route=route, body=markers[2])
    recovered = b.command("connect")
    assert recovered["fetched"] == 5, "expected two commits and three application events"
    assert [m["body"] for m in recovered["messages"]] == markers[:3]
    assert b.command("sync")["fetched"] == 0
    c.command("sync", expect_failure=True)
    c.command("send", expect_failure=True, route=route, body="must never leave this device")
    c.command("revoke")
    a.command("add", route=route, device=d.initial["device_id"])
    d.command("join", route=route)
    d.command("revoke")
    queued = a.command("pending-removals", route=route)
    assert len(queued) == 1 and queued[0]["value"]["revoked_device_id"] == d.initial["device_id"]
    a.command("remove", route=route, device=d.initial["device_id"])
    assert a.command("pending-removals", route=route) == []
    c.command("authenticate", expect_failure=True)
    c.kill(); c.start()
    c.command("authenticate", expect_failure=True)
    c.command("revoke")
    message = a.command("send", route=route, body=markers[3])
    b.realtime(message)
    b.command("sync")
    history = b.command("history", route=route)
    assert [m["body"] for m in history] == markers
    dump = helpers["remote"](args,"exec -T postgres pg_dump -U cords -d cords --no-owner --no-privileges")
    counts = [sum(dump.count(form) for form in helpers["marker_forms"](marker)) for marker in markers]
    assert counts == [0] * len(markers)
    report.update({"passed":True, "server_id":pin, "route":route, "offline_events":recovered["fetched"],
                   "offline_messages":len(recovered["messages"]), "database_marker_match_counts":counts,
                   "final_a":a.command("status"), "final_b":b.command("status"), "revoked_device":c.initial["device_id"]})
finally:
    for client in clients:
        client.kill()
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2) + "\n",encoding="utf-8")
print(json.dumps(report,indent=2))
