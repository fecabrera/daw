"""Check verified, transactional native runtime preparation."""
import hashlib
import io
import lzma
from pathlib import Path
import tarfile
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import prepare_macos_ort as ort


class PrepareOrtTests(unittest.TestCase):
    def archive(self, name='libonnxruntime.a', kind=tarfile.REGTYPE):
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode='w', format=tarfile.USTAR_FORMAT) as archive:
            entry = tarfile.TarInfo(name)
            entry.type = kind
            entry.linkname = '/outside'
            entry.size = 7 if kind == tarfile.REGTYPE else 0
            archive.addfile(entry, io.BytesIO(b'runtime') if entry.size else None)
        return lzma.compress(data.getvalue(), format=lzma.FORMAT_RAW,
                             filters=[{'id': lzma.FILTER_LZMA2, 'dict_size': 1 << 26}])

    def test_success_publishes_once_and_reuses_cache(self):
        payload = self.archive()
        digest = hashlib.sha256(payload).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            def download(command, **kwargs):
                Path(command[command.index('--output') + 1]).write_bytes(payload)
            with patch.object(ort, 'ARCHIVE_SHA256', digest), patch.object(ort.subprocess, 'run', side_effect=download) as run:
                root = Path(temporary)
                result = ort.prepare(root)
                self.assertEqual((result / 'libonnxruntime.a').read_bytes(), b'runtime')
                self.assertEqual(ort.prepare(root), result)
                self.assertEqual(run.call_count, 1)

    def test_bad_download_checksum_paths_and_links_never_publish(self):
        for name, kind in [('libonnxruntime.a', tarfile.REGTYPE),
                           ('../outside', tarfile.REGTYPE),
                           ('/outside', tarfile.REGTYPE),
                           ('libonnxruntime.a', tarfile.SYMTYPE),
                           ('libonnxruntime.a', tarfile.LNKTYPE),
                           ('other.a', tarfile.REGTYPE)]:
            with self.subTest(name=name, kind=kind), tempfile.TemporaryDirectory() as temporary:
                payload = self.archive(name, kind)
                # The ordinary archive case deliberately has the wrong digest.
                digest = 'bad' if name == 'libonnxruntime.a' and kind == tarfile.REGTYPE else hashlib.sha256(payload).hexdigest()
                def download(command, **kwargs):
                    Path(command[command.index('--output') + 1]).write_bytes(payload)
                with patch.object(ort, 'ARCHIVE_SHA256', digest), patch.object(ort.subprocess, 'run', side_effect=download):
                    with self.assertRaises((ValueError, tarfile.FilterError)):
                        ort.prepare(Path(temporary))
                self.assertEqual(list((Path(temporary) / 'dfbin/aarch64-apple-darwin').iterdir()), [])

    def test_failed_download_does_not_publish_or_leave_temporary_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(ort.subprocess, 'run', side_effect=subprocess.CalledProcessError(22, 'curl')):
                with self.assertRaises(subprocess.CalledProcessError):
                    ort.prepare(Path(temporary))
            self.assertEqual(list((Path(temporary) / 'dfbin/aarch64-apple-darwin').iterdir()), [])

    def test_dependency_pin_matches_lockfile(self):
        root = Path(__file__).resolve().parent.parent
        self.assertIn(f'git+https://github.com/pykeio/ort?rev={ort.ORT_REVISION}#{ort.ORT_REVISION}',
                      (root / 'Cargo.lock').read_text())


if __name__ == '__main__':
    unittest.main()
