"""AU format selection, checked identities, bundle metadata and rollback."""
import contextlib
import io
from pathlib import Path
import plistlib
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import zipfile
from package_plugins import ROOT, available_units, package, select_apis, supported_apis

class Auv2Packaging(unittest.TestCase):
    def test_distinct_products_have_metadata_and_native_targets(self):
        triples, bundles, classes = set(), set(), set()
        units = available_units()
        self.assertEqual(len(units), 21)
        workflow = (ROOT / '.github/workflows/plugins.yml').read_text()
        for slug, path in units.items():
            manifest = tomllib.loads(path.read_text())
            au = manifest['auv2']
            self.assertEqual(manifest['schema_version'], 2)
            self.assertEqual(au['factory'], 'DAWAudioUnitFactory')
            self.assertEqual(au['component_version'], 256)
            triple = tuple(au[key] for key in ('type', 'subtype', 'manufacturer'))
            self.assertNotIn(triple, triples)
            triples.add(triple)
            self.assertNotIn(au['bundle_id'], bundles)
            bundles.add(au['bundle_id'])
            self.assertNotIn(au['cocoa_class'], classes)
            classes.add(au['cocoa_class'])
            cargo = tomllib.loads((ROOT / f'plugins/{slug}-auv2/Cargo.toml').read_text())
            self.assertEqual(cargo['package']['name'], f'daw-{slug}-auv2')
            self.assertIn(f'slug: {slug},', workflow)

    def test_platform_relative_all_and_early_explicit_au_rejection(self):
        for target in ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu'):
            self.assertEqual(select_apis('all', target), ['clap', 'vst3'])
            for selection in ('auv2', 'all,auv2', 'clap, auv2'):
                with tempfile.TemporaryDirectory() as temp, patch('package_plugins.build_plugins') as build:
                    output = Path(temp) / 'absent'
                    with self.assertRaisesRegex(ValueError, 'macOS'):
                        package(target, Path(temp) / 'target', output, units='drive', api=selection)
                    build.assert_not_called()
                    self.assertFalse(output.exists())
        # Workspace checks allow an inert Intel AU crate, never a distributable component.
        with tempfile.TemporaryDirectory() as temp, patch('package_plugins.build_plugins') as build:
            output = Path(temp) / 'absent'
            with self.assertRaisesRegex(ValueError, 'unsupported plugin target'):
                package('x86_64-apple-darwin', Path(temp) / 'target', output, units='drive', api='auv2')
            build.assert_not_called()
            self.assertFalse(output.exists())
        self.assertEqual(select_apis('all,clap,auv2'), ['clap', 'vst3', 'auv2'])
        self.assertEqual(select_apis('auv2, clap,auv2'), ['clap', 'auv2'])

    def test_all_product_au_bundles_in_both_profiles(self):
        target = 'aarch64-apple-darwin'
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            for release, profile, suffix in ((False, 'debug', '-debug'), (True, 'release', '')):
                libraries = base / 'target' / target / profile
                libraries.mkdir(parents=True)
                for slug in available_units():
                    (libraries / f'libdaw_{slug.replace("-", "_")}_auv2.dylib').write_bytes(slug.encode())
                with patch('package_plugins.build_plugins') as build, patch('package_plugins.finalize_macos_bundle'), contextlib.redirect_stdout(io.StringIO()):
                    package(target, base / 'target', base / 'out', units='all', release=release, api='auv2')
                self.assertEqual(build.call_args.args[3], ['auv2'])
                archives = list((base / 'out').glob(f'*-auv2{suffix}.zip'))
                self.assertEqual(len(archives), 21)
                for archive in archives:
                    with zipfile.ZipFile(archive) as zipped:
                        info_name = next(n for n in zipped.namelist() if n.endswith('/Contents/Info.plist'))
                        info = plistlib.loads(zipped.read(info_name))
                        self.assertTrue(info_name.split('/')[0].endswith(suffix + '.component'))
                        self.assertTrue(info['CFBundleIdentifier'].endswith('.auv2'))
                        descriptor = info['AudioComponents'][0]
                        self.assertEqual(descriptor['type'], 'aufx')
                        self.assertEqual(descriptor['factoryFunction'], 'DAWAudioUnitFactory')
                        self.assertNotIn('sandboxSafe', descriptor)
                        self.assertNotIn('bindings.rs', '\n'.join(zipped.namelist()))
                        binary = info_name.removesuffix('Info.plist') + 'MacOS/' + info['CFBundleExecutable']
                        self.assertTrue(zipped.getinfo(binary).external_attr >> 16 & 0o111)
                        self.assertIn('Components/', zipped.read('README.txt').decode())

    def test_late_publication_failure_restores_previous_bytes_and_unselected_outputs(self):
        target = 'aarch64-apple-darwin'
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            libraries = base / 'target' / target / 'debug'
            libraries.mkdir(parents=True)
            for api in supported_apis(target):
                (libraries / f'libdaw_drive_{api}.dylib').write_bytes(api.encode())
            output = base / 'out'
            with patch('package_plugins.build_plugins'), patch('package_plugins.finalize_macos_bundle'), contextlib.redirect_stdout(io.StringIO()):
                package(target, base / 'target', output, units='drive')
                (output / 'unselected.zip').write_bytes(b'untouched')
                before = {p.name: p.read_bytes() for p in output.iterdir()}
                original = Path.replace
                calls = 0
                def fail_once(path, destination):
                    nonlocal calls
                    calls += 1
                    if calls == 3:
                        raise OSError('injected publication failure')
                    return original(path, destination)
                with patch.object(Path, 'replace', fail_once), self.assertRaises(OSError):
                    package(target, base / 'target', output, units='drive')
                self.assertEqual({p.name: p.read_bytes() for p in output.iterdir()}, before)
                # macOS CLAP/VST3 is a subset now and has its own checksum file.
                package(target, base / 'target', output, units='drive', api='clap,vst3')
                self.assertTrue(list(output.glob('*-clap-vst3-SHA256SUMS-debug.txt')))
                for name, payload in before.items():
                    if '-auv2' in name or name.endswith('-arm64-SHA256SUMS-debug.txt') or name == 'unselected.zip':
                        self.assertEqual((output / name).read_bytes(), payload)
