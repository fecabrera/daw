"""Native bundle seals and exact post-signing receipts; no application launch."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest

from desktop_packaging_fixtures import stage_notice_sources

ROOT = Path(__file__).resolve().parent.parent
NATIVE = sys.platform == 'darwin' and shutil.which('codesign') and shutil.which('cc')


@contextmanager
def fixture():
    with tempfile.TemporaryDirectory(prefix='signed DAW package ') as temporary:
        base = Path(temporary)
        root = base / 'repository'
        (root / 'tools').mkdir(parents=True)
        for name in ('package_macos.sh', 'package_macos_dmg.sh', 'build_desktop.py', 'package_sources.py', 'runtime_notices.json',
                     'copy_desktop_notices.sh', 'desktop_notices.txt', 'package_desktop_posix.sh'):
            shutil.copy(ROOT / 'tools' / name, root / 'tools' / name)
        shutil.copy(ROOT / 'LICENSE', root / 'LICENSE')
        stage_notice_sources(ROOT, root, app_icons=True)
        (root / 'fixture.c').write_text('int main(void) { return 0; }\n')
        seed = base / 'native executable'
        subprocess.run(['cc', str(root / 'fixture.c'), '-o', str(seed)], check=True, capture_output=True)
        shim = base / 'bin'
        shim.mkdir()
        host = 'aarch64-apple-darwin' if platform.machine() == 'arm64' else 'x86_64-apple-darwin'
        (shim / 'rustc').write_text('#!/bin/sh\necho "host: ' + host + '"\n')
        (shim / 'cargo').write_text('''#!/usr/bin/env python3
import json, os, pathlib, shutil, sys
args=sys.argv[1:]
if args[0]=='vendor':
    vendor=pathlib.Path(args[-1]); (vendor/'fixture-1.0').mkdir(parents=True)
    (vendor/'fixture-1.0/LICENSE').write_text('fixture source grant')
    print('[source.crates-io]\\nreplace-with = "vendored-sources"\\n[source.vendored-sources]')
    print('directory = "'+str(vendor)+'"')
else:
    output=pathlib.Path(args[args.index('--target-dir')+1])/args[args.index('--target')+1]/('release' if '--release' in args else 'debug')
    output.mkdir(parents=True,exist_ok=True)
    binary=output/'daw-desktop'; shutil.copyfile(os.environ['DAW_FIXTURE_BINARY'],binary)
    print(json.dumps({'reason':'compiler-artifact','target':{'name':'daw-desktop','kind':['bin']},'executable':str(binary)}))
''')
        (shim / 'mv').write_text('''#!/usr/bin/env python3
import os, pathlib, sys
args=sys.argv[1:]
if (os.environ.get('FAIL_METADATA_PUBLISH') and len(args)==2
    and pathlib.Path(args[0]).name=='DAW-source-metadata'
    and pathlib.Path(args[1]).name=='DAW-source-metadata'):
    sys.exit(9)
os.execv('/bin/mv',['mv',*args])
''')
        for path in shim.iterdir():
            path.chmod(0o755)
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        subprocess.run(['git', '-C', str(root), '-c', 'core.autocrlf=false', 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(root), '-c', 'user.name=Fixture', '-c',
                        'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture'], check=True)
        env = dict(os.environ, PATH=str(shim)+os.pathsep+os.environ['PATH'], DAW_FIXTURE_BINARY=str(seed))
        yield root, base, env


def snapshot(directory):
    return {p.relative_to(directory).as_posix(): p.read_bytes()
            for p in directory.rglob('*') if p.is_file()}


@unittest.skipUnless(os.name == 'posix', 'POSIX shell required')
class DiskImageRetries(unittest.TestCase):
    def package(self, mode):
        with tempfile.TemporaryDirectory(prefix='DMG retry ') as temporary:
            root = Path(temporary)
            (root / 'tools').mkdir()
            shutil.copy(ROOT / 'tools/package_macos_dmg.sh', root / 'tools')
            (root / 'tools/package_macos.sh').write_text('''#!/bin/sh
set -eu
mkdir -p target/debug/DAW.app target/debug/DAW-source-metadata
''')
            output = root / 'target/debug'
            output.mkdir(parents=True)
            artifact = output / 'DAW-macos-arm64.dmg'
            artifact.write_bytes(b'previous verified image')
            shim = root / 'bin'
            shim.mkdir()
            (shim / 'uname').write_text('''#!/bin/sh
case "$1" in
  -s) echo Darwin ;;
  -m) echo arm64 ;;
esac
''')
            (shim / 'codesign').write_text('#!/bin/sh\nexit 0\n')
            (shim / 'sleep').write_text('''#!/bin/sh
echo "$1" >> "$DMG_TEST_ROOT/delays"
''')
            (shim / 'hdiutil').write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(os.environ['DMG_TEST_ROOT'])
log = root / 'calls'
calls = log.read_text().splitlines() if log.exists() else []
with log.open('a') as output:
    output.write(json.dumps(sys.argv[1:]) + '\\n')
mode = os.environ['DMG_TEST_MODE']
image = pathlib.Path(sys.argv[-1])
if sys.argv[1] == 'create':
    assert not image.exists(), 'partial image reused'
    image.write_bytes(b'partial image')
    if mode == 'fatal':
        print('hdiutil: create failed - Permission denied', file=sys.stderr)
        sys.exit(13)
    if mode == 'busy' or (mode == 'recover' and len(calls) < 2):
        print('hdiutil: create failed - Resource busy', file=sys.stderr)
        sys.exit(16)
    image.write_bytes(b'new verified image')
elif sys.argv[1] == 'verify':
    assert image.read_bytes() == b'new verified image'
    if mode == 'verify_failure':
        sys.exit(9)
''')
            for path in shim.iterdir():
                path.chmod(0o755)
            result = subprocess.run(
                ['sh', str(root / 'tools/package_macos_dmg.sh'), 'debug'],
                cwd=root, capture_output=True, text=True,
                env=dict(os.environ, PATH=str(shim) + os.pathsep + os.environ['PATH'],
                         DMG_TEST_ROOT=str(root), DMG_TEST_MODE=mode),
            )
            calls = [json.loads(line) for line in (root / 'calls').read_text().splitlines()]
            delays = (root / 'delays').read_text().splitlines() if (root / 'delays').exists() else []
            self.assertFalse(list(output.glob('.daw-dmg.*')))
            return result, artifact.read_bytes(), [args[0] for args in calls], delays

    def test_busy_creation_recovers_and_verifies_before_publication(self):
        result, image, calls, delays = self.package('recover')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(image, b'new verified image')
        self.assertEqual(calls, ['create', 'create', 'create', 'verify'])
        self.assertEqual(delays, ['2', '4'])
        self.assertEqual(result.stderr.count('hdiutil: create failed - Resource busy'), 2)

    def test_persistent_busy_error_is_bounded_and_preserves_previous_image(self):
        result, image, calls, delays = self.package('busy')
        self.assertEqual(result.returncode, 16)
        self.assertEqual(image, b'previous verified image')
        self.assertEqual(calls, ['create'] * 5)
        self.assertEqual(delays, ['2', '4', '6', '8'])

    def test_other_creation_errors_fail_without_retry(self):
        result, image, calls, delays = self.package('fatal')
        self.assertEqual(result.returncode, 13)
        self.assertIn('Permission denied', result.stderr)
        self.assertEqual(image, b'previous verified image')
        self.assertEqual(calls, ['create'])
        self.assertEqual(delays, [])

    def test_verification_failure_is_not_retried_or_published(self):
        result, image, calls, delays = self.package('verify_failure')
        self.assertEqual(result.returncode, 9)
        self.assertEqual(image, b'previous verified image')
        self.assertEqual(calls, ['create', 'verify'])
        self.assertEqual(delays, [])


@unittest.skipUnless(NATIVE, 'native macOS compiler and codesign required')
class MacosSigning(unittest.TestCase):
    def test_complete_seal_final_receipts_copy_tampering_and_rollback(self):
        with fixture() as (root, base, env):
            def package(local=False, **overrides):
                command=['sh', str(root/'tools/package_macos.sh'), 'debug']
                if local:
                    command.append('--local')
                return subprocess.run(command, env=dict(env, **overrides), cwd=root,
                                      capture_output=True, text=True)
            result=package()
            self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
            app=root/'target/debug/DAW.app'
            receipts=root/'target/debug/DAW-source-metadata'
            subprocess.run(['codesign','--verify','--deep','--strict',str(app)], check=True, capture_output=True)
            manifest=json.loads((receipts/'source-manifest.json').read_text())
            self.assertEqual(hashlib.sha256((receipts/manifest['binary_path']).read_bytes()).hexdigest(), manifest['binary_sha256'])
            self.assertEqual(hashlib.sha256((receipts/manifest['source_archive']).read_bytes()).hexdigest(), manifest['source_sha256'])
            self.assertFalse((app/'Contents/Resources/source-manifest.json').exists())
            self.assertTrue((app/'Contents/Resources/SOURCE-ACCESS.txt').is_file())
            previous=(snapshot(app), snapshot(receipts))
            result=package(FAIL_METADATA_PUBLISH='1')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual((snapshot(app),snapshot(receipts)), previous)
            self.assertFalse(list((root/'target/debug').glob('.daw-app.*')))
            subprocess.run(['codesign','--verify','--deep','--strict',str(app)], check=True, capture_output=True)
            (root/'LICENSE').write_text('uncommitted local fixture')
            result=package(local=True)
            self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
            self.assertFalse(receipts.exists())
            self.assertTrue((app/'Contents/Resources/LOCAL-BUILD.txt').is_file())
            subprocess.run(['codesign','--verify','--deep','--strict',str(app)], check=True, capture_output=True)
            copied=base/'copied DAW.app'
            subprocess.run(['cp','-R',str(app),str(copied)], check=True)
            subprocess.run(['codesign','--verify','--deep','--strict',str(copied)], check=True, capture_output=True)
            (copied/'Contents/Resources/LICENSE.txt').write_text('changed after signing')
            result=subprocess.run(['codesign','--verify','--deep','--strict',str(copied)], capture_output=True)
            self.assertNotEqual(result.returncode,0)

    def test_disk_image_verification_and_failure_preservation(self):
        with fixture() as (root, base, env):
            shim = base / 'bin' / 'hdiutil'
            shim.write_text("""#!/usr/bin/env python3
import os, sys
if os.environ.get('FAIL_DMG_VERIFY') and sys.argv[1]=='verify':
    sys.exit(9)
os.execv('/usr/bin/hdiutil', ['hdiutil', *sys.argv[1:]])
""")
            shim.chmod(0o755)
            command = ['sh', str(root / 'tools/package_macos_dmg.sh'), 'debug']
            result = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            artifact = next((root / 'target/debug').glob('DAW-macos-*.dmg'))
            previous = artifact.read_bytes()
            subprocess.run(['/usr/bin/hdiutil', 'verify', str(artifact)],
                           check=True, capture_output=True)
            result = subprocess.run(command, cwd=root, env=dict(env, FAIL_DMG_VERIFY='1'),
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(artifact.read_bytes(), previous)
            self.assertFalse(list((root / 'target/debug').glob('.daw-dmg.*')))
