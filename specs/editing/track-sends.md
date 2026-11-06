# Track sends and Return tracks

Status: accepted and implemented. The controls, routing limits and defaults below follow the agreed design. Native device and platform validation is recorded separately in [validation](../validation/README.md).

Related contracts: [track groups](track-groups.md), [undo and redo](undo-redo.md),
[modular units](../architecture/modular-audio-units.md), and
[background preparation](../architecture/background-audio-preparation.md).

## Purpose and scope

A send copies part of a track's signal to a shared stereo Return bus while the
track retains its normal output. Several tracks can share one reverb, distortion
chain or parallel compressor. The Return sums its inputs, processes its effect
chain, then feeds Master through its own pan/balance and fader.

Audio, MIDI, beat parent tracks and groups can send. Instrument layers and beat
subtracks retain their current internal routing. Master and Return tracks have
no outgoing sends in the first version. Returns are root-level buses feeding
Master; they cannot belong to a group, contain children, clips or an instrument.
Return-to-Return routing, feedback routing, sidechain inputs, independent send
pan and send modulation/automation are outside this version.

## Signal flow

```text
Track source → track effects → source pan/balance
                                  ├─ Pre send → send gain → Return
                                  └─ track fader
                                       ├─ normal-path mute/solo → parent group / Master
                                       └─ Post send → send gain → Return

All sends into a Return → aligned stereo sum → Return effects
    → Return pan/balance and fader → Master effects → Master fader
```

Both tap modes are after the sender's effects and follow its pan/balance. Pre
ignores the sender's fader. Post follows that fader and is the default. Both
tap modes bypass source mute but obey source Solo eligibility. Mute gates the
normal output after the send branches. Post-fader therefore means after the
fader, not after mute. Do not derive Pre by dividing the Post signal by its
gain: the fader can be at exact silence.

A child's send bypasses its parent group's effects, pan and fader. Send from
the group itself to share the processed group mix instead. A group fader at
silence does not silence child sends. Group mute affects only the group's normal
output; it does not suppress its own sends or descendant sends. Muting a child
still removes that child's normal contribution from the group input, so the
group's own send reflects that changed sum. The child's individual sends remain
active. Hierarchical source Solo eligibility applies to both group and child sends.

## Controls and defaults

New projects contain no Return tracks. Older projects without Returns load
without creating any. Create a Return only through an explicit Add Return or
New Return action. Every new Return is an empty bus; there are no Return presets.

| Control | Behavior |
| --- | --- |
| Destination | An existing Return, selected by stable identity; renaming and reordering do not redirect sends. |
| Level | Shared gain control, exact silence to +12 dB; new sends start at silence. Double-click resets to silence. |
| Tap | Pre-fader / Post-fader, default Post-fader. |
| Power | On by default; Off ramps the feed to zero and retains its level, tap and destination. |
| Remove | Deletes this send as one undoable edit. |

Allow one send from a source to each Return. A second connection to the same
Return is rejected rather than silently doubling the signal. Do not impose a
fixed send or Return count; prepare and account for the required memory outside
audio processing. A silent/off send remains part of the latency plan so power
and level changes cannot change alignment.

Returns reuse track naming, colors, selection, meters, gain/pan, Mute/Solo and
the existing effects lane. New Returns start empty at 0 dB, centered, unmuted and
unsoloed. Use distinct numbered default names such as Return 1; names are editable.
Selectors disambiguate identical names using the Return's displayed ordinal,
while routes continue to use stable IDs.

Add effects to a Return through the existing effects catalog. Effect insertion
keeps the unit's usual defaults; do not preinstall effects, rewrite an existing
chain or automatically force inserted effects to fully wet. Users set Dry/Wet
for their chosen parallel-processing setup.

## Mixer and track presentation

Always show the compact Sends rack above Pan in Mixer. There is no visibility
toggle. Let the send stack grow with its rows, without a fixed height, clipping
box or nested scrollbar. Align Pan, faders and meters using the tallest send
stack. If the complete controls exceed the window height, scroll Mixer vertically
as a whole. Preserve usable fader height. Opening Mixer does not dirty the project.

Each send is one compact horizontal row. Put a shared input group containing a
power toggle and Return dropdown on the left, and a shared header-size gain knob
on the right. Use equal canonical item-padding insets on both sides of the row.
Keep power, destination and knob positions fixed across names, like the Tracks
panel. Destination changes preserve send identity, gain, tap and power. Disable
Returns already used by another send on the source. The gain uses
its standard tooltip and numeric editing gestures, without a separate readout. Pre/Post appears only in the
context menu, without a label in the rack. The rack's Plus opens the standard
dropdown listing existing Returns and a New Return action. A theme-border divider
runs across the source strip between Plus and Pan, with 8 visible logical points
on each side. Plus has an 8-point gap above it after the final send row.
M and S retain their 6-point horizontal gap. Output controls use 8-point visible vertical gaps between the Pan
face, its numeric field, the gain/peak fields, the fader/meter rails and Mute/Solo.
Remove hidden row padding rather than adding it to these gaps. Align meter rails
with the fader, retain 8-point section padding inside the border, and align all
track, Return and Master controls.
The send context menu contains Enable/Disable, Pre/Post, Change Return and Remove. Reuse shared dropdowns, Small selector text,
knobs, power labels, colors, focus, keyboard and text-edit guards. Truncated names
have full-name tooltips. Selecting a destination is explicit; hovering or closing
a menu never changes routing.

