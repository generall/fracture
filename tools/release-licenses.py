#!/usr/bin/env python3
"""Collect upstream notices from the exact locked dependencies in Cargo's cache."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
packages = {}
for target in ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-apple-darwin"):
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target,
    ], cwd=root))
    packages.update((package["id"], package) for package in metadata["packages"])
output = root / "target/release-tools/THIRD-PARTY-LICENSES.txt"
output.parent.mkdir(parents=True, exist_ok=True)
sections = ["Fracture — third-party Rust dependency notices\n"
            "Includes transitive dependencies and build tooling from Cargo.lock.\n"]
for package in sorted(packages.values(), key=lambda p: (p["name"], p["version"])):
    if package["source"] is None and package["name"] != "quad-snd":
        continue
    directory = Path(package["manifest_path"]).parent
    sections.append(f'\n{"=" * 72}\n{package["name"]} {package["version"]}\n'
                    f'License: {package["license"] or "See included license"}\n'
                    f'Source: {package["repository"] or package["source"] or "vendor/quad-snd"}\n')
    files = {p for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "license*", "licenses/*")
             for p in directory.glob(pattern) if p.is_file()}
    if package.get("license_file"):
        files.add(directory / package["license_file"])
    for path in sorted(files):
        sections.append(f'\n--- {path.name} ---\n{path.read_text(errors="replace")}\n')
output.write_text("".join(sections))
print(f"Wrote {output}")
