// Symphonia
// Copyright (c) 2019-2022 The Project Symphonia Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::convert::TryFrom;
use symphonia_core::errors::Result;
use symphonia_core::io::ReadBytes;

use crate::atoms::{Atom, AtomHeader};

#[derive(Debug)]
pub struct SampleDurationEntry {
    pub sample_count: u32,
    pub sample_delta: u32,
}

/// Time-to-sample atom.
#[allow(dead_code)]
#[derive(Debug)]
pub struct SttsAtom {
    /// Atom header.
    header: AtomHeader,
    pub entries: Vec<SampleDurationEntry>,
    pub total_duration: u64,
}

impl SttsAtom {
    /// Get the timestamp and duration for the sample indicated by `sample_num`. Note, `sample_num`
    /// is indexed relative to the `SttsAtom`. Complexity of this function in O(N).
    pub fn find_timing_for_sample(&self, sample_num: u32) -> Option<(u64, u32)> {
        let mut ts = 0_u64;
        let mut next_entry_first_sample = 0_u32;

        // The Stts atom compactly encodes a mapping between number of samples and sample duration.
        // Iterate through each entry until the entry containing the next sample is found. The next
        // packet timestamp is then the sum of the product of sample count and sample duration for
        // the n-1 iterated entries, plus the product of the number of consumed samples in the n-th
        // iterated entry and sample duration.
        for entry in &self.entries {
            let first_sample = next_entry_first_sample;
            next_entry_first_sample = first_sample.checked_add(entry.sample_count)?;

            if sample_num < next_entry_first_sample {
                let entry_sample_offset = sample_num.checked_sub(first_sample)?;
                ts =
                    ts.checked_add(u64::from(entry.sample_delta) * u64::from(entry_sample_offset))?;

                return Some((ts, entry.sample_delta));
            }

            ts = ts.checked_add(u64::from(entry.sample_count) * u64::from(entry.sample_delta))?;
        }

        None
    }

    /// Get the sample that contains the timestamp indicated by `ts`. Note, the returned `sample_num`
    /// is indexed relative to the `SttsAtom`. Complexity of this function in O(N).
    pub fn find_sample_for_timestamp(&self, ts: u64) -> Option<u32> {
        let mut ts_accum = 0_u64;
        let mut sample_num = 0_u32;

        for entry in &self.entries {
            let delta = u64::from(entry.sample_delta) * u64::from(entry.sample_count);

            let end_ts = ts_accum.checked_add(delta)?;
            if end_ts > ts {
                let offset = u32::try_from((ts - ts_accum) / u64::from(entry.sample_delta)).ok()?;
                return sample_num.checked_add(offset);
            }

            ts_accum = end_ts;
            sample_num = sample_num.checked_add(entry.sample_count)?;
        }

        None
    }
}

impl Atom for SttsAtom {
    fn header(&self) -> AtomHeader {
        self.header
    }

    fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
        let (_, _) = AtomHeader::read_extra(reader)?;

        let entry_count = reader.read_be_u32()?;

        let mut total_duration = 0_u64;
        let mut total_samples = 0_u32;

        let mut entries = crate::limits::table(header, entry_count, 8, 8)?;

        for _ in 0..entry_count {
            let sample_count = reader.read_be_u32()?;
            let sample_delta = reader.read_be_u32()?;

            total_samples = total_samples.checked_add(sample_count).ok_or(
                symphonia_core::errors::Error::DecodeError("isomp4: sample count overflow"),
            )?;

            total_duration = total_duration
                .checked_add(u64::from(sample_count) * u64::from(sample_delta))
                .ok_or(symphonia_core::errors::Error::DecodeError(
                    "isomp4: sample duration overflow",
                ))?;

            entries.push(SampleDurationEntry {
                sample_count,
                sample_delta,
            });
        }

        Ok(SttsAtom {
            header,
            entries,
            total_duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atoms::AtomType;
    use std::io::Cursor;
    use symphonia_core::io::{MediaSourceStream, MediaSourceStreamOptions};

    fn table(entries: &[(u32, u32)]) -> Result<SttsAtom> {
        let mut payload = vec![0; 4];
        payload.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for (count, delta) in entries {
            payload.extend_from_slice(&count.to_be_bytes());
            payload.extend_from_slice(&delta.to_be_bytes());
        }
        let header = AtomHeader {
            atype: AtomType::TimeToSample,
            atom_len: payload.len() as u64 + 8,
            data_len: payload.len() as u64,
            data_pos: 0,
        };
        let mut input = MediaSourceStream::new(
            Box::new(Cursor::new(payload)),
            MediaSourceStreamOptions::default(),
        );
        SttsAtom::read(&mut input, header)
    }

    #[test]
    fn rejects_sample_coordinate_overflow_at_parse_boundary() {
        assert!(table(&[(1, 1024), (u32::MAX, 1024)])
            .unwrap_err()
            .to_string()
            .contains("sample count overflow"));
    }

    #[test]
    fn large_valid_coordinates_avoid_intermediate_overflow_and_keep_exclusive_end() {
        let count = u32::MAX;
        let table = table(&[(count - 4, 2), (4, 3)]).unwrap();
        let second_start = u64::from(count - 4) * 2;
        assert_eq!(
            table.find_timing_for_sample(count - 5),
            Some((u64::from(count - 5) * 2, 2))
        );
        assert_eq!(
            table.find_timing_for_sample(count - 4),
            Some((second_start, 3))
        );
        let last = second_start + 9;
        assert_eq!(table.find_timing_for_sample(count - 1), Some((last, 3)));
        assert_eq!(table.find_sample_for_timestamp(last + 2), Some(count - 1));
        assert_eq!(table.find_timing_for_sample(count), None);
        assert_eq!(table.find_sample_for_timestamp(table.total_duration), None);
    }

    #[test]
    fn empty_fragment_tables_and_zero_duration_entries_keep_existing_timing() {
        let empty = table(&[]).unwrap();
        assert_eq!(empty.total_duration, 0);
        assert_eq!(empty.find_timing_for_sample(0), None);
        assert_eq!(empty.find_sample_for_timestamp(0), None);
        let table = table(&[(0, 10), (5, 0), (2, 3)]).unwrap();
        assert_eq!(table.find_timing_for_sample(0), Some((0, 0)));
        assert_eq!(table.find_timing_for_sample(5), Some((0, 3)));
        assert_eq!(table.find_sample_for_timestamp(0), Some(5));
        assert_eq!(table.find_sample_for_timestamp(5), Some(6));
        assert_eq!(table.find_sample_for_timestamp(6), None);
    }
}
