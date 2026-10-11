#!/usr/bin/env python3
"""Read-only Git inventory. Reports names and object identities, never source text."""

import argparse
import importlib.util
import json
import os
import re
import subprocess
import sys
from pathlib import Path

OWNER_NAMES = (
    "CI_KEY", "CI_KEY_B64", "CI_KEY_PASSPHRASE", "COSIGN_B64", "COSIGN_PASSWORD",
    "GH_KEY", "KEY_ALIAS", "KEY_PASSWORD", "KEYSTORE_BASE64", "KEYSTORE_PASSWORD",
)
PROPERTY = re.compile(r"\bsecrets\s*\.\s*([A-Za-z_][A-Za-z0-9_]*)", re.I)
BRACKET = re.compile(r"\bsecrets\s*\[\s*(['\"])([A-Za-z_][A-Za-z0-9_]*)\1\s*\]", re.I)
CONTEXT = re.compile(r"\bsecrets\b", re.I)
INHERIT = re.compile(r"^\s*secrets\s*:\s*inherit\s*(?:#.*)?$", re.I | re.M)
USES = re.compile(r"^\s*(?:-\s*)?uses\s*:\s*([^\s#]+)", re.M)
WORKFLOW = re.compile(r"^\.(?:forgejo|gitea|github)/workflows/[^/]+\.ya?ml$")
MAX_BLOB = 2 * 1024 * 1024


def references(source):
    direct = []
    covered = []
    for pattern, group in ((PROPERTY, 1), (BRACKET, 2)):
        for match in pattern.finditer(source):
            direct.append({"name": match[group].upper(), "line": source.count("\n", 0, match.start()) + 1})
            covered.append((match.start(), match.end()))
    # Unrecognized context use must never be classified as "no consumers".
    dynamic = [source.count("\n", 0, match.start()) + 1 for match in CONTEXT.finditer(source)
               if not any(start <= match.start() < end for start, end in covered)]
    uses = []
    for match in USES.finditer(source):
        value = match[1].strip("'\"")
        # Never print credentials potentially embedded in a source URL.
        if re.fullmatch(r"(?:https://[A-Za-z0-9.-]+/)?[A-Za-z0-9_./-]+@[A-Za-z0-9_./-]+", value):
            uses.append(value)
        elif value.startswith("./") and re.fullmatch(r"[A-Za-z0-9_./-]+", value):
            uses.append(value)
        else:
            uses.append("UNRESOLVED_OR_REDACTED_REFERENCE")
    env_names = [name for name in OWNER_NAMES if re.search(r"\b" + name + r"\b", source)]
    return {"direct": sorted(direct, key=lambda row: (row["line"], row["name"])),
            "dynamic_context_lines": sorted(set(dynamic)), "inherits_all": bool(INHERIT.search(source)),
            "uses": sorted(set(uses)), "owner_name_mentions": env_names}


def git(repo, *args):
    result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True,
                            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    if result.returncode:
        raise RuntimeError(f"Git {args[0]} failed with exit {result.returncode}; no source/credential buffers logged")
    return result.stdout


def scan(repo):
    refs = []
    raw = git(repo, "for-each-ref", "--format=%(refname)", "refs/heads", "refs/tags").decode()
    commits = {}
    for ref in raw.splitlines():
        commit = git(repo, "rev-parse", "--verify", ref + "^{commit}").decode().strip()
        refs.append({"ref": ref, "commit": commit})
        commits.setdefault(commit, []).append(ref)
    blobs = {}
    skipped = []
    workflow_files = []
    findings = []
    for commit, commit_refs in sorted(commits.items()):
        tree = git(repo, "ls-tree", "-r", "-z", commit)
        for entry in tree.split(b"\0"):
            if not entry:
                continue
            meta, raw_path = entry.split(b"\t", 1)
            mode, kind, oid = meta.decode().split()
            path = raw_path.decode("utf-8", errors="strict")
            if kind != "blob":
                skipped.append({"path": path, "kind": kind, "commit": commit})
                continue
            is_workflow = bool(WORKFLOW.fullmatch(path))
            if oid not in blobs:
                size = int(git(repo, "cat-file", "-s", oid))
                if size > MAX_BLOB:
                    blobs[oid] = None
                else:
                    contents = git(repo, "cat-file", "blob", oid)
                    try:
                        source = contents.decode("utf-8")
                    except UnicodeDecodeError:
                        blobs[oid] = False
                    else:
                        blobs[oid] = references(source) if "\0" not in source else False
            result = blobs[oid]
            if result is None:
                skipped.append({"path": path, "kind": "oversize", "commit": commit})
            elif result is False:
                if is_workflow:
                    raise ValueError("non-UTF8 workflow; manual inspection required")
            elif is_workflow or result["direct"] or result["inherits_all"]:
                row = {"path": path, "blob": oid, "commit": commit, "refs": commit_refs, **result}
                findings.append(row)
                if is_workflow:
                    workflow_files.append({"path": path, "commit": commit, "refs": commit_refs})
    return {"refs": refs, "unique_commit_trees": len(commits), "unique_blobs_inspected": len(blobs),
            "workflow_files": workflow_files, "findings": findings, "skipped": skipped,
            "limits": ["Static references are not runtime secret usage proof.",
                       "External actions, dynamic expressions, submodules and oversized blobs require review.",
                       "All ordinary workflows inherit owner secrets regardless of direct references.",
                       "Only currently advertised branch/tag trees are scanned, not deleted refs or queued historical tasks."]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repositories", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    config = json.loads(args.repositories.read_text(encoding="utf-8"))
    if config.get("instance") != "https://pubcode.archuser.org" or config.get("owner") != "firebadnofire":
        parser.error("unexpected inventory authority")
    names = config["repositories"]
    if not names or len(names) != len(set(names)) or any(
            not re.fullmatch(r"firebadnofire/[A-Za-z0-9_.-]+", name) for name in names):
        parser.error("empty, duplicate or invalid repository inventory")
    args.directory.mkdir(parents=True, exist_ok=True)
    report = {"version": 1, "instance": config["instance"], "owner": config["owner"],
              "owner_secrets": list(OWNER_NAMES), "repositories": []}
    try:
        for index, name in enumerate(names, 1):
            print(f"Inventory {index}/{len(names)}: {name}", flush=True)
            repo = args.directory / (name.split("/")[1] + ".git")
            url = config["instance"] + "/" + name + ".git"
            if repo.exists():
                if git(repo, "config", "--get", "remote.origin.url").decode().strip() != url:
                    raise ValueError("existing inventory origin differs; refusing fetch")
                git(repo, "fetch", "--prune", "origin")
            else:
                result = subprocess.run(["git", "clone", "--mirror", "--quiet", url, str(repo)],
                                        capture_output=True,
                                        env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
                if result.returncode:
                    raise RuntimeError(f"clone failed for {name}, exit {result.returncode}; no credential buffers logged")
            report["repositories"].append({"repository": name, **scan(repo)})
            args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"PASS: all {len(names)} repository ref trees accounted for; review limitations before migration")
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"error: inventory stopped: {error}\n")


if __name__ == "__main__":
    main()
