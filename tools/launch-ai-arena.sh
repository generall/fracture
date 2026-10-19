#!/usr/bin/env bash
set -euo pipefail
arena_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec python3 - "$arena_dir" "$@" <<'PY'
import hashlib,json,os,subprocess,sys
from pathlib import Path
root=Path(sys.argv[1])
for side in ('blue','red'):
    version=root/'agents'/side
    manifest=json.loads((version/'manifest.json').read_text())
    for name in ('agent','config.json'):
        if hashlib.sha256((version/name).read_bytes()).hexdigest()!=manifest[name]:
            raise SystemExit(f'Frozen {side} AI changed: {name}')
screen=power=None
sc=pc=None
try:
    import dbus
    bus=dbus.SessionBus()
    screen=dbus.Interface(bus.get_object('org.freedesktop.ScreenSaver','/ScreenSaver'),'org.freedesktop.ScreenSaver')
    power=dbus.Interface(bus.get_object('org.freedesktop.PowerManagement','/org/freedesktop/PowerManagement/Inhibit'),'org.freedesktop.PowerManagement.Inhibit')
    sc=screen.Inhibit('Fracture AI battlefield','Watching a four-minute AI round')
    pc=power.Inhibit('Fracture AI battlefield','Watching a four-minute AI round')
    screen.SimulateUserActivity()
except Exception:
    # The viewer also works on Linux desktops without the KDE D-Bus services.
    pass
try:
    child_env=dict(os.environ)
    if '--deck' in sys.argv[2:] or child_env.get('SteamDeck')=='1':
        child_env.setdefault('vblank_mode','0')
    result=subprocess.run([str(root/'fracture-ai-arena'),*sys.argv[2:]],env=child_env)
finally:
    if power is not None and pc is not None: power.UnInhibit(pc)
    if screen is not None and sc is not None: screen.UnInhibit(sc)
raise SystemExit(result.returncode)
PY
