#!/usr/bin/env python3
"""Build the pinned macOS SDK ABI host and test native Drive editors. Never installs plugins."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk-dir', type=Path, default=ROOT / 'work/drive-plugin-phase0/vst3sdk')
    parser.add_argument('--clap-dir', type=Path, default=ROOT / 'work/drive-plugin-phase0/clap')
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        parser.error('This native harness requires macOS. Windows/X11 checks remain separate.')
    work = ROOT / 'work/drive-plugin-phase4'
    work.mkdir(parents=True, exist_ok=True)
    report = {'run_completed': False, 'formats': {}}
    (work / 'native-report.json').write_text(json.dumps(report, indent=2) + '\n')
    env = dict(os.environ)
    if Path('/Applications/Xcode.app/Contents/Developer').is_dir():
        env.setdefault('DEVELOPER_DIR', '/Applications/Xcode.app/Contents/Developer')
    def run(command: list[str], name: str) -> str:
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        output = result.stdout + result.stderr
        (work / (name + '.log')).write_text(output)
        (work / (name + '.status')).write_text(str(result.returncode) + '\n')
        if result.returncode:
            raise RuntimeError(f'{name} failed ({result.returncode}); see {work / (name + ".log")}')
        return result.stdout
    sdk = args.sdk_dir.resolve()
    clap = args.clap_dir.resolve()
    sources = json.loads((ROOT / 'tools/drive_plugin_phase0/sources.json').read_text())
    # Match the same pinned interface revisions used by the processor validators.
    for checkout, expected in [(sdk, sources['vst3sdk']['revision']), (clap, sources['clap']['revision'])]:
        actual = subprocess.check_output(['git', '-C', str(checkout), 'rev-parse', 'HEAD'], text=True).strip()
        if actual != expected:
            raise RuntimeError(f'Unexpected SDK revision in {checkout}')
        if subprocess.run(['git', '-C', str(checkout), 'diff', '--quiet', 'HEAD', '--']).returncode:
            raise RuntimeError(f'Modified SDK files in {checkout}')
    submodules = subprocess.check_output(['git', '-C', str(sdk), 'submodule', 'status', '--recursive', 'base', 'cmake', 'pluginterfaces', 'public.sdk'], text=True)
    if any(line and line[0] != ' ' for line in submodules.splitlines()):
        raise RuntimeError('VST3 SDK submodules do not match the pinned gitlinks')
    command = ['cargo', 'build', '--locked', '--offline', '-p', 'daw-drive-clap', '-p', 'daw-drive-vst3']
    if args.profile == 'release':
        command.append('--release')
    run(command, 'plugin-build')
    host = work / 'native-host'
    run(['xcrun', 'clang++', '-std=c++17', '-fobjc-arc', '-O2', '-I' + str(clap / 'include'), '-I' + str(sdk),
         str(ROOT / 'tools/drive_plugin_phase4/native_host.mm'),
         *[str(sdk / p) for p in ['pluginterfaces/base/funknown.cpp', 'pluginterfaces/base/coreiids.cpp',
             'public.sdk/source/vst/vstinitiids.cpp', 'public.sdk/source/common/commoniids.cpp']],
         '-framework', 'Cocoa', '-o', str(host)], 'host-build')
    for fmt in ['clap', 'vst3']:
        binary = ROOT / f'target/{args.profile}/libdaw_drive_{fmt}.dylib'
        output = run([str(host), fmt, str(binary), str(work)], 'native-' + fmt)
        report['formats'][fmt] = json.loads(output.strip().splitlines()[-1])
    report['run_completed'] = True
    (work / 'native-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
