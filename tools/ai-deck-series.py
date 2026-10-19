#!/usr/bin/env python3
"""Run resumable mirrored native matches on the authorized Steam Deck."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
from pathlib import Path
import shlex
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("league", ROOT / "tools/ai-league.py")
league = importlib.util.module_from_spec(spec)
spec.loader.exec_module(league)
HOST = "deck@192.168.0.10"
REMOTE = "/home/deck/Games/fracture-ai-lab"


def ssh(*args, check=True):
    result = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", HOST,
                             shlex.join(str(a) for a in args)], text=True, capture_output=True)
    if check and result.returncode:
        raise RuntimeError(result.stderr.strip() or f"SSH failed: {args[0]}")
    return result


def copy(paths, destination, download=False):
    values = [f"{HOST}:{p}" if download else str(p) for p in paths]
    subprocess.run(["scp", "-pq", *values, str(destination) if download else f"{HOST}:{destination}"], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--incumbent", type=Path, required=True)
    parser.add_argument("--challenger", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--map", type=Path, default=ROOT / "levels/dota-level.json")
    parser.add_argument("--seeds", nargs="+", type=int, default=[42, 137])
    args = parser.parse_args()
    if len(set(args.seeds)) != len(args.seeds) or len(args.seeds) < 2:
        parser.error("At least two unique seeds are required")
    versions = [args.incumbent.resolve(), args.challenger.resolve()]
    manifests = [league.arena.validate(v) for v in versions]
    harness_hash, map_hash = league.arena.digest(args.binary), league.arena.digest(args.map)
    version_ids = [m["agent"][:12] + "-" + m["config.json"][:12] for m in manifests]
    group = "-".join([*version_ids, harness_hash[:12], map_hash[:12]])
    remote_versions = [REMOTE + "/versions/" + v for v in version_ids]
    remote_harness = REMOTE + "/harnesses/" + harness_hash + "/ai_arena"
    remote_map = REMOTE + "/maps/" + map_hash + ".json"
    ssh("mkdir", "-p", REMOTE + "/tools", REMOTE + "/maps", *remote_versions,
        str(Path(remote_harness).parent), REMOTE + "/runs/" + group)
    for folder, destination in zip(versions, remote_versions):
        copy([folder / name for name in ("agent", "config.json", "manifest.json", "source.tar.gz")], destination + "/")
    copy([args.binary], remote_harness)
    copy([args.map], remote_map)
    copy([ROOT / "tools/ai-arena.py"], REMOTE + "/tools/ai-arena.py")
    ssh("chmod", "u+x", remote_harness, *(v + "/agent" for v in remote_versions))
    args.output.mkdir(parents=True, exist_ok=True)
    jobs = []

    def run_case(case):
        seed, side = case
        label = f"seed-{seed}-challenger-{'blue' if side == 0 else 'red'}"
        remote_output = REMOTE + "/runs/" + group + "/" + label
        local_output = args.output / label
        unit = "fracture-ai-lab-" + harness_hash[:8] + "-" + version_ids[0][:8] + "-" + version_ids[1][:8] + f"-{seed}-{side}"
        job = {"unit": unit, "remote_output": remote_output, "seed": seed, "candidate_side": side}
        jobs.append(job)
        required = ("summary.json", "evaluation.jsonl", "events.json", "geometry.json", "map.json", "terrain.json", "run.log")
        if not all((local_output / name).exists() for name in required):
            report = remote_output + "/summary.json"
            while True:
                status = ssh("systemctl", "--user", "is-active", unit, check=False)
                result = ssh("test", "-f", report, check=False)
                directory = ssh("test", "-d", remote_output, check=False)
                if 255 in (status.returncode, result.returncode, directory.returncode):
                    time.sleep(2)
                    continue
                running = status.stdout.strip()
                completed = result.returncode == 0
                break
            if running not in ("active", "activating", "deactivating") and not completed:
                if directory.returncode == 0:
                    raise RuntimeError(f"Terminal incomplete match retained for inspection: {remote_output}")
                selected = [remote_versions[0], remote_versions[0]]
                selected[side] = remote_versions[1]
                command = ["systemd-run", "--user", "--collect", "--unit=" + unit,
                           "--property=WorkingDirectory=" + REMOTE, "--setenv=FRACTURE_SILENT=1",
                           "python3", REMOTE + "/tools/ai-arena.py", "run", "--blue", selected[0],
                           "--red", selected[1], "--binary", remote_harness, "--map", remote_map,
                           "--seed", str(seed), "--seconds", "240", "--summary-only", "--output", remote_output]
                ssh(*command)
                print("Started " + unit, flush=True)
            while True:
                state = ssh("systemctl", "--user", "is-active", unit, check=False)
                if state.returncode == 255:
                    # A network timeout is not proof that the persistent job died.
                    time.sleep(2)
                    continue
                if state.stdout.strip() not in ("active", "activating", "deactivating"):
                    if ssh("test", "-f", report, check=False).returncode != 0:
                        raise RuntimeError(f"Deck job stopped without a summary: {unit}")
                    break
                time.sleep(2)
            local_output.mkdir(exist_ok=True)
            copy([remote_output + "/" + name for name in required],
                 local_output, download=True)
        summary = json.loads((local_output / "summary.json").read_text())
        expected = [manifests[0], manifests[0]]
        expected[side] = manifests[1]
        if (summary["frozen_versions"] != expected or summary["harness_sha256"] != harness_hash
                or summary["map_sha256"] != map_hash or summary["seed"] != seed):
            raise RuntimeError("Deck match identity mismatch: " + label)
        print("Completed " + label + ": " + str([s["captures"] for s in summary["sides"]]), flush=True)

    with ThreadPoolExecutor(max_workers=2) as executor:
        list(executor.map(run_case, [(seed, side) for seed in args.seeds for side in range(2)]))
    subprocess.run(["python3", str(ROOT / "tools/ai-league.py"), "--incumbent", str(versions[0]),
                    "--challenger", str(versions[1]), "--binary", str(args.binary.resolve()),
                    "--map", str(args.map.resolve()), "--output", str(args.output.resolve()),
                    "--seeds", *(str(s) for s in args.seeds)], check=True)
    report = args.output / "series.json"
    result = json.loads(report.read_text())
    result["execution"] = {"host": HOST, "kind": "native-headless", "jobs": sorted(jobs, key=lambda j: (j["seed"], j["candidate_side"]))}
    report.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
