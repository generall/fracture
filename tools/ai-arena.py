#!/usr/bin/env python3
"""Freeze AI executables and run recorded four-minute winner/challenger rounds."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import base64
import math
import html
from collections import deque

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(version):
    manifest = json.loads((version / "manifest.json").read_text())
    for filename in ("agent", "config.json", "source.tar.gz"):
        if digest(version / filename) != manifest[filename]:
            raise SystemExit(f"Frozen version changed: {version / filename}")
    return manifest


def freeze(args):
    destination = args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    shutil.copy2(args.binary, destination / "agent")
    shutil.copy2(args.config, destination / "config.json")
    with tarfile.open(destination / "source.tar.gz", "w:gz") as archive:
        for name in ("crates/sim", "Cargo.toml", "Cargo.lock"):
            archive.add(ROOT / name, arcname=name)
    manifest = {name: digest(destination / name) for name in
                ("agent", "config.json", "source.tar.gz")}
    manifest["name"] = json.loads(args.config.read_text())["name"]
    if args.parent:
        manifest["parent"] = validate(args.parent.resolve())
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))


def replay(directory, summary):
    def source(name):
        path = directory / name
        return path.open("rt") if path.exists() else gzip.open(str(path) + ".gz", "rt")
    with source("frames.jsonl") as stream:
        frames = [json.loads(line) for line in stream]
    data = {"frames": frames, "summary": summary}
    for key in ("geometry", "map", "terrain", "events"):
        with source(key + ".json") as stream:
            data[key] = json.load(stream)
    for frame in frames:
        for view in frame["views"]:
            view["regions"] = [{"origin":r["origin"], "boundary":r["boundary"]} for r in view["regions"]]
    def visual_precision(value):
        if isinstance(value, float):
            return round(value, 3)
        if isinstance(value, list):
            return [visual_precision(v) for v in value]
        if isinstance(value, dict):
            return {k:visual_precision(v) for k,v in value.items()}
        return value
    packed = base64.b64encode(gzip.compress(json.dumps(visual_precision(data), separators=(",", ":")).encode())).decode()
    template = (ROOT / "tools/ai/replay.html").read_text()
    (directory / "replay.html").write_text(template.replace("/*MATCH_GZIP*/''", json.dumps(packed)))


def analyze(directory, summary):
    """Count stationary clearing stalls too; deliberate combat holds are separate."""
    history = [deque() for _ in range(summary.get("players_per_team",1)*2)]
    stalled = [0, 0]
    actor_stalls = [0] * len(history)
    actor_samples = [[], []]
    samples = [[], []]
    source = directory / "decisions.jsonl"
    opener = open if source.exists() else gzip.open
    if not source.exists():
        source = Path(str(source) + ".gz")
    for line in opener(source, "rt"):
        row = json.loads(line)
        side, tick = row["side"], row["tick"]
        o, t = row["observation"], row["decision"]["telemetry"]
        h = history[row.get("player",side)]
        if not o["alive"] or t["mode"] == "combat":
            h.clear()
            continue
        h.append((tick, o["position"]))
        while h and h[0][0] < tick - 360:
            h.popleft()
        if h and tick - h[0][0] >= 360 and max(math.dist(p, o["position"]) for _, p in h) < 0.75:
            stalled[side] += 1
            actor_stalls[row.get("player",side)] += 1
            if tick % 120 == 0:
                samples[side].append([tick / 120, *o["position"]])
                actor_samples[side].append({"player":row.get("player",side),"seconds":tick/120,"position":o["position"]})
    for side in range(2):
        summary["sides"][side]["stuck_seconds"] = stalled[side] / 10
        summary["sides"][side]["stuck_locations"] = samples[side]
        summary["sides"][side]["stuck_actors"] = actor_samples[side]
    summary["player_stall_seconds"] = [count/10 for count in actor_stalls]


def write_index(directory):
    rows=[]
    for report in sorted(directory.glob("round-*/summary.json")):
        s=json.loads(report.read_text())
        winner="tie" if s["winner"] is None else s["versions"][s["winner"]]["name"]
        values=[report.parent.name+f" ({s.get("players_per_team",1)}v{s.get("players_per_team",1)}, {s["seconds"]}s)",s["level"]," / ".join(v["name"] for v in s["versions"]),
                winner," / ".join(str(v["kills"]) for v in s["sides"]),
                " / ".join(f'{v["stuck_seconds"]:.1f}s' for v in s["sides"])]
        cells="".join("<td>"+html.escape(v)+"</td>" for v in values)
        rows.append(f'<tr>{cells}<td><a href="{report.parent.name}/replay.html">Replay</a> · <a href="{report.parent.name}/summary.json">Metrics</a></td></tr>')
    (directory/"index.html").write_text('''<!doctype html><meta charset="utf-8"><title>Fracture AI rounds</title>
<style>body{background:#111b25;color:#e6ecf2;font:16px system-ui;padding:30px}table{border-collapse:collapse}td,th{padding:14px;border-bottom:1px solid #40566b;text-align:left}a{color:#74c3ff}p{max-width:900px;line-height:1.6}</style>
<h1>Fracture AI rounds</h1><p>New rounds default to four minutes; historical rounds retain their recorded duration. The previous winner is frozen; the challenger improves the previous loser. Replays provide seeking, playback speed and both visibility views. Headless CPU timing is separate from rendered FPS. Stationary clearing and turn stalls remain marked for review.</p>
<table><tr><th>Round</th><th>Map</th><th>Blue / Red</th><th>Winner</th><th>Team kills</th><th>Stall time</th><th>Recording</th></tr>'''+"".join(rows)+"</table>")


def run(args):
    versions = [args.blue.resolve(), args.red.resolve()]
    manifests = [validate(v) for v in versions]
    if manifests[0]["agent"] == manifests[1]["agent"] and manifests[0]["config.json"] == manifests[1]["config.json"]:
        raise SystemExit("The two AI versions must differ")
    if args.previous:
        previous = json.loads(args.previous.read_text())
        winner = previous["winner"]
        # A tied round retains the incumbent, which is always Blue.
        expected = previous["frozen_versions"][0 if winner is None else winner]
        if manifests[0] != expected:
            raise SystemExit("Blue must be the unchanged previous winner (manifest mismatch)")
        if manifests[1] == expected:
            raise SystemExit("Red must be a different challenger")
        loser = previous["frozen_versions"][1 if winner in (None, 0) else 0]
        if manifests[1].get("parent") != loser:
            raise SystemExit("Freeze the challenger with --parent pointing to the previous loser")
    directory = args.output.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    command = [str(args.binary.resolve()), str(args.map.resolve()), str(versions[0] / "agent"),
               str(versions[0] / "config.json"), str(versions[1] / "agent"),
               str(versions[1] / "config.json"), str(directory), str(args.seconds), str(args.seed)]
    if args.summary_only:
        command.append("--summary-only")
    with (directory / "run.log").open("w") as log:
        subprocess.run(command, check=True, env=dict(os.environ, FRACTURE_SILENT="1"), stdout=log)
    summary = json.loads((directory / "summary.json").read_text())
    captures = [side["captures"] for side in summary["sides"]]
    summary["objective_winner"] = None if captures[0] == captures[1] else int(captures[1] > captures[0])
    if not args.summary_only:
        analyze(directory, summary)
    summary.update(frozen_versions=manifests, map_sha256=digest(args.map),
                   harness_sha256=digest(args.binary), previous=str(args.previous) if args.previous else None)
    (directory / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    if not args.summary_only:
        replay(directory, summary)
        write_index(directory.parent)
    for name in (() if args.summary_only else ("decisions.jsonl", "frames.jsonl", "geometry.json", "events.json", "map.json", "terrain.json")):
        path = directory / name
        with path.open("rb") as source, gzip.open(str(path) + ".gz", "wb") as target:
            shutil.copyfileobj(source, target)
        path.unlink()
    concise={key: summary[key] for key in ("seconds", "winner", "versions", "timing_ms")}
    concise["sides"]=[{k:v for k,v in s.items() if k not in ("stuck_locations","stuck_actors")} for s in summary["sides"]]
    print(json.dumps(concise, indent=2))
    print(f"Results: {directory}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="operation", required=True)
    binary = ROOT / "target/release/examples/ai_arena"
    p = sub.add_parser("freeze")
    p.add_argument("destination", type=Path)
    p.add_argument("config", type=Path)
    p.add_argument("--binary", type=Path, default=binary)
    p.add_argument("--parent", type=Path)
    p.set_defaults(action=freeze)
    p = sub.add_parser("run")
    p.add_argument("--blue", type=Path, required=True)
    p.add_argument("--red", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--previous", type=Path)
    p.add_argument("--seconds", type=int, default=240)
    p.add_argument("--seed", type=int, default=42)
    p.add_argument("--summary-only", action="store_true", help="Keep metrics and decision telemetry without the large visual recording")
    p.add_argument("--map", type=Path, default=ROOT / "levels/dota-level.json")
    p.add_argument("--binary", type=Path, default=binary)
    p.set_defaults(action=run)
    p = sub.add_parser("replay")
    p.add_argument("directory", type=Path)
    p.set_defaults(action=lambda args: replay(args.directory, json.loads((args.directory / "summary.json").read_text())))
    p = sub.add_parser("index")
    p.add_argument("directory", type=Path)
    p.set_defaults(action=lambda args: write_index(args.directory))
    args = parser.parse_args()
    args.action(args)


if __name__ == "__main__":
    main()
