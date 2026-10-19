#!/usr/bin/env python3
"""Run synthetic, headless release benchmarks, including serial/parallel cache preparation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEPENDENCIES = [
    'daw-core', 'daw-engine', 'daw-media', 'daw-project', 'daw-unit-core', 'daw-ui',
    'daw-equalizer', 'daw-drive', 'daw-auto-filter', 'daw-compressor', 'daw-ott',
    'daw-limiter', 'daw-direction-mixer', 'daw-stereo-spread', 'daw-bitcrusher',
    'egui', 'serde_json', 'timestretch', 'rubato',
]


def toml_value(value):
    if isinstance(value, dict):
        return '{ ' + ', '.join(f'{k} = {toml_value(v)}' for k, v in value.items()) + ' }'
    return json.dumps(value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--suite', choices=['all', 'dsp', 'engine', 'model', 'ui', 'media', 'cache'], default='all')
    parser.add_argument('--runs', type=int, default=7)
    parser.add_argument('--output', type=Path, default=ROOT / 'work/performance-audit/results.json')
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--compare', type=Path, help='Record before/after ratios for matching cases')
    parser.add_argument('--max-regression', type=float, help='Optional median ratio gate for controlled hardware; requires --compare')
    args = parser.parse_args()
    if args.max_regression is not None and (args.compare is None or args.max_regression <= 1):
        parser.error('--max-regression requires --compare and a ratio greater than 1')
    if args.runs < 3:
        parser.error('use at least three repetitions')
    scratch = ROOT / 'work/performance-audit'
    scratch.mkdir(parents=True, exist_ok=True)
    dependencies = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['dependencies']
    manifest = '[package]\nname = "daw-performance-audit"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n[[bin]]\nname = "daw-performance-audit"\npath = "../../tools/performance_audit.rs"\n[dependencies]\n'
    for name in DEPENDENCIES:
        value = dependencies[name]
        if isinstance(value, dict) and 'path' in value:
            value = {**value, 'path': str(ROOT / value['path'])}
        manifest += f'{name} = {toml_value(value)}\n'
    (scratch / 'Cargo.toml').write_text(manifest)
    lock = ROOT / 'Cargo.lock'
    (scratch / 'Cargo.lock').write_bytes(lock.read_bytes())
    env = {**os.environ, 'CARGO_BUILD_JOBS': os.environ.get('CARGO_BUILD_JOBS', '2')}
    cache_suite = args.suite == 'cache'
    executable = 'examples/cache_preparation' if cache_suite else 'daw-performance-audit'
    binary = ROOT / ('target/release/' + executable + ('.exe' if os.name == 'nt' else ''))
    if not args.skip_build:
        print('Building the release audit harness offline.', flush=True)
        with (scratch / 'build.log').open('w') as log:
            command = (['cargo', 'build', '--release', '--offline', '-p', 'daw-engine', '--example', 'cache_preparation']
                       if cache_suite else ['cargo', 'build', '--release', '--offline', '--manifest-path', str(scratch / 'Cargo.toml'), '--target-dir', str(ROOT / 'target')])
            subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    metadata = {
        'schema_version': 1,
        'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'root_lock_sha256': hashlib.sha256(lock.read_bytes()).hexdigest(),
        'harness_sha256': hashlib.sha256((ROOT / ('crates/engine/examples/cache_preparation.rs' if cache_suite else 'tools/performance_audit.rs')).read_bytes()).hexdigest(),
        'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'rustc': subprocess.check_output(['rustc', '-Vv'], cwd=ROOT, text=True).strip(),
        'platform': platform.platform(), 'machine': platform.machine(), 'logical_cpus': os.cpu_count(),
        'profile': 'release', 'suite': args.suite, 'repetitions': args.runs,
        'scope': ('synthetic exact-history serial/two-worker/auto cache timings; no worker allocation counters or native audio measurements' if cache_suite else 'synthetic serial headless CPU/allocator measurements; no native window, GPU submission, CPAL device, pacing, or xrun measurement'),
    }
    if sys.platform == 'darwin':
        hardware = subprocess.run(['/usr/sbin/system_profiler', 'SPHardwareDataType', '-json'], capture_output=True, text=True)
        if hardware.returncode == 0:
            info = json.loads(hardware.stdout)['SPHardwareDataType'][0]
            metadata['hardware'] = {k: info[k] for k in ['machine_model', 'chip_type', 'physical_memory', 'number_processors'] if k in info}
    source_files = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', 'crates', 'apps', 'tools', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'], cwd=ROOT, text=True).splitlines()
    source_hash = hashlib.sha256()
    for name in sorted(set(source_files)):
        path = ROOT / name
        if path.is_file() and path.suffix in {'.rs', '.toml', '.lock', '.py', '.sh'}:
            source_hash.update(name.encode())
            source_hash.update(path.read_bytes())
    metadata['source_tree_sha256'] = source_hash.hexdigest()
    metadata['working_tree_changed'] = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip())
    metadata['rustflags_set'] = bool(os.environ.get('RUSTFLAGS'))
    rows = []
    command = [str(binary), args.suite, str(args.runs), str(scratch)]
    with (scratch / f'{args.suite}.stderr.log').open('w') as err:
        child = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=err, text=True)
        try:
            for line in child.stdout:
                row = json.loads(line)
                rows.append(row)
                print(f"{row['suite']}: {row['case']} median={row['median_us']:.2f} us", flush=True)
            if hasattr(os, 'wait4'):
                # Collect this process only. Build-tool memory must not enter the result.
                _, status, usage = os.wait4(child.pid, 0)
                code = os.waitstatus_to_exitcode(status)
                child.returncode = code
                metadata['process_peak_rss_bytes'] = int(usage.ru_maxrss * (1 if sys.platform == 'darwin' else 1024))
            else:
                code = child.wait()
        except BaseException:
            child.terminate()
            child.wait()
            raise
    result = {'metadata': metadata, 'status': 'complete' if code == 0 else 'failed', 'rows': rows}
    if args.compare:
        baseline = json.loads(args.compare.read_text())
        if baseline['status'] != 'complete':
            raise ValueError('comparison baseline is incomplete')
        old = {(r['suite'], r['case']): r for r in baseline['rows']}
        comparison = []
        for row in rows:
            prior = old.get((row['suite'], row['case']))
            if prior and prior['median_us'] > 0:
                comparison.append({'suite': row['suite'], 'case': row['case'],
                    'baseline_median_us': prior['median_us'], 'median_us': row['median_us'],
                    'median_ratio': row['median_us'] / prior['median_us']})
        result['comparison'] = {'baseline_sha256': hashlib.sha256(args.compare.read_bytes()).hexdigest(),
            'matched_cases': len(comparison), 'rows': comparison,
            'unmatched_cases': [r['case'] for r in rows if (r['suite'], r['case']) not in old]}
        if args.max_regression is not None:
            failures = [r['case'] for r in comparison if r['median_ratio'] > args.max_regression]
            result['comparison']['gate_failures'] = failures
            if failures:
                code = code or 2
                result['status'] = 'regression'
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    if code:
        print(f'Benchmark failed; see {scratch / (args.suite + ".stderr.log")}', file=sys.stderr)
    return code


if __name__ == '__main__':
    raise SystemExit(main())
