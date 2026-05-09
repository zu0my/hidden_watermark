use crc32fast::Hasher;

pub(crate) const MAGIC: &[u8; 4] = b"HWM2";
pub(crate) const VERSION: u8 = 2;
pub(crate) const MAX_ID_BYTES: usize = 64;
pub(crate) const HEADER_BYTES: usize = 6;
pub(crate) const CRC_BYTES: usize = 4;

#[derive(Clone, Debug)]
pub(crate) enum ParseFrameResult {
    Decoded(String),
    NoWatermark,
    CrcMismatch,
    UnsupportedPayload,
}

#[derive(Clone, Debug)]
pub(crate) enum HeaderStatus {
    Valid(usize),
    Invalid,
    Unsupported,
}

pub(crate) fn build_frame(id: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_BYTES + id.len() + CRC_BYTES);
    frame.extend_from_slice(MAGIC);
    frame.push(VERSION);
    frame.push(id.len() as u8);
    frame.extend_from_slice(id);

    let mut hasher = Hasher::new();
    hasher.update(&frame);
    let crc = hasher.finalize();
    frame.extend_from_slice(&crc.to_be_bytes());
    frame
}

pub(crate) fn parse_frame(frame: &[u8]) -> ParseFrameResult {
    if frame.len() < HEADER_BYTES + CRC_BYTES {
        return ParseFrameResult::NoWatermark;
    }
    if &frame[..4] != MAGIC || frame[4] != VERSION {
        return ParseFrameResult::NoWatermark;
    }
    let len = frame[5] as usize;
    if len > MAX_ID_BYTES {
        return ParseFrameResult::UnsupportedPayload;
    }
    let total = HEADER_BYTES + len + CRC_BYTES;
    if frame.len() < total {
        return ParseFrameResult::UnsupportedPayload;
    }

    let expected = u32::from_be_bytes([
        frame[HEADER_BYTES + len],
        frame[HEADER_BYTES + len + 1],
        frame[HEADER_BYTES + len + 2],
        frame[HEADER_BYTES + len + 3],
    ]);
    let mut hasher = Hasher::new();
    hasher.update(&frame[..HEADER_BYTES + len]);
    if hasher.finalize() != expected {
        return ParseFrameResult::CrcMismatch;
    }

    match String::from_utf8(frame[HEADER_BYTES..HEADER_BYTES + len].to_vec()) {
        Ok(id) => ParseFrameResult::Decoded(id),
        Err(_) => ParseFrameResult::UnsupportedPayload,
    }
}

pub(crate) fn frame_bits(frame: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(frame.len() * 8);
    for &byte in frame {
        push_byte_bits(byte, &mut bits);
    }
    bits
}

pub(crate) fn push_byte_bits(byte: u8, bits: &mut Vec<bool>) {
    for shift in (0..8).rev() {
        bits.push(((byte >> shift) & 1) == 1);
    }
}

pub(crate) fn bits_to_bytes(bits: &[bool], raw_bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw_bytes);
    for byte_index in 0..raw_bytes {
        let mut byte = 0_u8;
        for bit_index in 0..8 {
            if bits
                .get(byte_index * 8 + bit_index)
                .copied()
                .unwrap_or(false)
            {
                byte |= 1 << (7 - bit_index);
            }
        }
        out.push(byte);
    }
    out
}

pub(crate) fn parse_frame_header(header: &[u8]) -> HeaderStatus {
    if header.len() != HEADER_BYTES || &header[..4] != MAGIC || header[4] != VERSION {
        return HeaderStatus::Invalid;
    }
    let len = header[5] as usize;
    if len > MAX_ID_BYTES {
        HeaderStatus::Unsupported
    } else {
        HeaderStatus::Valid(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let frame = build_frame(b"asset-123");
        let bits = frame_bits(&frame);
        let decoded = bits_to_bytes(&bits, frame.len());
        assert!(matches!(
            parse_frame(&decoded),
            ParseFrameResult::Decoded(id) if id == "asset-123"
        ));
    }

    #[test]
    fn rejects_invalid_crc() {
        let mut frame = build_frame(b"asset-123");
        frame[HEADER_BYTES] ^= 1;
        assert!(matches!(parse_frame(&frame), ParseFrameResult::CrcMismatch));
    }
}
