# Dotted command catalog

Place `--project FOLDER` and optional `--json` before the dotted command.
`S` is a selector in the form `id:UUID` or `name:EXACT`. `F` counts frames at
the project rate. Use `COMMAND --help` to see required arguments and choices.
For a repeated option, pass multiple values after the flag or repeat the flag.

### Files, projects, and plugin catalogs

| Dotted path | Options after path | Purpose |
| --- | --- | --- |
| `track.list` | `[--name NAME]` | List tracks. |
| `track.show` | `--track S` | Inspect one track. |
| `track.add` | `--type audio/midi/beat --name NAME` | Add an empty root track. |
| `track.rename` | `--track S --name NAME` | Rename a track. |
| `project.validate` | none | Validate the saved project and prepare audio. |
| `project.source.collect` | `[--dry-run]` | Collect original source files. |
| `project.export.mix` | `--output FILE [--format wav/mp3/aac] [--codec pcm16/pcm24/float32] [--bitrate N] [--overwrite] [--allow-missing-plugins]` | Export the full stereo mix. |
| `plugin.catalog.list` | `[--cache FILE]` | Read the saved plugin catalog. |
| `plugin.catalog.scan` | `[--mode changed/retry-failed/rescan/rebuild] [--cache FILE] [--scanner FILE]` | Refresh the local plugin catalog. |

### Project, tracks and routing

| Dotted path | Options after path | Purpose and effects |
| --- | --- | --- |
| `project.show` | none | Show saved identity, rate, buffer, timing and arrangement counts. |
| `project.create` | none | Use `--project` as an absent destination with existing parent. |
| `project.rename` | `--name NAME` | Change display name while retaining the folder. |
| `project.settings.show` | none | Show saved rate, sample format, buffer and stretch engine/revision. |
| `project.settings.set` | `[--sample-rate-hz HZ] [--buffer-frames N] [--sample-format FORMAT] [--stretch-engine KEY]` | Set project processing and sample format settings. |
| `project.source.list` | `[--storage external/project/embedded]` | List storage, provenance, source IDs, paths and owners, including unused assets. |
| `project.source.check` | none | Expose verified collection preflight as a source health view with owner diagnostics. |
| `track.duplicate` | `--track S ...` | Copy settings with fresh IDs and empty clips. |
| `track.delete` | `--track S ...` | Batch removal with shared effects on routing. |
| `track.move` | `--track S ... --parent S/master --index N` | Move ordered subtree roots into the Group or root using a zero-based index in the resulting sibling list. |
| `track.group.add` | `--name NAME [--parent S]` | Add an empty Group at root or in the selected Group. |
| `track.group.form` | `--name NAME --track S ...` | Form a Group from validated roots as one edit. |
| `track.group.dissolve` | `--track S` | Ungroup and retain children through shared rules. |
| `track.mix.set` | `--track S [--gain-db DB] [--pan N] [--muted BOOL] [--soloed BOOL]` | Update absolute saved mix settings with one validated transaction. |
| `track.color.set` | `--track S --color '#RRGGBB'` | Set saved color inherited by clips without an override. |
| `track.return.add` | `--name NAME` | Add a Return with shared defaults. |
| `track.return.move` | `--track S --before S/end` | Reorder the Return bank with shared validation. |
| `track.send.list` | `--track S` | List saved Return IDs, routing taps and levels. |
| `track.send.set` | `--track S --return S --gain-db DB --tap pre/post` | Create or update one send. |
| `track.send.remove` | `--track S --return S` | Remove the matching send without changing other sends. |
| `master.mix.show` | none | Report saved Master mix values. |
| `master.mix.set` | `--gain-db DB` | Update saved Master gain via shared validation. |

### Timing and saved transport options

