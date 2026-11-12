"""Exercise native shell packaging with generated artifacts and build shims."""
import os
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent


@unittest.skipUnless(os.name == "posix" and shutil.which("zip"), "POSIX shell and ZIP required")
class DesktopPackagingTests(unittest.TestCase):
    def test_target_directory_overrides_cannot_select_stale_executables(self):
        for platform in ('macos', 'linux'):
            for profile in ('debug', 'release'):
                for override in ('environment', 'config', 'target_environment', 'target_config'):
                    with self.subTest(platform=platform, profile=profile, override=override), tempfile.TemporaryDirectory() as tmp:
                        root = Path(tmp)
                        (root / 'tools').mkdir()
                        (root / 'docs/development').mkdir(parents=True)
                        shutil.copy(ROOT / 'docs/development/linux-runtime.txt', root / 'docs/development/linux-runtime.txt')
                        script = ROOT / 'tools' / f'package_{platform}.sh'
                        shutil.copy(script, root / 'tools' / script.name)
                        shutil.copy(ROOT / 'tools/package_sources.py', root / 'tools/package_sources.py')
                        shutil.copy(ROOT / 'tools/build_desktop.py', root / 'tools/build_desktop.py')
                        shutil.copy(ROOT / 'tools/runtime_notices.json', root / 'tools/runtime_notices.json')
                        shutil.copy(ROOT / 'LICENSE', root / 'LICENSE')
                        for directory in ('crates/unit-ui/assets/fonts', 'crates/ui/assets/icons', 'crates/ui/assets/licenses', 'crates/unit-ui/assets/licenses', 'crates/separation/assets/licenses', 'crates/media/assets/licenses', 'crates/timestretch/licenses', 'crates/oscillator/assets/licenses', 'crates/equalizer/assets/licenses'):
                            shutil.copytree(ROOT / directory, root / directory)
                        default = root / 'target' / profile
                        default.mkdir(parents=True)
                        (default / 'daw-desktop').write_text('stale')
                        (root / '.cargo').mkdir()
                        (root / '.cargo/config.toml').write_text('[build]\ntarget-dir = "elsewhere"\n' + ('target = "aarch64-unknown-linux-gnu"\n' if override == 'target_config' else ''))
                        shim = root / 'bin'
                        shim.mkdir()
                        cargo = shim / 'cargo'
                        cargo.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys, tomllib
args=sys.argv[1:]
if args[0] == 'vendor':
    if os.environ.get('FAIL_SOURCE_VENDOR'): sys.exit(7)
    vendor=pathlib.Path(args[-1]); (vendor/'fixture-1.0').mkdir(parents=True)
    (vendor/'fixture-1.0/LICENSE').write_text('fixture grant')
    print('[source.crates-io]'); print('replace-with = "vendored-sources"')
    print('[source.vendored-sources]'); print('directory = "'+str(vendor)+'"')
    sys.exit(0)
base=pathlib.Path(os.environ.get('CARGO_TARGET_DIR', 'elsewhere'))
if '--target-dir' in args: base=pathlib.Path(args[args.index('--target-dir')+1])
config=tomllib.loads(pathlib.Path('.cargo/config.toml').read_text())
target=os.environ.get('CARGO_BUILD_TARGET', config.get('build', {}).get('target'))
if '--target' in args: target=args[args.index('--target')+1]
if target: base=base/target
p=base/('release' if '--release' in args else 'debug')
p.mkdir(parents=True, exist_ok=True)
(p/'daw-desktop').write_text('unpublished' if os.environ.get('FAIL_SOURCE_VENDOR') else 'current')
if os.environ.get('FAIL_CARGO_BUILD'): sys.exit(8)
if not os.environ.get('OMIT_CARGO_ARTIFACT'):
    print(json.dumps({'reason':'compiler-artifact','target':{'name':'daw-desktop','kind':['bin']},'executable':str(p/'daw-desktop')}))
