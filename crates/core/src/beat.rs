//! Beat content and processing ownership. Preparation and presentation stay in their hosts.
//!
//! Project/Asset own the persisted payloads. Preparation compiles them into the shared
//! instrument and bus graph; no virtual processing tracks enter the saved project.
use crate::{Error, Id, InstrumentBusSettings, InstrumentSettings, MIDI_TICKS_PER_BEAT, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const BEAT_STATE_VERSION: u32 = 1;
const MAX_TICKS: u32 = 16 * 4 * MIDI_TICKS_PER_BEAT;
const MIN_STEP_TICKS: u32 = MIDI_TICKS_PER_BEAT / 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeatResolution {
    Quarter,
    Eighth,
    #[default]
    Sixteenth,
    ThirtySecond,
}
impl BeatResolution {
    pub const ALL: [Self; 4] = [
        Self::Quarter,
        Self::Eighth,
        Self::Sixteenth,
        Self::ThirtySecond,
    ];

    pub fn step_ticks(self) -> u32 {
        MIDI_TICKS_PER_BEAT
            / match self {
                Self::Quarter => 1,
                Self::Eighth => 2,
                Self::Sixteenth => 4,
                Self::ThirtySecond => 8,
            }
    }
}

/// An enabled hit. Ticks refer to the finest supported grid, independent of display order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatStep {
    pub id: Id,
    pub subtrack_id: Id,
    pub start_tick: u32,
    pub velocity: u8,
}

/// Immutable source content. Grid changes retain hidden hits without rescaling them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatPattern {
    pub version: u32,
    pub bars: u8,
    pub resolution: BeatResolution,
    pub steps: Vec<BeatStep>,
}
impl Default for BeatPattern {
    fn default() -> Self {
        Self {
            version: BEAT_STATE_VERSION,
            bars: 1,
            resolution: BeatResolution::default(),
            steps: Vec::new(),
        }
    }
}
impl BeatPattern {
    pub fn length_ticks(&self) -> u32 {
        u32::from(self.bars) * 4 * MIDI_TICKS_PER_BEAT
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != BEAT_STATE_VERSION {
            return Err(Error("Unsupported beat pattern version".into()));
        }
        if !(1..=16).contains(&self.bars) {
            return Err(Error("Beat pattern length must be 1–16 bars".into()));
        }
        let mut ids = HashSet::new();
        let mut positions = HashSet::new();
        for step in &self.steps {
            if step.id.is_nil()
                || step.subtrack_id.is_nil()
                || !ids.insert(step.id)
                || !positions.insert((step.subtrack_id, step.start_tick))
                || step.start_tick >= MAX_TICKS
                || !step.start_tick.is_multiple_of(MIN_STEP_TICKS)
                || !(1..=127).contains(&step.velocity)
            {
                return Err(Error("Invalid or duplicate beat step".into()));
            }
        }
        Ok(())
    }

    /// Off-grid, out-of-length and deleted-subtrack hits remain stored but are silent.
    pub fn visible_steps(&self) -> impl Iterator<Item = &BeatStep> {
        self.steps
            .iter()
            .filter(|step| self.contains_tick(step.start_tick))
    }

    fn contains_tick(&self, tick: u32) -> bool {
        tick < self.length_ticks() && tick.is_multiple_of(self.resolution.step_ticks())
    }

    pub fn with_grid(&self, bars: u8, resolution: BeatResolution) -> Result<Self> {
        self.validate()?;
        let next = Self {
            bars,
            resolution,
            ..self.clone()
        };
        next.validate()?;
        Ok(next)
    }

    /// None disables a hit. Existing hit identity survives a velocity adjustment.
    pub fn with_step(&self, subtrack_id: Id, tick: u32, velocity: Option<u8>) -> Result<Self> {
        self.validate()?;
        if subtrack_id.is_nil() || !self.contains_tick(tick) {
            return Err(Error("Beat step must be on the visible grid".into()));
        }
        let mut next = self.clone();
        let existing = next
            .steps
            .iter()
            .position(|s| s.subtrack_id == subtrack_id && s.start_tick == tick);
        match (existing, velocity) {
            (Some(index), Some(velocity)) => next.steps[index].velocity = velocity,
            (Some(index), None) => {
                next.steps.remove(index);
            }
            (None, Some(velocity)) => next.steps.push(BeatStep {
                id: Id::new_v4(),
                subtrack_id,
                start_tick: tick,
                velocity,
            }),
            (None, None) => {}
        }
        next.steps
            .sort_by_key(|s| (s.start_tick, s.subtrack_id, s.id));
        next.validate()?;
        Ok(next)
    }

    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.steps)
    }
}

