#!/usr/bin/env python3
"""Review a completed 1v2 recording's compact decisions without changing the game."""
import argparse
import collections
import gzip
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LABELS = ('Human', 'V11 / Red 1', 'V11 / Red 2')


def load(path):
    return json.loads(path.read_text())


def percentile(values, fraction):
    values = sorted(values)
    return values[min(len(values)-1, int((len(values)-1)*fraction))] if values else None


def review(folder):
    summary = load(next(folder.glob('*.summary.json')))
    metadata = load(next(folder.glob('*.meta.json')))
    if not summary['complete'] or summary['players'] != 3 or summary['human_player'] != 0:
        raise SystemExit('Expected a completed human 1v2 recording')
    samples = [[] for _ in range(3)]
    frames = {}
    with gzip.open(folder/'compact.jsonl.gz', 'rt') as stream:
        for line in stream:
            row = json.loads(line)
            player = row['player']
            observation = row.get('human_view', row.get('observation'))
            decision = row.get('decision', {})
            state = dict(observation, tick=row['tick'], telemetry=decision.get('telemetry', {}),
                         cpu_ms=decision.get('cpu_ms'), geometry_counts=row['geometry_counts'])
            samples[player].append(state)
            frames.setdefault(row['tick'], [None]*3)[player] = state
    inputs = []
    with next(folder.glob('*.inputs.jsonl')).open() as stream:
        for tick, line in enumerate(stream):
            row = json.loads(line)
            if row['tick'] != tick or len(row['inputs']) != 3:
                raise SystemExit('Gap or invalid roster in applied input recording')
            inputs.append(row['inputs'])
    if len(inputs) != summary['ticks'] or any(None in row for row in frames.values()):
        raise SystemExit('Incomplete input or decision recording')
    flags = dict(metadata['map']['flags'])
    stats = []
    for player, states in enumerate(samples):
        distance = 0.0
        lives, life, stationary = [], [], []
        anchor = None
        def close_stationary(last):
            if anchor is not None and last['tick']-anchor['tick'] >= 360:
                interval = [s for s in states if anchor['tick'] <= s['tick'] <= last['tick']]
                stationary.append(dict(start=anchor['tick']/120, end=last['tick']/120,
                    position=anchor['position'], modes=dict(collections.Counter(s['telemetry'].get('mode','human') for s in interval)),
                    active_tracks_fraction=sum(inputs[s['tick']][player]['tracks'] != [0.0,0.0] for s in interval)/len(interval)))
        previous = None
        for state in states:
            if state['alive']:
                if previous and previous['alive'] and previous['vehicle'] == state['vehicle']:
                    distance += math.dist(previous['position'], state['position'])
                life.append(state)
                if anchor is None:
                    anchor = state
                elif math.dist(anchor['position'], state['position']) > .75:
                    close_stationary(previous)
                    anchor = state
            else:
                if life:
                    lives.append(life)
                    life = []
                if anchor:
                    close_stationary(previous)
                    anchor = None
            previous = state
        if life:
            lives.append(life)
        if anchor:
            close_stationary(states[-1])
        objective = flags['red' if player == 0 else 'blue']
        alive = [s for s in states if s['alive']]
        cpu = [s['cpu_ms'] for s in states if s['cpu_ms'] is not None]
        stats.append(dict(player=player, label=LABELS[player], distance_m=distance,
            closest_enemy_flag_m=min(math.dist(s['position'],objective) for s in alive),
            first_within_m={str(radius): next((s['tick']/120 for s in alive if math.dist(s['position'],objective)<radius),None) for radius in (30,14,5)},
            modes=dict(collections.Counter(s['telemetry'].get('mode','human') for s in states)),
            cpu_ms=dict(p95=percentile(cpu,.95),p99=percentile(cpu,.99),maximum=max(cpu) if cpu else None),
            lives=[dict(start=a[0]['tick']/120,end=a[-1]['tick']/120,vehicle=a[0]['vehicle'],
                        end_position=a[-1]['position'],end_health=a[-1]['health'],
                        closest_flag_m=min(math.dist(s['position'],objective)for s in a))for a in lives],
            stationary=stationary))
    events, counts = [], collections.Counter()
    for line in next(folder.glob('*.events.jsonl')).open():
        row = json.loads(line)
        for event in row['events']:
            kind = event if isinstance(event,str) else next(iter(event))
            counts[kind] += 1
            if kind in ('FlagCaptured','TankDestroyed','TankRespawned','BridgeCollapsed'):
                events.append(dict(seconds=row['tick']/120,kind=kind,details=event if isinstance(event,str)else event[kind]))
    human_changes = []
    previous = None
    for tick, controls in enumerate(inputs):
        value = controls[0]['ammunition']
        if value != previous:
            human_changes.append(dict(seconds=tick/120,ammunition=value))
            previous = value
    report = dict(summary=summary, provenance=load(folder/'provenance.json'), players=stats,
        event_counts=dict(counts), events=events, human_ammunition=human_changes,
        human_fire_requests=sum(row[0]['fire']for row in inputs),
        human_special_requests=[tick/120 for tick,row in enumerate(inputs)if row[0]['special']],
        limitations=['Stationary intervals include intentional aiming/turning; they are not automatically failures.',
                     'Death counts do not identify the attacker. Fire requests are not actual shot counts.',
                     'Route view uses the public starting layout; destruction is in the separate full-field recording.'])
    (folder/'review.json').write_text(json.dumps(report,indent=2)+'\n')
    level=load(ROOT/'levels/dota-level.json')
    outlines=[]
    for item in level['items']:
        outline=item.get('outline',item.get('asset',{}).get('outline'))
        if not outline or item['kind'] not in ('solid','object'):
            continue
        angle=item.get('angle',0);c,s=math.cos(angle),math.sin(angle);x,y=item.get('position',[0,0])
        outlines.append([[x+c*a-s*b,y+s*a+c*b]for a,b in outline])
    compact=[]
    for tick, states in sorted(frames.items()):
        compact.append([tick/120,[{k:s[k]for k in ('position','alive','health','vehicle','hull_angle','turret_angle')}for s in states]])
    data=dict(frames=compact,map=metadata['map'],outlines=outlines,
              bridges=[i for i in level['items']if i['kind']=='bridge'],summary=summary)
    template=(ROOT/'tools/ai/human-route-review.html').read_text()
    (folder/'route-review.html').write_text(template.replace('/*RECORDING*/{}',json.dumps(data,separators=(',',':')).replace('<','\\u003c')))
    print(json.dumps({k:report[k] for k in ('summary','players','event_counts','human_ammunition','human_fire_requests','human_special_requests')},indent=2))


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory',type=Path)
    review(parser.parse_args().directory)
