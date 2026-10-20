//! Placeholder flat memory: 64K RAM with the system ROM mapped at $D000-$FFFF.
//! Slice 2 replaces this with the real paging (main/AUX/ROM, soft switches); the
//! `read`/`write` boundary is what the board uses.

const ROM_BASE: usize = 0xD000;
const ROM_SIZE: usize = 0x3000;

pub struct Memory {
    ram: Box<[u8; 0x10000]>,
    /// Last 12K of the ROM image ($D000-$FFFF); empty means no ROM (RAM is visible).
    rom: Vec<u8>,
}

impl Memory {
    /// For images larger than 12K (//e 16K, Base64A 48K) only the 12K mapped at $D000 is kept:
    /// the last 12K, except the Base64A where page 0 is used until banking exists.
    pub fn new(rom: &[u8]) -> Self {
        let rom = if rom.len() > ROM_SIZE && rom.len() != 0xC000 {
            rom[rom.len() - ROM_SIZE..].to_vec()
        } else if rom.len() == 0xC000 {
            rom[..ROM_SIZE].to_vec()
        } else {
            rom.to_vec()
        };
        Memory { ram: Box::new([0; 0x10000]), rom }
    }

    pub fn clear_ram(&mut self) {
        self.ram.fill(0);
    }

    pub fn read(&self, addr: u16) -> u8 {
        let a = addr as usize;
        if a >= ROM_BASE && a - ROM_BASE < self.rom.len() {
            self.rom[a - ROM_BASE]
        } else {
            self.ram[a]
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        let a = addr as usize;
        if a < ROM_BASE || a - ROM_BASE >= self.rom.len() {
            self.ram[a] = value;
        }
    }

    pub fn ram_mut(&mut self) -> &mut [u8; 0x10000] {
        &mut self.ram
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_is_read_only_above_d000() {
        let mut rom = vec![0u8; 0x3000];
        rom[0] = 0xAB;
        let mut m = Memory::new(&rom);
        m.write(0xD000, 1);
        m.write(0x1000, 2);
        assert_eq!(m.read(0xD000), 0xAB);
        assert_eq!(m.read(0x1000), 2);
    }

    #[test]
    fn large_images_map_their_top_12k() {
        let mut rom = vec![0u8; 0x4000];
        rom[0x1000] = 0x5A;
        assert_eq!(Memory::new(&rom).read(0xD000), 0x5A);
    }
}
