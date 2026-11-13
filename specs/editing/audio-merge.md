# Audio clip Merge

Audio Merge creates one visible clip over an immutable compound audio source.
It uses the existing Clip menu, context command and Command/Ctrl+J dispatch in
Timeline and Editor. MIDI and beat Merge retain their own construction rules.

## Transaction and identity

`core::Project::merge_targets` checks unique selected identities, one track,
non-overlapping ranges and intervening unselected clips. Audio members can have
different settings. Merge spans `[first.start_frame, last.end())`; gaps remain
silent. Keep the track and earliest clip IDs, name and color. Create a new asset
and private member IDs. Retain every original asset. An unchanged, complete,
neutral compound is a clean single-clip no-op.

`Asset.compound_audio` contains version 1 and ordered `Clip` members with local
integer-frame positions. Each member captures its source ID/window, repeat
base/phase, stretch, transpose, gain, channel operations, mute and fades. The
exclusive source kind is `compound_audio`, storage and path kind are `embedded`,
and the path is empty. Source metadata describes project-rate stereo float32.
The source kind is also the compatibility gate: readers that only support file,
MIDI and beat kinds reject the project rather than ignore the recipe. Legacy
assets omit the optional payload and retain their representation.

Validate positive bounded ranges, source availability in the model, unique
private IDs per recipe, ordered non-overlap, exact outer bounds, exclusive
payloads, supported version and the dependency DAG. Check cycles iteratively.
Nested merges reference existing assets. They do not copy recordings or expand
repetitions. Rate changes rescale member coordinates, repeat clocks, fades and
compound bounds with the shared checked frame mapping. Tempo changes retain the
audio source clock and move only arranged audio starts.

The wrapper starts enabled, at unity gain, default channels, zero transpose and
source offset, without repeat/stretch. `ClipFade.inherit_source` marks each outer
edge as delegated to the captured source. It defaults to false for legacy clips
and is invalid on file or pattern clips. Explicit fade edits clear delegation
on the changed edge. New trims, splits and retained-range cuts clear delegation
only on newly exposed edges; unchanged edges retain it.

## Prepared source evaluation

Core compiles neutral recipes into ordered source spans and envelope clocks.
The clocks preserve captured durations and local positions when spans are
cropped. Additional wrapper envelopes use their own clocks across all spans,
including gaps. Channel masks, swaps and polarity compose in source order.
Cropped non-repeating wrappers use an admitted unity source view so cuts retain
internal transitions from the complete immutable source clock. Source repeats
remain bounded descriptors. Non-neutral repeated compounds use
prepared nested source schedules instead of unfolding loop occurrences.

Engine `AudioTrack` owns the compiled spans, range indexes, readers, envelope
clocks and transitions. Playback does not resolve serialized asset references.
The primary member uses `min(ordinary envelope, repeat fade)`. Extended owners
retain the existing transition weights and repeat attenuation. Internal joins
run once; exterior transitions bind against the current unselected neighbor.
Original edge geometry supplies half-length, channel and source-handle caps.
Missing or muted participants retain the normal transition fallback.

The compound ends before track processing. Effects, track gain/pan, sends,
groups, Return and Master processing remain live. Per-frame mono/stereo/absent
source tags remain available, including gaps and transformed compounds. Source
fingerprints use prepared audible schedules rather than private member IDs or
presentation. A transparent Merge can inherit effect history without starting a
new output transition. Equivalent sources transfer their prepared read-ahead,
loop pins and staged PCM; the unused candidate retires with the old renderer.
Checkpoint compatibility still requires the complete
processing model and prepared owner identities to match.

