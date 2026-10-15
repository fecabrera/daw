#!/usr/bin/env python3
"""Serial three-engine benchmarks. Uses Python's standard library and a supplied binary."""
import argparse
import csv
import json
import hashlib
from fractions import Fraction
import pathlib
import statistics
import subprocess
import sys
import time

ENGINES = ['timestretch', 'rubberband-r2', 'rubberband-r3']


def summarize(values):
    median = statistics.median(values)
    return dict(median=median, minimum=min(values), maximum=max(values),
                median_absolute_deviation=statistics.median(abs(v - median) for v in values))


def run(args):
    directory = pathlib.Path(args.output_dir)
    if directory.exists() and (not directory.is_dir() or any(directory.iterdir())):
        raise ValueError('benchmark directory must be new or empty')
    directory.mkdir(parents=True, exist_ok=True)
    rows, failures = [], []
    for rate in args.sample_rates:
        for case, ratio in enumerate(args.ratios):
            identity = None
            for repetition in range(args.warmups + args.runs):
                # Rotate every repetition, including warmups, with deterministic order.
                offset = repetition % len(ENGINES)
                for engine in ENGINES[offset:] + ENGINES[:offset]:
                    stem = directory / f'{rate}-case{case}-run{repetition}-{engine}'
                    report_path = stem.with_suffix('.json')
                    command = [args.binary, 'process', '--input', args.input,
                               '--output', str(stem.with_suffix('.wav')), '--report', str(report_path),
                               '--engine', engine, '--ratio', ratio, '--sample-rate', str(rate)]
                    started = time.monotonic()
                    child = None
                    try:
                        with stem.with_suffix('.log').open('w') as log:
                            child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
                            code = child.wait(timeout=args.timeout_seconds)
                        if code:
                            raise RuntimeError(f'engine exited with {code}; see {stem}.log')
                        report = json.loads(report_path.read_text())
                        current = (report['file_sha256'], report['prepared_sha256'], report['expected_frames'])
                        if hashlib.sha256(stem.with_suffix('.wav').read_bytes()).hexdigest() != report['output_file_sha256']:
                            raise RuntimeError('output hash mismatch')
                        if report['status'] != 'complete' or report['engine'] != engine or report['output_frames'] != report['expected_frames']:
                            raise RuntimeError('invalid result identity or output length')
                        if identity is not None and identity != current:
                            raise RuntimeError('input identity changed across engines or repetitions')
                        identity = current
                        rows.append(dict(rate=rate, ratio=ratio, engine=engine, repetition=repetition,
                                         warmup=repetition < args.warmups,
                                         engine_seconds=report['timings_seconds']['engine'],
                                         child_wall_seconds=time.monotonic()-started,
                                         report=report_path.name))
                    except (OSError, ValueError, KeyError, RuntimeError, subprocess.TimeoutExpired) as error:
                        if child and child.poll() is None:
                            child.kill()
                            child.wait()
                        failures.append(dict(rate=rate, ratio=ratio, engine=engine,
                                             repetition=repetition, error=str(error)))
                    except KeyboardInterrupt:
                        if child and child.poll() is None:
                            child.terminate()
                            child.wait()
                        failures.append(dict(rate=rate, ratio=ratio, engine=engine, error='cancelled'))
                        (directory / 'failures.json').write_text(json.dumps(failures, indent=2))
                        raise
    summary = []
    for rate in args.sample_rates:
        for ratio in args.ratios:
            for engine in ENGINES:
                values = [r['engine_seconds'] for r in rows if not r['warmup']
                          and (r['rate'], r['ratio'], r['engine']) == (rate, ratio, engine)]
                if len(values) == args.runs:
                    summary.append(dict(rate=rate, ratio=ratio, engine=engine, **summarize(values)))
    for item in summary:
        reference = next((r for r in summary if (r['rate'], r['ratio'], r['engine']) == (item['rate'], item['ratio'], 'rubberband-r3')), None)
        item['median_time_relative_to_r3'] = item['median'] / reference['median'] if reference and reference['median'] else None
    for name, data in [('runs', rows), ('summary', summary), ('failures', failures)]:
        (directory / f'{name}.json').write_text(json.dumps(data, indent=2))
        if data:
            with (directory / f'{name}.csv').open('w', newline='') as out:
                writer = csv.DictWriter(out, fieldnames=data[0].keys())
                writer.writeheader()
                writer.writerows(data)
    (directory / 'benchmark.json').write_text(json.dumps(dict(
        schema_version=1, status='failed' if failures else 'complete', failures=failures,
        baseline='rubberband-r3', summary=summary,
        acceptance='unassessed; timings are not full-corpus acceptance'), indent=2))
    return 1 if failures else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--input', required=True)
    parser.add_argument('--ratios', nargs='+', required=True)
    parser.add_argument('--sample-rates', type=int, nargs='+', default=[48000])
    parser.add_argument('--warmups', type=int, default=1)
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--timeout-seconds', type=int, default=600)
    parser.add_argument('--output-dir', required=True)
    args = parser.parse_args()
    if args.warmups < 0 or args.runs < 1 or args.timeout_seconds < 1:
        parser.error('warmups must be nonnegative; runs and timeout must be positive')
    try:
        if any(Fraction(r) < Fraction(1, 8) or Fraction(r) > 8 for r in args.ratios):
            parser.error('ratios must be between 1/8 and 8')
        if any(r not in [44100, 48000, 88200, 96000, 176400, 192000] for r in args.sample_rates):
            parser.error('unsupported sample rate')
        return run(args)
    except (OSError, ValueError, ZeroDivisionError) as error:
        print(error, file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        return 1


if __name__ == '__main__':
    sys.exit(main())
