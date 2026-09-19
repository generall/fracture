#!/usr/bin/env python3
"""Serve the built sandbox locally without a Node toolchain."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--port", type=int, default=8000)
args = parser.parse_args()
web = Path(__file__).resolve().parent.parent / "web"
if not (web / "sandbox.wasm").is_file():
    parser.error("Missing web/sandbox.wasm. Run bash tools/build-web.sh first.")
handler = partial(SimpleHTTPRequestHandler, directory=str(web))
server = ThreadingHTTPServer(("127.0.0.1", args.port), handler)
print(f"Fracture sandbox: http://127.0.0.1:{args.port}", flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
