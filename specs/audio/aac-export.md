# AAC-LC export

`daw-media::ExportEncoder` owns the portable AAC export adapter in `aac_export.rs`.
Use the pinned, unmodified MIT-licensed `oxideav-aac` 0.1.7 streaming encoder and
locked `oxideav-core` 0.1.41. The adapter writes AAC-LC ADTS `.aac`; M4A/MP4
muxing and HE-AAC encoding are outside this scope. The dependency uses Rust and
requires no native codec runtime. Package its MIT/source notices from
`crates/media/assets/licenses` through the existing native packaging scripts.

`ExportSettings::aac_bitrate` retains an independent selection. `AacBitrate`
shares the existing 128/192/256/320 kbps choices, default 192. These are target
rates, not a strict constant byte rate; the codec varies frame sizes within its
bit budget. Label the format AAC, codec AAC-LC (ADTS), and bitrate mode Target.
Keep existing WAV and MP3 defaults and selections.

Encode stereo. Keep project rates of 44.1/48 kHz; convert higher rates to 48 kHz
with streaming Rubato `FftFixedIn` on the export worker. Remove resampler delay
and keep exactly the ceiling-rounded converted source frame count before codec
padding. Retain converter/codec state across renderer block writes. Input,
converted PCM and encoder blocks remain bounded; admit a working reservation
through the shared audio memory budget. Reject non-finite input, track clipping,
clamp normalized input and quantize to the encoder's signed 16-bit PCM interface.

The codec consumes 1024 frames per channel and flushes one overlap frame. ADTS
has no gapless sample-count metadata; output contains priming and final padding.
For nonempty input, decoded output length is
`(ceil(converted_frames / 1024) + 1) * 1024`. Do not promise exact round-trip clip
alignment or trim that padding implicitly on import. Keep this limit visible in
the user guide.

Desktop and CLI call the same session renderer and encoder. Reuse temporary-file
publication, source/manifest protection, overwrite confirmation, progress,
clipping warnings and failure cleanup. Progress stays below completion while
codec flush and file publication are pending. CLI infers AAC from `.aac` and
accepts `--format aac`; `--bitrate` chooses its target rate. Reject `--codec` for
AAC, incompatible known extensions and unsupported bitrates. AAC exports cannot
use `.m4a`/`.mp4` names. Both UI dialogs report the actual output sample rate.

Dependency evaluation used the published streaming API and license, then encoded
synthetic stereo tones and decoded the ADTS independently with FFmpeg. Regression
checks use the existing Symphonia decoder as an independent implementation and
cover all project rates, bitrate choices, deterministic bytes, arbitrary write
block sizes, tails, silence, clipping, invalid input and session publication.
Manual listening and native Windows/Linux validation remain separate checks.
