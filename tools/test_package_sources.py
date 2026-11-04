"""Corresponding source must match the build and remain usable after relocation."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import package_sources as sources


@contextmanager
def fixture():
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        for name, value in [('user.name', 'Source fixture'), ('user.email', 'fixture@example.invalid')]:
            subprocess.run(['git', '-C', str(root), 'config', name, value], check=True)
        (root/'Cargo.lock').write_text('exact locked sources\n')
        (root/'.cargo').mkdir()
        (root/'.cargo/config.toml').write_text('[build]\njobs = 2\n')
        (root/'vendor/charon-audio').mkdir(parents=True)
        (root/'vendor/charon-audio/DAW-PATCH.md').write_text('local patch\n')
        subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'fixture'], check=True)
        binary = root/'daw-desktop'
        binary.write_bytes(b'binary built from the fixture')
        (root/'private-recording.wav').write_bytes(b'must not ship')
        with patch.object(sources, 'ROOT', root):
            yield root, binary, sources.revision()


class SourcePackages(unittest.TestCase):
    def test_source_archive_contains_exact_patch_lock_and_relocatable_vendor(self):
        with fixture() as (root, binary, revision):
            check_output = subprocess.check_output
            def command(args, **kwargs):
                if args[:2] == ['cargo', 'vendor']:
                    vendor = Path(args[-1])
                    (vendor/'dependency-1.0').mkdir(parents=True)
                    (vendor/'dependency-1.0/LICENSE').write_text('upstream grant')
                    os.utime(vendor/'dependency-1.0/LICENSE',(0,0))
                    return f'[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "{vendor}"\n'.encode()
                return check_output(args, **kwargs)
            destination = root/'package'
            with patch.object(sources.subprocess, 'check_output', side_effect=command):
                sources.bundle(destination, binary, revision, 'release')
            with zipfile.ZipFile(destination/'DAW-source.zip') as archive:
                self.assertEqual(archive.read('DAW/Cargo.lock'), b'exact locked sources\n')
                self.assertEqual(archive.read('DAW/vendor/charon-audio/DAW-PATCH.md'), b'local patch\n')
                self.assertEqual(archive.read('DAW/cargo-vendor/dependency-1.0/LICENSE'), b'upstream grant')
                self.assertEqual(archive.getinfo('DAW/cargo-vendor/dependency-1.0/LICENSE').date_time[0],1980)
                config = archive.read('DAW/.cargo/config.toml').decode()
                self.assertIn('jobs = 2', config)
                self.assertIn('directory = "cargo-vendor"', config)
                self.assertNotIn(str(root), config)
                self.assertFalse(any('private-recording' in name or '.git/' in name for name in archive.namelist()))
                self.assertEqual(len(archive.namelist()),len(set(archive.namelist())))
            manifest=json.loads((destination/'source-manifest.json').read_text())
            self.assertEqual(manifest['revision'],revision)
            self.assertEqual(manifest['binary_sha256'],hashlib.sha256(binary.read_bytes()).hexdigest())
            self.assertEqual(manifest['source_sha256'],hashlib.sha256((destination/'DAW-source.zip').read_bytes()).hexdigest())
            self.assertIn(revision,(destination/'SOURCE-ACCESS.txt').read_text())

    def test_dirty_or_changed_revision_cannot_publish_source_for_a_binary(self):
        with fixture() as (root,binary,revision):
            destination=root/'package'
            with self.assertRaisesRegex(RuntimeError,'revision changed'):
                sources.bundle(destination,binary,'0'*40,'debug')
            (root/'Cargo.lock').write_text('uncommitted dependency change')
            with self.assertRaisesRegex(RuntimeError,'Commit tracked changes'):
                sources.bundle(destination,binary,revision,'debug')
            self.assertFalse(destination.exists())
