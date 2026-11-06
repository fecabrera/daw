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

Opus fixtures use FFmpeg's libopus encoder. `tone-1.opus` and `tone-2.opus`
contain the same 0.25-second signals, decoded at the Opus clock of 48 kHz.
`short.opus` contains 144 frames using 2.5 ms packets; `long.opus` contains
60,000 frames across multiple Ogg audio pages. `surround.opus` checks rejection
of unsupported channels. `gain.opus` sets the stereo header gain to +6 dB;
`delayed.opus` increases its pre-skip by 1,920 frames. Header variants retain the
encoded packets and refresh the Ogg page CRC.

Regenerate only Opus fixtures with
`python3 crates/media/tests/fixtures/generate.py --opus-only`. Tests do not need
FFmpeg. Input-rate header hints do not change the 48 kHz Opus decoding clock.

`recorder.opus` is a synthetic 102,408-frame mono tone. It uses 120 ms packets,
a nonzero initial granule offset and no final EOS flag, matching a recorder
layout seen in WhatsApp files. It contains no user recording. Regenerate only
this fixture with `python3 crates/media/tests/fixtures/generate.py --recorder-only`.
