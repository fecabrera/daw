use super::*;

/// A fixed tone isolates unwanted delay-address modulation from intended Motion
/// and pitch regeneration. The second-order residual rejects that input tone.
#[test]
fn delay_drags_do_not_turn_a_steady_tone_into_broadband_noise() {
    let mut failures = Vec::new();
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for control in [
            "size",
            "predelay",
            "modulated size",
            "modulated predelay",
            "synced tempo",
        ] {
            let mut s = Settings {
                depth: 0.0,
                predelay_ms: 200.0,
                sync: control == "synced tempo",
                dry_wet: 1.0,
                ..Default::default()
            };
            let mut unit = ShimmerReverb::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &s,
            );
            let omega = std::f64::consts::TAU * 1000.0 / f64::from(rate);
            let mut history = [[0.0; 2]; 2];
            let mut energy = 0.0;
            let mut residual_energy = 0.0;
            let mut offsets = [0.0; 12];
            let mut tempo = 120.0;
            for frame in 0..rate * 6 {
                if frame >= rate && frame % (rate / 120) == 0 {
                    let phase =
                        std::f64::consts::TAU * 0.6 * f64::from(frame - rate) / f64::from(rate);
                    let value = 0.5 + 0.45 * phase.sin() as f32;
                    match control {
                        "size" => {
                            s.size = value;
                            unit.retarget(&s);
                        }
                        "predelay" => {
                            s.predelay_ms = value * 1000.0;
                            unit.retarget(&s);
                        }
                        "modulated size" => {
                            offsets[0] = value - s.size;
                        }
                        "modulated predelay" => {
                            offsets[1] = value * 1000.0 - s.predelay_ms;
                        }
                        _ => {
                            tempo = 80.0 + 80.0 * value;
                        }
                    }
                }
                let input = [(omega * f64::from(frame)).sin() as f32 * 0.1; 2];
                let output = unit.process_modulated_at(input, u64::from(frame), tempo, offsets);
                for ch in 0..2 {
                    let y = f64::from(output[ch]);
                    let residual = y - 2.0 * omega.cos() * history[0][ch] + history[1][ch];
                    if frame >= rate * 2 {
                        energy += y * y;
                        residual_energy += residual * residual;
                    }
                    history[1][ch] = history[0][ch];
                    history[0][ch] = y;
                }
            }
            let noise_ratio = residual_energy / energy * (f64::from(rate) / 48000.0).powi(4);
            println!("{rate} {control}: tone residual {noise_ratio:e}");
            assert!(energy > 1e-10);
            if noise_ratio >= 1e-5 {
                failures.push((rate, control, noise_ratio));
            }
        }
    }
    assert!(failures.is_empty(), "drag noise: {failures:?}");
}
