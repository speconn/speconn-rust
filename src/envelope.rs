pub const FLAG_COMPRESSED: u8 = 0x01;
pub const FLAG_END_STREAM: u8 = 0x02;

pub fn encode_envelope(flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.push(flags);
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

pub fn decode_envelope(frame: &[u8]) -> Result<(u8, &[u8]), String> {
    if frame.len() < 5 {
        return Err(format!("envelope: frame too short ({} bytes)", frame.len()));
    }
    let flags = frame[0];
    let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
    if frame.len() < 5 + length {
        return Err(format!("envelope: expected {} payload bytes, got {}", length, frame.len() - 5));
    }
    Ok((flags, &frame[5..5 + length]))
}
