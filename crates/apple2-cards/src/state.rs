//! Tiny byte codec for [`Card::save`](apple2_core::Card::save) / `load` (little-endian).

#[derive(Default)]
pub struct Enc(pub Vec<u8>);

impl Enc {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(v as u8)
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.0.extend(v);
        self
    }
}

pub struct Dec<'a>(&'a [u8]);

impl<'a> Dec<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Dec(b)
    }
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (h, t) = self.0.split_first_chunk::<N>()?;
        self.0 = t;
        Some(*h)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    pub fn bool(&mut self) -> Option<bool> {
        self.u8().map(|v| v != 0)
    }
    pub fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    pub fn bytes(&mut self, out: &mut [u8]) -> Option<()> {
        let (h, t) = self.0.split_at_checked(out.len())?;
        out.copy_from_slice(h);
        self.0 = t;
        Some(())
    }
    /// True when every byte was consumed.
    pub fn done(&self) -> bool {
        self.0.is_empty()
    }
}
