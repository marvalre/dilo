#!/usr/bin/env python3
"""Writes latest.json, the file the in-app updater reads.
Usage: make-manifest.py <version> <base_url> <notes_file|-> <out.json>
Reads the signature from src-tauri/target/release/bundle/macos/Dilo.app.tar.gz.sig."""
import json, sys, datetime, pathlib
version, base_url, notes_file, out = sys.argv[1:5]
root = pathlib.Path(__file__).resolve().parent.parent
sig = (root / "src-tauri/target/release/bundle/macos/Dilo.app.tar.gz.sig").read_text().strip()
notes = "" if notes_file == "-" else pathlib.Path(notes_file).read_text().strip()
url = f"{base_url.rstrip('/')}/Dilo.app.tar.gz"
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {"darwin-aarch64": {"signature": sig, "url": url}},
}
pathlib.Path(out).write_text(json.dumps(manifest, indent=2) + "\n")
print(f"wrote {out} ({version} -> {url})")