| Dotted path | Options after path | Purpose and effects |
| --- | --- | --- |
| `timeline.timing.show` | none | Report origin BPM/signature and saved event IDs, frames and sections. |
| `timeline.tempo.set` | `--bpm N` | Change origin tempo through the shared timing edit. |
| `timeline.tempo.event.set` | `--at-frame F --bpm N [--id ID]` | Add or update one non-origin event through the section/timing transaction. |
| `timeline.tempo.event.remove` | `--id ID` | Delete a non-origin tempo event using shared timing validation. |
| `timeline.signature.set` | `--numerator N --denominator N` | Change origin signature using the shared display/grouping contract. |
| `timeline.signature.event.set` | `--at-frame F --numerator N --denominator N [--id ID]` | Add or update an event subject to shared signature-block rules. |
| `timeline.signature.event.remove` | `--id ID` | Remove a non-origin signature event subject to shared block validation. |
| `timeline.marker.list` | none | List sections and IDs with frame bounds. |
| `timeline.marker.add` | `--name NAME --at-frame F` | Add a section with the shared initial length and conductor rules. |
| `timeline.marker.set` | `--marker S [--name NAME] [--at-frame F] [--length-frames N]` | Edit one saved section and preserve event attachment rules. |
| `timeline.marker.remove` | `--marker S` | Remove the section through the shared conductor transaction. |
| `timeline.loop.show` | none | Show the saved exclusive range and enablement. |
| `timeline.loop.set` | `[--start-frame F --end-frame F] [--enabled BOOL]` | Set saved loop parameters. |

### Arrangement and audio

`--track` scopes clip resolution where supported. `--to-track` identifies a destination. Later complex placement leaves retain shared overlap and source rules and must return final placements and affected IDs.

| Dotted path | Options after path | Purpose and effects |
| --- | --- | --- |
| `clip.list` | `[--track S]` | List saved clips by track and timeline, with IDs, sources and frame ranges. |
| `clip.show` | `--clip S [--track S]` | Show saved source, arrangement, repeat, gain, color, channel and fade data as applicable. |
| `clip.rename` | `--clip S --name NAME [--track S]` | Rename a clip through the core edit, with no source rename. |
| `clip.delete` | `--clip S ...` | Delete selected clips in one transaction and retain saved assets and source files. |
| `clip.color.set` | `--clip S (--color '#RRGGBB'/--inherit)` | Set an override or restore track color inheritance. |
| `clip.mix.set` | `--clip S [--gain-db DB] [--muted BOOL]` | Set supported clip gain or mute with shared validation. |
| `clip.move` | `--clip S --at-frame F [--to-track S]` | Move while retaining identity. |
| `clip.copy` | `--clip S --at-frame F [--to-track S]` | Copy with fresh clip identity and shared source. |
| `clip.split` | `--clip S ... --at-frame F [--midi-note-mode split/preserve-first]` | Split clips containing an interior frame as one edit. |
| `clip.edge.trim` | `--clip S ... --edge left/right --delta-frames N` | Apply a shared trim delta, retaining source bounds and transactional overlap checks. |
| `clip.boundary.trim` | `--clip S --edge left/right --delta-frames N [--also-clip S ...]` | Transfer time across a touching audio boundary with paired limits. |
| `clip.repeat.resize` | `--clip S ... --edge left/right --delta-frames N` | Resize loop edges through the shared repeat and directional overlap rules. |
| `clip.stretch.resize` | `--clip S ... --edge left/right --length-frames N` | Use the first scoped clip as the length-ratio anchor and apply that ratio to the group. |
| `clip.range.delete` | `--track S ... --start-frame F --end-frame F` | Remove only content within the interval using shared retained clip slices. |
| `clip.range.move` | `--track S ... --start-frame F --end-frame F --at-frame F` | Move the full interval on the same tracks, preserving relative timing and source phases. |
| `clip.range.copy` | `--track S ... --start-frame F --end-frame F --at-frame F` | Copy interval content with fresh clip IDs and shared overwrite rules. |
| `clip.audio.import` | `--input FILE --at-frame F [--track S] [--placement reject/overwrite]` | Decode and convert at project rate. |
| `clip.audio.channel.set` | `--clip S [--left BOOL] [--right BOOL] [--swap BOOL] [--invert BOOL]` | Change playback channel flags and polarity on a saved audio clip. |
| `clip.audio.fade.set` | `--clip S --edge in/out --mode default/fade/crossfade [--length-frames N] [--midpoint N] [--position N]` | Set one edge using shared duration, neighbor and normalized curve bounds. |
| `clip.audio.transpose` | `--clip S --semitones N` | Prepare transposed audio using the saved processing backend and commit the validated metadata. |
| `clip.audio.normalize` | `--clip S ... --mode sample-peak/true-peak/integrated --target N [--no-increase]` | Measure dry arranged audio and batch gain changes. |
| `clip.audio.merge` | `--clip S ...` | Merge compatible clips via shared compound audio. |
| `clip.audio.transients.list` | `--clip S` | Report clip transients mapped from the prepared source to timeline frames. |
| `clip.bounce` | `--clip S ...` | Capture selected targets through shared routing and preserve originals under the Bounce contract. |
| `track.bounce` | `--track S ...` | Capture selected roots with correct sends, routing, latency and tails. |

