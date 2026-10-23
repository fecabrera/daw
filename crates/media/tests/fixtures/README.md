# Synthetic compressed audio fixtures

These recordings contain generated sine waves only. They contain no imported recordings.
They are repository test data under the repository license.

`generate.py` uses FFmpeg to encode 0.25 seconds at 44.1 kHz as MP3 (LAME) and
AAC-LC (ADTS `.aac` and MP4 `.m4a`). Mono uses a 440 Hz tone at amplitude 0.25.
Stereo adds an 880 Hz right channel at amplitude 0.125. Tests read the committed
files and do not require FFmpeg. MP3 includes encoder delay/padding metadata;
AAC duration can include codec/container padding. A short silent 5.1 AAC file
checks rejection of unsupported channel counts.

Regenerate with `python3 crates/media/tests/fixtures/generate.py` from the repository root.
