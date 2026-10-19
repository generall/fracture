#!/usr/bin/env python3
"""Run on the Deck: silent, foreground before/after scout frame captures."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

import dbus


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def windows():
    return re.findall(r"0x[0-9a-fA-F]+", output("xprop", "-root", "_NET_CLIENT_LIST"))


def interrupt(_signum, _frame):
    raise KeyboardInterrupt


def capture(binary, label, scenario, seconds, destination, screensaver):
    report = destination / f"{label}-{scenario}.json"
    report.unlink(missing_ok=True)
    log = destination / f"{label}-{scenario}.log"
    env = dict(os.environ, FRACTURE_SILENT="1",
               FRACTURE_DATA_DIR=str(destination / "isolated-save"),
               FRACTURE_PERF_SECONDS=str(seconds), FRACTURE_PERF_OUTPUT=str(report),
               FRACTURE_PERF_SCENARIO="river-scout" + ("-combat" if scenario == "combat" else ""))
    old_windows = set(windows())
    screensaver.SimulateUserActivity()
    with log.open("w") as stream:
        process = subprocess.Popen([str(binary), "--deck"], env=env, stdout=stream, stderr=stream)
        try:
            window = None
            started = time.monotonic()
            corrections = 0
            while process.poll() is None:
                if time.monotonic() - started > seconds + 40:
                    raise RuntimeError(f"capture timed out: {log}")
                if window is None:
                    for candidate in windows():
                        if candidate not in old_windows and "Fracture" in output("xprop", "-id", candidate, "_NET_WM_NAME"):
                            window = candidate
                            break
                if window:
                    active = output("xprop", "-root", "_NET_ACTIVE_WINDOW")
                    if window not in re.findall(r"0x[0-9a-fA-F]+", active):
                        try:
                            subprocess.run(["xdotool", "windowactivate", "--sync", window], check=True, timeout=5)
                        except subprocess.CalledProcessError:
                            # The report is written before the game destroys its
                            # window. Do not reactivate a window during shutdown.
                            if not report.exists():
                                raise
                            process.wait(timeout=10)
                            break
                        corrections += 1
                time.sleep(0.25)
            if process.returncode != 0 or not report.exists() or window is None:
                raise RuntimeError(f"capture failed: {log.read_text()}")
            result = json.loads(report.read_text())
            result["capture_conditions"] = {"idle_inhibited": True, "focus_corrections": corrections}
            report.write_text(json.dumps(result))
            print(label, scenario, json.dumps({"fps": result["average_fps"],
                  "over_50ms": result["over_50ms"], "frame": result["stages"]["frame"],
                  "simulation_seconds": result["simulation_seconds"]}), flush=True)
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=30)
    parser.add_argument("--scenario", choices=["both", "drive", "combat"], default="both")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    signal.signal(signal.SIGTERM, interrupt)
    bus = dbus.SessionBus()
    screensaver = dbus.Interface(bus.get_object("org.freedesktop.ScreenSaver", "/ScreenSaver"), "org.freedesktop.ScreenSaver")
    power = dbus.Interface(bus.get_object("org.freedesktop.PowerManagement", "/org/freedesktop/PowerManagement/Inhibit"), "org.freedesktop.PowerManagement.Inhibit")
    screen_cookie = screensaver.Inhibit("Fracture validation", "Temporary silent performance capture")
    power_cookie = None
    player_pid = 0
    try:
        power_cookie = power.Inhibit("Fracture validation", "Temporary silent performance capture")
        player_pid = int(output("systemctl", "--user", "show", "fracture-game.service", "--property=MainPID", "--value") or "0")
        if player_pid:
            os.kill(player_pid, signal.SIGSTOP)
        scenarios = ["drive", "combat"] if args.scenario == "both" else [args.scenario]
        for scenario in scenarios:
            for label, binary in [("before", args.before), ("after", args.after)]:
                capture(binary, label, scenario, args.seconds, args.output, screensaver)
    finally:
        if player_pid:
            try:
                os.kill(player_pid, signal.SIGCONT)
            except ProcessLookupError:
                pass
        if power_cookie is not None:
            power.UnInhibit(power_cookie)
        screensaver.UnInhibit(screen_cookie)


if __name__ == "__main__":
    main()
