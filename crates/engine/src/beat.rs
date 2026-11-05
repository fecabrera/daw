//! Worker-side beat compilation into the existing instrument and bus renderer.
use super::RenderPlan;
use daw_core::{Asset, BeatInstrument, Clip, Id, MidiNote, MidiPattern, TrackKind};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default, PartialEq)]
pub struct BeatVoicePlan {
    /// Container duration also covers patterns with no subtracks.
    pub logical_end_frame: u64,
    /// Post-instrument clip gain, after the final Output effects.
    pub clip_gains: Vec<(u64, u64, f32)>,
    /// Stops keyed by the compiled MIDI asset. Coordinates remain source ticks.
    pub stops: HashMap<Id, Vec<u32>>,
}
struct SourceSchedule {
    asset: Asset,
    length_ticks: u32,
    notes: HashMap<Id, Vec<MidiNote>>,
    stops: HashMap<Id, Vec<u32>>,
}
fn fresh_derived(a: Id, b: Id, tag: u128, ids: &mut HashSet<Id>) -> Id {
    let mut bits = a.as_u128() ^ b.as_u128().rotate_left(37) ^ tag;
    loop {
        let id = Id::from_u128(bits);
        if !id.is_nil() && ids.insert(id) {
            return id;
        }
        bits = bits.wrapping_add(1);
    }
}
fn source_schedules(
    assets: &[Asset],
    clips: &[Clip],
    beat: &BeatInstrument,
) -> HashMap<Id, SourceSchedule> {
    let mut sources = HashMap::new();
    for clip in clips {
        if sources.contains_key(&clip.asset_id) {
            continue;
        }
        let Some(asset) = assets.iter().find(|a| a.id == clip.asset_id) else {
            continue;
        };
        let Some(source) = &asset.beat_pattern else {
            continue;
        };
        let Ok(hits) = beat.triggers(source) else {
            continue;
        };
        let mut notes: HashMap<_, Vec<_>> = HashMap::new();
        let mut stops: HashMap<_, Vec<_>> = HashMap::new();
        for hit in hits {
            if !hit.suppressed {
                notes.entry(hit.subtrack_id).or_default().push(MidiNote {
                    id: hit.step_id,
                    start_tick: hit.start_tick,
                    length_ticks: hit.length_ticks,
                    pitch: hit.pitch,
                    velocity: hit.velocity,
                });
            }
            for target in hit.stops {
                stops.entry(target).or_default().push(hit.start_tick);
            }
        }
        for ticks in stops.values_mut() {
            ticks.sort_unstable();
            ticks.dedup();
        }
        sources.insert(
            asset.id,
            SourceSchedule {
                asset: asset.clone(),
                length_ticks: source.pattern.length_ticks(),
                notes,
                stops,
            },
        );
    }
    sources
}
impl RenderPlan {
    /// Idempotent; only owned, prepared plans gain virtual tracks. Saved projects never do.
    pub fn prepare_beats(mut self) -> Self {
        if !self
            .project
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Beat)
        {
            return self;
        }
        let mut ids: HashSet<_> = self
            .project
            .assets
            .iter()
            .map(|a| a.id)
            .chain(self.project.tracks.iter().map(|t| t.id))
            .chain(
                self.project
                    .tracks
                    .iter()
                    .flat_map(|t| t.clips.iter().map(|c| c.id)),
            )
            .chain(self.project.all_effects().map(|e| e.id))
            .chain(
                self.project
                    .tracks
                    .iter()
                    .flat_map(|t| t.sends.iter().map(|s| s.id)),
            )
            .chain(
                self.project
                    .tracks
                    .iter()
                    .filter_map(|t| t.beat.as_ref())
                    .flat_map(|b| {
                        std::iter::once(b.output.id).chain(b.subtracks.iter().map(|s| s.id))
                    }),
            )
            .collect();
        let mut tracks = Vec::new();
        let originals = std::mem::take(&mut self.project.tracks);
        for mut parent in originals {
            let Some(beat) = parent.beat.take() else {
                tracks.push(parent);
                continue;
            };
            let clips = std::mem::take(&mut parent.clips);
            // Compile each shared source once, then partition by owner. Neither clips nor
            // subtracks multiply source validation and stop-graph traversal.
            let sources = source_schedules(&self.project.assets, &clips, &beat);
            parent.kind = TrackKind::Group;
            self.beat_tracks.insert(
                beat.output.id,
                BeatVoicePlan {
                    logical_end_frame: clips.iter().map(Clip::end).max().unwrap_or(0),
                    clip_gains: clips
                        .iter()
                        .filter(|c| !c.muted)
                        .map(|c| (c.start_frame, c.end(), daw_core::linear_gain(c.gain_db)))
                        .collect(),
                    ..Default::default()
                },
            );
            let mut output = parent.clone();
            output.id = beat.output.id;
            output.sends.clear();
            output.name = "Output".into();
            output.group_id = Some(parent.id);
            let mut graph = beat.output_settings();
            graph.main.effects.clear();
            graph
                .main
                .component_order
                .retain(|u| !matches!(u, daw_core::InstrumentComponent::Effect(_)));
            output.instrument = Some(graph);
            output.effects = beat.output.effects.clone();
            output.gain_db = beat.output.gain_db;
            output.pan = beat.output.pan;
            output.muted = false;
            output.soloed = false;
            tracks.push(parent.clone());
            tracks.push(output);
            let mut rows: Vec<_> = beat.subtracks.iter().collect();
            rows.sort_by_key(|r| r.id);
            for row in rows {
                let mut child = parent.clone();
                child.kind = TrackKind::Midi;
                child.id = row.id;
                child.sends.clear();
                child.name = row.name.clone();
                child.group_id = Some(beat.output.id);
                child.instrument = Some(row.instrument.clone());
                child.effects.clear();
                child.gain_db = 0.0;
                child.pan = 0.0;
                child.muted = false;
                child.soloed = false;
                let mut voice = BeatVoicePlan::default();
                let mut compiled = HashMap::new();
                for clip in &clips {
                    let Some(source) = sources.get(&clip.asset_id) else {
                        continue;
                    };
                    let asset_id = *compiled.entry(clip.asset_id).or_insert_with(|| {
                        let id = fresh_derived(source.asset.id, row.id, 0xb3a70001, &mut ids);
                        let mut asset = source.asset.clone();
                        asset.id = id;
                        asset.beat_pattern = None;
                        asset.midi_pattern = Some(MidiPattern {
                            length_ticks: source.length_ticks,
                            notes: source.notes.get(&row.id).cloned().unwrap_or_default(),
                        });
                        asset.source.kind = "midi_pattern".into();
                        asset.source_metadata.sample_format = "midi".into();
                        self.project.assets.push(asset);
                        voice
                            .stops
                            .insert(id, source.stops.get(&row.id).cloned().unwrap_or_default());
                        id
                    });
                    let mut c = clip.clone();
                    c.id = fresh_derived(clip.id, row.id, 0xb3a70002, &mut ids);
                    c.asset_id = asset_id;
                    c.gain_db = 0.0;
                    child.clips.push(c);
                }
                self.beat_tracks.insert(child.id, voice);
                tracks.push(child);
            }
        }
        self.project.tracks = tracks;
        self
    }
}
