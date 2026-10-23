"""Regenerate synthetic import fixtures. Requires ffmpeg; tests do not."""
import pathlib
import subprocess

root = pathlib.Path(__file__).parent
for channels in (1, 2):
    signal = '0.25*sin(2*PI*440*t)'
    if channels == 2:
        signal += '|0.125*sin(2*PI*880*t)'
    for extension, codec, extra in (
        ('mp3', 'libmp3lame', []),
        ('aac', 'aac', ['-f', 'adts']),
        ('m4a', 'aac', []),
    ):
        subprocess.run([
            'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y',
            '-f', 'lavfi', '-i', f'aevalsrc={signal}:s=44100:d=0.25',
            '-ac', str(channels), '-c:a', codec, '-b:a', '128k',
            *extra, str(root / f'tone-{channels}.{extension}'),
        ], check=True)

subprocess.run([
    'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi',
    '-i', 'anullsrc=r=44100:cl=5.1', '-t', '0.1', '-c:a', 'aac',
    str(root / 'surround.m4a'),
], check=True)