/// Binds immutable content to its owning beat container. Names and row indices are not bindings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatPatternSource {
    pub id: Id,
    pub instrument_id: Id,
    pub pattern: BeatPattern,
}
impl BeatPatternSource {
    pub fn new(instrument_id: Id, pattern: BeatPattern) -> Result<Self> {
        let source = Self {
            id: Id::new_v4(),
            instrument_id,
            pattern,
        };
        source.validate()?;
        Ok(source)
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.is_nil() || self.instrument_id.is_nil() || self.id == self.instrument_id {
            return Err(Error("Invalid beat pattern source identity".into()));
        }
        self.pattern.validate()
    }

    /// A clip edit receives a new source; another clip's source remains unchanged.
    pub fn replaced(&self, pattern: BeatPattern) -> Result<Self> {
        self.validate()?;
        Self::new(self.instrument_id, pattern)
    }

    pub fn check_destination(&self, instrument_id: Id) -> Result<()> {
        self.validate()?;
        if self.instrument_id != instrument_id {
            return Err(Error(
                "Beat clips cannot be transferred between beat tracks".into(),
            ));
        }
        Ok(())
    }
}

/// One instrument, with its own layers and fixed LFO/Envelope lanes.
/// Main is the named group output and owns the only subtrack gain/pan controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BeatSubtrack {
    pub id: Id,
    pub name: String,
    pub trigger_pitch: u8,
    pub muted: bool,
    pub soloed: bool,
    pub instrument: InstrumentSettings,
    /// Directed trigger links, not modulation routes or audio sends.
    pub stops: Vec<Id>,
}
impl BeatSubtrack {
    pub fn new(name: String) -> Self {
        Self {
            id: Id::new_v4(),
            name,
            trigger_pitch: 60,
            muted: false,
            soloed: false,
            instrument: InstrumentSettings::default(),
            stops: Vec::new(),
        }
    }

    pub fn heap_bytes(&self) -> u64 {
        self.name.capacity() as u64
            + self.instrument.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.stops)
    }
}

/// A parent track's single source container. Output is fixed outside subtrack ordering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BeatInstrument {
    pub version: u32,
    pub id: Id,
    pub subtracks: Vec<BeatSubtrack>,
    pub output: InstrumentBusSettings,
}
impl Default for BeatInstrument {
    fn default() -> Self {
        Self {
            version: BEAT_STATE_VERSION,
            id: Id::new_v4(),
            subtracks: Vec::new(),
            output: InstrumentBusSettings::default(),
        }
    }
}

/// One prepared hit. The engine must still apply its existing frame mapping and 5 ms stop fade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeatTrigger {
    pub step_id: Id,
    pub subtrack_id: Id,
    pub start_tick: u32,
    pub length_ticks: u32,
    pub pitch: u8,
    pub velocity: u8,
    pub stops: Vec<Id>,
    pub suppressed: bool,
}

/// Structural edits never rewrite shared pattern content.
#[derive(Clone, Debug)]
pub enum BeatEdit {
    Add {
        subtrack: Box<BeatSubtrack>,
        index: usize,
    },
    Remove {
        ids: Vec<Id>,
    },
    /// Index is in the survivor list, after removing the selected rows.
    Reorder {
        ids: Vec<Id>,
        index: usize,
    },
    Rename {
        id: Id,
        name: String,
    },
    SetInstrument {
        id: Id,
        instrument: Box<InstrumentSettings>,
    },
    SetMix {
        id: Id,
        gain_db: f32,
        pan: f32,
        muted: bool,
        soloed: bool,
    },
    SetPitch {
        id: Id,
        pitch: u8,
    },
    SetStops {
        id: Id,
        targets: Vec<Id>,
    },
    SetOutput(Box<InstrumentBusSettings>),
    Batch(Vec<BeatEdit>),
}

