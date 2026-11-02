"""Exercise native shell packaging with generated artifacts and build shims."""
import os
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
                for override in ('environment', 'config'):
                    with self.subTest(platform=platform, profile=profile, override=override), tempfile.TemporaryDirectory() as tmp:
                        root = Path(tmp)
                        (root / 'tools').mkdir()
                        script = ROOT / 'tools' / f'package_{platform}.sh'
                        shutil.copy(script, root / 'tools' / script.name)
                        shutil.copy(ROOT / 'LICENSE', root / 'LICENSE')
                        for directory in ('crates/unit-ui/assets/fonts', 'crates/ui/assets/icons', 'crates/ui/assets/licenses', 'crates/unit-ui/assets/licenses', 'crates/separation/assets/licenses', 'crates/media/assets/licenses', 'crates/timestretch/licenses', 'crates/oscillator/assets/licenses', 'crates/equalizer/assets/licenses'):
                            shutil.copytree(ROOT / directory, root / directory)
                        default = root / 'target' / profile
                        default.mkdir(parents=True)
                        (default / 'daw-desktop').write_text('stale')
                        (root / '.cargo').mkdir()
                        (root / '.cargo/config.toml').write_text('[build]\ntarget-dir = "elsewhere"\n')
                        shim = root / 'bin'
                        shim.mkdir()
                        cargo = shim / 'cargo'
                        cargo.write_text('''#!/usr/bin/env python3
import os, pathlib, sys
args=sys.argv[1:]
base=pathlib.Path(os.environ.get('CARGO_TARGET_DIR', 'elsewhere'))
if '--target-dir' in args: base=pathlib.Path(args[args.index('--target-dir')+1])
p=base/('release' if '--release' in args else 'debug')
p.mkdir(parents=True, exist_ok=True)
(p/'daw-desktop').write_text('current')
''')
                        cargo.chmod(0o755)
                        uname = shim / 'uname'
                        uname.write_text('#!/bin/sh\nif [ "$1" = -s ]; then echo Linux; else echo x86_64; fi\n')
                        uname.chmod(0o755)
                        env = dict(os.environ, PATH=str(shim) + os.pathsep + os.environ['PATH'])
                        env.pop('CARGO_TARGET_DIR', None)
                        if override == 'environment':
                            env['CARGO_TARGET_DIR'] = str(root / 'custom build')
                        subprocess.run(['sh', str(root / 'tools' / script.name), profile], cwd=root, env=env, check=True, capture_output=True)
                        if platform == 'macos':
                            data = (default / 'DAW.app/Contents/MacOS/daw-desktop').read_bytes()
                        else:
                            with zipfile.ZipFile(default / 'DAW-linux-x64.zip') as archive:
                                data = archive.read('DAW/daw-desktop')
                        self.assertEqual(data, b'current')


if __name__ == '__main__':
    unittest.main()
