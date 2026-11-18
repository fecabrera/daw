// Host frame, tempo, playing flag and expected DSP frame at the start of each block.
pub const SEEK_AND_TEMPO: [(u64, f32, bool, u64); 6] = [
    (48000, 90.0, true, 48000),
    (96000, 180.0, true, 96000),
    (12000, 110.0, true, 12000),
    (12000, 110.0, false, 12512),
    (12000, 110.0, false, 13024),
    (72000, 110.0, true, 72000),
];

pub const OFFLINE_FRAMES: [u64; 4] = [48000, 48512, 49024, 49536];