impl BeatInstrument {
    pub fn validate(&self) -> Result<()> {
        if self.version != BEAT_STATE_VERSION {
            return Err(Error("Unsupported beat instrument version".into()));
        }
        let mut ids = HashSet::from([self.id]);
        if self.id.is_nil()
            || !self.output.valid()
            || !ids.insert(self.output.id)
            || !self.output.lfos.is_empty()
            || !self.output.envelopes.is_empty()
        {
            return Err(Error("Invalid beat output".into()));
        }
        // Scope remains per instrument. Never combine sibling provider lists.
        for row in &self.subtracks {
            if row.id.is_nil()
                || !ids.insert(row.id)
                || row.trigger_pitch > 127
                || row.name.trim().is_empty()
                || !row.instrument.valid()
            {
                return Err(Error("Invalid beat subtrack".into()));
            }
            for owner in row
                .instrument
                .layers
                .iter()
                .map(|l| l.id)
                .chain(row.instrument.buses().map(|b| b.id))
            {
                if owner.is_nil() || !ids.insert(owner) {
                    return Err(Error(
                        "Beat instrument owner identities must be unique".into(),
                    ));
                }
            }
            crate::instrument_validation::validate_routes(Some(&row.instrument), &[])?;
        }
        for sampler in self
            .subtracks
            .iter()
            .flat_map(|row| &row.instrument.layers)
            .filter_map(|layer| layer.sampler.as_ref())
        {
            if !ids.insert(sampler.id) {
                return Err(Error("Beat sampler identities must be unique".into()));
            }
        }
        for effect in self
            .output
            .effects
            .iter()
            .chain(self.subtracks.iter().flat_map(|row| {
                row.instrument
                    .layers
                    .iter()
                    .flat_map(|l| &l.effects)
                    .chain(row.instrument.buses().flat_map(|b| &b.effects))
            }))
        {
            if effect.id.is_nil() || !ids.insert(effect.id) {
                return Err(Error("Beat effect identities must be unique".into()));
            }
        }
        for route in self
            .output
            .effects
            .iter()
            .flat_map(|e| e.unit.modulations())
            .chain(&self.output.macro_module.modulations)
        {
            if route
                .source
                .is_some_and(|id| !self.output.modulation_sources().any(|s| s.id == id))
            {
                return Err(Error(
                    "Beat output effects cannot reference subtrack providers".into(),
                ));
            }
        }
        let output_destinations: Vec<_> = self
            .output
            .macro_module
            .controls
            .iter()
            .map(|control| {
                (
                    crate::ModulationSourceId {
                        layer: self.output.id,
                        unit: control.id,
                    },
                    self.output.macro_module.modulations.as_slice(),
                )
            })
            .collect();
        if daw_unit_core::modulation_dependency_order(&output_destinations).is_none() {
            return Err(Error(
                "Beat output modulation contains a dependency cycle".into(),
            ));
        }
        let row_ids: HashSet<_> = self.subtracks.iter().map(|row| row.id).collect();
        for row in &self.subtracks {
            let mut targets = HashSet::new();
            if row
                .stops
                .iter()
                .any(|id| *id == row.id || !targets.insert(*id) || !row_ids.contains(id))
            {
                return Err(Error(
                    "Beat stop links require distinct subtracks in the same instrument".into(),
                ));
            }
        }
        // Kahn's algorithm avoids recursive traversal of imported, unbounded row counts.
        let mut incoming: HashMap<_, usize> = self.subtracks.iter().map(|s| (s.id, 0)).collect();
        for target in self.subtracks.iter().flat_map(|s| &s.stops) {
            *incoming.get_mut(target).unwrap() += 1;
        }
        let rows: HashMap<_, _> = self.subtracks.iter().map(|s| (s.id, s)).collect();
        let mut ready: Vec<_> = incoming
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut visited = 0;
        while let Some(id) = ready.pop() {
            visited += 1;
            for target in &rows[&id].stops {
                let count = incoming.get_mut(target).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.push(*target);
                }
            }
        }
        if visited != self.subtracks.len() {
            return Err(Error("Beat stop links cannot contain cycles".into()));
        }
        Ok(())
    }

    /// Validate the final transaction before publishing any state.
    pub fn edit(&mut self, edit: BeatEdit) -> Result<()> {
        let mut next = self.clone();
        next.apply(edit)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Append an empty source instrument. Reuse the first available numbered name.
    pub fn add_subtrack(&mut self) -> Result<Id> {
        let mut number = 1;
        while self
            .subtracks
            .iter()
            .any(|s| s.name == format!("Subtrack {number}"))
        {
            number += 1;
        }
        let row = BeatSubtrack::new(format!("Subtrack {number}"));
        let id = row.id;
        self.edit(BeatEdit::Add {
            subtrack: Box::new(row),
            index: self.subtracks.len(),
        })?;
        Ok(id)
    }

    fn row_mut(&mut self, id: Id) -> Result<&mut BeatSubtrack> {
        self.subtracks
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| Error("Beat subtrack not found".into()))
    }

    fn checked_ids(&self, ids: &[Id]) -> Result<HashSet<Id>> {
        let selected: HashSet<_> = ids.iter().copied().collect();
        if selected.len() != ids.len()
            || ids
                .iter()
                .any(|id| !self.subtracks.iter().any(|s| s.id == *id))
        {
            return Err(Error(
                "Beat selection contains missing or duplicate subtracks".into(),
            ));
        }
        Ok(selected)
    }

    fn apply(&mut self, edit: BeatEdit) -> Result<()> {
        match edit {
            BeatEdit::Add { subtrack, index } => {
                if index > self.subtracks.len() {
                    return Err(Error("Beat insertion index is out of range".into()));
                }
                self.subtracks.insert(index, *subtrack);
            }
            BeatEdit::Remove { ids } => {
                let selected = self.checked_ids(&ids)?;
                self.subtracks.retain(|s| !selected.contains(&s.id));
                for row in &mut self.subtracks {
                    row.stops.retain(|id| !selected.contains(id));
                }
            }
            BeatEdit::Reorder { ids, index } => {
                let selected = self.checked_ids(&ids)?;
                let mut moving = Vec::new();
                let mut remaining = Vec::new();
                for row in self.subtracks.drain(..) {
                    if selected.contains(&row.id) {
                        moving.push(row);
                    } else {
                        remaining.push(row);
                    }
                }
                if index > remaining.len() {
                    return Err(Error("Beat insertion index is out of range".into()));
                }
                remaining.splice(index..index, moving);
                self.subtracks = remaining;
            }
            BeatEdit::Rename { id, name } => self.row_mut(id)?.name = name,
            BeatEdit::SetInstrument { id, instrument } => {
                self.row_mut(id)?.instrument = *instrument
            }
            BeatEdit::SetMix {
                id,
                gain_db,
                pan,
                muted,
                soloed,
            } => {
                let row = self.row_mut(id)?;
                row.instrument.main.gain_db = gain_db;
                row.instrument.main.pan = pan;
                row.muted = muted;
                row.soloed = soloed;
            }
            BeatEdit::SetPitch { id, pitch } => self.row_mut(id)?.trigger_pitch = pitch,
            BeatEdit::SetStops { id, targets } => self.row_mut(id)?.stops = targets,
            BeatEdit::SetOutput(output) => {
                if output.id != self.output.id {
                    return Err(Error("Beat output identity cannot change".into()));
                }
                self.output = *output;
            }
            BeatEdit::Batch(edits) => {
                for edit in edits {
                    self.apply(edit)?;
                }
            }
        }
        Ok(())
    }

    /// Control/worker-only schedule compilation. Never call this on an audio callback.
    /// All audible trigger intents issue stops before simultaneous hits are admitted.
    pub fn triggers(&self, source: &BeatPatternSource) -> Result<Vec<BeatTrigger>> {
        self.validate()?;
        source.check_destination(self.id)?;
        let soloed = self.subtracks.iter().any(|s| s.soloed);
        let rows: HashMap<_, _> = self
            .subtracks
            .iter()
            .filter(|s| !s.muted && (!soloed || s.soloed))
            .map(|s| (s.id, s))
            .collect();
        let mut triggers: Vec<_> = source
            .pattern
            .visible_steps()
            .filter_map(|step| {
                let row = rows.get(&step.subtrack_id)?;
                Some(BeatTrigger {
                    step_id: step.id,
                    subtrack_id: row.id,
                    start_tick: step.start_tick,
                    length_ticks: source.pattern.resolution.step_ticks(),
                    pitch: row.trigger_pitch,
                    velocity: step.velocity,
                    stops: row.stops.clone(),
                    suppressed: false,
                })
            })
            .collect();
        triggers.sort_by_key(|hit| (hit.start_tick, hit.subtrack_id, hit.step_id));
        let stopped: HashSet<_> = triggers
            .iter()
            .flat_map(|hit| hit.stops.iter().map(move |id| (hit.start_tick, *id)))
            .collect();
        for hit in &mut triggers {
            hit.suppressed = stopped.contains(&(hit.start_tick, hit.subtrack_id));
        }
        Ok(triggers)
    }

    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.subtracks)
            + self
                .subtracks
                .iter()
                .map(BeatSubtrack::heap_bytes)
                .sum::<u64>()
            + self.output.heap_bytes()
    }
}

#[cfg(test)]
mod tests;
