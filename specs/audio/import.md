# Audio import

`daw-media` owns source decoding. `decode_audio_cancellable` probes file content;
WAV uses the existing strict Hound header and Symphonia PCM path. MP3 and AAC-LC
use Symphonia 0.5.5 with the `mp3`, `aac` and `isomp4` features. AAC-LC supports
ADTS `.aac` and ISO MP4 `.m4a`/`.mp4` files. Do not enable other codecs through
container support. Reject unsupported codecs, non-finite samples, empty audio,
more than two channels, and sample-rate/channel changes inside a stream.
HE-AAC and encrypted audio are unsupported. MP4 imports choose the first MP3 or
AAC audio track and ignore other tracks.

Keep source metadata at its original rate and channel count. Compressed assets
store `sample_format` as `mp3` or `aac` and `bits_per_sample` as zero, because the
bitstream has no PCM bit depth. Core manifest validation accepts exactly these
new combinations, with existing WAV combinations unchanged. No manifest version
change or reinterpretation of existing frame coordinates is needed.

Decode mono to duplicated stereo and preserve stereo channel order. Enable
Symphonia gapless packets; MP3 applies trimming in its decoder, while the adapter
applies AAC packet trims. AAC container/encoder padding can remain when no trim
information is available. Use actual decoded duration, not bitrate estimates, for
clip lengths and resampling. Reject premature EOF when exact MP3/MP4 packet end
bounds are present; ADTS duration is only an estimate. Preserve strict WAV length
checks.

Run decoding, Rubato conversion and peaks on workers. Do not allocate from a
compressed duration header. Admit PCM and peaks in bounded growth increments
through `AudioReservation`, including original PCM during resampling, resampler
scratch and a codec working allowance. Retain the shared 2 GiB budget and packet,
conversion and peak cancellation checks. Audio retained by history and renderers
continues to count against the shared budget.

`decode_audio_cached` retains the existing weak canonical-path/rate/revision cache.
The legacy `decode_wav_cached` name aliases the generalized cache for existing
callers; explicit `decode_wav*` functions retain strict WAV-only behavior.
Project imports, reopen, processing-rate changes, Sampler and file-hover preparation
use the shared cache. Menu selection exposes WAV/MP3/AAC/M4A/MP4. Publication,
placement, source references, failure rollback and overwrite behavior use the
existing session transactions. Playback and CLI export use the same prepared PCM.

The weak cache's length/mtime key is not a content receipt. The [source bundling
Stage 0 contract](../projects/source-bundling-stage0.md#source-revisions-and-prepared-audio)
defines worker-only SHA-256 receipts and uncached comparisons against original
project-rate PCM. Normal import/open does not hash files in this stage. Later
collection must verify its private staged file before publication; it must not
assume cached PCM still matches the current source path.

Symphonia source and MPL 2.0 notices are in `crates/media/assets/licenses`; all
native package scripts copy this directory. No runtime FFmpeg or system codec is
required. Synthetic fixture generation uses FFmpeg separately; tests consume
committed sine recordings. See [validation history](../validation/README.md).
