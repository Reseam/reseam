//! Little-endian byte readers used by code-item decoding.

/// Reads a `u16` from `buf` at `off` without additional bounds checks.
pub fn u16_at(buf: &[u8], off: usize) -> u16 {
    let b0 = buf[off] as u16;
    let b1 = buf[off + 1] as u16;
    b0 | (b1 << 8)
}

/// Reads a `u32` from `buf` at `off` without additional bounds checks.
pub fn u32_at(buf: &[u8], off: usize) -> u32 {
    u16_at(buf, off) as u32 | (u16_at(buf, off + 2) as u32) << 16
}

/// Reads an `i32` from `buf` at `off` without additional bounds checks.
pub fn i32_at(buf: &[u8], off: usize) -> i32 {
    u32_at(buf, off) as i32
}
