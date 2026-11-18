use crate::tone::plan;
use daw_core::EffectInstance;
use daw_engine::Renderer;

pub fn check(configured: fn() -> EffectInstance, latency: u64) {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for master in [false, true] {
            let mut p = plan(rate, false);
            let effect = configured();
            let id = effect.id;
            if master {
                p.project.master.effects.push(effect);
            } else {
                p.project.tracks[0].effects.push(effect);
            }
            p.project.validate().unwrap();
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(p.clone());
            assert_eq!(live.latency_frames(), latency);
            live.play();
            for (i, want) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if i > rate as usize / 200 {
                    assert!((actual[0] - want[0]).abs() < 1e-6);
                }
            }
            assert!(expected.iter().any(|v| v[0].abs() > 0.01));
            assert!(
                live.modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                    .is_some()
            );
            if rate == 48000 {
                for block in [32, 512, 4096] {
                    let mut renderer = Renderer::new(p.clone());
                    let mut out = vec![[0.0; 2]; 8192];
                    for (i, chunk) in out.chunks_mut(block).enumerate() {
                        renderer.render_offline_at((i * block) as u64, chunk);
                    }
                    assert_eq!(out, expected);
                }
                let mut renderer = Renderer::new(p);
                let mut seek = vec![[0.0; 2]; 1024];
                renderer.render_offline_at(4096, &mut seek);
                assert_eq!(seek, expected[4096..5120]);
            }
        }
    }
}
