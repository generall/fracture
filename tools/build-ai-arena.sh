#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
cargo build --release --locked -p fracture-ai-arena
python3 - "${1:-target/ai-arena/versions/v40}" "${2:-target/ai-arena/versions/v11}" <<'PY'
import hashlib,importlib.util,json,shutil,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('arena','tools/ai-arena.py')
arena=importlib.util.module_from_spec(spec);spec.loader.exec_module(arena)
versions=[Path(p).resolve() for p in sys.argv[1:]]
manifests=[arena.validate(p) for p in versions]
if (manifests[0]['agent'],manifests[0]['config.json'])==(manifests[1]['agent'],manifests[1]['config.json']):
    raise SystemExit('Select two different frozen AI versions')
package=Path('target/linux-x86_64/fracture-ai-arena');package.mkdir(parents=True,exist_ok=True)
shutil.copy2('target/release/fracture-ai-arena',package/'fracture-ai-arena')
shutil.copy2('tools/launch-ai-arena.sh',package/'fracture-ai-arena.sh')
(package/'fracture-ai-arena.sh').chmod(0o755)
shutil.copy2('docs/AI_ARENA.md',package/'README.md')
for report in Path('docs').glob('AI_ARENA_VALIDATION_*.md'):
    shutil.copy2(report,package/report.name)
(package/'validation').mkdir(exist_ok=True)
for report in Path('docs/validation').glob('ai-*.json.gz'):
    shutil.copy2(report,package/'validation'/report.name)
shutil.copy2('crates/sandbox/assets/FONT-LICENSE',package/'FONT-LICENSE')
for side,version in zip(('blue','red'),versions):
    destination=package/'agents'/side;destination.mkdir(parents=True,exist_ok=True)
    for name in ('agent','config.json','manifest.json'):
        shutil.copy2(version/name,destination/name)
(package/'package.json').write_text(json.dumps({'spectator_sha256':hashlib.sha256((package/'fracture-ai-arena').read_bytes()).hexdigest(),
    'blue':manifests[0],'red':manifests[1]},indent=2)+'\n')
PY
tar -C target/linux-x86_64 -czf target/fracture-ai-arena-linux-x86_64.tar.gz fracture-ai-arena
printf 'Ready: target/fracture-ai-arena-linux-x86_64.tar.gz\n'
