"""Corresponding source must match the build and remain usable after relocation."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import package_sources as sources

LICENSE_PATHS = ('LICENSE', 'NOTICE', 'crates/audio-analysis/LICENSE',
                 'crates/timestretch/LICENSE', 'crates/timestretch-cli/LICENSE')
PAPER_FIXTURE = b'paper excerpt fixture for packaging tests'


@contextmanager
def fixture(line_ending=b'\n'):
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        for name, value in [('user.name', 'Source fixture'), ('user.email', 'fixture@example.invalid')]:
            subprocess.run(['git', '-C', str(root), 'config', name, value], check=True)
        # Explicit bytes below must reach the commit unchanged on every host.
        subprocess.run(['git', '-C', str(root), 'config', 'core.autocrlf', 'false'], check=True)
        entries=[e for e in json.loads((sources.ROOT/'tools/runtime_notices.json').read_text())['notices']
                 if e['component'] in ('valuable-0.1.1', 'realfft-3.5.0', 'accesskit-0.24.1-chromium',
                                       'zlib-rs-0.6.8-paper-100k', 'vst3-0.3.0-steinberg-pluginterfaces',
                                       'clap-sys-0.5.0-clap-interface')]
        entries.sort(key=lambda e: e['component'] != 'valuable-0.1.1')
        for entry in entries:
            if 'file' in entry['source_package']:
                entry['source_package']['file_sha256'] = hashlib.sha256(PAPER_FIXTURE).hexdigest()
        (root/'tools').mkdir()
        (root/'tools/runtime_notices.json').write_text(json.dumps({'notices':entries}))
        for entry in entries:
            notice=root/entry['notice']; notice.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(sources.ROOT/entry['notice'],notice)
        (root/'Cargo.lock').write_bytes(b'exact locked sources' + line_ending)
        for name in LICENSE_PATHS:
            target = root/name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(sources.ROOT/name, target)
        (root/'.cargo').mkdir()
        (root/'.cargo/config.toml').write_text('[build]\njobs = 2\n')
        (root/'vendor/charon-audio').mkdir(parents=True)
        (root/'vendor/charon-audio/DAW-PATCH.md').write_bytes(b'local patch' + line_ending)
        subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'fixture'], check=True)
        binary = root/'daw-desktop'
        binary.write_bytes(b'binary built from the fixture')
        (root/'private-recording.wav').write_bytes(b'must not ship')
        with patch.object(sources, 'ROOT', root):
            yield root, binary, sources.revision()


class SourcePackages(unittest.TestCase):
    def test_source_archive_contains_exact_patch_lock_and_relocatable_vendor(self):
        for line_ending in (b'\n', b'\r\n'):
            with self.subTest(line_ending=line_ending):
                self._assert_source_archive(line_ending)

    def test_external_receipt_refresh_preserves_sealed_resources_and_tracks_signed_bytes(self):
        self._assert_source_archive(b'\n', external_receipt=True)

    def test_shared_plugin_source_archive_omits_desktop_receipts(self):
        self._assert_source_archive(b'\n', archive_only=True)

    def _assert_source_archive(self, line_ending, external_receipt=False, archive_only=False):
        with fixture(line_ending) as (root, binary, revision):
            check_output = subprocess.check_output
            def command(args, **kwargs):
                if args[:2] == ['cargo', 'vendor']:
                    vendor = Path(args[-1])
                    (vendor/'dependency-1.0').mkdir(parents=True)
                    (vendor/'dependency-1.0/LICENSE').write_text('upstream grant')
                    for entry in json.loads((root/'tools/runtime_notices.json').read_text())['notices']:
                        package=entry['source_package']; source=vendor/(package['name']+'-'+package['version']); source.mkdir()
                        (source/'.cargo_vcs_info.json').write_text(json.dumps({'git':{'sha1':package['git_revision']},'path_in_vcs':package['path_in_vcs']}))
                        (source/'.cargo-checksum.json').write_text(json.dumps({'files':{},'package':package['registry_checksum']}))
                        if 'file' in package:
                            payload = source/package['file']
                            payload.parent.mkdir(parents=True)
                            payload.write_bytes(PAPER_FIXTURE)

                    os.utime(vendor/'dependency-1.0/LICENSE',(0,0))
                    return f'[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "{vendor}"\n'.encode()
                return check_output(args, **kwargs)
            destination = root/'package'
            receipts = root/'DAW-source-metadata' if external_receipt else destination
            with patch.object(sources.subprocess, 'check_output', side_effect=command):
                if archive_only:
                    self.assertEqual(sources.source_archive(destination, revision, 'release'),
                                     destination/'DAW-source.zip')
                else:
                    sources.bundle(destination, binary, revision, 'release',
                                   receipt_destination=receipts if external_receipt else None)
            with zipfile.ZipFile(destination/'DAW-source.zip') as archive:
                for name in LICENSE_PATHS:
                    self.assertEqual(archive.read('DAW/'+name), (root/name).read_bytes())
                self.assertIn(b'cargo build --locked --lib --target <target> -p <package>',
                              archive.read('DAW/SOURCE-BUILD.txt'))
                self.assertEqual(archive.read('DAW/Cargo.lock'), b'exact locked sources' + line_ending)
                self.assertEqual(archive.read('DAW/vendor/charon-audio/DAW-PATCH.md'), b'local patch' + line_ending)
                self.assertEqual(archive.read('DAW/cargo-vendor/dependency-1.0/LICENSE'), b'upstream grant')
                self.assertEqual(archive.getinfo('DAW/cargo-vendor/dependency-1.0/LICENSE').date_time[0],1980)
                config = archive.read('DAW/.cargo/config.toml').decode()
                self.assertIn('jobs = 2', config)
                self.assertIn('directory = "cargo-vendor"', config)
                self.assertNotIn(str(root), config)
                self.assertFalse(any('private-recording' in name or '.git/' in name for name in archive.namelist()))
                self.assertEqual(len(archive.namelist()),len(set(archive.namelist())))
                notices=json.loads(archive.read('DAW/SOURCE-LICENSES/index.json'))
                self.assertEqual(len(notices),6)
                notice=archive.read('DAW/SOURCE-LICENSES/'+notices[0]['notice'])
                self.assertIn(b'Copyright (c) 2021 Valuable Contributors',notice)
                self.assertEqual(hashlib.sha256(notice).hexdigest(),notices[0]['sha256'])
                self.assertEqual(notices[0]['git_revision'],'9efc29b6e58cef28f6566a47aa7e142a55fead77')
                realfft=next(entry for entry in notices if entry['name']=='realfft')
                grant=archive.read('DAW/SOURCE-LICENSES/'+realfft['notice'])
                self.assertIn(b'Copyright (c) 2020 Henrik Enquist and RealFFT contributors',grant)
                self.assertIn(b'Permission is hereby granted',grant)
                self.assertEqual(hashlib.sha256(grant).hexdigest(),realfft['sha256'])
                self.assertEqual(realfft['version'],'3.5.0')
                self.assertEqual(realfft['git_revision'],'d0d4eee0525fd27c96c8a046d6d107acd5ed84a6')
                self.assertIn('9fdcf59c60c7ecacbf75774d378a068811a2e6ea',realfft['license_url'])
                accesskit=next(entry for entry in notices if entry['name']=='accesskit')
                grant=archive.read('DAW/SOURCE-LICENSES/'+accesskit['notice'])
                self.assertIn(b'Copyright 2015 The Chromium Authors',grant)
                self.assertIn(b'Redistributions in binary form must reproduce',grant)
                self.assertIn(b'THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS',grant)
                self.assertEqual(hashlib.sha256(grant).hexdigest(),accesskit['sha256'])
                self.assertEqual(accesskit['version'],'0.24.1')
                self.assertEqual(accesskit['path_in_vcs'],'common')
                self.assertEqual(accesskit['git_revision'],'a55d3e1a18bb9ef0e4bccc9083fb13c3e0ad8969')
                paper = next(entry for entry in notices if entry['name'] == 'zlib-rs')
                self.assertEqual(paper['license_expression'], 'CC-BY-4.0')
                self.assertEqual(archive.read('DAW/cargo-vendor/zlib-rs-0.6.8/'+paper['file']), PAPER_FIXTURE)
                self.assertEqual(paper['file_sha256'], hashlib.sha256(PAPER_FIXTURE).hexdigest())
                attribution = archive.read('DAW/SOURCE-LICENSES/'+paper['notice'])
                self.assertIn(b'Federico Comoglio and Renato Paro', attribution)
                self.assertIn(b'https://doi.org/10.1371/journal.pcbi.1003419', attribution)
                self.assertIn(b'https://creativecommons.org/licenses/by/4.0/', attribution)
                self.assertEqual(hashlib.sha256(attribution).hexdigest(), paper['sha256'])
                for name, upstream_ref, copyright_holder in (
                    ('vst3', '31d6eeba6daaa3e2a8bfbe3e7a90ca0b7fbfbc1c', b'Steinberg Media Technologies GmbH'),
                    ('clap-sys', '1.2.2', b'Alexandre BIQUE'),
                ):
                    entry = next(item for item in notices if item['name'] == name)
                    self.assertEqual(entry['license_expression'], 'MIT')
                    self.assertEqual(entry['upstream_ref'], upstream_ref)
                    notice = archive.read('DAW/SOURCE-LICENSES/'+entry['notice'])
                    self.assertIn(copyright_holder, notice)
                    self.assertEqual(hashlib.sha256(notice).hexdigest(), entry['sha256'])

            if archive_only:
                self.assertEqual({entry.name for entry in destination.iterdir()}, {'DAW-source.zip'})
                return
            manifest=json.loads((receipts/'source-manifest.json').read_text())
            self.assertEqual(manifest['revision'],revision)
            self.assertEqual(manifest['binary_sha256'],hashlib.sha256(binary.read_bytes()).hexdigest())
            self.assertEqual(manifest['source_sha256'],hashlib.sha256((destination/'DAW-source.zip').read_bytes()).hexdigest())
            self.assertIn(revision,(destination/'SOURCE-ACCESS.txt').read_text())
            if external_receipt:
                self.assertFalse((destination/'source-manifest.json').exists())
                self.assertEqual(manifest['binary_path'], '../daw-desktop')
                self.assertEqual(manifest['source_archive'], '../package/DAW-source.zip')
                sealed = {p.name:p.read_bytes() for p in destination.iterdir()}
                binary.write_bytes(b'executable after final bundle signing')
                sources.refresh_receipt(receipts, binary, destination/'DAW-source.zip', revision, 'release')
                refreshed = json.loads((receipts/'source-manifest.json').read_text())
                self.assertEqual(refreshed['binary_sha256'], hashlib.sha256(binary.read_bytes()).hexdigest())
                self.assertEqual({p.name:p.read_bytes() for p in destination.iterdir()}, sealed)
                before = (receipts/'source-manifest.json').read_bytes()
                (destination/'DAW-source.zip').write_bytes(b'changed source archive')
                with self.assertRaisesRegex(RuntimeError, 'source archive changed'):
                    sources.refresh_receipt(receipts, binary, destination/'DAW-source.zip', revision, 'release')
                self.assertEqual((receipts/'source-manifest.json').read_bytes(), before)
                (destination/'DAW-source.zip').write_bytes(sealed['DAW-source.zip'])
                (root/'Cargo.lock').write_bytes(b'changed tracked source')
                with self.assertRaisesRegex(RuntimeError, 'Commit tracked changes'):
                    sources.refresh_receipt(receipts, binary, destination/'DAW-source.zip', revision, 'release')
                self.assertEqual((receipts/'source-manifest.json').read_bytes(), before)
            else:
                self.assertNotIn('binary_path', manifest)

    def test_dirty_or_changed_revision_cannot_publish_source_for_a_binary(self):
        with fixture() as (root,binary,revision):
            destination=root/'package'
            with self.assertRaisesRegex(RuntimeError,'revision changed'):
                sources.bundle(destination,binary,'0'*40,'debug')
            (root/'Cargo.lock').write_text('uncommitted dependency change')
            with self.assertRaisesRegex(RuntimeError,'Commit tracked changes'):
                sources.bundle(destination,binary,revision,'debug')
            self.assertFalse(destination.exists())

    def test_wrong_source_revision_and_notice_bytes_are_rejected(self):
        with fixture() as (root,_,_):
            entry=json.loads((root/'tools/runtime_notices.json').read_text())['notices'][0]
            package=entry['source_package']; vendor=root/'cargo-vendor'; valuable=vendor/'valuable-0.1.1'; valuable.mkdir(parents=True)
            vcs=valuable/'.cargo_vcs_info.json'; checksum=valuable/'.cargo-checksum.json'
            vcs.write_text(json.dumps({'git':{'sha1':'0'*40},'path_in_vcs':package['path_in_vcs']}))
            checksum.write_text(json.dumps({'package':package['registry_checksum']}))
            with self.assertRaisesRegex(RuntimeError,'does not match shipped source'):
                sources.source_notices(vendor)
            vcs.write_text(json.dumps({'git':{'sha1':package['git_revision']},'path_in_vcs':package['path_in_vcs']}))
            checksum.write_text(json.dumps({'package':'0'*64}))
            with self.assertRaisesRegex(RuntimeError,'does not match shipped source'):
                sources.source_notices(vendor)
            checksum.write_text(json.dumps({'package':package['registry_checksum']}))
            (root/entry['notice']).write_text('incomplete grant')
            with self.assertRaisesRegex(RuntimeError,'hash mismatch'):
                sources.source_notices(vendor)

    def test_attribution_rejects_a_missing_or_changed_source_file(self):
        with fixture() as (root, _, _):
            entries = json.loads((root/'tools/runtime_notices.json').read_text())['notices']
            package = next(e['source_package'] for e in entries if e['source_package']['name'] == 'zlib-rs')
            vendor = root/'cargo-vendor'
            source = vendor/'zlib-rs-0.6.8'
            source.mkdir(parents=True)
            (source/'.cargo_vcs_info.json').write_text(json.dumps({
                'git': {'sha1': package['git_revision']}, 'path_in_vcs': package['path_in_vcs']}))
            (source/'.cargo-checksum.json').write_text(json.dumps({'package': package['registry_checksum']}))
            with self.assertRaisesRegex(RuntimeError, 'attribution does not match shipped file'):
                sources.source_notices(vendor)
            file = source/package['file']
            file.parent.mkdir(parents=True)
            file.write_bytes(b'changed paper excerpt')
            with self.assertRaisesRegex(RuntimeError, 'attribution does not match shipped file'):
                sources.source_notices(vendor)