Keep Returns in a pinned output section, separate from scrolling source tracks.
In Timeline, pin the Return rows immediately above Master at the bottom. Normal
track scrolling must not move either the Return section or Master. In Mixer,
pin the Return bank immediately to the left of the pinned Master strip; ordinary
track scrolling must not move either bank.

Returns reorder only within their bank. Pinning is presentation, not group
membership or a routing change. For more Returns than fit, give the Return bank
its own vertical scroll in Timeline and horizontal scroll in Mixer. Keep Master
visible outside that scroll area and reserve a usable viewport for ordinary
tracks. In Mixer, size the bank to the Return count times the standard strip
width, limited by the space before Master after reserving one source-strip width
when source tracks exist. Remove the fixed-width and percentage caps; horizontal
scrolling is only needed when this available space cannot fit all Returns. Mixer
bank width is automatic, with no resize handle. Timeline bank height remains
adjustable in workspace UI state without creating project history entries.
With no Returns, the Return area occupies no space; Master keeps its normal
pinned position.

Timeline and the shared Tracks browser show Returns as root-level bus rows with
a Return icon and no clip content. Use the existing invalid-drop behavior for
clips, samples, instruments and group membership. Selecting a Return targets its
effects chain. Shared Track/Plus menus include Add Return, which creates an empty
root Return. Group-context creation does not create a Return inside the group.

## Mute, Solo and tails

| Action | Audible result |
| --- | --- |
| Mute a source | Silence only its normal output. Its Pre and Post sends remain active, subject to send power/level and source Solo eligibility. |
| Mute a group | Silence only the group's normal output. Its own sends and descendant sends remain active. |
| Disable a send | Stop only that feed. The source's normal output stays active. |
| Mute a Return | Silence its output while continuing input and effect processing. |
| Solo a source/group | Keep only the solo-eligible source/group branches and their sends; suppress other sources' send feeds. Automatically include Returns receiving eligible sends. |
| Solo only a Return | Keep its send contributors running, including muted sources, silence their direct paths, and hear only the selected Return output. |
| Solo sources and Returns | Source solos restrict the contributor set; selected Return solos restrict the Return outputs. Retain the selected sources' direct paths. |

Keep the existing Solo-over-Mute precedence on the same owner, including a Return.
Use separate masks for normal-output audibility and send Solo eligibility; do
not reuse a combined mute/solo gate for send feeds. The send mask ignores source
and ancestor mute, but retains hierarchical Solo selection. With several sources
soloed, include the union of their eligible branches. Without source solos, every
enabled send can feed its destination regardless of source mute. Solo suppression
is temporary and does not change stored send power, level or connections.

Route-specific audibility is required: running a contributor to feed a soloed
Return must not also open its dry path. Returns do not participate in ordinary
root-track sibling solo exclusion; their automatic eligibility follows send paths.

Stopping a feed does not erase the Return's existing tail. Shared effect history
can still contain audio from previously audible contributors. Do not clear or
reconstruct a Return merely to implement a mute/solo toggle. Continue processing
through gaps, pause/stop/seek and loops under the existing effect lifecycle.
Export includes Return tails under the existing tail budget and truncation rules.

## Model, edits and history

Introduce a Return track kind and a source-owned send collection. Each send has
its own stable ID, Return ID, enabled state, tap enum and gain in the shared
exact-silence gain domain. Missing send fields in older projects default to an
empty collection. Validate source kinds, root-only Returns, finite bounds,
identity uniqueness, destination existence and duplicate source/destination pairs.
Invalid edits reject atomically with the original project and playback retained.

Core owns Add/Set/Remove Send and Add Return transactions. Creating an empty Return
and its first send through New Return is one transaction and one Undo step. Knob drags reuse existing
history grouping. Routing edits and gains persist through save/open and Undo/Redo;
menu opening and selection do not create edits.

Deleting a Return removes all incoming sends in the same transaction; Undo restores
the bus and complete routes with their original IDs/settings. Deleting a source
removes only its own sends. Duplicating a source/group copies outgoing settings
with fresh send IDs and keeps their Return targets. Duplicating a Return copies
its effects and mix controls but does not copy incoming sends. Existing
empty-track duplication and fresh effect/modulation identities still apply.
Return effects do not gain access to a sender's instrument-local modulators.

## Rendering and latency

