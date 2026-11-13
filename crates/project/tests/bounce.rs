use daw_core::{BounceMode, BounceRequest, Edit, Id, SendTap, TrackSend};
use daw_project::{BounceError, BounceProgress, Session, SessionHistory};
use std::{fs, path::PathBuf, sync::atomic::Ordering};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-bounce-test-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn session(&self) -> Session {
        let path = self.0.join("source.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48_000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for i in 0..2400 {
            writer.write_sample((i as f32 * 0.07).sin() * 0.1).unwrap();
            writer.write_sample((i as f32 * 0.11).sin() * 0.05).unwrap();
        }
        writer.finalize().unwrap();
        let mut session = Session::default();
        session.import(&path, None, 1200).unwrap();
        session
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn render(s: &Session, frames: usize) -> Vec<[f32; 2]> {
    let mut r = daw_engine::Renderer::new(s.plan());
    let mut out = vec![[0.0; 2]; frames];
    r.render_offline(&mut out);
    assert!(r.source_error().is_none());
    out
}
fn assert_close(a: &[[f32; 2]], b: &[[f32; 2]]) {
    let error = a
        .iter()
        .zip(b)
        .flat_map(|(a, b)| [(a[0] - b[0]).abs(), (a[1] - b[1]).abs()])
        .fold(0.0, f32::max);
    assert!(error < 1e-5, "PCM difference {error}");
}
fn tracks(s: &Session) -> BounceRequest {
    BounceRequest {
        mode: BounceMode::Tracks,
        ids: vec![s.project.tracks[0].id],
    }
}
#[test]
fn whole_track_bakes_local_mix_preserves_gaps_and_undo_redo() {
    let fixture = Fixture::new();
    let mut s = fixture.session();
    s.project.tracks[0].gain_db = -9.0;
    s.project.tracks[0].pan = 0.6;
    s.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    let mut later = s.project.tracks[0].clips[0].clone();
    later.id = Id::new_v4();
    later.start_frame = 6000;
    s.project.tracks[0].clips.push(later);
    let expected = render(&s, 9000);
    let original = s.project.clone();
    let mut history = SessionHistory::new(&s, false).unwrap();
    let p = s.bounce(&tracks(&s), &BounceProgress::default()).unwrap();
    let mut next = p.session;
    assert_eq!(next.project.tracks.len(), 2);
    assert!(next.project.tracks[0].clips.iter().all(|c| c.muted));
    assert_eq!(next.project.tracks[1].clips[0].start_frame, 1200);
    assert!(next.project.tracks[1].effects.is_empty());
    assert_eq!(next.project.tracks[1].gain_db, 0.0);
    assert_close(&expected, &render(&next, 9000));
    history.record(&next).unwrap();
    let undo = history.undo(&next).unwrap();
    assert_eq!(undo.project, original);
    next = history.redo(&undo).unwrap();
    assert_close(&expected, &render(&next, 9000));
}
#[test]
fn pre_sends_survive_silent_fader_and_hard_pan_without_double_processing() {
    for tap in [SendTap::PreFader, SendTap::PostFader] {
        for gain in [-12.0, daw_core::SILENT_GAIN_DB] {
            let fixture = Fixture::new();
            let mut s = fixture.session();
            let source = s.project.tracks[0].id;
            let ret = s.project.add_return_track().unwrap();
            s.project.tracks[0].gain_db = gain;
            s.project.tracks[0].pan = 1.0;
            let mut send = TrackSend::new(ret);
            send.tap = tap;
            send.gain_db = -6.0;
            s.project.tracks[0].sends.push(send);
            s.project.tracks[1]
                .effects
                .push(daw_core::EffectInstance::drive());
            s.project
                .master
                .effects
                .push(daw_core::EffectInstance::drive());
            s.project.master.gain_db = -3.0;
            let expected = render(&s, 4000);
            let next = s
                .bounce(
                    &BounceRequest {
                        mode: BounceMode::Tracks,
                        ids: vec![source],
                    },
                    &BounceProgress::default(),
                )
                .unwrap()
                .session;
            assert_ne!(
                next.project.tracks[0].sends[0].id,
                next.project.tracks[1].sends[0].id
            );
            assert_close(&expected, &render(&next, 4000));
        }
    }
}
#[test]
fn cancel_and_unpublished_drop_leave_original_and_remove_generated_media() {
    let fixture = Fixture::new();
    let s = fixture.session();
    let original = s.project.clone();
    let progress = BounceProgress::default();
    progress.cancel.store(true, Ordering::Release);
    assert!(matches!(
        s.bounce(&tracks(&s), &progress),
        Err(BounceError::Cancelled)
    ));
    assert_eq!(s.project, original);
    let prepared = s.bounce(&tracks(&s), &BounceProgress::default()).unwrap();
    let path = PathBuf::from(&prepared.session.project.assets.last().unwrap().source.path);
    assert!(path.exists());
    drop(prepared);
    assert!(!path.exists());
    assert_eq!(s.project, original);
}
#[test]
fn save_as_preserves_history_only_main_and_routing_leaves() {
    let fixture = Fixture::new();
    let s = fixture.session();
    let mut history = SessionHistory::new(&s, false).unwrap();
    let bounced = s
        .bounce(&tracks(&s), &BounceProgress::default())
        .unwrap()
        .session;
    history.record(&bounced).unwrap();
    let mut undone = history.undo(&bounced).unwrap();
    let folder = fixture.0.join("saved");
    let receipt = undone
        .save_snapshot(Some(&history))
        .unwrap()
        .save(&folder)
        .unwrap();
    receipt.apply(&mut undone, Some(&mut history)).unwrap();
    drop(bounced);
    drop(s);
    let redone = history.redo(&undone).unwrap();
    redone.project.validate().unwrap();
    for asset in redone
        .project
        .assets
        .iter()
        .filter(|a| a.source.is_generated())
    {
        assert!(redone.source_path(asset).unwrap().is_file());
    }
    let mut saved = redone.clone();
    saved.save(&folder).unwrap();
    let reopened = Session::open(&folder).unwrap();
    assert_close(&render(&redone, 4000), &render(&reopened, 4000));
}
#[test]
fn atomic_core_rejects_incomplete_outputs_and_invalid_targets() {
    let fixture = Fixture::new();
    let s = fixture.session();
    let original = s.project.clone();
    let mut project = original.clone();
    assert!(
        project
            .edit(Edit::Bounce {
                request: tracks(&s),
                outputs: vec![]
            })
            .is_err()
    );
    assert_eq!(project, original);
    assert!(
        s.project
            .bounce_targets(&BounceRequest {
                mode: BounceMode::Clips,
                ids: vec![Id::new_v4()]
            })
            .is_err()
    );
}

fn sidechain(s: &mut Session, source: Id, destination: Id, tap: daw_unit_core::SidechainTap) {
    let mut effect = daw_core::EffectInstance::drive();
    let daw_core::EffectSettings::Drive(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.gain = 0.0;
    let mut route = daw_core::Modulation::new(daw_core::ModulationParameter::Volume);
    route.amount = -12.0;
    let mut input = daw_unit_core::AudioSidechain::new(source);
    input.tap = tap;
    input.attack_ms = 0.0;
    input.release_ms = 0.0;
    input.lower_db = -60.0;
    input.upper_db = -24.0;
    route.sidechain = Some(input);
    settings.modulations.push(route);
    s.project
        .tracks
        .iter_mut()
        .find(|t| t.id == destination)
        .unwrap()
        .effects
        .push(effect);
}
#[test]
fn sidechains_keep_external_context_and_aggregate_residual_before_the_follower() {
    for tap in [
        daw_unit_core::SidechainTap::PreFader,
        daw_unit_core::SidechainTap::PostFader,
    ] {
        for latency in [false, true] {
            let f = Fixture::new();
            let mut s = f.session();
            let source = s.project.tracks[0].id;
            let destination = s.project.add_track().unwrap();
            let mut clip = s.project.tracks[0].clips[0].clone();
            clip.id = Id::new_v4();
            s.project.tracks[1].clips.push(clip);
            s.project.tracks[0].gain_db = -6.0;
            s.project.tracks[0].pan = 1.0;
            if latency {
                s.project.tracks[0]
                    .effects
                    .push(daw_core::EffectInstance::limiter());
                s.project.tracks[1]
                    .effects
                    .push(daw_core::EffectInstance::saturator());
            }
            sidechain(&mut s, source, destination, tap);
            let expected = render(&s, 5000);
            let source_bounce = s
                .bounce(
                    &BounceRequest {
                        mode: BounceMode::Tracks,
                        ids: vec![source],
                    },
                    &BounceProgress::default(),
                )
                .unwrap()
                .session;
            assert_close(&expected, &render(&source_bounce, 5000));
            let destination_bounce = s
                .bounce(
                    &BounceRequest {
                        mode: BounceMode::Tracks,
                        ids: vec![destination],
                    },
                    &BounceProgress::default(),
                )
                .unwrap()
                .session;
            assert_close(&expected, &render(&destination_bounce, 5000));
            let mut muted = source_bounce.clone();
            muted.project.tracks[1].muted = true;
            let mut original_muted = s.clone();
            original_muted.project.tracks[0].muted = true;
            assert_close(&render(&muted, 5000), &render(&original_muted, 5000));
        }
    }
}
#[test]
fn rebounce_flattens_old_provider_lineage_and_copy_duplicates_contributions() {
    let f = Fixture::new();
    let mut s = f.session();
    let source = s.project.tracks[0].id;
    let destination = s.project.add_track().unwrap();
    let mut clip = s.project.tracks[0].clips[0].clone();
    clip.id = Id::new_v4();
    s.project.tracks[1].clips.push(clip);
    sidechain(
        &mut s,
        source,
        destination,
        daw_unit_core::SidechainTap::PreFader,
    );
    let expected = render(&s, 5000);
    let once = s
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![source],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    let output = once.project.tracks[1].id;
    let twice = once
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![output],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    assert_close(&expected, &render(&twice, 5000));
    let asset = twice
        .project
        .audio_asset(twice.project.tracks[2].clips[0].asset_id)
        .unwrap();
    assert_eq!(asset.rendered_audio.as_ref().unwrap().feeds.len(), 2);
    let mut deleted = twice.clone();
    deleted.project.edit(Edit::DeleteTrack(source)).unwrap();
    deleted.project.validate().unwrap();
}
#[test]
fn midi_and_beat_parent_bounce_render_the_source_container_once() {
    for beat in [false, true] {
        let mut s = Session::default();
        let track = if beat {
            s.project.add_beat_track().unwrap()
        } else {
            s.project.add_midi_track().unwrap()
        };
        if beat {
            let mut instrument = s.project.beat(track).unwrap().clone();
            let row = instrument.add_subtrack().unwrap();
            instrument.subtracks.last_mut().unwrap().instrument =
                daw_core::OscillatorSettings::default().into();
            s.project
                .edit(Edit::Beat {
                    track_id: track,
                    edit: daw_core::BeatEdit::Add {
                        subtrack: Box::new(instrument.subtracks.pop().unwrap()),
                        index: 0,
                    },
                })
                .unwrap();
            let clip = s.project.add_beat_pattern(track, 1200).unwrap();
            let pattern = daw_core::BeatPattern::default()
                .with_step(row, 0, Some(100))
                .unwrap();
            s.project
                .edit(Edit::ReplaceBeatPattern {
                    clip_id: clip,
                    pattern,
                })
                .unwrap();
        } else {
            s.project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
            let clip = s.project.add_midi_pattern(track, 1200).unwrap();
            let asset = s.project.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == clip)
                .unwrap()
                .asset_id;
            s.project
                .assets
                .iter_mut()
                .find(|a| a.id == asset)
                .unwrap()
                .midi_pattern
                .as_mut()
                .unwrap()
                .notes
                .push(daw_core::MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 240,
                    pitch: 60,
                    velocity: 100,
                });
        }
        s.project.tracks[0].gain_db = -6.0;
        s.project.tracks[0].pan = -0.4;
        s.prepare_stretches().unwrap();
        let frames = s.project.end() as usize + 1000;
        let expected = render(&s, frames);
        assert!(expected.iter().flatten().any(|v| v.abs() > 1e-4));
        let next = s
            .bounce(&tracks(&s), &BounceProgress::default())
            .unwrap()
            .session;
        assert_close(&expected, &render(&next, frames));
        assert_eq!(next.project.tracks[1].kind, daw_core::TrackKind::Audio);
        assert!(next.project.tracks[0].clips[0].muted);
    }
}

