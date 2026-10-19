#!/usr/bin/env python3
"""Evaluate a frozen challenger on both sides before switching development teams."""
import argparse
import importlib.util
import json
import math
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("arena", ROOT / "tools/ai-arena.py")
arena = importlib.util.module_from_spec(spec)
spec.loader.exec_module(arena)


def assess(rounds):
    """Kills/damage cannot turn an objective draw into demonstrated dominance."""
    wins = losses = draws = 0
    captures = [0, 0]
    kills = [0, 0]
    for row in rounds:
        candidate = row["candidate_side"]
        sides = row["summary"]["sides"]
        a, b = sides[candidate], sides[1 - candidate]
        wins += a["captures"] > b["captures"]
        losses += a["captures"] < b["captures"]
        draws += a["captures"] == b["captures"]
        for total, key in ((captures, "captures"), (kills, "kills")):
            total[0] += a[key]
            total[1] += b[key]
    dominant = (len(rounds) >= 4 and wins >= math.ceil(len(rounds) * .75)
                and losses == 0 and captures[0] >= 3 * max(1, captures[1]))
    return dict(dominant=dominant, objective_wins=wins, objective_losses=losses,
                objective_draws=draws, captures=captures, kills=kills,
                criterion="At least four 240-second matches, both colors per seed; "
                "at least 75% objective wins, no objective losses, at least 3:1 captures. "
                "Kills and damage do not break objective draws.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--incumbent", type=Path, required=True)
    parser.add_argument("--challenger", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 137])
    parser.add_argument("--map", type=Path, default=ROOT / "levels/dota-level.json")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/examples/ai_arena")
    args = parser.parse_args()
    if len(set(args.seeds)) < 2 or len(set(args.seeds)) != len(args.seeds):
        parser.error("Use at least two unique seeds, each played on both colors")
    incumbent, challenger = args.incumbent.resolve(), args.challenger.resolve()
    manifests = [arena.validate(incumbent), arena.validate(challenger)]
    if (manifests[0]["agent"], manifests[0]["config.json"]) == (manifests[1]["agent"], manifests[1]["config.json"]):
        parser.error("The versions must differ")
    args.output.mkdir(parents=True, exist_ok=True)
    rounds = []
    for seed in args.seeds:
        for candidate_side in range(2):
            versions = [incumbent, incumbent]
            versions[candidate_side] = challenger
            destination = args.output / f"seed-{seed}-challenger-{'blue' if candidate_side == 0 else 'red'}"
            report = destination / "summary.json"
            if not report.exists():
                subprocess.run(["python3", str(ROOT / "tools/ai-arena.py"), "run",
                                "--blue", str(versions[0]), "--red", str(versions[1]),
                                "--output", str(destination), "--seed", str(seed),
                                "--map", str(args.map), "--binary", str(args.binary), "--summary-only"], check=True,
                               stdout=subprocess.DEVNULL)
            summary = json.loads(report.read_text())
            if (summary["frozen_versions"] != [arena.validate(v) for v in versions]
                    or summary["seed"] != seed or summary["seconds"] != 240
                    or summary["ticks"] != 28800 or summary["players_per_team"] != 2
                    or summary["respawn_seconds"] != 10
                    or summary["harness_sha256"] != arena.digest(args.binary)
                    or summary["map_sha256"] != arena.digest(args.map)):
                raise SystemExit(f"Existing result is from a different comparison: {report}")
            rounds.append(dict(candidate_side=candidate_side, report=str(report.resolve()), summary=summary))
            verdict = assess(rounds)
            result = dict(incumbent=manifests[0], challenger=manifests[1], rounds=rounds,
                          incumbent_path=str(incumbent), challenger_path=str(challenger),
                          harness_path=str(args.binary.resolve()), map_path=str(args.map.resolve()),
                          assessment=verdict,
                          next_action=("Freeze challenger; improve incumbent branch" if verdict["dominant"]
                                       else "Keep incumbent frozen; improve challenger branch"))
            (args.output / "series.json").write_text(json.dumps(result, indent=2) + "\n")
            print(f"seed {seed}, challenger {'Blue' if candidate_side == 0 else 'Red'}: "
                  f"captures {[s['captures'] for s in summary['sides']]}, "
                  f"kills {[s['kills'] for s in summary['sides']]}", flush=True)
    print(json.dumps(verdict, indent=2))
    print(result["next_action"])


if __name__ == "__main__":
    main()
