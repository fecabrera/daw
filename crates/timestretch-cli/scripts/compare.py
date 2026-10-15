#!/usr/bin/env python3
"""Run the standalone three-engine comparison with a prebuilt binary."""
import argparse
import os
import signal
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--input', required=True)
    parser.add_argument('--ratio', required=True)
    parser.add_argument('--sample-rate', type=int, default=48000)
    parser.add_argument('--output-dir', required=True)
    parser.add_argument('--fixture-manifest')
    parser.add_argument('--timeout-seconds', type=int, default=600)
    args = parser.parse_args()
    command = [args.binary, 'compare', '--input', args.input, '--ratio', args.ratio,
               '--sample-rate', str(args.sample_rate), '--output-dir', args.output_dir,
               '--timeout-seconds', str(args.timeout_seconds)]
    if args.fixture_manifest:
        command += ['--fixture-manifest', args.fixture_manifest]
    child = subprocess.Popen(command, creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
    try:
        return child.wait()
    except KeyboardInterrupt:
        child.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT)
        child.wait()
        return 1


if __name__ == '__main__':
    sys.exit(main())
