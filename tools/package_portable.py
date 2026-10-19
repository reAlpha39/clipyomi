#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Package the release build as a portable Windows zip.

Usage:
    npx tauri build
    python tools/package_portable.py [--tag v0.1.0]

Writes target/portable/ClipYomi-<version>-windows-x64-portable.zip holding
clipyomi.exe, both licence files and a `data` folder. That folder is what
makes the copy portable: the app keeps its settings, dictionary and boundary
hints in it instead of %APPDATA% (see `portable_data_dir` in
src-tauri/src/state.rs). `--tag` fails the run when a release tag does not
match the version in src-tauri/tauri.conf.json.
"""

import argparse
import json
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXE = ROOT / "target" / "release" / "clipyomi.exe"
OUT_DIR = ROOT / "target" / "portable"
TOP = "ClipYomi"

DATA_README = """\
This folder makes ClipYomi portable.

Settings, the dictionary and the boundary hints are stored here instead of in
%APPDATA%, so the whole ClipYomi folder can be moved or copied as it is.
Without this folder, ClipYomi stores them in %APPDATA%\\dev.jparser.clipyomi.
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", help="release tag that must equal v<version>")
    args = parser.parse_args()

    version = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text("utf-8"))["version"]
    if args.tag is not None and args.tag != f"v{version}":
        print(f"tag {args.tag} does not match tauri.conf.json version {version}", file=sys.stderr)
        return 1
    if not EXE.is_file():
        print(f"{EXE} is missing; run `npx tauri build` first", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"ClipYomi-{version}-windows-x64-portable.zip"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.write(EXE, f"{TOP}/clipyomi.exe")
        for licence in ("LICENSE-MIT", "LICENSE-APACHE"):
            zf.write(ROOT / licence, f"{TOP}/{licence}")
        zf.writestr(f"{TOP}/data/README.txt", DATA_README)
    print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
