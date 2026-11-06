//! Ogg Opus import timing. Symphonia 0.5's generic gapless timing does not model
//! Opus pre-skip and short final pages together; keep the RFC 7845 bounds here.
use crate::{Error, Result};
use std::{fs::File, io::Read, path::Path};

pub(crate) struct OpusImport {
    skip: u64,
    remaining: u64,
    pub gain: f32,
}

impl OpusImport {
    pub fn read(
        path: &Path,
        head: &[u8],
        start_ts: u64,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        if head.len() < 19 || &head[..8] != b"OpusHead" || head[18] != 0 {
            return Err(Error(
                "Only mono/stereo Opus channel mapping family 0 is supported".into(),
            ));
        }
        let skip = u64::from(u16::from_le_bytes([head[10], head[11]]));
        let gain_db = f32::from(i16::from_le_bytes([head[16], head[17]])) / 256.0;
        let mut file = File::open(path).map_err(|e| Error(e.to_string()))?;
        let length = file.metadata().map_err(|e| Error(e.to_string()))?.len();
        let mut serial = None;
        let mut sequence = 0_u32;
        let mut final_granule = None;
        let mut previous_granule = 0;
        let mut last_complete_granule = None;
        let mut position = 0_u64;
        while position < length {
            if cancelled() {
                return Err(Error("Audio preparation cancelled".into()));
            }
            if final_granule.is_some() {
                return Err(Error(
                    "Chained or multiplexed Ogg streams are unsupported".into(),
                ));
            }
            let mut header = [0_u8; 27];
            file.read_exact(&mut header)
                .map_err(|e| Error(format!("Incomplete Ogg page: {e}")))?;
            if &header[..4] != b"OggS" || header[4] != 0 || header[5] & !7 != 0 {
                return Err(Error("Invalid Ogg page header".into()));
            }
            let page_serial = u32::from_le_bytes(header[14..18].try_into().unwrap());
            let page_sequence = u32::from_le_bytes(header[18..22].try_into().unwrap());
            if serial.is_some_and(|id| id != page_serial)
                || page_sequence != sequence
                || (sequence == 0) != (header[5] & 2 != 0)
            {
                return Err(Error(
                    "Chained, multiplexed or discontinuous Ogg streams are unsupported".into(),
                ));
            }
            serial = Some(page_serial);
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| Error("Ogg page sequence overflow".into()))?;
            let count = usize::from(header[26]);
            let mut lacing = [0_u8; 255];
            file.read_exact(&mut lacing[..count])
                .map_err(|e| Error(format!("Incomplete Ogg page: {e}")))?;
            let body: u64 = lacing[..count].iter().map(|&n| u64::from(n)).sum();
            position = position
                .checked_add(27 + count as u64 + body)
                .filter(|&end| end <= length)
                .ok_or_else(|| Error("Incomplete Ogg page data".into()))?;
            let expected_crc = u32::from_le_bytes(header[22..26].try_into().unwrap());
            let mut crc_header = header;
            crc_header[22..26].fill(0);
            let mut crc = crc_update(crc_update(0, &crc_header), &lacing[..count]);
            let mut remaining_body = body;
            let mut block = [0_u8; 8192];
            while remaining_body > 0 {
                if cancelled() {
                    return Err(Error("Audio preparation cancelled".into()));
                }
                let size = remaining_body.min(block.len() as u64) as usize;
                file.read_exact(&mut block[..size])
                    .map_err(|e| Error(e.to_string()))?;
                crc = crc_update(crc, &block[..size]);
                remaining_body -= size as u64;
            }
            if crc != expected_crc {
                return Err(Error("Ogg page checksum mismatch".into()));
            }
            let granule = u64::from_le_bytes(header[6..14].try_into().unwrap());
            if granule != u64::MAX {
                if granule < previous_granule {
                    return Err(Error("Ogg granule position moves backward".into()));
                }
                previous_granule = granule;
            }
            // Some recorders (including WhatsApp) omit EOS. A checksum-valid
            // final page with a complete packet still supplies its exact bound.
            last_complete_granule =
                (granule != u64::MAX && count > 0 && lacing[count - 1] < 255).then_some(granule);
            if header[5] & 4 != 0 {
                if granule == u64::MAX || count == 0 || lacing[count - 1] == 255 {
                    return Err(Error("Invalid final Ogg Opus page".into()));
                }
                final_granule = Some(granule);
            }
        }
        let remaining = final_granule
            .or(last_complete_granule)
            .and_then(|n| n.checked_sub(start_ts))
            .and_then(|n| n.checked_sub(skip))
            .filter(|&n| n > 0)
            .ok_or_else(|| {
                Error("Ogg Opus requires a complete final page and nonempty audio".into())
            })?;
        Ok(Self {
            skip,
            remaining,
            gain: 10.0_f32.powf(gain_db / 20.0),
        })
    }

    /// Trim pre-skip across any number of packets, then keep only the final
    /// granule's playable frames. Never use a declared duration to allocate PCM.
    pub fn trim(&mut self, frames: usize) -> (usize, usize) {
        let skip = self.skip.min(frames as u64) as usize;
        self.skip -= skip as u64;
        let keep = self.remaining.min((frames - skip) as u64) as usize;
        self.remaining -= keep as u64;
        (skip, frames - skip - keep)
    }

    pub fn complete(&self) -> bool {
        self.skip == 0 && self.remaining == 0
    }
}

// Ogg uses the unreflected CRC-32 polynomial with zero initial/final XOR.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < 256 {
        let mut crc = (index as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            crc = (crc << 1)
                ^ if crc & 0x8000_0000 != 0 {
                    0x04c1_1db7
                } else {
                    0
                };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
};

fn crc_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        crc = (crc << 8) ^ CRC_TABLE[((crc >> 24) as u8 ^ byte) as usize];
    }
    crc
}