''')
                        cargo.chmod(0o755)
                        rustc = shim / 'rustc'
                        native_target = 'x86_64-apple-darwin' if platform == 'macos' else 'x86_64-unknown-linux-gnu'
                        rustc.write_text('#!/bin/sh\necho "host: ' + native_target + '"\n')
                        rustc.chmod(0o755)
                        uname = shim / 'uname'
                        # A translated toolchain can differ from the machine architecture.
                        uname.write_text('#!/bin/sh\nif [ "$1" = -s ]; then echo Linux; else echo aarch64; fi\n')
                        uname.chmod(0o755)
                        env = dict(os.environ, PATH=str(shim) + os.pathsep + os.environ['PATH'])
                        env.pop('CARGO_TARGET_DIR', None)
                        env.pop('CARGO_BUILD_TARGET', None)
                        if override == 'target_environment':
                            env['CARGO_BUILD_TARGET'] = 'aarch64-unknown-linux-gnu'
                        if override == 'environment':
                            env['CARGO_TARGET_DIR'] = str(root / 'custom build')
                        subprocess.run(['git', 'init', '-q', str(root)], check=True)
                        subprocess.run(['git', '-C', str(root), '-c', 'core.autocrlf=false', 'add', 'tools', 'crates', 'LICENSE', '.cargo'], check=True)
                        subprocess.run(['git', '-C', str(root), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture'], check=True)
                        subprocess.run(['sh', str(root / 'tools' / script.name), profile], cwd=root, env=env, check=True, capture_output=True)
                        if platform == 'macos':
                            data = (default / 'DAW.app/Contents/MacOS/daw-desktop').read_bytes()
                            source = default / 'DAW.app/Contents/Resources'
                            self.assertTrue((source/'DAW-source.zip').is_file())
                            manifest=json.loads((source/'source-manifest.json').read_text())
                            self.assertEqual(manifest['binary_sha256'],hashlib.sha256(data).hexdigest())
                            notices = default / 'DAW.app/Contents/Resources/licenses'
                            for entry in json.loads((ROOT / 'tools/runtime_notices.json').read_text())['notices']:
                                if 'desktop' in entry['deliverables']:
                                    payload = (notices / Path(entry['notice']).name).read_bytes()
                                    self.assertEqual(hashlib.sha256(payload).hexdigest(), entry['sha256'])
                        else:
                            with zipfile.ZipFile(default / 'DAW-linux-x64.zip') as archive:
                                self.assertIn(b'GTK 4.8', archive.read('DAW/LINUX-REQUIREMENTS.txt'))
                                data = archive.read('DAW/daw-desktop')
                                manifest=json.loads(archive.read('DAW/source-manifest.json'))
                                self.assertEqual(manifest['binary_sha256'],hashlib.sha256(data).hexdigest())
                                source=archive.read('DAW/DAW-source.zip')
                                self.assertEqual(manifest['source_sha256'],hashlib.sha256(source).hexdigest())
                                self.assertIn(manifest['revision'],archive.read('DAW/SOURCE-ACCESS.txt').decode())
                                for entry in json.loads((ROOT / 'tools/runtime_notices.json').read_text())['notices']:
                                    if 'desktop' in entry['deliverables']:
                                        payload = archive.read('DAW/licenses/' + Path(entry['notice']).name)
                                        self.assertEqual(hashlib.sha256(payload).hexdigest(), entry['sha256'])
                        self.assertEqual(data, b'current')
                        old_archive=(default/'DAW-linux-x64.zip').read_bytes() if platform=='linux' else None
                        for failure in ('FAIL_SOURCE_VENDOR', 'FAIL_CARGO_BUILD', 'OMIT_CARGO_ARTIFACT'):
                            env[failure] = '1'
                            failed=subprocess.run(['sh', str(root/'tools'/script.name),profile],cwd=root,env=env,capture_output=True)
                            self.assertNotEqual(failed.returncode,0, failed.stdout.decode())
                            if platform=='macos':
                                self.assertEqual((default/'DAW.app/Contents/MacOS/daw-desktop').read_bytes(),data)
                                self.assertTrue((default/'DAW.app/Contents/Resources/DAW-source.zip').is_file())
                            else:
                                self.assertEqual((default/'DAW-linux-x64.zip').read_bytes(),old_archive)
                            del env[failure]



if __name__ == '__main__':
    unittest.main()
