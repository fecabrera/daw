use daw_core::{Id, Project, linear_gain};
use daw_media::AudioData;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RenderPlan {
    pub project: Project,
    pub audio: HashMap<Id, AudioData>,
}
impl RenderPlan {
    pub fn sample_at(&self, frame: u64) -> [f32; 2] {
        let mut mix = [0.0; 2];
        for t in &self.project.tracks {
            if !self.project.audible(t) {
                continue;
            }
            let gain = linear_gain(t.gain_db);
            if let Some(c) = t
                .clips
                .iter()
                .find(|c| frame >= c.start_frame && frame < c.end())
            {
                let Some(audio) = self.audio.get(&c.asset_id) else {
                    continue;
                };
                let local = frame - c.start_frame;
                let Some(source) = audio.samples.get((c.source_offset_frame + local) as usize)
                else {
                    continue;
                };
                let fade = clip_fade(local, c.length_frames);
                let coefficients = pan_coefficients(t.pan, audio.metadata.channels == 1);
                for ch in 0..2 {
                    mix[ch] += source[ch] * gain * fade * coefficients[ch];
                }
            }
        }
        let gain = linear_gain(self.project.master.gain_db);
        [mix[0] * gain, mix[1] * gain]
    }
    pub fn render(&self, start: u64, out: &mut [[f32; 2]]) {
        for (i, s) in out.iter_mut().enumerate() {
            *s = self.sample_at(start + i as u64);
        }
    }
}
pub fn clip_fade(position: u64, length: u64) -> f32 {
    let fade = 240.min(length / 2);
    if fade == 0 {
        return 0.0;
    }
    ((position as f32 / fade as f32).min((length - 1 - position) as f32 / fade as f32))
        .clamp(0.0, 1.0)
}
pub fn pan_coefficients(pan: f32, mono: bool) -> [f32; 2] {
    if mono {
        let theta = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        [theta.cos().max(0.0), theta.sin().max(0.0)]
    } else {
        [1.0 - pan.max(0.0), 1.0 + pan.min(0.0)]
    }
}