### MIDI and Beat


| Dotted path | Options after path | Purpose and effects |
| --- | --- | --- |
| `midi.pattern.add` | `--track S --at-frame F` | Create a default empty pattern at the given position on a MIDI track. |
| `midi.pattern.show` | `--clip S` | Show pattern source, length, ticks and arrangement mapping. |
| `midi.pattern.resize` | `--clip S --start-tick N --end-tick N` | Crop/pad the immutable source through the shared resized-pattern transaction. |
| `midi.pattern.merge` | `--clip S ...` | Combine compatible MIDI clips through the shared merge and preparation paths. |
| `midi.pattern.note.list` | `--clip S` | List stable note IDs, local ticks, pitch and velocity. |
| `midi.pattern.note.set` | `--clip S --start-tick N --length-ticks N --pitch N --velocity N [--id ID]` | Add a note with fresh ID or update the specified note. |
| `midi.pattern.note.remove` | `--clip S --id ID ...` | Remove notes in one source-aware and preparation-aware transaction. |
| `midi.chord.list` | none | List shared catalog keys and note offsets. |
| `midi.chord.add` | `--track S --at-frame F --root MIDI_NOTE --quality KEY` | Place a chord through the shared catalog and pattern rules. |
| `midi.options.set` | `--track S [--channel KEY] [--monitor KEY]` | Update supported saved MIDI settings with documented choices. |
| `beat.pattern.add` | `--track S --at-frame F` | Add a default pattern for the selected Beat instrument. |
| `beat.pattern.show` | `--clip S` | Show grid, subtrack and step IDs, hidden steps and source-local time. |
| `beat.pattern.grid.set` | `--clip S --bars N --resolution quarter/eighth/sixteenth/thirty-second` | Change grid through core and retain hidden steps. |
| `beat.pattern.step.set` | `--clip S --subtrack S --at-tick N --velocity N` | Set one step at its validated source tick for a scoped subtrack. |
| `beat.pattern.step.remove` | `--clip S --subtrack S --at-tick N` | Clear the same scoped step. |
| `beat.pattern.merge` | `--clip S ...` | Use the shared Beat merge after validating pattern ownership and range. |
| `beat.subtrack.list` | `--track S` | List saved Beat subtrack IDs, instrument owners, pitch, mix and stop targets. |
| `beat.subtrack.add` | `--track S --name NAME` | Add one row with shared default instrument and stable ID. |
| `beat.subtrack.rename` | `--track S --subtrack S --name NAME` | Rename one row without replacing its identity. |
| `beat.subtrack.remove` | `--track S --subtrack S ...` | Remove named rows with shared stop and pattern ownership handling. |
| `beat.subtrack.move` | `--track S --subtrack S ... --index N` | Reorder the chosen rows at a zero-based survivor index. |
| `beat.subtrack.mix.set` | `--track S --subtrack S [--gain-db DB] [--pan N] [--muted BOOL] [--soloed BOOL]` | Update saved mix through the shared edit. |
| `beat.subtrack.pitch.set` | `--track S --subtrack S --pitch MIDI_NOTE` | Set trigger pitch and prepare affected sample playback. |
| `beat.subtrack.stop.set` | `--track S --subtrack S (--target S .../--clear)` | Replace same-instrument stop targets or clear them explicitly. |
| `beat.output.mix.set` | `--track S [--gain-db DB] [--pan N]` | Change supported Beat output-bus settings through the shared bus edit. |

### Instruments, effects, modulation and hosting


