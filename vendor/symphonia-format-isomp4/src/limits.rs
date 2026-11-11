// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// DAW resource-bound patch. Original component remains MPL-2.0.
use crate::atoms::AtomHeader;
use std::convert::TryFrom;
use symphonia_core::errors::{decode_error, Error, Result};
use symphonia_core::io::ReadBytes;

pub const METADATA_BYTES: u64 = 8 * 1024 * 1024;
pub const PACKET_BYTES: usize = 16 * 1024 * 1024;
// Conservative allowance for parsed metadata/index expansion, one packet and
// decoder scratch. This is known working admission, not a process RSS bound.
pub const WORKING_BYTES: u64 = METADATA_BYTES * 16 + PACKET_BYTES as u64 + 1024 * 1024;

pub fn table<T>(header: AtomHeader, count: u32, width: u64, prefix: u64) -> Result<Vec<T>> {
    let available = header
        .data_len
        .checked_sub(prefix)
        .ok_or(Error::DecodeError("isomp4: truncated table header"))?;
    if width == 0 || u64::from(count) > available / width {
        return decode_error("isomp4: table count exceeds atom extent");
    }
    let count = usize::try_from(count).map_err(|_| Error::LimitError("isomp4: table count"))?;
    if count
        .checked_mul(std::mem::size_of::<T>())
        .is_none_or(|n| n as u64 > METADATA_BYTES)
    {
        return Err(Error::LimitError("isomp4: table storage limit"));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::LimitError("isomp4: table allocation"))?;
    Ok(values)
}

pub fn bytes<B: ReadBytes>(reader: &mut B, len: usize, limit: usize) -> Result<Box<[u8]>> {
    if len > limit {
        return Err(Error::LimitError("isomp4: byte storage limit"));
    }
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| Error::LimitError("isomp4: byte allocation"))?;
    data.resize(len, 0);
    reader.read_buf_exact(&mut data)?;
    Ok(data.into_boxed_slice())
}
