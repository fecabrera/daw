"""Export all platform icons from the shared waveform artwork.

macOS uses Xcode's asset compiler and sips. Windows/Linux use librsvg, plus
GLib for Linux. Ordinary builds use the committed exports without these tools.
"""
import argparse
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / 'crates/ui/assets/app-icon'
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
WAVEFORM = ASSETS / 'DAW.icon/Assets/Waveform.svg'


def resize(source, destination, size):
    destination.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(['sips', '-z', str(size), str(size), str(source),
                    '--out', str(destination)], check=True, capture_output=True)


def xcode_tools():
    candidates = []
    if os.environ.get('DEVELOPER_DIR'):
        candidates.append(Path(os.environ['DEVELOPER_DIR']))
    selected = subprocess.run(['xcode-select', '-p'], capture_output=True, text=True)
    if selected.returncode == 0:
        candidates.append(Path(selected.stdout.strip()))
    candidates.append(Path('/Applications/Xcode.app/Contents/Developer'))
    for developer in candidates:
        actool = developer / 'usr/bin/actool'
        ictool = developer.parent / 'Applications/Icon Composer.app/Contents/Executables/ictool'
        if actool.is_file() and ictool.is_file():
            return actool, ictool
    raise RuntimeError('Xcode with Icon Composer is required for macOS icon exports.')


def export_macos():
    actool, _ = xcode_tools()
    with tempfile.TemporaryDirectory(prefix='daw-native-icons-') as temporary:
        output = Path(temporary)
        subprocess.run([
            str(actool), str(ASSETS / 'DAW.icon'), '--compile', str(output),
            '--platform', 'macosx', '--minimum-deployment-target', '13.0',
            '--app-icon', 'DAW', '--output-partial-info-plist',
            str(output / 'icon-info.plist'), '--output-format', 'human-readable-text',
        ], check=True)
        # Decode Apple's padded legacy rendition, preserving its optical bounds
        # for direct Cargo launches. Bundles use the layered catalog instead.
        fallback = output / 'fallback.png'
        subprocess.run(['sips', '-s', 'format', 'png', str(output / 'DAW.icns'),
                        '--out', str(fallback)], check=True, capture_output=True)
        resize(fallback, output / 'macos.png', 512)
        for name in ('Assets.car', 'DAW.icns', 'macos.png'):
            shutil.copyfile(output / name, ASSETS / name)


def compose_desktop():
    background = ET.parse(ASSETS / 'desktop-background.svg').getroot()
    background.append(ET.parse(WAVEFORM).getroot())
    ET.register_namespace('', 'http://www.w3.org/2000/svg')
    return ET.tostring(background, encoding='utf-8', xml_declaration=True)


def render_desktop(source, destination, size):
    destination.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(['rsvg-convert', '--width', str(size), '--height', str(size),
                    '--output', str(destination), str(source)], check=True)


def export_windows():
    frames = []
    with tempfile.TemporaryDirectory(prefix='daw-windows-icons-') as temporary:
        source = Path(temporary) / 'icon.svg'
        source.write_bytes(compose_desktop())
        for size in SIZES:
            if size > 256:
                continue
            destination = Path(temporary) / f'{size}.png'
            render_desktop(source, destination, size)
            frames.append((size, destination.read_bytes()))
        shutil.copyfile(Path(temporary) / '256.png', ASSETS / 'windows.png')
    offset = 6 + len(frames) * 16
    entries = []
    for size, data in frames:
        entries.append(struct.pack('<BBBBHHII', size % 256, size % 256,
                                   0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (ASSETS / 'DAW.ico').write_bytes(
        struct.pack('<HHH', 0, 1, len(frames)) + b''.join(entries)
        + b''.join(data for _, data in frames))


def export_linux():
    # Both the installed scalable icon and all raster fallbacks come from the
    # same SVG imported by Icon Composer, over the portable graphite background.
    scalable = ASSETS / 'hicolor/scalable/apps/org.daw.DAW.svg'
    scalable.parent.mkdir(parents=True, exist_ok=True)
    scalable.write_bytes(compose_desktop())
    for size in SIZES:
        destination = ASSETS / f'hicolor/{size}x{size}/apps/org.daw.DAW.png'
        render_desktop(scalable, destination, size)
    files = '\n'.join(
        f'    <file>hicolor/{size}x{size}/apps/org.daw.DAW.png</file>' for size in SIZES)
    files += '\n    <file>hicolor/scalable/apps/org.daw.DAW.svg</file>'
    (ASSETS / 'icons.gresource.xml').write_text(
        '<gresources>\n  <gresource prefix="/org/daw/DAW/icons">\n'
        + files + '\n  </gresource>\n</gresources>\n')
    subprocess.run(['glib-compile-resources', str(ASSETS / 'icons.gresource.xml'),
                    '--sourcedir', str(ASSETS), '--target',
                    str(ASSETS / 'icons.gresource')], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=('all', 'macos', 'windows', 'linux', 'desktop'), default='all')
    args = parser.parse_args()
    if args.platform in ('all', 'macos'):
        export_macos()
    if args.platform in ('all', 'windows', 'desktop'):
        export_windows()
    if args.platform in ('all', 'linux', 'desktop'):
        export_linux()


if __name__ == '__main__':
    main()
