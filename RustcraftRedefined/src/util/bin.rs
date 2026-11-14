//! Tiny binary reader and writer for save files (little endian, LEB128 variable-length integers).
//! Reads never panic: truncated or corrupt data yields `None`.

#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer { buf: Vec::new() }
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    /// Unsigned LEB128.
    pub fn var(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.buf.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        self.buf.push(v as u8);
    }
    pub fn str(&mut self, s: &str) {
        self.var(s.len() as u64);
        self.buf.extend_from_slice(s.as_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    pub fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.remaining() < n {
            return None;
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(out)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
    pub fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }
    pub fn f64(&mut self) -> Option<f64> {
        self.u64().map(f64::from_bits)
    }
    pub fn var(&mut self) -> Option<u64> {
        let mut out = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            out |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Some(out);
            }
        }
        None
    }
    pub fn str(&mut self) -> Option<String> {
        let n = self.var()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_truncation() {
        let mut w = Writer::new();
        w.u8(7);
        w.u16(0xBEEF);
        w.i32(-5);
        w.u64(u64::MAX - 1);
        w.f32(1.5);
        w.f64(-2.25);
        for v in [0u64, 1, 127, 128, 300, 1 << 40, u64::MAX] {
            w.var(v);
        }
        w.str("hello world");
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.u8(), Some(7));
        assert_eq!(r.u16(), Some(0xBEEF));
        assert_eq!(r.i32(), Some(-5));
        assert_eq!(r.u64(), Some(u64::MAX - 1));
        assert_eq!(r.f32(), Some(1.5));
        assert_eq!(r.f64(), Some(-2.25));
        for v in [0u64, 1, 127, 128, 300, 1 << 40, u64::MAX] {
            assert_eq!(r.var(), Some(v));
        }
        assert_eq!(r.str().as_deref(), Some("hello world"));
        assert_eq!(r.remaining(), 0);
        assert_eq!(r.u8(), None);
        // Truncated data never panics.
        for cut in 0..w.buf.len() {
            let mut r = Reader::new(&w.buf[..cut]);
            let _ = (r.u8(), r.u16(), r.i32(), r.u64(), r.f32(), r.f64(), r.var(), r.str());
        }
    }
}
