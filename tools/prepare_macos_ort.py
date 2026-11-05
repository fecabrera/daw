"""Prepare the pinned ARM macOS ONNX archive without streaming decompression."""
import hashlib
import lzma
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile

# Match the coreml row in ort-sys at the revision in Cargo.lock.
ORT_REVISION = 'f2a5f73c9dd9ff3682303481cc792bf4669e3e94'
ARCHIVE_URL = 'https://cdn.pyke.io/0/pyke:ort-rs/ms@1.31.0/aarch64-apple-darwin+coreml.tar.lzma2'
ARCHIVE_SHA256 = '4033efca0e6c1606bd2ec3870d4cb19d0dba0ad0160dbfb616e66cae969ed68c'


def extract_verified(archive, destination):
    with archive.open('rb') as source:
        if hashlib.file_digest(source, 'sha256').hexdigest() != ARCHIVE_SHA256:
            raise ValueError('ONNX Runtime archive checksum mismatch')
    # Upstream uses raw LZMA2 with a 64 MiB dictionary, then a tar stream.
    with lzma.open(archive, format=lzma.FORMAT_RAW,
                   filters=[{'id': lzma.FILTER_LZMA2, 'dict_size': 1 << 26}]) as source:
        with tarfile.open(fileobj=source, mode='r|') as entries:
            for member in entries:
                # The native distribution needs only directories and regular files.
                path = PurePosixPath(member.name)
                if path.is_absolute() or '..' in path.parts:
                    raise ValueError('Unsafe ONNX Runtime archive path')
                if not (member.isdir() or member.isfile()):
                    raise ValueError('Unsupported ONNX Runtime archive entry')
                entries.extract(member, destination, filter='data')
    if not (destination / 'libonnxruntime.a').is_file():
        raise ValueError('ONNX Runtime archive is missing libonnxruntime.a')


def prepare(cache_root):
    cache = cache_root / 'dfbin' / 'aarch64-apple-darwin'
    destination = cache / ARCHIVE_SHA256
    if (destination / 'libonnxruntime.a').is_file():
        return destination
    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='daw-ort-', dir=cache) as temporary:
        temporary = Path(temporary)
        archive = temporary / 'runtime.tar.lzma2'
        subprocess.run(['curl', '--fail', '--location', '--retry', '3',
                        '--retry-all-errors', '--proto', '=https', '--proto-redir', '=https',
                        '--connect-timeout', '30',
                        '--max-time', '300', '--output', str(archive), ARCHIVE_URL], check=True)
        staging = temporary / 'extracted'
        staging.mkdir()
        extract_verified(archive, staging)
        # Publish only after checksum and extraction succeed. Never expose partial files.
        if destination.exists():
            raise ValueError(f'Incomplete ONNX Runtime cache: {destination}')
        staging.rename(destination)
    return destination


if __name__ == '__main__':
    print(prepare(Path.home() / 'Library' / 'Caches' / 'ort.pyke.io'))
