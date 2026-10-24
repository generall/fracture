#!/usr/bin/env python3
"""Verify the exact four release archives, executable modes, provenance and hashes."""
import hashlib
import json
from pathlib import Path
import plistlib
import subprocess
import tarfile
import zipfile

import importlib.util

spec = importlib.util.spec_from_file_location("release_packaging", Path(__file__).with_name("package-release.py"))
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)
root = packaging.ROOT
version = packaging.version()
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()


def verify_checksum(path):
    expected = path.with_name(path.name + ".sha256").read_text().strip()
    actual = f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}"
    if actual != expected:
        raise SystemExit(f"Checksum mismatch: {path}")


for platform, suffix in (("linux", "linux-x86_64.tar.gz"), ("steamdeck", "steamdeck-x86_64.tar.gz"),
                         ("macos-arm64", "macos-arm64.zip"), ("macos-x86_64", "macos-x86_64.zip")):
    name = f"fracture-{version}-{suffix}"
    archive = root / "target/dist" / name
    verify_checksum(archive)
    prefix = name.removesuffix(".tar.gz").removesuffix(".zip") + "/"
    files = {}
    modes = {}
    if name.endswith(".zip"):
        with zipfile.ZipFile(archive) as package:
            if package.testzip() is not None:
                raise SystemExit(f"Corrupt archive: {archive}")
            for member in package.infolist():
                files[member.filename] = package.read(member)
                modes[member.filename] = member.external_attr >> 16
    else:
        with tarfile.open(archive) as package:
            for member in package.getmembers():
                if member.isfile():
                    files[member.name] = package.extractfile(member).read()
                    modes[member.name] = member.mode
    if any(not name.startswith(prefix) or ".." in Path(name).parts for name in files):
        raise SystemExit(f"Unsafe paths in {archive}")
    metadata = json.loads(files[prefix + "BUILD.json"])
    if metadata.get("source_dirty", True):
        raise SystemExit(f"{archive} was built with uncommitted changes; rebuild from a clean commit")
    if (metadata["version"], metadata["commit"], metadata["platform"]) != (version, commit, platform):
        raise SystemExit(f"Stale package {archive}: {metadata}; rebuild from current HEAD")
    for license_name in (*packaging.LICENSES.values(), "SDL2-LICENSE.txt", "THIRD-PARTY-LICENSES.txt"):
        assert files[prefix + "licenses/" + license_name], license_name
    if platform.startswith("macos"):
        for app, binary in (("Fracture", "game"), ("Fracture Sandbox", "sandbox")):
            base = prefix + f"{app}.app/Contents/"
            info = plistlib.loads(files[base + "Info.plist"])
            assert info["CFBundleShortVersionString"] == version
            for name in (f"MacOS/{binary}", "MacOS/" + info["CFBundleExecutable"], "Frameworks/libSDL2.dylib"):
                assert modes[base + name] & 0o111, name
    else:
        for name in ("game", "sandbox", "fracture.sh", "fracture-deck.sh", "sandbox.sh", "install.sh", "lib/libSDL2-2.0.so.0"):
            assert modes[prefix + name] & 0o111, name
        assert (prefix + "STEAMDECK" in files) == (platform == "steamdeck")
    print(f"Verified {archive.name}")

source = root / "target/dist" / f"fracture-{version}-source.tar.gz"
verify_checksum(source)
with tarfile.open(source) as archive:
    manifest = archive.extractfile(f"fracture-{version}-source/Cargo.toml").read()
    assert f'version = "{version}"'.encode() in manifest
    expected_manifest = subprocess.check_output(["git", "show", "HEAD:Cargo.toml"], cwd=root)
    assert manifest == expected_manifest
print(f"Verified {source.name}")