#[test]
fn multi_track_clip_bounce_keeps_residual_clips_order_groups_and_local_monitoring() {
    let f = Fixture::new();
    let mut s = f.session();
    let a = s.project.tracks[0].id;
    let mut unselected = s.project.tracks[0].clips[0].clone();
    unselected.id = Id::new_v4();
    unselected.start_frame = 6000;
    s.project.tracks[0].clips.push(unselected.clone());
    let b = s.project.add_track().unwrap();
    let mut clip = s.project.tracks[0].clips[0].clone();
    clip.id = Id::new_v4();
    clip.start_frame = 2400;
    s.project.tracks[1].clips.push(clip.clone());
    let group = Id::new_v4();
    s.project
        .edit(Edit::GroupTracks {
            track_ids: vec![a, b],
            group_id: group,
            name: "Group".into(),
        })
        .unwrap();
    s.project
        .tracks
        .iter_mut()
        .find(|t| t.id == group)
        .unwrap()
        .effects
        .push(daw_core::EffectInstance::drive());
    s.project
        .tracks
        .iter_mut()
        .find(|t| t.id == group)
        .unwrap()
        .gain_db = -6.0;
    s.project
        .tracks
        .iter_mut()
        .find(|t| t.id == b)
        .unwrap()
        .muted = true;
    let first = s.project.tracks.iter().find(|t| t.id == a).unwrap().clips[0].id;
    let expected = render(&s, 9000);
    let prepared = s
        .bounce(
            &BounceRequest {
                mode: BounceMode::Clips,
                ids: vec![clip.id, first],
            },
            &BounceProgress::default(),
        )
        .unwrap();
    assert!(
        prepared
            .warnings
            .iter()
            .any(|w| w.contains("remaining clips"))
    );
    let next = prepared.session;
    assert_eq!(next.project.tracks.len(), 5);
    let index = next.project.tracks.iter().position(|t| t.id == a).unwrap();
    assert_eq!(next.project.tracks[index + 1].group_id, Some(group));
    assert!(
        !next.project.tracks[index]
            .clips
            .iter()
            .find(|c| c.id == unselected.id)
            .unwrap()
            .muted
    );
    let index = next.project.tracks.iter().position(|t| t.id == b).unwrap();
    assert!(next.project.tracks[index + 1].muted);
    assert_close(&expected, &render(&next, 9000));
}
#[test]
fn disabled_only_tracks_keep_extent_and_generate_silence_with_a_notice() {
    let f = Fixture::new();
    let mut s = f.session();
    s.project.tracks[0].clips[0].muted = true;
    let prepared = s.bounce(&tracks(&s), &BounceProgress::default()).unwrap();
    assert!(
        prepared
            .warnings
            .iter()
            .any(|w| w.contains("silent output"))
    );
    let c = &prepared.session.project.tracks[1].clips[0];
    assert_eq!((c.start_frame, c.length_frames), (1200, 2400));
    assert!(
        render(&prepared.session, 5000)
            .iter()
            .flatten()
            .all(|v| *v == 0.0)
    );
}
#[test]
fn routing_leaf_clocks_follow_clip_gain_channels_position_and_repetition() {
    let f = Fixture::new();
    let mut original = f.session();
    let source = original.project.tracks[0].id;
    let ret = original.project.add_return_track().unwrap();
    let mut send = TrackSend::new(ret);
    send.tap = SendTap::PreFader;
    send.gain_db = 0.0;
    original.project.tracks[0].sends.push(send);
    original.project.tracks[0].muted = true;
    original.project.tracks[0].gain_db = daw_core::SILENT_GAIN_DB;
    let mut next = original
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![source],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    for (s, index) in [(&mut original, 0), (&mut next, 1)] {
        let c = &mut s.project.tracks[index].clips[0];
        c.start_frame = 3000;
        c.gain_db = -9.0;
        c.channels.swapped = true;
        c.channels.inverted = true;
        c.repeat = Some(daw_core::ClipLoop {
            length_frames: 2400,
            phase_frame: 0,
        });
        c.length_frames = 4800;
    }
    original.project.validate().unwrap();
    next.project.validate().unwrap();
    assert_close(&render(&original, 8500), &render(&next, 8500));
}
#[test]
fn alias_cycles_and_malformed_rendered_capabilities_are_rejected_atomically() {
    let f = Fixture::new();
    let mut s = f.session();
    let source = s.project.tracks[0].id;
    let destination = s.project.add_track().unwrap();
    sidechain(
        &mut s,
        source,
        destination,
        daw_unit_core::SidechainTap::PreFader,
    );
    let mut next = s
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![source],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    let output = next.project.tracks[1].id;
    assert!(!next.project.sidechain_source_available(output, destination));
    let asset = next.project.tracks[1].clips[0].asset_id;
    let index = next
        .project
        .assets
        .iter()
        .position(|a| a.id == asset)
        .unwrap();
    let before = next.project.clone();
    next.project.assets[index]
        .rendered_audio
        .as_mut()
        .unwrap()
        .version = 99;
    assert!(next.project.validate().is_err());
    next.project = before;
    next.project.assets[index].source.kind = "generated".into();
    assert!(next.project.validate().is_err());
}

