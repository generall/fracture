#!/usr/bin/env python3
"""Record verified, alternating winner/loser promotions toward five AI iterations."""
import argparse
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("league", ROOT / "tools/ai-league.py")
league = importlib.util.module_from_spec(spec)
spec.loader.exec_module(league)


def identity(manifest):
    return {key: manifest[key] for key in ("name", "agent", "config.json", "source.tar.gz")}


def descends_from(manifest, parent):
    node = manifest.get("parent")
    while node:
        if identity(node) == parent:
            return True
        node = node.get("parent")
    return False


def verify_series(path, state, require_deck=False):
    series = json.loads(path.read_text())
    if require_deck and series.get("execution", {}).get("host") != "deck@192.168.0.10":
        raise ValueError("A promotion requires the mirrored series to run on the Deck")
    for key in ("incumbent", "challenger"):
        if league.arena.validate(Path(series[key + "_path"])) != series[key]:
            raise ValueError("Frozen executable/config/source changed")
    if identity(series["incumbent"]) != state["champion"]:
        raise ValueError("Series does not face the frozen current champion")
    if not descends_from(series["challenger"], state["development_parent"]):
        raise ValueError("Challenger must improve the previous losing branch")
    coverage = set()
    proofs = []
    harnesses, maps = set(), set()
    for row in series["rounds"]:
        report = Path(row["report"])
        raw = report.read_bytes()
        summary = json.loads(raw)
        if summary != row["summary"]:
            raise ValueError(f"Series summary changed: {report}")
        seed, side = summary["seed"], row["candidate_side"]
        if side not in (0, 1) or (seed, side) in coverage:
            raise ValueError("Duplicate seed/side or invalid side")
        coverage.add((seed, side))
        if (summary["seconds"], summary["ticks"], summary["players_per_team"], summary["respawn_seconds"]) != (240, 28800, 2, 10):
            raise ValueError("Promotion requires four-minute 2v2 matches and ten-second respawns")
        expected = [series["incumbent"], series["incumbent"]]
        expected[side] = series["challenger"]
        if summary["frozen_versions"] != expected:
            raise ValueError("Wrong frozen match versions")
        harnesses.add(summary["harness_sha256"])
        maps.add(summary["map_sha256"])
        events_path = report.parent / "events.json"
        opener = events_path.open
        if not events_path.exists():
            events_path = Path(str(events_path) + ".gz")
            opener = lambda mode: gzip.open(events_path, mode)
        with opener("rt") as stream:
            events = json.load(stream)
        captures = [0, 0]
        for frame in events:
            for event in frame["events"]:
                if "FlagCaptured" in event:
                    team = event["FlagCaptured"]["team"]
                    if team not in ("blue", "red"):
                        raise ValueError("Invalid capturing team")
                    captures[team == "red"] += 1
        if captures != [s["captures"] for s in summary["sides"]]:
            raise ValueError("Summary captures do not match recorded physics events")
        proofs.append({"summary": str(report), "summary_sha256": hashlib.sha256(raw).hexdigest(),
                       "events_sha256": hashlib.sha256(events_path.read_bytes()).hexdigest()})
    seeds = {seed for seed, _ in coverage}
    if len(seeds) < 2 or coverage != {(seed, side) for seed in seeds for side in (0, 1)}:
        raise ValueError("Each of at least two seeds must be played on both colors")
    if len(harnesses) != 1 or len(maps) != 1:
        raise ValueError("Mirrored matches must use the same harness and map")
    if harnesses != {league.arena.digest(Path(series["harness_path"]))} or maps != {league.arena.digest(Path(series["map_path"]))}:
        raise ValueError("Harness or map changed")
    assessment = league.assess(series["rounds"])
    if not assessment["dominant"]:
        raise ValueError("Challenger has not demonstrated objective dominance")
    return series, assessment, proofs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("status", "promote"))
    parser.add_argument("--series", type=Path)
    parser.add_argument("--state", type=Path, default=ROOT / "docs/validation/ai-evolution-state.json")
    parser.add_argument("--initial-incumbent", type=Path, default=ROOT / "target/ai-arena/versions/v11")
    parser.add_argument("--initial-parent", type=Path, default=ROOT / "target/ai-arena/versions/v40")
    args = parser.parse_args()
    state = (json.loads(args.state.read_text()) if args.state.exists() else
             {"required_iterations": 5, "champion": identity(league.arena.validate(args.initial_incumbent)),
              "development_parent": identity(league.arena.validate(args.initial_parent)), "promotions": []})
    if args.operation == "promote":
        if not args.series:
            parser.error("--series is required for promotion")
        series, assessment, proofs = verify_series(args.series, state, require_deck=True)
        number = len(state["promotions"]) + 1
        args.state.parent.mkdir(parents=True, exist_ok=True)
        destination = args.state.parent / f"ai-evolution-iteration-{number:02d}.json.gz"
        if destination.exists():
            raise SystemExit(f"Promotion evidence already exists: {destination}")
        with gzip.open(destination, "wt") as stream:
            json.dump({"series": series, "assessment": assessment, "proofs": proofs}, stream, sort_keys=True)
        old_champion = state["champion"]
        state["champion"] = identity(series["challenger"])
        state["development_parent"] = old_champion
        state["promotions"].append({"iteration": number, "winner": state["champion"],
                                     "loser": old_champion, "assessment": assessment,
                                     "evidence": str(destination.relative_to(ROOT)) if destination.is_relative_to(ROOT) else str(destination)})
    args.state.parent.mkdir(parents=True, exist_ok=True)
    args.state.write_text(json.dumps(state, indent=2) + "\n")
    print(json.dumps({"verified_iterations": len(state["promotions"]), "required_iterations": state["required_iterations"],
                      "champion": state["champion"]["name"], "improve_branch": state["development_parent"]["name"]}, indent=2))


if __name__ == "__main__":
    main()