| Dotted path | Options after path | Purpose and effects |
| --- | --- | --- |
| `instrument.layer.list` | `--owner OWNER` | Report MIDI or Beat instrument layers, IDs and saved source kinds. |
| `instrument.layer.add` | `--owner OWNER --name NAME` | Create a regular layer using a shared constructor and owner validation. |
| `instrument.layer.rename` | `--owner OWNER --layer S --name NAME` | Rename one layer and preserve ID. |
| `instrument.layer.remove` | `--owner OWNER --layer S` | Remove one compatible layer with the shared source and routing cleanup. |
| `instrument.layer.move` | `--owner OWNER --layer S --before S/end` | Reorder regular layers while retaining Main and stable owners. |
| `instrument.source.set` | `--owner OWNER --layer S --type oscillator/noise --replace` | Replace one regular layer's source explicitly, enforcing exclusivity. |
| `instrument.source.sample.load` | `--owner OWNER --layer S --input FILE --type sampler/granulizer --replace` | Decode the external sample and prepare the replacement bank before committing. |
| `instrument.component.list` | `--owner OWNER --layer S` | Report installed components, IDs, ownership and supported keys. |
| `instrument.component.add` | `--owner OWNER --layer S --type KEY` | Use validated descriptors for supported oscillator strips/modulators and enforce source constraints. |
| `instrument.component.move` | `--owner OWNER --unit ID --to-layer S --before ID/end` | Move within a supported instrument using shared owner and route retargeting. |
| `instrument.component.copy` | `--owner OWNER --unit ID --to-layer S --before ID/end` | Copy a supported component and assign fresh identities through shared retargeting. |
| `instrument.component.remove` | `--owner OWNER --unit ID` | Remove a component and the routes it owns. |
| `effect.catalog.list` | none | List supported built-in kinds and stable keys from a shared catalog. |
| `effect.chain.list` | `--owner OWNER` | List installed IDs, chain order, power and kind. |
| `effect.chain.add` | `--owner OWNER --type KEY [--before ID]` | Install a supported built-in unit. |
| `effect.chain.remove` | `--owner OWNER --unit ID ...` | Remove named instances in one validated chain edit. |
| `effect.chain.move` | `--owner OWNER --unit ID --to-owner OWNER --before ID/end` | Perform a supported chain transfer while retaining identity. |
| `effect.chain.copy` | `--owner OWNER --unit ID --to-owner OWNER --before ID/end` | Transfer a copy with fresh identities under owner constraints. |
| `effect.power.set` | `--owner OWNER --unit ID --enabled BOOL` | Set saved power while preserving shared bypass and latency rules. |
| `unit.parameter.list` | `--owner OWNER --unit ID` | List editable built-in controls with saved values, types, and units. |
| `unit.parameter.set` | `--owner OWNER --unit ID --parameter KEY --value VALUE` | Apply one documented value through typed shared checks and stage dependent preparation. |
| `modulation.route.list` | `--owner OWNER` | Report route IDs, typed source/destination addresses and native amounts. |
| `modulation.route.set` | `--owner OWNER --source KEY --destination KEY --amount N [--id ID]` | Add or update a validated route with stable source/destination keys and destination-native units. |
| `modulation.route.remove` | `--owner OWNER --id ID` | Remove one route without changing unrelated controls. |
| `plugin.instance.list` | `[--owner OWNER]` | Inspect saved hosted instance identities and roles without activating them. |

### Copy, recovery, batch edits, and advanced processing

| Dotted path | Options after path | Purpose and effects |
| --- | --- | --- |
| `project.copy` | `--destination NEW_FOLDER` | Copy the saved project and relocate project-owned content. |
| `project.source.relink` | `--asset S --input FILE` | Relink a file-backed source to a byte-identical recording. |
| `project.export.range` | `--start-frame F --end-frame F --output FILE` | Export a bounded mix interval. |
| `project.export.stems` | `--track S ... --output-dir NEW_DIR` | Render selected routing targets as separate deliveries. |
| `clip.audio.stems.separate` | `--clip S --model-dir DIR` | Prepare generated stems from a selected audio clip. |
| `plugin.instance.add` | `--owner OWNER --plugin-id KEY` | Install a compatible hosted effect or source by scanner identity. |
| `plugin.instance.remove` | `--owner OWNER --unit ID` | Remove a hosted unit and its saved state references safely. |
| `plugin.instance.parameter.set` | `--owner OWNER --unit ID --parameter KEY --value VALUE` | Change a supported hosted parameter and capture coherent state. |
| `plugin.instance.sidechain.set` | `--owner OWNER --unit ID (--source-track S/--clear)` | Configure a supported hosted sidechain with validated routing. |
| `project.recovery.list` | none | List recoverable entries for the saved project ID. |
| `project.recovery.restore` | `--entry ID --destination NEW_FOLDER` | Restore the chosen entry to a new project folder with verified media. |
| `project.edits.apply` | `--input FILE` | Apply a validated set of supported typed operations in one save. |