#[test]
fn clip_source_edits_and_latency_are_baked_once() {
    let f = Fixture::new();
    let mut s = f.session();
    let c = &mut s.project.tracks[0].clips[0];
    c.stretch = Some(daw_core::ClipStretch::new(3, 2).unwrap());
    c.transpose = 5;
    c.source_offset_frame = 100;
    c.repeat = Some(daw_core::ClipLoop {
        length_frames: 1200,
        phase_frame: 100,
    });
    c.length_frames = 4000;
    c.gain_db = -3.0;
    c.channels.right = false;
    c.channels.inverted = true;
    c.fades.fade_in.length_frames = 150;
    c.fades.fade_out.length_frames = 200;
    s.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    s.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::limiter());
    s.project.validate().unwrap();
    s.prepare_stretches().unwrap();
    let expected = render(&s, 7000);
    let next = s
        .bounce(&tracks(&s), &BounceProgress::default())
        .unwrap()
        .session;
    assert_close(&expected, &render(&next, 7000));
}
#[test]
fn rebounce_keeps_prior_post_provider_current_fader_and_pan() {
    let f = Fixture::new();
    let mut s = f.session();
    let source = s.project.tracks[0].id;
    let destination = s.project.add_track().unwrap();
    let mut clip = s.project.tracks[0].clips[0].clone();
    clip.id = Id::new_v4();
    s.project.tracks[1].clips.push(clip);
    sidechain(
        &mut s,
        source,
        destination,
        daw_unit_core::SidechainTap::PostFader,
    );
    let mut once = s
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![source],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    once.project.tracks[1].gain_db = -9.0;
    once.project.tracks[1].pan = 0.75;
    let expected = render(&once, 5000);
    let output = once.project.tracks[1].id;
    let twice = once
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![output],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    assert_close(&expected, &render(&twice, 5000));
}
#[test]
fn frame_zero_prefix_cancellation_retains_the_original_transaction() {
    let f = Fixture::new();
    let mut s = f.session();
    s.project.tracks[0].clips[0].start_frame = 1_000_000;
    let before = s.project.clone();
    let progress = BounceProgress::default();
    let cancel = progress.clone();
    let worker = std::thread::spawn(move || s.bounce(&tracks(&s), &cancel));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while progress.done.load(Ordering::Acquire) < 8192 {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    progress.cancel.store(true, Ordering::Release);
    assert!(matches!(
        worker.join().unwrap(),
        Err(BounceError::Cancelled)
    ));
    assert!(!before.tracks[0].clips[0].muted);
}

thread_local! {static COUNT_HEAP:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};static HEAP_OPS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
struct CallbackAllocator;
fn heap_operation() {
    COUNT_HEAP.with(|enabled| {
        if enabled.get() {
            HEAP_OPS.with(|count| count.set(count.get() + 1));
        }
    });
}
// SAFETY: Every operation delegates the unchanged pointer/layout to System.
unsafe impl std::alloc::GlobalAlloc for CallbackAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        heap_operation();
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        heap_operation();
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        heap_operation();
        unsafe { std::alloc::System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static CALLBACK_ALLOCATOR: CallbackAllocator = CallbackAllocator;
#[test]
fn captured_disk_taps_use_callback_read_ahead_without_heap_operations() {
    let f = Fixture::new();
    let mut s = f.session();
    let source = s.project.tracks[0].id;
    let destination = s.project.add_track().unwrap();
    let mut clip = s.project.tracks[0].clips[0].clone();
    clip.id = Id::new_v4();
    s.project.tracks[1].clips.push(clip);
    sidechain(
        &mut s,
        source,
        destination,
        daw_unit_core::SidechainTap::PreFader,
    );
    let ret = s.project.add_return_track().unwrap();
    let mut send = TrackSend::new(ret);
    send.tap = SendTap::PreFader;
    send.gain_db = -6.0;
    s.project.tracks[0].sends.push(send);
    let mut next = s
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![source],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    let mut resident = daw_engine::Renderer::new(next.plan());
    resident.play();
    for asset in next
        .project
        .assets
        .iter()
        .filter(|a| a.source.is_generated())
    {
        let audio = next.audio.get_mut(&asset.id).unwrap();
        let mut disk = daw_media::DiskAudioWriter::new(audio.frame_count()).unwrap();
        disk.write(&audio.samples).unwrap();
        *audio = disk
            .finish(audio.sample_rate_hz, audio.metadata.clone())
            .unwrap();
    }
    let mut renderer = daw_engine::Renderer::new(next.plan());
    renderer.prepare_streaming().unwrap();
    renderer.play();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut peak = 0.0f32;
    for _ in 0..5000 {
        let frame = loop {
            let position = renderer.playhead;
            HEAP_OPS.with(|count| count.set(0));
            COUNT_HEAP.with(|enabled| enabled.set(true));
            let frame = renderer.next_sample();
            COUNT_HEAP.with(|enabled| enabled.set(false));
            assert_eq!(HEAP_OPS.with(|count| count.get()), 0);
            if !renderer.buffering() {
                break frame;
            }
            assert_eq!(renderer.playhead, position);
            assert!(
                !renderer.source_failed(),
                "read-ahead failed: {:?}",
                renderer.source_error()
            );
            assert!(
                std::time::Instant::now() < deadline,
                "bounce read-ahead timed out"
            );
            // Yield only in the test harness. Every callback attempt, including
            // buffering, remains covered by the allocation/deallocation guard.
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        assert_eq!(frame, resident.next_sample());
        assert_eq!(renderer.playhead, resident.playhead);
        peak = peak.max(frame[0].abs()).max(frame[1].abs());
    }
    assert!(peak > 0.0);
    assert!(renderer.source_error().is_none());
}

#[test]
fn bounced_arrangements_reconstruct_instead_of_inheriting_original_effect_tails() {
    let f = Fixture::new();
    let mut s = f.session();
    s.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::plate_reverb());
    let next = s
        .bounce(&tracks(&s), &BounceProgress::default())
        .unwrap()
        .session;
    let old = daw_engine::Renderer::new(s.plan());
    let candidate = daw_engine::Renderer::new(next.plan());
    assert!(!candidate.can_inherit_without_preroll(&old));
    assert!(!old.can_inherit_without_preroll(&candidate));
    let mut prepared = daw_engine::Renderer::new(next.plan());
    let mut state = old.transport_state();
    state.frame = 5000;
    prepared.prepare_transport(state);
    let mut actual = vec![[0.0; 2]; 1000];
    prepared.render_offline(&mut actual);
    let expected = render(&next, 6000);
    assert_close(&actual, &expected[5000..]);
}

#[test]
fn the_normal_tail_cap_and_terminal_fade_do_not_generate_a_notice() {
    let f = Fixture::new();
    let mut s = f.session();
    s.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::drive());
    let prepared = s.bounce(&tracks(&s), &BounceProgress::default()).unwrap();
    assert!(prepared.warnings.is_empty(), "{:?}", prepared.warnings);
    let clip = &prepared.session.project.tracks[1].clips[0];
    assert!(clip.end() >= s.project.end());
    assert!(clip.end() < s.project.end() + 24_000);
}

#[test]
fn floating_point_bounce_preserves_headroom_without_a_warning() {
    let f = Fixture::new();
    let mut s = f.session();
    s.project.tracks[0].clips[0].gain_db = 30.0;
    let prepared = s.bounce(&tracks(&s), &BounceProgress::default()).unwrap();
    assert!(prepared.warnings.is_empty(), "{:?}", prepared.warnings);
    let asset = prepared.session.project.tracks[1].clips[0].asset_id;
    assert!(
        prepared.session.audio[&asset]
            .samples
            .iter()
            .flatten()
            .any(|v| v.abs() > 1.0)
    );
}

#[test]
fn sampler_granulizer_and_noise_keep_source_layer_and_output_processing() {
    for source in ["sampler", "granulizer", "noise"] {
        let f = Fixture::new();
        let mut s = f.session();
        s.project.tracks.clear();
        let track = s.project.add_midi_track().unwrap();
        s.project.add_midi_pattern(track, 1200).unwrap();
        let asset = s.project.tracks[0].clips[0].asset_id;
        s.project
            .assets
            .iter_mut()
            .find(|a| a.id == asset)
            .unwrap()
            .midi_pattern
            .as_mut()
            .unwrap()
            .notes
            .push(daw_core::MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 240,
                pitch: 60,
                velocity: 100,
            });
        let layer = s.project.tracks[0].instrument.as_ref().unwrap().layers[0].id;
        if source == "noise" {
            s.project.tracks[0].instrument.as_mut().unwrap().layers[0].noise =
                Some(daw_core::NoiseSettings::default());
        } else {
            let decoded = s.audio.values().next().unwrap().clone();
            s = s
                .with_layer_sampler_decoded(track, layer, &f.0.join("source.wav"), decoded, &|| {
                    false
                })
                .unwrap();
            if source == "granulizer" {
                let sampler = s.project.tracks[0].instrument.as_mut().unwrap().layers[0]
                    .sampler
                    .as_mut()
                    .unwrap();
                sampler.version = 2;
                sampler.trigger = daw_unit_core::SamplerTrigger::Gate;
                sampler.granulizer = Some(daw_core::GranulizerSettings::default());
            }
        }
        let instrument = s.project.tracks[0].instrument.as_mut().unwrap();
        instrument.layers[0]
            .effects
            .push(daw_core::EffectInstance::drive());
        instrument
            .main
            .effects
            .push(daw_core::EffectInstance::drive());
        s.project.validate().unwrap();
        s.prepare_stretches().unwrap();
        let frames = s.project.end() as usize + 4000;
        let expected = render(&s, frames);
        assert!(
            expected.iter().flatten().any(|v| v.abs() > 1e-6),
            "{source}"
        );
        let next = s
            .bounce(&tracks(&s), &BounceProgress::default())
            .unwrap()
            .session;
        assert_close(&expected, &render(&next, frames));
    }
}