The current renderer prepares a parent/group tree. Sends require a prepared
acyclic routing graph containing normal parent edges and send edges. Core validates
the complete graph; the first-version routing limits prevent feedback by
construction, and validation rejects a Return used as a source or parent, which prevents cycles.

Prepare topological order, dense source/Return indices, tap metadata, per-edge
alignment delays, stereo accumulation storage and capacity on workers. Process
each source and Return once, not once per send. Sum in deterministic project/send
order. Keep latency and effect input clocks coherent on both direct and Return
paths, including groups, pre/post taps and serial latency-bearing effects. A
single delay attached to the source track cannot align every outgoing path.
Preserve current mono pan/stereo balance laws and gain-control timing; validate
projects without sends against the existing renderer's samples and clocks.

Use the same graph and effect instances for playback, export, seek reconstruction
and serial/parallel cache preparation. Parallel workers may prepare independent
sources, but dependent Return sums/effects must run after all contributors.
Reconstruct Return history before publishing a structural routing change.
Place normal-path mute gates after the source send taps. Separate Solo feed
gates suppress non-solo contributors without silencing a muted source's sends.
Retain compatible DSP history for level/power edits and preserve Return identity
when changing one contributor. Fade feeds with the existing 5 ms mix ramps;
pre/post tap changes also require an audio transition rather than a sample jump.
Prepare changed alignment/topology outside audio and publish a complete plan
atomically. Failure retains the previous playable graph. The callback cannot
allocate, resolve IDs, wait, lock or retire final owners.

## Implementation and acceptance

Implement model/validation/history first, then shared rendering and latency,
then Mixer/track controls and empty Return creation. The accepted contract supersedes the earlier deferred-sends scope.

Acceptance covers multi-source summing, two Returns, nested groups, pre/post at
silent and moving faders, pan laws, duplicate/dangling/cyclic routes, Return-only
and combined Solo, source/group/Return mute, continuing tails, deletion/duplication,
save/reopen and Undo/Redo. At all six rates verify impulse alignment with unequal
effect latency, loop/seek/export parity, serial/parallel reconstruction, compatible
history and zero callback heap work. Verify live route edits, tap/power transitions,
group collapse and send-free project parity. Check pinned Return/Master geometry,
independent source/Return scrolling, overflow, narrow/short windows, menu bindings,
text/focus guards and native control gestures separately.
Verify that new/legacy projects create no Returns and reserve no empty bank
space. Test muted-source Pre/Post feeds, muted-group and muted-child distinctions,
single/multiple source Solo suppressing other sends, muted contributors during
Return-only Solo, and restoring feeds after clearing Solo without editing routes.

## Implementation ownership

`core::track_sends` owns typed sends, Return creation and separate normal/send audibility masks. Core transactions validate and preserve identities. `engine::send_routing` owns prepared dense edges, per-edge delays and 5 ms level/tap ramps. Source taps precede primary-path alignment; Return ingress uses the longest intrinsic sender path. Groups run child-first, then Returns, then Master. Cache workers retain raw pre-alignment samples as well as normal outputs. The UI module owns send racks and pinned banks; shared gain, power labels and dropdowns retain existing control behavior.

Timeline pinned boundaries use one full-width divider above the first Return and
above Master. Master retains this divider when no Returns exist. The pinned bank
owns these separators; Return rows do not add a second bottom stroke beside Master.
Paint selected Return header outlines in the final Timeline selection pass, after
the shared control-column border and pinned dividers, alongside regular tracks
and Master. Reuse `ControlPanel::selected_row` with the original header bounds
and Return viewport clip. Do not inset or shift the outline to avoid overpainting.

Mixer track, group, Return and Master stereo rails show independent per-channel
held maxima using the shared secondary-grey, one-point peak marker. Audio
publication latches finite positive amplitudes per channel without allocation or
locking. Silence retains holds. The numeric peak reads their maximum. Clicking
either rail, the gap between L/R rails, or the readout resets both holds and the
shared clipping warning without changing current readings or project history.

Timeline track, group, Return and Master headers reuse the Mixer held-marker and
inter-channel reset helper, including collapsed rows. They read the same stereo
holds as Mixer; resetting in either view clears both channel holds and clipping
without changing levels, routing, gain or project history.

Mixer has a standard selected Returns toggle at the transport toolbar's right
edge. It defaults to shown and lives in workspace UI state across view switches.
Hiding Returns gives their bank zero width; Master stays pinned and ordinary
tracks gain the space. Keep Return selection, effects targets, sends and DSP
active. The toggle creates no project/history edits and does not hide Timeline
Returns. When shown, retain automatic bank sizing and overflow scrolling.

Bottom-align each source's send controls within the shared rack height. Keep send
order and row spacing, with Plus at the same baseline for zero, one or many sends.
Retain the 8-point visible Plus-to-divider gap. Shorter stacks leave unused space
above their rows; the tallest stack still controls common rack height and global
Mixer overflow.
