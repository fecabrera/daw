use super::*;
#[test]
fn ambient_reverb_track_master_export_seek_and_compatible_edits_share_history_without_allocating() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            let mut p = plan(rate, false);
            let effect = EffectInstance::ambient_reverb();
            let id = effect.id;
            if master {
                p.project.master.effects.push(EffectInstance::saturator());
                p.project.master.effects.push(effect);
            } else {
                p.project.tracks[0]
                    .effects
                    .push(EffectInstance::saturator());
                p.project.tracks[0].effects.push(effect);
            }
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(p.clone());
            live.play();
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            for (frame, expected) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame > rate as usize / 200 {
                    assert_eq!(&actual, expected);
                }
            }
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            let mut prepared = Renderer::new(p.clone());
            let mut state = live.transport_state();
            state.frame = 4096;
            prepared.prepare_transport(state);
            for (frame, expected_sample) in expected.iter().enumerate().skip(4096) {
                let actual = prepared.next_sample();
                if frame > 4096 + rate as usize / 200 {
                    assert_eq!(actual, *expected_sample);
                }
            }
            let mut current = Renderer::new(p.clone());
            let mut reference = Renderer::new(p.clone());
            current.play();
            reference.play();
            for _ in 0..3000 {
                assert_eq!(current.next_sample(), reference.next_sample());
            }
            let chain = if master {
                &mut p.project.master.effects
            } else {
                &mut p.project.tracks[0].effects
            };
            let EffectSettings::AmbientReverb(settings) =
                &mut chain.iter_mut().find(|e| e.id == id).unwrap().unit
            else {
                panic!()
            };
            settings.collapsed = true;
            let mut next = Renderer::new(p);
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            next.inherit(&mut current);
            for _ in 0..1024 {
                assert_eq!(next.next_sample(), reference.next_sample());
            }
            let snapshot = next
                .modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                .unwrap();
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            assert!((snapshot[0] - 7.5).abs() < 1e-6);
        }
    }
}
