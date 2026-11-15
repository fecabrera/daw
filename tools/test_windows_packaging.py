"""Exercise Windows packaging in PowerShell with a fake Cargo executable.

Set DAW_TEST_PWSH to a portable PowerShell path when it is not on PATH.
These are packaging fixtures, not native MSVC build checks.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent
POWERSHELL = os.environ.get('DAW_TEST_PWSH') or shutil.which('pwsh') or shutil.which('powershell.exe')
NOTICE_DIRS = (
    'crates/unit-ui/assets/fonts', 'crates/ui/assets/icons',
    'crates/ui/assets/licenses', 'crates/unit-ui/assets/licenses',
    'crates/media/assets/licenses', 'crates/oscillator/assets/licenses',
    'crates/equalizer/assets/licenses', 'crates/timestretch/licenses',
    'crates/separation/assets/licenses',
)


@unittest.skipUnless(POWERSHELL, 'PowerShell required')
class WindowsPackagingTests(unittest.TestCase):
    def test_local_flags_contents_and_failure_preservation(self):
        with tempfile.TemporaryDirectory(prefix='windows packaging 音 % # ') as temporary:
            root = Path(temporary)
            (root / 'tools').mkdir()
            shutil.copy(ROOT / 'tools/package_windows.ps1', root / 'tools/package_windows.ps1')
            shutil.copy(ROOT / 'LICENSE', root / 'LICENSE')
            for directory in NOTICE_DIRS:
                shutil.copytree(ROOT / directory, root / directory)
            extra_notice = 'archive test 音 % #.txt'
            (root / 'crates/ui/assets/licenses' / extra_notice).write_bytes(b'exact fixture notice')
            wrapper = root / 'run.ps1'
            wrapper.write_text(r'''
$ErrorActionPreference = 'Stop'
$env:OS = 'Windows_NT'
function global:python { throw 'SOURCE_PACKAGING_CALLED' }
function global:cargo {
    $buildProfile = if ($args -contains '--release') { 'release' } else { 'debug' }
    $outputDirectory = Join-Path $env:DAW_TEST_ROOT "outputs/$buildProfile"
    [void][System.IO.Directory]::CreateDirectory($outputDirectory)
    $binary = Join-Path $outputDirectory 'daw-desktop.exe'
    [System.IO.File]::WriteAllText($binary, "current-$buildProfile-$env:DAW_TEST_SCENARIO")
    [System.IO.File]::WriteAllText((Join-Path $env:DAW_TEST_ROOT 'cargo-called'), 'yes')
    $global:LASTEXITCODE = 0
    if ($env:DAW_TEST_FAILURE -eq 'build') { $global:LASTEXITCODE = 8; return }
    if ($env:DAW_TEST_FAILURE -eq 'artifact') { return }
    if ($env:DAW_TEST_SCENARIO -eq 'drive') {
        New-PSDrive -Name FixtureOutput -PSProvider FileSystem -Root (Join-Path $env:DAW_TEST_ROOT 'outputs') -Scope Global | Out-Null
        $binary = Join-Path "FixtureOutput:/$buildProfile" 'daw-desktop.exe'
    }
    @{reason='compiler-artifact'; target=@{name='daw-desktop'}; executable=$binary} | ConvertTo-Json -Compress
}
$packager = Join-Path $env:DAW_TEST_ROOT 'tools/package_windows.ps1'
switch ($env:DAW_TEST_SCENARIO) {
    'before' { & $packager --local release }
    'after' { & $packager release --local }
    'default' { & $packager --local }
    'native' { & $packager -Profile release -Local }
    'drive' { & $packager release --local }
    'distribution' { & $packager release }
    'invalid' { & $packager fast --local }
}
''')
            env = dict(os.environ, DAW_TEST_ROOT=str(root), POWERSHELL_TELEMETRY_OPTOUT='1')

            def run(scenario, failure=''):
                return subprocess.run([POWERSHELL, '-NoLogo', '-NoProfile', '-File', str(wrapper)],
                                      env=dict(env, DAW_TEST_SCENARIO=scenario, DAW_TEST_FAILURE=failure),
                                      capture_output=True, text=True)

            for scenario in ('distribution', 'invalid'):
                result = run(scenario)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((root / 'cargo-called').exists())
                if scenario == 'distribution':
                    self.assertIn('SOURCE_PACKAGING_CALLED', result.stderr)

            notices = json.loads((ROOT / 'tools/runtime_notices.json').read_text())['notices']
            for scenario in ('before', 'after', 'default', 'native', 'drive'):
                with self.subTest(scenario=scenario):
                    result = run(scenario)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    profile = 'debug' if scenario == 'default' else 'release'
                    archive_path = root / f'outputs/{profile}/DAW.zip'
                    with zipfile.ZipFile(archive_path) as archive:
                        names = archive.namelist()
                        self.assertIn('DAW/daw-desktop.exe', names)
                        for name in names:
                            self.assertTrue(name.startswith('DAW/'), name)
                            self.assertNotIn('\\', name)
                            self.assertFalse(any(part in ('', '.', '..') for part in name.split('/')), name)
                        self.assertEqual(archive.read('DAW/licenses/' + extra_notice), b'exact fixture notice')
                        self.assertEqual(archive.read('DAW/daw-desktop.exe'), f'current-{profile}-{scenario}'.encode())
                        self.assertIn(b'Local development build', archive.read('DAW/LOCAL-BUILD.txt'))
                        self.assertEqual(archive.read('DAW/LICENSE.txt'), (root / 'LICENSE').read_bytes())
                        for name in ('DAW-source.zip', 'source-manifest.json', 'SOURCE-ACCESS.txt'):
                            self.assertNotIn('DAW/' + name, archive.namelist())
                        for entry in notices:
                            if 'desktop' in entry['deliverables']:
                                payload = archive.read('DAW/licenses/' + Path(entry['notice']).name)
                                self.assertEqual(hashlib.sha256(payload).hexdigest(), entry['sha256'])
            previous = (root / 'outputs/release/DAW.zip').read_bytes()
            for failure in ('build', 'artifact', 'assembly'):
                if failure == 'assembly':
                    (root / 'LICENSE').unlink()
                result = run('after', failure)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual((root / 'outputs/release/DAW.zip').read_bytes(), previous)
                self.assertFalse(list((root / 'outputs/release').glob('.daw-package-*')))


if __name__ == '__main__':
    unittest.main()
