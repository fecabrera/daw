"""Check packaged AU binaries with a headless native host; do not install plugins."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import struct
import subprocess
import tempfile
import zipfile
from package_plugins import ROOT


def validate_macho_linkedit(data):
    """Reject the pinned Rust/LLVM strip defect before relying on host dyld."""
    if len(data) < 32 or struct.unpack_from('<I', data)[0] != 0xFEEDFACF:
        raise ValueError('expected a little-endian 64-bit Mach-O library')
    commands, command_bytes = struct.unpack_from('<II', data, 16)
    end = 32 + command_bytes
    if end > len(data) or commands > command_bytes // 8:
        raise ValueError('invalid Mach-O load command bounds')
    offset = 32
    for _ in range(commands):
        if offset + 8 > end:
            raise ValueError('truncated Mach-O load command')
        command, size = struct.unpack_from('<II', data, offset)
        if size < 8 or offset + size > end:
            raise ValueError('invalid Mach-O load command size')
        if command == 2:  # LC_SYMTAB, stable public Mach-O C ABI.
            if size < 24:
                raise ValueError('truncated LC_SYMTAB')
            strings, length = struct.unpack_from('<II', data, offset + 16)
            if strings % 8 or strings + length > len(data):
                raise ValueError('invalid or misaligned LINKEDIT string pool')
        offset += size


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archives', type=Path, nargs='+')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='daw-au-host-') as temporary:
        base = Path(temporary)
        host = base / 'host'
        subprocess.run(['xcrun', 'clang', str(ROOT / 'crates/plugin-auv2/tests/host.m'),
                        '-framework', 'Foundation', '-framework', 'AudioToolbox',
                        '-framework', 'CoreFoundation', '-o', str(host)], check=True)
        bundles = []
        for index, archive in enumerate(args.archives):
            directory = base / str(index)
            with zipfile.ZipFile(archive) as zipped:
                for item in zipped.infolist():
                    path = (directory / item.filename).resolve()
                    if not path.is_relative_to(directory.resolve()):
                        raise ValueError('archive entry escapes extraction directory')
                zipped.extractall(directory)
                for item in zipped.infolist():
                    (directory / item.filename).chmod(item.external_attr >> 16 & 0o777)
            bundle = next(directory.glob('*.component'))
            info = plistlib.loads((bundle / 'Contents/Info.plist').read_bytes())
            binary = bundle / 'Contents/MacOS' / info['CFBundleExecutable']
            metadata = json.loads((directory / 'build.json').read_text())
            binary_data = binary.read_bytes()
            validate_macho_linkedit(binary_data)
            if hashlib.sha256(binary_data).hexdigest() != metadata['binary_sha256']:
                raise ValueError('packaged binary hash mismatch')
            subprocess.run(['codesign', '--verify', '--strict', str(bundle)], check=True)
            bundles.append(str(bundle.resolve()))
        subprocess.run([str(host), *bundles], check=True)


if __name__ == '__main__':
    main()
