"""Compatibility entry point for the versioned Apple Silicon app package."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
subprocess.run([
    sys.executable, str(root / "tools/package-release.py"), "macos-arm64",
    "--sdl", str(root / "target/release-tools/sdl-macos-arm64/libSDL2.dylib"),
], check=True)