Compounds containing rendered members carry `RenderedAudio` capability version 2.
Their Pre-send, raw-provider and Post-provider assets are companion compound
recipes on the same source clock, without their own routing capability. Each
recipe keeps member edits, gaps, boundary geometry and wrapper clocks; it selects
the corresponding captured leaf at each nested source. Pre recipes mark ordinary
members inactive so those spans keep the current track’s post-effect Pre stage.
Compiled source/layout activity follows crops, repeats and transformed wrapper
clocks without depending on sample energy or decoded availability. A member absent
from a historical provider keeps its geometry at silent gain. Provider identity unions also participate in
core feedback checks. Main-source extraction never mixes these auxiliary recipes.

Core constructs the companion recipes transactionally and admits their metadata
before cloning each recipe. Shared worker preparation discovers transforms in
both arranged clips and immutable members, including auxiliary compounds. Normal
source schedules handle repeats, crops, stretch, pitch and full-source baked seams.
The combined source DAG includes compound and routing dependencies; self-Post file
references remain allowed, but other cycles are rejected. Captured file leaves
may retain delegated envelopes inside companion members.

Manifest loading upgrades older compounds that contain rendered members before
publishing the editor model. Derived private IDs are deterministic; read-only
inspection and opening agree, and repeated upgrades add no assets. Saving retains
the companion recipes. Older readers reject version 2 rather than silently lose
the feeds. File-based Bounce sources retain capability version 1.

## Preparation, consumers and storage

Project preparation discovers transforms in visible clips, all recipe members
and Sampler inputs. It prepares dependencies in topological order. Initial
Merge compiles metadata and reuses original media; it allocates no PCM or peak
bins for elapsed gaps. Sparse waveform clocks reuse leaf source peaks and query
only visible ranges. Optional transient analysis uses a cancellable source
extraction worker and its existing bounded disposable cache.

`Session::render_clip_source` provides worker-only extraction for Normalize,
Sampler, separation and transforms without track processing. Whole-source
stretch/pitch and cropped source windows use admitted continuous PCM. It also prepares signed exterior
continuation, transforms that context and exposes the zero-based body plus its
mapped handles. Prepared stretch memory accounting includes the retained
context. Channel-layout indexes preserve source-time tags through the transform.
Generated stems clear delegated edge markers and use the normal file boundary
policy; their source input already contains captured member edits. Read errors fail extraction; they must not become a successful silent buffer.
Verified missing leaves remain referenced, warned and silent only where absent.
Undecoded leaves block Merge.

Source/transform preparation builds a candidate Session and publishes its maps
only after every stage succeeds. Desktop uses the existing atomic prepared-edit
job, selection publication and history path. Cancellation or failure leaves the
arrangement, selection, prepared owners and history unchanged. One success adds
one history entry. Renderer preparation and retirement stay outside the callback;
callback processing does not allocate, perform file I/O, wait or retire final
source owners.

Recipes persist in the manifest; compiled clocks and PCM are disposable. File
loading, path resolution, source copying and overwrite protection distinguish
file audio from embedded compounds. Retained leaf assets participate in the
existing current/pending/Undo/Redo/saved source inventory, including temporary
stem ownership. Save As and collection relocate their paths without changing
recipe references. Reopen prepares from the current original recordings and
retains existing changed/missing-source warnings.

## Admission and checks

The compound metadata operation limit is 128 MiB. Admission counts recipe
storage, retained/candidate waveform clocks and compiled schedules; compilation
also bounds expansion, source-reader nesting and layout metadata before callback
publication. Nested source/layout readers admit at most 64 levels. Fallible
reservations and checked frame conversions report failures. Preparation checks
cancellation between stages, at graph/compiler intervals and within 1,024-frame
source rendering chunks. Continuous extraction and transforms use the existing
2 GiB PCM/known-scratch admission, including retained context. These are explicit
preparation limits, not a process RSS guarantee. Oversized linearization fails
atomically; it does not prevent sparse Merge.

Automated acceptance covers source samples, external/internal fades, all six
rates, gaps and nested recipes, retained assets/schema/cycles, later source
operations, source collection/save/history, UI publication/failure and callback
allocation/state continuity. See the [validation record](../validation/audio-merge.md)
for executed commands and remaining native/device checks.
