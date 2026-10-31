#!/usr/bin/env python3
"""Measure generated-source preparation with empty, memory, and restarted disk caches."""
import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=int, default=58)
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--output', type=Path, default=ROOT / 'work/audio-preparation/results.json')
    args = parser.parse_args()
    if args.seconds < 1 or args.seconds > 120 or args.runs < 3:
        parser.error('use 1–120 seconds and at least three runs')
    scratch = ROOT / 'work/audio-preparation'
    scratch.mkdir(parents=True, exist_ok=True)
    if not args.skip_build:
        with (scratch / 'build.log').open('w') as log:
            subprocess.run(['cargo', 'build', '--offline', '--locked', '--release', '-p', 'daw-project', '--example', 'audio_preparation'], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)
    suffix = '.exe' if platform.system() == 'Windows' else ''
    binary = ROOT / f'target/release/examples/audio_preparation{suffix}'
    rows = []
    with tempfile.TemporaryDirectory(prefix='generated-', dir=scratch) as temporary:
        directory = Path(temporary)
        project = directory / 'project'
        subprocess.run([str(binary), 'init', str(project), str(args.seconds)], check=True)
        for run in range(args.runs):
            cache = directory / f'cache-{run}'
            for label, mode in [('uncached', 'uncached'), ('cold_cache', 'cached'), ('restarted_cache', 'cached')]:
                output = subprocess.check_output([str(binary), mode, str(project), str(cache)], text=True)
                for line in output.splitlines():
                    row = json.loads(line)
                    rows.append(dict(row, mode=label, run=run))
                print(f'Completed {label}, run {run + 1}/{args.runs}', flush=True)
    summary = []
    for mode, case in sorted({(row['mode'], row['case']) for row in rows}):
        values = [row['milliseconds'] for row in rows if (row['mode'], row['case']) == (mode, case)]
        summary.append(dict(mode=mode, case=case, median_ms=statistics.median(values), max_ms=max(values)))
    files = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', 'crates', 'apps', 'tools', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT, text=True).splitlines()
    digest = hashlib.sha256()
    for name in sorted(set(files)):
        path = ROOT / name
        if path.is_file() and path.suffix in {'.rs', '.py', '.toml', '.lock'}:
            digest.update(name.encode())
            digest.update(path.read_bytes())
    result = dict(schema_version=1, platform=platform.platform(), machine=platform.machine(), logical_cpus=os.cpu_count(), profile='release', seconds=args.seconds, runs=args.runs,
                  rustc=subprocess.check_output(['rustc', '-Vv'], text=True), source_sha256=digest.hexdigest(),
                  scope='Headless synthetic media preparation; editor_model excludes native UI presentation. Shared audio bytes include predecessor/cache owners at each operation return; peak RSS, native scratch and physical-device latency are not measured.', rows=rows, summary=summary)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
