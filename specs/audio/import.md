# Audio import

`daw-media` owns source decoding. `decode_audio_cancellable` probes file content;
WAV uses the existing strict Hound header and Symphonia PCM path. MP3 and AAC-LC
use Symphonia 0.5.5 with the `mp3`, `aac` and `isomp4` features. Ogg Opus uses
the `ogg` reader and symphonia-adapter-libopus 0.2.9, registered beside the existing
codecs. opusic-sys 0.7.7 bundles libopus 1.6.1; CMake and a C compiler are build
requirements. No system libopus installation is required. AAC-LC supports
ADTS `.aac` and ISO MP4 `.m4a`/`.mp4` files. Do not enable other codecs through
container support. Reject unsupported codecs, non-finite samples, empty audio,
more than two channels, and sample-rate/channel changes inside a stream.
HE-AAC and encrypted audio are unsupported. MP4 imports choose the first MP3 or
AAC audio track and ignore other tracks.

Keep source metadata at its original rate and channel count. Compressed assets
store `sample_format` as `mp3`, `aac` or `opus` and `bits_per_sample` as zero, because the
bitstream has no PCM bit depth. Core manifest validation accepts exactly these
new combinations, with existing WAV combinations unchanged. No manifest version
change or reinterpretation of existing frame coordinates is needed.

Decode mono to duplicated stereo and preserve stereo channel order. Enable
Symphonia gapless packets for MP3/AAC; MP3 applies trimming in its decoder, while the adapter
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

`decode_audio_cached` uses verified content for worker-only reuse. It canonicalizes
the source path and reads a SHA-256 `SourceRevision` before looking up live PCM.
A live hit requires matching metadata revision and content receipt. Otherwise,
the persistent cache uses the encoded digest and processing rate in its key.
Cache domain `decoded-verified-v2` excludes earlier weak-cache entries.

On a persistent-cache miss, the builder stages encoded bytes in a private
immutable snapshot, verifies its receipt and decodes only that snapshot. Before
publication, it rechecks the original source for mutation. Persistent reuse also
rechecks the original source before publication; a cache hit does not require a
new snapshot decode. Failure or cancellation retains the existing session.
See [verified encoded input](../architecture/background-audio-preparation.md#verified-encoded-input--2026-11-02).

The legacy `decode_wav_cached` name aliases the generalized cache for existing
callers; explicit `decode_wav*` functions retain strict WAV-only behavior.
Project imports, reopen, processing-rate changes, Sampler and file-hover preparation
use the shared cache. Menu selection exposes WAV/MP3/AAC/M4A/MP4/OPUS/OGG. Ogg accepts Opus only. Publication,
placement, source references, failure rollback and overwrite behavior use the
existing session transactions. Playback and CLI export use the same prepared PCM.

Import hashing does not replace [collection verification](../projects/source-bundling-stage0.md#source-revisions-and-prepared-audio).
Collection must verify private staged bytes against the expected encoded receipt
and retained original project-rate PCM before publication. It must not assume
cached PCM still matches an externally mutable source path.

Symphonia source and MPL 2.0 notices are in `crates/media/assets/licenses`; all
native package scripts copy this directory. No runtime FFmpeg or system codec is
required. Synthetic fixture generation uses FFmpeg separately; tests consume
committed sine recordings. See [validation history](../validation/README.md).

## Ogg Opus timing and validation

Support one mono/stereo Ogg Opus logical stream with channel mapping
family 0. Reject Vorbis, multichannel/multistream mappings, chained/multiplexed
streams, incomplete pages, bad CRCs and discontinuous page sequences. Accept a
recorder file without an end-of-stream flag only when it ends at a checksum-valid
page with a completed packet and a usable granule position. Use that final
granule as the available-audio bound; loss of whole trailing pages cannot be
distinguished from intentional recording completion. `opus_import.rs` scans bounded page headers/lacing and checksums
on a worker; Symphonia retains packet assembly and libopus retains codec decoding.
The scan does not allocate from declared granules or duration.

Disable generic Ogg gapless packet trimming. Its inferred start delay does not
represent Opus pre-skip correctly for short final pages. Zero the pre-skip in the
adapter's private CodecParameters copy, then discard the original header pre-skip
across as many decoded packets as needed. Keep only final-granule minus stream
start timestamp minus pre-skip frames, and verify enough audio was decoded. Apply
the signed Q7.8 dB OpusHead output gain once. Do not apply optional ReplayGain tags.
Decode at 48 kHz regardless of the original-rate hint, retain 48 kHz in source
metadata, then reuse project-rate conversion, resident/disk admission and peaks.
Retain cancellation during page checks, packet decoding and resampling.

The original bitstream remains unchanged. Collection and Save As name verified
Ogg Opus sources with `.opus`, including `.ogg` or misleading input names.
The `opus`/zero-bit-depth metadata combination is accepted without a manifest
version change. Existing decoded-cache domains remain valid: earlier builds
could not produce an Opus decoded entry.

Codec/binding copyright and license texts are in `crates/media/assets/licenses`;
all desktop packagers already copy that directory. Keep the adapter MIT/Apache,
opusic-sys BSD and bundled Opus COPYING notices and exact source links together.
Import support adds no Opus export mode.
