//! Deterministic little-endian IEEE-754 binary encoding helpers for `CanonicalSaveablePort`.

/// Writes a single byte into `out`.
pub fn write_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}

/// Writes an unsigned 16-bit integer in little-endian order.
pub fn write_u16_le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Writes an unsigned 32-bit integer in little-endian order.
pub fn write_u32_le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Writes a signed 32-bit integer in little-endian order.
pub fn write_i32_le(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Writes an unsigned 64-bit integer in little-endian order.
pub fn write_u64_le(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Writes a 32-bit float by its exact bit representation.
pub fn write_f32_bits(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_bits().to_le_bytes());
}

/// Writes a 64-bit float by its exact bit representation.
pub fn write_f64_bits(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(&v.to_bits().to_le_bytes());
}

/// Writes a UTF-8 string prefixed by its `u32` byte length.
pub fn write_str(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_u32_le(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

/// Writes a byte slice prefixed by its `u32` length.
pub fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_u32_le(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

fn take_exact<'a>(input: &mut &'a [u8], len: usize) -> Result<&'a [u8], String> {
    if input.len() < len {
        return Err(format!(
            "Unexpected EOF reading save stream: needed {len} bytes, {} remaining",
            input.len()
        ));
    }
    let (head, tail) = input.split_at(len);
    *input = tail;
    Ok(head)
}

/// Reads a single byte from `input`.
pub fn read_u8(input: &mut &[u8]) -> Result<u8, String> {
    Ok(take_exact(input, 1)?[0])
}

/// Reads an unsigned 16-bit integer in little-endian order.
pub fn read_u16_le(input: &mut &[u8]) -> Result<u16, String> {
    let b = take_exact(input, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

/// Reads an unsigned 32-bit integer in little-endian order.
pub fn read_u32_le(input: &mut &[u8]) -> Result<u32, String> {
    let b = take_exact(input, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Reads a signed 32-bit integer in little-endian order.
pub fn read_i32_le(input: &mut &[u8]) -> Result<i32, String> {
    let b = take_exact(input, 4)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Reads an unsigned 64-bit integer in little-endian order.
pub fn read_u64_le(input: &mut &[u8]) -> Result<u64, String> {
    let b = take_exact(input, 8)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// Reads an exact 32-bit float bit pattern.
pub fn read_f32_bits(input: &mut &[u8]) -> Result<f32, String> {
    Ok(f32::from_bits(read_u32_le(input)?))
}

/// Reads an exact 64-bit float bit pattern.
pub fn read_f64_bits(input: &mut &[u8]) -> Result<f64, String> {
    Ok(f64::from_bits(read_u64_le(input)?))
}

/// Reads a length-prefixed UTF-8 string.
pub fn read_str(input: &mut &[u8]) -> Result<String, String> {
    let len = read_u32_le(input)? as usize;
    let bytes = take_exact(input, len)?;
    std::str::from_utf8(bytes)
        .map(ToOwned::to_owned)
        .map_err(|e| format!("Invalid UTF-8 in canonical save stream: {e}"))
}

/// Reads a length-prefixed byte vector.
pub fn read_bytes(input: &mut &[u8]) -> Result<Vec<u8>, String> {
    let len = read_u32_le(input)? as usize;
    Ok(take_exact(input, len)?.to_vec())
}
