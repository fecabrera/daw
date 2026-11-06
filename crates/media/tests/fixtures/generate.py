"""Regenerate synthetic import fixtures. Requires ffmpeg; tests do not."""
import argparse
import pathlib
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("--opus-only", action="store_true")
parser.add_argument("--recorder-only", action="store_true")
args = parser.parse_args()
root = pathlib.Path(__file__).parent
def refresh_page_crc(data, start, end):
    data[start + 22:start + 26] = bytes(4)
    crc = 0
    for byte in data[start:end]:
        crc ^= byte << 24
        for _ in range(8):
            crc = ((crc << 1) ^ (0x04c11db7 if crc & 0x80000000 else 0)) & 0xffffffff
    data[start + 22:start + 26] = crc.to_bytes(4, 'little')


# Synthetic recorder layout: 120 ms packets, nonzero initial timestamps and no EOS.
subprocess.run([
    'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi',
    '-i', 'aevalsrc=0.25*sin(2*PI*440*t):s=48000:d=2.1335',
    '-c:a', 'libopus', '-frame_duration', '120', '-b:a', '24k',
    '-page_duration', '1680000', str(root / 'recorder.opus'),
], check=True)
data = bytearray((root / 'recorder.opus').read_bytes())
start = 0
while start < len(data):
    segments = data[start + 26]
    body = start + 27 + segments
    end = body + sum(data[start + 27:body])
    granule = int.from_bytes(data[start + 6:start + 14], 'little')
    if granule > 0:
        data[start + 6:start + 14] = (granule + 312).to_bytes(8, 'little')
    data[start + 5] &= ~4
    refresh_page_crc(data, start, end)
    start = end
(root / 'recorder.opus').write_bytes(data)
if args.recorder_only:
    raise SystemExit(0)

for channels in (1, 2):
    signal = '0.25*sin(2*PI*440*t)'
    if channels == 2:
        signal += '|0.125*sin(2*PI*880*t)'
    for extension, codec, extra in (
        ('mp3', 'libmp3lame', []),
        ('aac', 'aac', ['-f', 'adts']),
        ('m4a', 'aac', []),
        ('opus', 'libopus', []),
    ):
        if args.opus_only and extension != "opus":
            continue
        subprocess.run([
            'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y',
            '-f', 'lavfi', '-i', f'aevalsrc={signal}:s=44100:d=0.25',
            '-ac', str(channels), '-c:a', codec, '-b:a', '128k',
            *extra, str(root / f'tone-{channels}.{extension}'),
        ], check=True)

if not args.opus_only:
    subprocess.run([
        'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi',
        '-i', 'anullsrc=r=44100:cl=5.1', '-t', '0.1', '-c:a', 'aac',
        str(root / 'surround.m4a'),
    ], check=True)

# Low-delay encoder packets exercise pre-skip across multiple 2.5 ms packets.
subprocess.run([
    'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi',
    '-i', 'aevalsrc=0.25*sin(2*PI*440*t):s=48000:d=0.003',
    '-c:a', 'libopus', '-frame_duration', '2.5', '-b:a', '128k',
    str(root / 'short.opus'),
], check=True)
subprocess.run([
    'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi',
    '-i', 'anullsrc=r=48000:cl=5.1', '-t', '0.1', '-c:a', 'libopus',
    str(root / 'surround.opus'),
], check=True)
subprocess.run([
    'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi',
    '-i', 'aevalsrc=0.25*sin(2*PI*440*t):s=48000:d=1.25',
    '-c:a', 'libopus', '-b:a', '128k', str(root / 'long.opus'),
], check=True)

# Mutate only OpusHead fields and refresh the first Ogg page's CRC.
def header_variant(name, offset, value):
    data = bytearray((root / 'tone-2.opus').read_bytes())
    segments = data[26]
    body_start = 27 + segments
    data[body_start + offset:body_start + offset + 2] = value.to_bytes(2, 'little', signed=True)
    page_end = body_start + sum(data[27:body_start])
    refresh_page_crc(data, 0, page_end)
    (root / name).write_bytes(data)

header_variant('gain.opus', 16, 6 * 256)
header_variant('delayed.opus', 10, 312 + 1920)
