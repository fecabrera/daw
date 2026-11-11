// Symphonia
// Copyright (c) 2019-2022 The Project Symphonia Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use symphonia_core::errors::{decode_error, Result};
use symphonia_core::io::ReadBytes;

use crate::atoms::{Atom, AtomHeader};

#[derive(Debug)]
pub enum ReferenceType {
    Segment,
    Media,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SidxReference {
    pub reference_type: ReferenceType,
    pub reference_size: u32,
    pub subsegment_duration: u32,
    // pub starts_with_sap: bool,
    // pub sap_type: u8,
    // pub sap_delta_time: u32,
}

/// Segment index atom.
#[allow(dead_code)]
#[derive(Debug)]
pub struct SidxAtom {
    /// Atom header.
    header: AtomHeader,
    pub reference_id: u32,
    pub timescale: u32,
    pub earliest_pts: u64,
    pub first_offset: u64,
    pub references: Vec<SidxReference>,
}

impl Atom for SidxAtom {
    fn header(&self) -> AtomHeader {
        self.header
    }

    fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
        // The anchor point for segment offsets is the first byte after this atom.
        let start = reader.pos();
        let anchor = start.checked_add(header.data_len).ok_or(symphonia_core::errors::Error::DecodeError("isomp4: index extent overflow"))?;

        let (version, _) = AtomHeader::read_extra(reader)?;

        let reference_id = reader.read_be_u32()?;
        let timescale = reader.read_be_u32()?;

        let (earliest_pts, first_offset) = match version {
            0 => (u64::from(reader.read_be_u32()?), anchor.checked_add(u64::from(reader.read_be_u32()?)).ok_or(symphonia_core::errors::Error::DecodeError("isomp4: index offset overflow"))?),
            1 => (reader.read_be_u64()?, anchor.checked_add(reader.read_be_u64()?).ok_or(symphonia_core::errors::Error::DecodeError("isomp4: index offset overflow"))?),
            _ => {
                return decode_error("isomp4: invalid sidx version");
            }
        };

        let _reserved = reader.read_be_u16()?;
        let reference_count = reader.read_be_u16()?;

        let mut references = crate::limits::table(header, u32::from(reference_count), 12, reader.pos() - start)?;

        for _ in 0..reference_count {
            let reference = reader.read_be_u32()?;
            let subsegment_duration = reader.read_be_u32()?;

            let reference_type = match (reference & 0x8000_0000) != 0 {
                false => ReferenceType::Media,
                true => ReferenceType::Segment,
            };

            let reference_size = reference & !0x8000_0000;

            // Ignore SAP
            let _ = reader.read_be_u32()?;

            references.push(SidxReference { reference_type, reference_size, subsegment_duration });
        }

        Ok(SidxAtom { header, reference_id, timescale, earliest_pts, first_offset, references })
    }
}
