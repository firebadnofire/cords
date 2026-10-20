"""Verify staged SDK bytes and a working-tree snapshot inside the Ubuntu guest."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tarfile

home = Path.home()
for name in ('qt-linux.tar', 'qt-windows.tar'):
    with tarfile.open(home / 'tools' / name) as archive:
        for member in archive:
            if member.isfile():
                actual = home / 'Qt' / member.name
                if not actual.resolve().is_relative_to(home / 'Qt'):
                    raise RuntimeError('SDK path escapes Qt directory')
                if hashlib.file_digest(actual.open('rb'), 'sha256').digest() != hashlib.file_digest(archive.extractfile(member), 'sha256').digest():
                    raise RuntimeError('Extracted SDK differs from archive: ' + member.name)
manifest = json.loads((home / 'tools/input.json').read_text())
archive = home / 'tools/input.tar.gz'
if hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() != manifest['archive_sha256']:
    raise RuntimeError('Guest source archive checksum mismatch')
source = Path(sys.argv[1])
source.mkdir(exist_ok=True)
with tarfile.open(archive) as bundle:
    bundle.extractall(source, filter='data')
for name, digest in manifest['files'].items():
    if hashlib.file_digest((source / name).open('rb'), 'sha256').hexdigest() != digest:
        raise RuntimeError('Guest source file checksum mismatch: ' + name)
    os.utime(source / name, None)
print('SDK and source bytes verified')
