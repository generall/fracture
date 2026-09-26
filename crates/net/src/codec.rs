//! Bounded binary frames, optionally compressed with LZ4. The outer header is
//! fixed-size so a peer cannot make us allocate an unchecked message length.
use bincode::Options;
use serde::{Serialize, de::DeserializeOwned};
use std::io;

pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;
const HEADER: usize = 9;
fn options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_varint_encoding()
        .with_limit(MAX_MESSAGE as u64)
        .reject_trailing_bytes()
}
fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

pub fn encode<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let raw = options().serialize(value).map_err(invalid)?;
    if raw.len() > MAX_MESSAGE {
        return Err(invalid("Message exceeds limit"));
    }
    let compressed = lz4_flex::block::compress(&raw);
    let (kind, payload) = if compressed.len() + 16 < raw.len() {
        (1, compressed.as_slice())
    } else {
        (0, raw.as_slice())
    };
    let mut result = Vec::with_capacity(HEADER + payload.len());
    result.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    result.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    result.push(kind);
    result.extend_from_slice(payload);
    Ok(result)
}
/// None means an incomplete frame; no bytes are consumed until fully validated.
pub fn decode<T: DeserializeOwned>(buffer: &[u8]) -> io::Result<Option<(T, usize)>> {
    if buffer.len() < HEADER {
        return Ok(None);
    }
    let packed = u32::from_le_bytes(buffer[0..4].try_into().unwrap()) as usize;
    let raw = u32::from_le_bytes(buffer[4..8].try_into().unwrap()) as usize;
    if packed > MAX_MESSAGE
        || raw > MAX_MESSAGE
        || buffer[8] > 1
        || (buffer[8] == 0 && packed != raw)
    {
        return Err(invalid("Invalid frame header"));
    }
    let end = HEADER + packed;
    if buffer.len() < end {
        return Ok(None);
    }
    let unpacked;
    let payload = if buffer[8] == 1 {
        unpacked = lz4_flex::block::decompress(&buffer[HEADER..end], raw).map_err(invalid)?;
        if unpacked.len() != raw {
            return Err(invalid("Invalid expanded length"));
        }
        unpacked.as_slice()
    } else {
        &buffer[HEADER..end]
    };
    Ok(Some((
        options().deserialize(payload).map_err(invalid)?,
        end,
    )))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compressed_frames_round_trip_and_incremental_reads_wait() {
        let value = vec![123u32; 10000];
        let data = encode(&value).unwrap();
        assert!(data.len() < 300);
        for end in 0..data.len() {
            assert!(decode::<Vec<u32>>(&data[..end]).unwrap().is_none());
        }
        let (got, consumed) = decode::<Vec<u32>>(&data).unwrap().unwrap();
        assert_eq!(got, value);
        assert_eq!(consumed, data.len());
    }
    #[test]
    fn invalid_headers_and_compression_are_rejected_before_allocation() {
        let mut data = vec![255; 9];
        assert!(decode::<Vec<u8>>(&data).is_err());
        data[..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..8].copy_from_slice(&2u32.to_le_bytes());
        data[8] = 0;
        assert!(decode::<Vec<u8>>(&data).is_err());
        data[8] = 1;
        data.push(0xff);
        assert!(decode::<Vec<u8>>(&data).is_err());
    }
}
