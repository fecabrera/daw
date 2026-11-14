"""Validate installed AUv2 products with Apple's validator; does not install them."""
import argparse
from pathlib import Path
import subprocess
import sys
import tomllib
from package_plugins import ROOT, available_units, select_units


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--units', required=True)
    parser.add_argument('--log-dir', type=Path, default=ROOT / 'work/auv2-validation')
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('AUv2 validation requires macOS')
    selected = select_units(args.units)
    args.log_dir.mkdir(parents=True, exist_ok=True)
    failed = []
    for slug in selected:
        manifest = tomllib.loads(available_units()[slug].read_text())['auv2']
        result = subprocess.run(['auval', '-strict', '-v', manifest['type'],
                                 manifest['subtype'], manifest['manufacturer']],
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        print(result.stdout, end='')
        (args.log_dir / f'{slug}.log').write_text(result.stdout)
        if result.returncode or 'AU VALIDATION SUCCEEDED' not in result.stdout:
            failed.append(slug)
    if failed:
        parser.exit(1, 'AU validation failed: ' + ', '.join(failed) + '\n')


if __name__ == '__main__':
    main()
