use alloc::string::String;
use yane::core::Cartridge;

/// Parse an iNES ROM image and return the PRG and CHR ROM sizes in bytes.
pub fn rom_sizes(bytes: &[u8]) -> Result<(usize, usize), String> {
    let cart = Cartridge::from_ines(bytes, None)?;
    Ok((cart.memory.prg_rom.len(), cart.memory.chr_rom.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_header() {
        let mut rom = vec![0u8; 16 + 0x4000];
        rom[0..4].copy_from_slice(b"NES\x1a");
        rom[4] = 1; // PRG ROM banks
        rom[5] = 0; // CHR ROM banks
        let (prg, chr) = rom_sizes(&rom).unwrap();
        assert_eq!(prg, 0x4000);
        assert_eq!(chr, 0);
    }
}
