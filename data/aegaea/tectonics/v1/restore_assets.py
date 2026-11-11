#!/usr/bin/env python3
"""Restore Aegaea's chunked binary assets and verify all original SHA-256 checksums."""
import argparse
import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MANIFEST = json.loads((ROOT / "ASSET_MANIFEST.json").read_text(encoding="utf-8"))["assets"]

def verify(path, entry):
    if not path.is_file():
        return False
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return size == entry["bytes"] and digest.hexdigest() == entry["sha256"]

def restore(entry, verify_only=False):
    path = ROOT / entry["path"]
    if verify(path, entry):
        print("OK", entry["path"])
        return True
    if verify_only or entry["storage"] == "direct":
        print("MISSING/CORRUPT", entry["path"])
        return False

    part_base = ROOT / ".binary-parts" / entry["path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".restoring")
    try:
        with temporary.open("wb") as output:
            for i in range(entry["parts"]):
                part = part_base.with_name(part_base.name + f".part{i:03d}")
                if not part.is_file():
                    raise FileNotFoundError(part)
                with part.open("rb") as source:
                    while block := source.read(1024 * 1024):
                        output.write(block)
        if not verify(temporary, entry):
            raise ValueError(f"SHA-256 or byte-length mismatch for {entry['path']}")
        os.replace(temporary, path)
        print("RESTORED", entry["path"])
        return True
    except Exception as error:
        temporary.unlink(missing_ok=True)
        print("FAILED", entry["path"], error)
        return False

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify-only", action="store_true", help="Require all full original files to exist")
    args = parser.parse_args()
    if not all(restore(entry, args.verify_only) for entry in MANIFEST):
        raise SystemExit(1)
    print(f"Verified {len(MANIFEST)} binary datasets")

if __name__ == "__main__":
    main()
