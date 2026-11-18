//! Bounded byte transfer for typed plugin state, used outside audio processing.
use std::io::{self, ErrorKind, Read, Write};

/// Read through EOF with one byte reserved to detect excess input.
/// None means the size or transfer-attempt limit was exceeded.
pub fn read_payload(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<Option<usize>> {
    let mut length = 0;
    for _ in 0..2 * buffer.len() {
        match reader.read(&mut buffer[length..]) {
            Ok(0) => return Ok(Some(length)),
            Ok(count) => {
                if count >= buffer.len() - length {
                    return Ok(None);
                }
                length += count;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

/// Write the complete payload. False means the transfer-attempt limit was exceeded.
pub fn write_payload(writer: &mut impl Write, payload: &[u8]) -> io::Result<bool> {
    let mut offset = 0;
    for _ in 0..2 * payload.len() {
        match writer.write(&payload[offset..]) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(count) => {
                offset += count;
                if offset == payload.len() {
                    return Ok(true);
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}