#[test]
fn merge_keeps_captured_pre_sends_through_nested_mixed_recipes_and_storage() {
    let fixture = Fixture::new();
    let mut s = fixture.session();
    let ret = s.project.add_return_track().unwrap();
    s.project.tracks[0].gain_db = daw_core::SILENT_GAIN_DB;
    let mut send = TrackSend::new(ret);
    send.tap = SendTap::PreFader;
    send.gain_db = 0.0;
    s.project.tracks[0].sends.push(send);
    let bounced = s
        .bounce(&tracks(&s), &BounceProgress::default())
        .unwrap()
        .session;
    let expected = render(&bounced, 8000);
    assert!(expected.iter().flatten().any(|s| s.abs() > 0.01));
    let id = bounced.project.tracks[1].clips[0].id;
    let (mut merged, id) = bounced.merged_audio_clips(&[id], &|| false).unwrap();
    assert_close(&expected, &render(&merged, 8000));
    let mut history = SessionHistory::new(&bounced, false).unwrap();
    history.record(&merged).unwrap();
    let undone = history.undo(&merged).unwrap();
    assert_close(&expected, &render(&undone, 8000));
    assert_close(&expected, &render(&history.redo(&undone).unwrap(), 8000));
    let extracted = merged
        .render_clip_source(&merged.project.tracks[1].clips[0], &|| false)
        .unwrap();
    assert!(extracted.samples.iter().flatten().all(|v| *v == 0.0));
    let mut copy = merged.project.tracks[1].clips[0].clone();
    copy.id = Id::new_v4();
    copy.start_frame = 6000;
    merged.project.tracks[1].clips.push(copy.clone());
    // An ordinary source member must retain the live Pre send alongside capture.
    let mono_path = fixture.0.join("mono.wav");
    let mut writer = hound::WavWriter::create(
        &mono_path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for _ in 0..2400 {
        writer.write_sample(0.1f32).unwrap();
    }
    writer.finalize().unwrap();
    let output = merged.project.tracks[1].id;
    let ordinary = merged.import(&mono_path, Some(output), 11000).unwrap();
    merged.project.tracks[1].pan = -0.4;
    let mut effect = daw_core::EffectInstance::drive();
    let daw_core::EffectSettings::Drive(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.gain = 0.8;
    merged.project.tracks[1].effects.push(effect);
    merged.project.tracks[1]
        .effects
        .push(daw_core::EffectInstance::limiter());
    let expected = render(&merged, 16000);
    let (mut nested, _) = merged
        .merged_audio_clips(&[id, copy.id, ordinary], &|| false)
        .unwrap();
    assert_close(&expected, &render(&nested, 16000));
    let rebounced = nested
        .bounce(
            &BounceRequest {
                mode: BounceMode::Tracks,
                ids: vec![output],
            },
            &BounceProgress::default(),
        )
        .unwrap()
        .session;
    assert_close(&expected, &render(&rebounced, 16000));
    let mut repeated = nested.clone();
    let clip = &mut repeated.project.tracks[1].clips[0];
    let cycle_frames = clip.length_frames;
    clip.repeat = Some(daw_core::ClipLoop {
        length_frames: cycle_frames,
        phase_frame: 0,
    });
    clip.length_frames *= 2;
    repeated.prepare_stretches().unwrap();
    let mut unfolded = merged.clone();
    let mut second_cycle = unfolded.project.tracks[1].clips.clone();
    for clip in &mut second_cycle {
        clip.id = Id::new_v4();
        clip.start_frame += cycle_frames;
    }
    unfolded.project.tracks[1].clips.extend(second_cycle);
    assert_close(&render(&unfolded, 30000), &render(&repeated, 30000));
    let mut crop = nested.clone();
    let clip = &mut crop.project.tracks[1].clips[0];
    clip.start_frame += 9000;
    clip.source_offset_frame += 9000;
    clip.length_frames -= 9000;
    clip.fades.fade_in.inherit_source = false;
    crop.prepare_stretches().unwrap();
    let mut ordinary_only = merged.clone();
    ordinary_only.project.tracks[1]
        .clips
        .retain(|c| c.id == ordinary);
    assert_close(&render(&ordinary_only, 16000), &render(&crop, 16000));
    nested.save(&fixture.0.join("merged")).unwrap();
    let reopened = Session::open(&fixture.0.join("merged")).unwrap();
    assert_close(&expected, &render(&reopened, 16000));
    // Legacy recipes are repaired deterministically without touching recordings.
    let mut legacy = nested.project.clone();
    for asset in &mut legacy.assets {
        if asset.compound_audio.is_some() {
            asset.rendered_audio = None;
        }
    }
    // Remove only generated companion recipes; original member references remain.
    let main = nested.project.tracks[1].clips[0].asset_id;
    let mut needed = std::collections::HashSet::new();
    let mut pending = vec![main];
    while let Some(id) = pending.pop() {
        if !needed.insert(id) {
            continue;
        }
        if let Some(source) = legacy
            .audio_asset(id)
            .and_then(|a| a.compound_audio.as_ref())
        {
            pending.extend(source.members.iter().map(|c| c.asset_id));
        }
    }
    legacy
        .assets
        .retain(|a| a.compound_audio.is_none() || needed.contains(&a.id));
    fs::write(
        fixture.0.join("merged/project.json"),
        serde_json::to_vec_pretty(&legacy).unwrap(),
    )
    .unwrap();
    let inspected = daw_project::inspect_project(&fixture.0.join("merged")).unwrap();
    let repaired = Session::open(&fixture.0.join("merged")).unwrap();
    assert_eq!(inspected, repaired.project);
    assert_close(&expected, &render(&repaired, 16000));
}

#[test]
fn merged_capture_preserves_old_sidechain_feeds_and_shared_clip_edits() {
    for tap in [
        daw_unit_core::SidechainTap::PreFader,
        daw_unit_core::SidechainTap::PostFader,
    ] {
        let fixture = Fixture::new();
        let mut s = fixture.session();
        let source = s.project.tracks[0].id;
        let destination = s.project.add_track().unwrap();
        let mut clip = s.project.tracks[0].clips[0].clone();
        clip.id = Id::new_v4();
        s.project.tracks[1].clips.push(clip);
        sidechain(&mut s, source, destination, tap);
        let bounced = s
            .bounce(&tracks(&s), &BounceProgress::default())
            .unwrap()
            .session;
        let id = bounced.project.tracks[1].clips[0].id;
        let (merged, _) = bounced.merged_audio_clips(&[id], &|| false).unwrap();
        for mode in 0..5 {
            let mut reference = bounced.clone();
            let mut actual = merged.clone();
            for session in [&mut reference, &mut actual] {
                let clip = &mut session.project.tracks[1].clips[0];
                clip.gain_db = -6.0;
                clip.channels.swapped = true;
                match mode {
                    0 => {
                        clip.source_offset_frame = 300;
                        clip.length_frames = 1200;
                    }
                    1 => {
                        clip.repeat = Some(daw_core::ClipLoop {
                            length_frames: 700,
                            phase_frame: 110,
                        });
                        clip.length_frames = 2100;
                    }
                    2 => {
                        clip.transpose = 3;
                    }
                    3 => {
                        clip.stretch = daw_core::ClipStretch::new(1, 2);
                        clip.length_frames *= 2;
                    }
                    _ => {
                        clip.repeat = Some(daw_core::ClipLoop {
                            length_frames: clip.length_frames,
                            phase_frame: 0,
                        });
                        clip.length_frames *= 2;
                    }
                }
                session.prepare_stretches().unwrap();
            }
            assert_close(&render(&reference, 10000), &render(&actual, 10000));
        }
    }
}

#[test]
fn merged_provider_union_keeps_absent_members_silent_and_rejects_feedback() {
    for tap in [
        daw_unit_core::SidechainTap::PreFader,
        daw_unit_core::SidechainTap::PostFader,
    ] {
        let fixture = Fixture::new();
        let mut s = fixture.session();
        let a = s.project.tracks[0].id;
        let b = s.project.add_track().unwrap();
        let mut clip = s.project.tracks[0].clips[0].clone();
        clip.id = Id::new_v4();
        s.project.tracks[1].clips.push(clip.clone());
        s.project.tracks[1].gain_db = -9.0;
        let destination = s.project.add_track().unwrap();
        clip.id = Id::new_v4();
        s.project.tracks[2].clips.push(clip);
        sidechain(&mut s, a, destination, tap);
        sidechain(&mut s, b, destination, tap);
        let mut bounced = s
            .bounce(
                &BounceRequest {
                    mode: BounceMode::Tracks,
                    ids: vec![a, b],
                },
                &BounceProgress::default(),
            )
            .unwrap()
            .session;
        let mut second = bounced.project.tracks[3].clips.remove(0);
        second.start_frame = 6000;
        let ids = vec![bounced.project.tracks[1].clips[0].id, second.id];
        bounced.project.tracks[1].clips.push(second);
        bounced.project.validate().unwrap();
        let expected = render(&bounced, 10000);
        let (merged, _) = bounced.merged_audio_clips(&ids, &|| false).unwrap();
        assert_close(&expected, &render(&merged, 10000));
        assert_eq!(
            merged.project.rendered_providers(&merged.project.tracks[1]),
            {
                let mut ids = vec![a, b];
                ids.sort_unstable();
                ids
            }
        );
        let mut feedback = merged.clone();
        let output = feedback.project.tracks[1].id;
        sidechain(&mut feedback, destination, output, tap);
        assert!(feedback.project.validate().is_err());
        // Include routing dependency edges in source-DAG validation as well.
        let mut cycle = merged.project.clone();
        let main = cycle.tracks[1].clips[0].asset_id;
        let aux = cycle
            .audio_asset(main)
            .unwrap()
            .rendered_audio
            .as_ref()
            .unwrap()
            .pre_send_asset;
        let member = &mut cycle
            .assets
            .iter_mut()
            .find(|asset| asset.id == aux)
            .unwrap()
            .compound_audio
            .as_mut()
            .unwrap()
            .members[0];
        member.asset_id = main;
        assert!(cycle.validate().unwrap_err().0.contains("cycle"));
    }
}
