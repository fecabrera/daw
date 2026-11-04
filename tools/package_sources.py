"""Bundle the exact application source and locked dependencies beside desktop binaries."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args])


def revision():
    # Untracked recordings and scratch files are never source-package inputs.
    if git('status', '--porcelain', '--untracked-files=no').strip():
        raise RuntimeError('Commit tracked changes before packaging corresponding source.')
    return git('rev-parse', 'HEAD').decode().strip()


def bundle(destination, binary, expected_revision, profile):
    if revision() != expected_revision:
        raise RuntimeError('Source revision changed during the desktop build.')
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / 'DAW-source.zip'
    with tempfile.TemporaryDirectory(prefix='daw-source-') as temporary:
        vendor = Path(temporary) / 'vendor'
        config = subprocess.check_output(
            ['cargo', 'vendor', '--locked', '--versioned-dirs', str(vendor)], cwd=ROOT
        ).decode()
        # Cargo's generated config must refer to the bundled location, not this machine.
        config = config.replace(str(vendor).replace('\\', '\\\\'), 'vendor').replace(str(vendor), 'vendor')
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as output:
            original_config = ''
            with tarfile.open(fileobj=io.BytesIO(git('archive', expected_revision)), mode='r:') as source:
                for entry in source:
                    if entry.issym() or entry.islnk():
                        raise RuntimeError('Source packages require regular tracked files, not symlinks.')
                    if entry.isfile():
                        data = source.extractfile(entry).read()
                        if entry.name == '.cargo/config.toml':
                            original_config = data.decode()
                            continue
                        info = zipfile.ZipInfo('DAW/' + entry.name)
                        info.external_attr = (entry.mode | 0o100000) << 16
                        info.compress_type = zipfile.ZIP_DEFLATED
                        output.writestr(info, data)
            for path in sorted(vendor.rglob('*')):
                if path.is_symlink():
                    raise RuntimeError('Vendored source contains a symlink.')
                if path.is_file():
                    output.write(path, 'DAW/vendor/' + path.relative_to(vendor).as_posix())
            # Retain the local Charon patch already present in the committed vendor tree.
            output.writestr('DAW/.cargo/config.toml', original_config + '\n' + config)
            output.writestr('DAW/SOURCE-BUILD.txt', f'''Corresponding application source for DAW commit {expected_revision}.
Build profile: {profile}. See docs/development for native platform prerequisites.
This archive contains the committed application, local vendor patches, Cargo.lock,
and the exact registry/Git Rust dependency sources, including bundled LAME and
Rubber Band sources. The generated .cargo/config.toml selects this vendor directory.
Run cargo build --locked -p daw-desktop{' --release' if profile == 'release' else ''} to rebuild/relink.
Modify the application or LGPL libraries and rebuild with the same command to relink.
Native ONNX Runtime is downloaded by its checksum-pinned build adapter; exact
native component source locations, revisions, archive hashes and license terms are
in crates/separation/assets/licenses/Native-Stem-SOURCE-NOTICE.txt and
specs/audio/stem-native-dependencies.md. These immutable public source locations
are the equivalent source-access route for unchanged native inputs. Model weights
and operating-system libraries are not included. No model is needed to build.
See crates/media/assets/licenses for GPL/LGPL and other combined-work terms;
the root BSD license applies to original DAW code, not every combined component.
''')
        if revision() != expected_revision:
            raise RuntimeError('Tracked source changed while preparing the source package.')
    def digest(path):
        value = hashlib.sha256()
        with path.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                value.update(chunk)
        return value.hexdigest()
    manifest = {'revision': expected_revision, 'profile': profile,
                'binary': binary.name, 'binary_sha256': digest(binary),
                'source_archive': archive.name, 'source_sha256': digest(archive)}
    (destination / 'source-manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    (destination / 'SOURCE-ACCESS.txt').write_text(f'''DAW corresponding source and relinking materials
Build commit: {expected_revision}
Binary SHA-256: {manifest['binary_sha256']}

The exact application source and locked Rust/native media dependency sources are
provided beside this notice in DAW-source.zip (SHA-256 {manifest['source_sha256']}).
Unzip it and read DAW/SOURCE-BUILD.txt for build/relink steps and the immutable
source routes for the unchanged ONNX Runtime native components. Local patches,
locked Cargo inputs and native source/notice inventories are included.
Keep this entire source directory with the binary when redistributing this package.
''')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision-only', action='store_true')
    parser.add_argument('--revision')
    parser.add_argument('--destination', type=Path)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    if args.revision_only:
        print(revision())
    elif args.destination and args.binary and args.revision:
        bundle(args.destination, args.binary, args.revision, args.profile)
    else:
        parser.error('bundle requires --destination, --binary and --revision')


if __name__ == '__main__':
    main()