## Saved owners and controls

- Instrument owners are `track:TRACK_UUID` or `subtrack:SUBTRACK_UUID`.
  Use `instrument.layer.list` to find a layer. Component listings show the
  stable IDs of oscillator strips, envelopes, LFOs and sample sources.
- Effect owners are `master`, `track:TRACK_UUID`, `subtrack:SUBTRACK_UUID`,
  or `layer:OWNER_UUID:LAYER_UUID` for a regular layer. Hosted instrument
  sources use `source:OWNER_UUID:LAYER_UUID`.
- Use `effect.catalog.list` and `midi.chord.list` for built-in keys.
  `midi.chord.add` uses the full MIDI note number for the root.
  `midi.options.set --channel` accepts `all` or `1` through `16`. Monitor
  `auto` follows the armed or selected tracks, `on` always monitors this
  track, and `off` disables listening while retaining recording eligibility.
- Use `unit.parameter.list` to find a built-in parameter key and its type.
  Child controls use dotted keys, with saved IDs for effect bands. Use the
  given JSON type for numbers and booleans. Use plain text for enum values.
  Each unit checks its numeric bounds and enum choices during the edit.
- Modulation sources use `LAYER_UUID:PROVIDER_UUID`. Destinations use
  `UNIT_UUID:PARAMETER[:TARGET_UUID]`, with the target ID for a band or other
  repeated control. Amounts use the destination's own units and bounds.
- Use the `key` reported by `daw-cli --json plugin.catalog.list` to install
  an available plugin. In JSON, CLAP keys start with `clap:` and VST3 keys with
  `vst3:`. Hosted parameter keys are the saved binding UUID or the native
  parameter number shown by `plugin.instance.list`.
  Parameter edits require restorable plugin state. Sidechain assignment requires
  exactly one compatible plugin input. `--clear` removes the saved input routes.

## Publication and media

- `project.copy` and `project.recovery.restore` require an absent destination
  with an existing parent and create a new project ID. Owned media moves into
  the copy's storage. External media remains referenced. Recovery lists the
  latest verified saved generation per inactive owner. Restore claims that
  generation while copying it and leaves it available for future recovery.
- Relink compares encoded source bytes before updating the reference. It
  requires the original file to be readable and the replacement to match it.
- Range export writes `[start, end)` after rendering the preceding audio to
  reconstruct processing state. It infers the same codec defaults as mix
  export and ends exactly at the requested frame.
- Stem export captures the selected tracks or groups at their post-fader
  outputs. Each stem starts at timeline frame zero and includes its bounded
  track tail. All files have the same duration and use stereo Float32 WAV at
  the project rate. Stems exclude downstream ancestor, Return, and Master
  processing. Files use track IDs as names. The destination directory must
  be absent. The CLI publishes it in one step after completing every file.
- Separation uses the installed backend and a local model directory. Supply
  at most one compatible ONNX and one compatible CoreML model. The command
  validates the model through the shared separation backend before processing.

## Edit batches

Use `project.edits.apply --input FILE` with a version 1 JSON document up to
1 MiB and up to 256 operations. Provide the same option tokens as the matching
single command. Operations run in file order, so later selectors can reference
an earlier rename. The CLI saves once after every operation succeeds.

```json
{
  "version": 1,
  "operations": [
    {"command": "track.rename", "arguments": ["--track", "name:Lead", "--name", "Lead Vocal"]},
    {"command": "track.mix.set", "arguments": ["--track", "name:Lead Vocal", "--gain-db", "-3"]}
  ]
}
```

Version 1 accepts `project.rename`, `track.rename`, `track.mix.set`,
`track.color.set`, `master.mix.set`, `clip.rename`, `clip.mix.set`,
`clip.color.set`, `clip.move`, `clip.copy`, `clip.delete`,
`clip.audio.channel.set`, `clip.audio.fade.set`, `timeline.loop.set`,
`beat.subtrack.rename`, `beat.subtrack.mix.set`, and `beat.output.mix.set`.
