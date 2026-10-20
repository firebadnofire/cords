"""Capture exact non-secret source bytes without a commit or push."""
import hashlib
import gzip
import io
import json
from pathlib import Path
import subprocess
import tarfile


def snapshot(root, destination):
    root, destination = Path(root).resolve(), Path(destination).resolve()
    paths = subprocess.check_output(['git', '-C', str(root), 'ls-files', '-z']).decode().split('\0')
    untracked = subprocess.check_output(['git', '-C', str(root), 'ls-files', '--others', '--exclude-standard', '-z']).decode().split('\0')
    # Untracked source must be deliberately included; no arbitrary user files/secrets.
    paths += [p for p in untracked if p.startswith('tests/e2e/') or p in ('test-e2e.sh', 'test-e2e.ps1')]
    entries = {}
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as compressed, tarfile.open(fileobj=compressed, mode='w') as archive:
        for name in sorted(set(filter(None, paths))):
            path = root / name
            if path.is_symlink():
                raise RuntimeError('source symlinks require explicit review: ' + name)
            if not path.is_file():
                continue  # Tracked deletions are represented by absence plus git diff.
            if any(p in ('target', 'dist', '__pycache__', '.git') for p in Path(name).parts):
                continue
            if path.name.startswith('.env') or path.suffix.lower() in ('.key', '.pem', '.pfx'):
                raise RuntimeError('potential secret in source snapshot: ' + name)
            data = path.read_bytes()
            entries[name] = hashlib.sha256(data).hexdigest()
            info = tarfile.TarInfo(name)
            info.size, info.mtime = len(data), 0
            info.mode = 0o755 if path.suffix == '.sh' else 0o644
            archive.addfile(info, io.BytesIO(data))
    with destination.open('rb') as completed:
        archive_digest = hashlib.file_digest(completed, 'sha256').hexdigest()
    manifest = dict(head=subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip(),
                    status=subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True),
                    diff=subprocess.check_output(['git', '-C', str(root), 'diff', 'HEAD', '--binary'], text=True),
                    untracked_included=[p for p in untracked if p in entries], files=entries,
                    archive_sha256=archive_digest)
    destination.with_suffix('.json').write_text(json.dumps(manifest, indent=2) + '\n')
    return manifest


if __name__ == '__main__':
    import sys
    print(json.dumps(snapshot(Path(__file__).resolve().parents[2], sys.argv[1]), indent=2))