pub struct Renderer {
    pub plan: RenderPlan,
    pub playhead: u64,
    pub playing: bool,
    paused: bool,
    playback_start: Option<u64>,
    pub ramp: f32,
    gains: Vec<TrackGains>,
    master: f32,
    master_target: f32,
    master_remaining: u32,
    transition: u32,
    transition_from: [f32; 2],
    previous: [f32; 2],
}
struct TrackGains {
    id: Id,
    value: [f32; 4],
    target: [f32; 4],
    remaining: u32,
    peak: [f32; 2],
}
impl Renderer {
    pub fn new(plan: RenderPlan) -> Self {
        let gains = plan
            .project
            .tracks
            .iter()
            .map(|t| {
                let gate = if plan.project.audible(t) {
                    linear_gain(t.gain_db)
                } else {
                    0.0
                };
                let mono = pan_coefficients(t.pan, true);
                let stereo = pan_coefficients(t.pan, false);
                let target = [
                    mono[0] * gate,
                    mono[1] * gate,
                    stereo[0] * gate,
                    stereo[1] * gate,
                ];
                TrackGains {
                    id: t.id,
                    value: target,
                    target,
                    remaining: 0,
                    peak: [0.0; 2],
                }
            })
            .collect();
        let master = linear_gain(plan.project.master.gain_db);
        let playhead = plan.project.transport.playhead_frame;
        Self {
            plan,
            playhead,
            playing: false,
            paused: false,
            playback_start: None,
            ramp: 0.0,
            gains,
            master,
            master_target: master,
            master_remaining: 0,
            transition: 0,
            transition_from: [0.0; 2],
            previous: [0.0; 2],
        }
    }
    pub fn inherit(&mut self, old: &Self) {
        self.playhead = old.playhead;
        self.playing = old.playing;
        self.paused = old.paused;
        self.playback_start = old.playback_start;
        self.ramp = old.ramp;
        self.master = old.master;
        self.master_remaining = 240;
        self.previous = old.previous;
        self.transition_from = old.previous;
        self.transition = 240;
        for g in &mut self.gains {
            if let Some(o) = old.gains.iter().find(|o| o.id == g.id) {
                g.value = o.value;
            } else {
                g.value = [0.0; 4];
            }
            g.remaining = 240;
        }
    }
    pub fn seek(&mut self, frame: u64) {
        self.playhead = frame;
        self.transition_from = self.previous;
        self.transition = 240;
    }
    pub fn play(&mut self) {
        if self.plan.project.end() > 0 {
            if !self.playing && !self.paused {
                self.playback_start = Some(self.playhead);
            }
            self.paused = false;
            self.playing = true;
        }
    }
    pub fn pause(&mut self) {
        if self.playing {
            self.paused = true;
        }
        self.playing = false;
    }
    pub fn stop(&mut self) {
        self.playing = false;
        self.paused = false;
        if let Some(start) = self.playback_start.take() {
            self.seek(start);
        }
    }
    /// Reset once per output buffer, outside the per-sample loop.
    pub fn reset_track_peaks(&mut self) {
        for g in &mut self.gains {
            g.peak = [0.0; 2];
        }
    }
    /// Post-fader stereo peaks before master gain, in project track order.
    pub fn track_peaks(&self) -> impl Iterator<Item = [f32; 2]> + '_ {
        self.gains.iter().map(|g| g.peak)
    }
    pub fn next_sample(&mut self) -> [f32; 2] {
        let target = if self.playing { 1.0 } else { 0.0 };
        self.ramp = if self.ramp < target {
            (self.ramp + 1.0 / 240.0).min(target)
        } else {
            (self.ramp - 1.0 / 240.0).max(target)
        };
        let mut mix = [0.0; 2];
        if self.playing || self.ramp > 0.0 {
            let l = &self.plan.project.transport.r#loop;
            if self.playing
                && l.enabled
                && (self.playhead < l.start_frame || self.playhead >= l.end_frame)
            {
                self.playhead = l.start_frame;
                self.transition_from = self.previous;
                self.transition = 240;
            }
            if self.playing && !l.enabled && self.playhead >= self.plan.project.end() {
                self.playing = false;
            }
            for (t, g) in self.plan.project.tracks.iter().zip(&mut self.gains) {
                if g.remaining > 0 {
                    for ch in 0..4 {
                        g.value[ch] += (g.target[ch] - g.value[ch]) / g.remaining as f32;
                    }
                    g.remaining -= 1;
                }
                if let Some(c) = t
                    .clips
                    .iter()
                    .find(|c| self.playhead >= c.start_frame && self.playhead < c.end())
                {
                    let Some(audio) = self.plan.audio.get(&c.asset_id) else {
                        continue;
                    };
                    let local = self.playhead - c.start_frame;
                    if let Some(s) = audio.samples.get((c.source_offset_frame + local) as usize) {
                        let base = if audio.metadata.channels == 1 { 0 } else { 2 };
                        let fade = clip_fade(local, c.length_frames);
                        for ch in 0..2 {
                            let sample = s[ch] * g.value[base + ch] * fade;
                            mix[ch] += sample;
                            g.peak[ch] = g.peak[ch].max((sample * self.ramp).abs());
                        }
                    }
                }
            }
            if self.playing {
                self.playhead = self.playhead.saturating_add(1);
            }
        }
        if self.master_remaining > 0 {
            self.master += (self.master_target - self.master) / self.master_remaining as f32;
            self.master_remaining -= 1;
        }
        for x in &mut mix {
            *x *= self.master * self.ramp;
        }
        if self.transition > 0 {
            let progress = 1.0 - self.transition as f32 / 240.0;
            for (ch, value) in mix.iter_mut().enumerate() {
                *value = self.transition_from[ch] * (1.0 - progress) + *value * progress;
            }
            self.transition -= 1;
        }
        self.previous = mix;
        mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mono_power_and_stereo_balance() {
        let center = pan_coefficients(0.0, true);
        assert!((center[0] * center[0] + center[1] * center[1] - 1.0).abs() < 1e-6);
        assert_eq!(pan_coefficients(0.0, false), [1.0, 1.0]);
        assert_eq!(pan_coefficients(1.0, false), [0.0, 1.0]);
    }
    #[test]
    fn clip_fades_cover_short_clips() {
        assert_eq!(clip_fade(0, 1000), 0.0);
        assert_eq!(clip_fade(999, 1000), 0.0);
        assert_eq!(clip_fade(240, 1000), 1.0);
        assert_eq!(clip_fade(50, 100), 0.98);
    }
    fn plan() -> RenderPlan {
        use daw_core::{Asset, Clip, Source, SourceMetadata};
        use std::sync::Arc;
        let mut project = Project::default();
        project.add_track().unwrap();
        let id = Id::new_v4();
        let metadata = SourceMetadata {
            sample_rate_hz: 48_000,
            channels: 2,
            sample_format: "ieee_float".into(),
            bits_per_sample: 32,
        };
        project.assets.push(Asset {
            id,
            name: "Test".into(),
            source: Source {
                kind: "external".into(),
                path: "/test.wav".into(),
                path_kind: "absolute".into(),
            },
            source_metadata: metadata.clone(),
            decoded_frame_count: 2000,
        });
        project.tracks[0].clips.push(Clip {
            id: Id::new_v4(),
            asset_id: id,
            name: "Test".into(),
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 2000,
        });
        RenderPlan {
            project,
            audio: HashMap::from([(
                id,
                AudioData {
                    samples: Arc::new(vec![[0.25, -0.25]; 2000]),
                    metadata,
                    peaks: Arc::new(vec![]),
                },
            )]),
        }
    }
    #[test]
    fn transport_pauses_at_position_wraps_and_stops_at_end() {
        let mut renderer = Renderer::new(plan());
        renderer.play();
        for _ in 0..600 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 600);
        let expected = renderer.plan.sample_at(600);
        assert_eq!(renderer.next_sample(), expected);
        renderer.pause();
        for _ in 0..300 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 601);
        assert_eq!(renderer.next_sample(), [0.0; 2]);
        renderer.seek(900);
        renderer.play();
        renderer.plan.project.transport.r#loop = daw_core::Loop {
            enabled: true,
            start_frame: 500,
            end_frame: 1000,
        };
        for _ in 0..101 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 501);
        renderer.stop();
        for _ in 0..300 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 0);
        renderer.plan.project.transport.r#loop.enabled = false;
        renderer.seek(1999);
        renderer.play();
        renderer.next_sample();
        renderer.next_sample();
        assert!(!renderer.playing);
        assert_eq!(renderer.playhead, 2000);
        renderer.stop();
        assert_eq!(renderer.playhead, 1999);
        renderer.play();
        renderer.next_sample();
        renderer.next_sample();
        renderer.seek(700);
        renderer.play();
        renderer.next_sample();
        renderer.stop();
        assert_eq!(renderer.playhead, 700);
    }
    #[test]
    fn stop_returns_to_original_start_after_pause_seek_and_live_edit() {
        let mut renderer = Renderer::new(plan());
        renderer.seek(400);
        renderer.play();
        for _ in 0..260 {
            renderer.next_sample();
        }
        renderer.pause();
        for _ in 0..300 {
            renderer.next_sample();
        }
        assert_eq!(renderer.playhead, 660);
        assert_eq!(renderer.next_sample(), [0.0; 2]);
        renderer.play();
        renderer.seek(850);
        for _ in 0..20 {
            renderer.next_sample();
        }
        let mut next = Renderer::new(plan());
        next.inherit(&renderer);
        assert!(next.playing);
        next.pause();
        next.stop();
        assert!(!next.playing);
        assert_eq!(next.playhead, 400);
        for _ in 0..300 {
            next.next_sample();
        }
        assert_eq!(next.next_sample(), [0.0; 2]);

        next.seek(1200);
        next.play();
        for _ in 0..20 {
            next.next_sample();
        }
        next.stop();
        assert_eq!(next.playhead, 1200);
        next.seek(1500);
        next.stop();
        assert_eq!(next.playhead, 1500);
    }
    #[test]
    fn track_peaks_measure_separate_post_fader_channels_before_master() {
        let mut p = plan();
        let mut second = p.project.tracks[0].clone();
        second.id = Id::new_v4();
        second.pan = -1.0;
        p.project.tracks.push(second);
        p.project.tracks[0].gain_db = 20.0;
        p.project.tracks[0].pan = 0.5;
        p.project.master.gain_db = -20.0;
        let mut renderer = Renderer::new(p.clone());
        renderer.playing = true;
        for _ in 0..500 {
            renderer.next_sample();
        }
        assert_eq!(
            renderer.track_peaks().collect::<Vec<_>>(),
            vec![[1.25, 2.5], [0.25, 0.0]]
        );
        assert!(renderer.next_sample().iter().all(|v| v.abs() < 1.0));

        p.project.tracks[0].muted = true;
        let mut muted = Renderer::new(p.clone());
        muted.playing = true;
        for _ in 0..500 {
            muted.next_sample();
        }
        assert_eq!(
            muted.track_peaks().collect::<Vec<_>>(),
            vec![[0.0, 0.0], [0.25, 0.0]]
        );

        p.project.tracks[0].soloed = true;
        let mut solo = Renderer::new(p);
        solo.playing = true;
        for _ in 0..500 {
            solo.next_sample();
        }
        assert_eq!(
            solo.track_peaks().collect::<Vec<_>>(),
            vec![[1.25, 2.5], [0.0, 0.0]]
        );
        solo.playing = false;
        for _ in 0..300 {
            solo.next_sample();
        }
        solo.reset_track_peaks();
        solo.next_sample();
        assert!(solo.track_peaks().all(|peak| peak == [0.0; 2]));
    }
    #[test]
    fn live_controls_reach_offline_targets_after_five_ms() {
        let mut old = Renderer::new(plan());
        old.playing = true;
        for _ in 0..500 {
            old.next_sample();
        }
        let mut next_plan = old.plan.clone();
        next_plan.project.tracks[0].gain_db = -6.0;
        next_plan.project.tracks[0].pan = 0.5;
        next_plan.project.master.gain_db = -6.0;
        let mut next = Renderer::new(next_plan);
        next.inherit(&old);
        for _ in 0..240 {
            next.next_sample();
        }
        let target = next.plan.sample_at(next.playhead);
        assert_eq!(next.next_sample(), target);
        let previous = next.previous;
        next.seek(1000);
        assert_eq!(next.next_sample(), previous);
    }
}
