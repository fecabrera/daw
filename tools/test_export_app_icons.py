"""Check that both portable icon sets follow edits to the shared SVG."""
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
from unittest.mock import patch

import export_app_icons as exporter


@unittest.skipUnless(shutil.which('rsvg-convert') and
                     shutil.which('glib-compile-resources') and shutil.which('gresource'),
                     'librsvg and GLib tools required')
class SharedWaveformTests(unittest.TestCase):
    def test_source_edits_reach_windows_and_linux_without_a_raster_master(self):
        with tempfile.TemporaryDirectory(prefix='shared-waveform-') as temporary:
            assets = Path(temporary)
            source = assets / 'DAW.icon/Assets/Waveform.svg'
            source.parent.mkdir(parents=True)
            catalog = assets / 'Assets.car'
            catalog.write_bytes(b'macOS catalog must remain unchanged')
            shutil.copyfile(exporter.ASSETS / 'desktop-background.svg', assets / 'desktop-background.svg')
            previous = None
            for color in ('#ff9500', '#0088ff'):
                source.write_text(
                    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
                    f'<rect x="200" y="200" width="624" height="624" fill="{color}"/>'
                    '</svg>')
                with patch.object(exporter, 'ASSETS', assets), patch.object(exporter, 'WAVEFORM', source):
                    exporter.export_windows()
                    exporter.export_linux()
                svg = assets / 'hicolor/scalable/apps/org.daw.DAW.svg'
                composite = ET.parse(svg).getroot()
                nested = composite.find('{http://www.w3.org/2000/svg}svg')
                self.assertEqual(ET.tostring(nested), ET.tostring(ET.parse(source).getroot()))
                self.assertIsNotNone(composite.find('{http://www.w3.org/2000/svg}rect'))
                embedded = subprocess.check_output([
                    'gresource', 'extract', str(assets / 'icons.gresource'),
                    '/org/daw/DAW/icons/hicolor/scalable/apps/org.daw.DAW.svg'])
                self.assertEqual(embedded, svg.read_bytes())
                ico = (assets / 'DAW.ico').read_bytes()
                reserved, kind, count = struct.unpack_from('<HHH', ico)
                self.assertEqual((reserved, kind, count), (0, 1, 7))
                for index, size in enumerate(exporter.SIZES[:-1]):
                    entry = struct.unpack_from('<BBBBHHII', ico, 6 + 16 * index)
                    payload = ico[entry[7]:entry[7] + entry[6]]
                    png = (assets / f'hicolor/{size}x{size}/apps/org.daw.DAW.png').read_bytes()
                    self.assertEqual(payload, png)
                    self.assertEqual(struct.unpack_from('>II', png, 16), (size, size))
                window_png = (assets / 'windows.png').read_bytes()
                self.assertEqual(window_png,
                                 (assets / 'hicolor/256x256/apps/org.daw.DAW.png').read_bytes())
                if previous is not None:
                    self.assertNotEqual(ico, previous[0])
                    self.assertNotEqual(window_png, previous[1])
                previous = ico, window_png
                self.assertEqual(catalog.read_bytes(), b'macOS catalog must remain unchanged')


if __name__ == '__main__':
    unittest.main()
