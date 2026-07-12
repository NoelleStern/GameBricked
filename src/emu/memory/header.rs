//!
//! Follows https://gbdev.io/pandocs/The_Cartridge_Header.html bit by bit.
//!
//!     There really isn't much to talk about. I think it's a great entry point.
//!     It's easy to follow, fast to implement and it's rewarding 
//!     since you can get some observable results pretty fast.
//!


use color_eyre::eyre;

use crate::emu::{processor::instruction_info::InstInfo, rendering::{palette::RawPalette, ppu::{RawShade, Tile}}};


/// Header read error
const ROM_SHORT_ERROR: &str = "ROM too short for header";
/// Header entry point address
const ENTRY_POINT: usize = 0x0100;
/// Header logo begin address
const LOGO_BEGIN: usize = 0x0104;
/// Header logo end address
const LOGO_END: usize = 0x0133;
/// Header logo size
/// 48B -> 24 tiles, 12 per row. It's also not your usual tile data!
const LOGO_SIZE: usize = LOGO_END-LOGO_BEGIN+1;
/// Main header info begin
const HEADER_BEGIN: usize = 0x0134;
/// Main header info end
const HEADER_END: usize = 0x014C;


pub struct CartridgeType {
    mapper: Mapper,
    sensor: bool,
    rumble: bool,
    timer: bool,
    ram: bool,
    battery: bool,
    description: String,
}
impl CartridgeType {
    pub fn new(mapper: Mapper, sensor: bool, rumble: bool, timer: bool, ram: bool, battery: bool, description: &str) -> Self {
        Self { mapper, ram, battery, timer, rumble, sensor, description: description.to_owned() }
    }
}

pub enum Mapper {
    None,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
    Mbc6,
    Mbc7,
    Mmm01,
    Tama5,
    HuC1,
    HuC3,
    Camera,
    Undefined
}

pub struct Header {
    /// Entry point disassembly
    pub entry: String,
    /// Logo tiles
    pub logo: [Tile; 24],
    /// Game title
    pub title: String,
    /// Manufacturer number
    /// Not really meaningful
    pub manufacturer: [u8; 4],
    /// GameBoy color support
    pub cgb: u8,
    /// Super GameBoy support
    pub sgb: bool,
    /// Contains important mapper info
    pub cartridge_type: CartridgeType,
    /// ROM size in banks (16KB each)
    pub rom_size: u8,
    /// RAM size in KB
    pub ram_size: u8,
    /// Designated region
    pub destination: String,
    /// Who licensed the cart
    pub licensee: String,
    /// Version number
    pub version_number: u8,
    /// Does header checksum match?
    pub header_checksum: bool,
    /// Does global checksum match?
    /// (excluding the checksum number itself)
    pub global_checksum: bool,
}
impl Header {
    pub fn new(buffer: &[u8]) -> eyre::Result<Self> {
        let header = RawHeader::new(buffer)?;
        
        Ok(
            Self { 
                entry:              InstInfo::disassemble(&header.entry.to_vec()),
                logo:               Self::get_logo(header.logo),
                title:              Self::get_title(&header.title)?,
                manufacturer:       header.manufacturer,
                cgb:                header.cgb,
                sgb:                Self::get_sgb(header.sgb),
                cartridge_type:     Self::get_cartridge_type(header.cartridge_type),
                rom_size:           Self::get_rom_size(header.rom_size),
                ram_size:           Self::get_ram_size(header.ram_size),
                destination:        Self::get_destination(header.destination),
                licensee:           Self::get_licensee(header.old_licensee, &header.new_licensee)?,
                version_number:     header.version_number,
                header_checksum:    Self::get_header_checksum(header.header_checksum, buffer),
                global_checksum:    Self::get_global_checksum(&header.global_checksum, buffer),
            }
        )
    }

    pub fn printable_info(&self) -> String {
        let manufacturer_hex = self.manufacturer.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>().join(" ");

        [
            format!("Entry: {}",                self.entry                     ),
            format!("Logo: \r\n{}",             self.printable_logo()          ),
            format!("Title: \"{}\"",            self.title                     ),
            format!("Manufacturer: {}",         manufacturer_hex               ),
            format!("CGB: {:02X}",              self.cgb                       ),
            format!("SGB: {}",                  self.sgb                       ),
            format!("Cartridge Type: {}",       self.cartridge_type.description),
            format!("ROM Size: {} Banks",       self.rom_size as u16 * 2       ), // Convert to u16 just in case
            format!("RAM Size: {} KB",          self.ram_size                  ),
            format!("Destination: \"{}\"",      self.destination               ),
            format!("Licensee: \"{}\"",         self.licensee                  ),
            format!("Version Number: {:02X}",   self.version_number            ),
            format!("Header Checksum: {}",      self.header_checksum           ),
            format!("Global Checksum: {}",      self.global_checksum           ),
        ].join("\n")
    }
    pub fn printable_logo(&self) -> String {
        std::array::from_fn::<String, 8, _>(|y| {
            std::array::from_fn::<&str, 48, _>(|x| {
                let mut j = x/4; if y >= 4 { j += 12; }
                let color = self.logo[j][(y*2)%8][(x*2)%8];
                RawPalette::match_pixel_utf8(color)
            }).join("")
        }).join("\r\n")
    }

    pub fn get_logo(value: [u8; LOGO_SIZE]) -> [Tile; 24] {
        // This is a compressed logo consisting of 4x4 1bit tiles
        let compressed: [[u8; 4]; 24] = std::array::from_fn(|i| {
            let j: usize = i*2; // Since we convert 48 bytes to 24 tiles
            [
                value[j]   >> 4, value[j]   & 0xF,
                value[j+1] >> 4, value[j+1] & 0xF,
            ]
        });

        // This is probably hard to read, but it's not all that difficult
        // Here we're inflating the compressed logo so that we get an array of tiles
        let result: [Tile; 24] = std::array::from_fn(|i: usize| { // All tiles
            std::array::from_fn(|j| { // Individual tile
                let j_index: usize = j/2;
                std::array::from_fn(|k| { // Individual row
                    let value = compressed[i][j_index];
                    let k_index: usize = 3 - (k/2); // Reverse the index here since we go from left to right
                    if ((value >> k_index) & 1) == 0 { RawShade::Zero } else { RawShade::Three }
                })
            })
        });

        result
    }
    pub fn get_title<T: AsRef<[u8]>>(value: &T) -> eyre::Result<String> { 
        let string = String::from_utf8_lossy(value.as_ref());
        let clean: String = string.chars()
            .filter(|c| c.is_ascii() && *c != '\0')
            .collect(); // Sanitizing is important
        Ok(clean)
    }
    pub fn get_sgb(value: u8) -> bool { value == 0x03 }
    pub fn get_cartridge_type(value: u8) -> CartridgeType {
        match value {
            0x00 => CartridgeType::new(Mapper::None,        false, false, false, false, false, "ROM ONLY"                      ),
            0x01 => CartridgeType::new(Mapper::Mbc1,        false, false, false, false, false, "MBC1"                          ),
            0x02 => CartridgeType::new(Mapper::Mbc1,        false, false, false,  true, false, "MBC1+RAM"                      ),
            0x03 => CartridgeType::new(Mapper::Mbc1,        false, false, false,  true,  true, "MBC1+RAM+BATTERY"              ),
            0x05 => CartridgeType::new(Mapper::Mbc2,        false, false, false, false, false, "MBC2"                          ),
            0x06 => CartridgeType::new(Mapper::Mbc2,        false, false, false, false,  true, "MBC2+BATTERY"                  ),
            0x08 => CartridgeType::new(Mapper::None,        false, false, false,  true, false, "ROM+RAM"                       ),
            0x09 => CartridgeType::new(Mapper::None,        false, false, false,  true,  true, "ROM+RAM+BATTERY"               ),
            0x0B => CartridgeType::new(Mapper::Mmm01,       false, false, false, false, false, "MMM01"                         ),
            0x0C => CartridgeType::new(Mapper::Mmm01,       false, false, false,  true, false, "MMM01+RAM"                     ),
            0x0D => CartridgeType::new(Mapper::Mmm01,       false, false, false,  true,  true, "MMM01+RAM+BATTERY"             ),
            0x0F => CartridgeType::new(Mapper::Mbc3,        false, false,  true, false,  true, "MBC3+TIMER+BATTERY"            ),
            0x10 => CartridgeType::new(Mapper::Mbc3,        false, false,  true,  true,  true, "MBC3+TIMER+RAM+BATTERY"        ),
            0x11 => CartridgeType::new(Mapper::Mbc3,        false, false, false, false, false, "MBC3"                          ),
            0x12 => CartridgeType::new(Mapper::Mbc3,        false, false, false,  true, false, "MBC3+RAM"                      ),
            0x13 => CartridgeType::new(Mapper::Mbc3,        false, false, false,  true,  true, "MBC3+RAM+BATTERY"              ),
            0x19 => CartridgeType::new(Mapper::Mbc5,        false, false, false, false, false, "MBC5"                          ),
            0x1A => CartridgeType::new(Mapper::Mbc5,        false, false, false,  true, false, "MBC5+RAM"                      ),
            0x1B => CartridgeType::new(Mapper::Mbc5,        false, false, false,  true,  true, "MBC5+RAM+BATTERY"              ),
            0x1C => CartridgeType::new(Mapper::Mbc5,        false,  true, false, false, false, "MBC5+RUMBLE"                   ),
            0x1D => CartridgeType::new(Mapper::Mbc5,        false,  true, false,  true, false, "MBC5+RUMBLE+RAM"               ),
            0x1E => CartridgeType::new(Mapper::Mbc5,        false,  true, false,  true,  true, "MBC5+RUMBLE+RAM+BATTERY"       ),
            0x20 => CartridgeType::new(Mapper::Mbc6,        false, false, false, false, false, "MBC6"                          ),
            0x22 => CartridgeType::new(Mapper::Mbc7,        true,  true, false,   true,  true, "MBC7+SENSOR+RUMBLE+RAM+BATTERY"),
            0xFC => CartridgeType::new(Mapper::Camera,      false, false, false, false, false, "POCKET CAMERA"                 ),
            0xFD => CartridgeType::new(Mapper::Tama5,       false, false, false, false, false, "BANDAI TAMA5"                  ),
            0xFE => CartridgeType::new(Mapper::HuC3,        false, false, false, false, false, "HuC3"                          ),
            0xFF => CartridgeType::new(Mapper::HuC1,        false, false, false,  true,  true, "HuC1+RAM+BATTERY"              ),
            _ =>    CartridgeType::new(Mapper::Undefined,   false, false, false, false, false, "UNDEFINED"                     ),
        }
    }
    pub fn get_rom_size(value: u8) -> u8 { (1 << value) * 2 }
    pub fn get_ram_size(value: u8) -> u8 {
        match value {
            0x02 => 8, 0x03 => 32,
            0x04 => 128, 0x05 => 64,
            _ => 0
        }
    }
    pub fn get_destination(value: u8) -> String {
        match value {
            0x00 => "Japan/Overseas",
            0x01 => "Overseas Only",
            _ => "Unknown",
        }.to_string()
    }
    pub fn get_old_licensee(value: u8) -> String {
        match value {
            0x01 => "Nintendo",
            0x08 => "Capcom",
            0x09 => "HOT-B",
            0x0A => "Jaleco",
            0x0B => "Coconuts Japan",
            0x0C => "Elite Systems",
            0x13 => "EA (Electronic Arts)",
            0x18 => "Hudson Soft",
            0x19 => "ITC Entertainment",
            0x1A => "Yanoman",
            0x1D => "Japan Clary",
            0x1F => "Virgin Games Ltd.3",
            0x24 => "PCM Complete",
            0x25 => "San-X",
            0x28 => "Kemco",
            0x29 => "SETA Corporation",
            0x30 => "Infogrames5",
            0x31 => "Nintendo",
            0x32 => "Bandai",
            0x33 => "", // Indicates that the New licensee code should be used instead.
            0x34 => "Konami",
            0x35 => "HectorSoft",
            0x38 => "Capcom",
            0x39 => "Banpresto",
            0x3C => "Entertainment Interactive (stub)",
            0x3E => "Gremlin",
            0x41 => "Ubi Soft1",
            0x42 => "Atlus",
            0x44 => "Malibu Interactive",
            0x46 => "Angel",
            0x47 => "Spectrum HoloByte",
            0x49 => "Irem",
            0x4A => "Virgin Games Ltd.3",
            0x4D => "Malibu Interactive",
            0x4F => "U.S. Gold",
            0x50 => "Absolute",
            0x51 => "Acclaim Entertainment",
            0x52 => "Activision",
            0x53 => "Sammy USA Corporation",
            0x54 => "GameTek",
            0x55 => "Park Place15",
            0x56 => "LJN",
            0x57 => "Matchbox",
            0x59 => "Milton Bradley Company",
            0x5A => "Mindscape",
            0x5B => "Romstar",
            0x5C => "Naxat Soft16",
            0x5D => "Tradewest",
            0x60 => "Titus Interactive",
            0x61 => "Virgin Games Ltd.3",
            0x67 => "Ocean Software",
            0x69 => "EA (Electronic Arts)",
            0x6E => "Elite Systems",
            0x6F => "Electro Brain",
            0x70 => "Infogrames5",
            0x71 => "Interplay Entertainment",
            0x72 => "Broderbund",
            0x73 => "Sculptured Software6",
            0x75 => "The Sales Curve Limited7",
            0x78 => "THQ",
            0x79 => "Accolade8",
            0x7A => "Triffix Entertainment",
            0x7C => "MicroProse",
            0x7F => "Kemco",
            0x80 => "Misawa Entertainment",
            0x83 => "LOZC G.",
            0x86 => "Tokuma Shoten",
            0x8B => "Bullet-Proof Software2",
            0x8C => "Vic Tokai Corp.17",
            0x8E => "Ape Inc.18",
            0x8F => "I’Max19",
            0x91 => "Chunsoft Co.9",
            0x92 => "Video System",
            0x93 => "Tsubaraya Productions",
            0x95 => "Varie",
            0x96 => "Yonezawa10/S’Pal",
            0x97 => "Kemco",
            0x99 => "Arc",
            0x9A => "Nihon Bussan",
            0x9B => "Tecmo",
            0x9C => "Imagineer",
            0x9D => "Banpresto",
            0x9F => "Nova",
            0xA1 => "Hori Electric",
            0xA2 => "Bandai",
            0xA4 => "Konami",
            0xA6 => "Kawada",
            0xA7 => "Takara",
            0xA9 => "Technos Japan",
            0xAA => "Broderbund",
            0xAC => "Toei Animation",
            0xAD => "Toho",
            0xAF => "Namco",
            0xB0 => "Acclaim Entertainment",
            0xB1 => "ASCII Corporation or Nexsoft",
            0xB2 => "Bandai",
            0xB4 => "Square Enix",
            0xB6 => "HAL Laboratory",
            0xB7 => "SNK",
            0xB9 => "Pony Canyon",
            0xBA => "Culture Brain",
            0xBB => "Sunsoft",
            0xBD => "Sony Imagesoft",
            0xBF => "Sammy Corporation",
            0xC0 => "Taito",
            0xC2 => "Kemco",
            0xC3 => "Square",
            0xC4 => "Tokuma Shoten",
            0xC5 => "Data East",
            0xC6 => "Tonkin House",
            0xC8 => "Koei",
            0xC9 => "UFL",
            0xCA => "Ultra Games",
            0xCB => "VAP, Inc.",
            0xCC => "Use Corporation",
            0xCD => "Meldac",
            0xCE => "Pony Canyon",
            0xCF => "Angel",
            0xD0 => "Taito",
            0xD1 => "SOFEL (Software Engineering Lab)",
            0xD2 => "Quest",
            0xD3 => "Sigma Enterprises",
            0xD4 => "ASK Kodansha Co.",
            0xD6 => "Naxat Soft16",
            0xD7 => "Copya System",
            0xD9 => "Banpresto",
            0xDA => "Tomy",
            0xDB => "LJN",
            0xDD => "Nippon Computer Systems",
            0xDE => "Human Ent.",
            0xDF => "Altron",
            0xE0 => "Jaleco",
            0xE1 => "Towa Chiki",
            0xE2 => "Yutaka # Needs more info",
            0xE3 => "Varie",
            0xE5 => "Epoch",
            0xE7 => "Athena",
            0xE8 => "Asmik Ace Entertainment",
            0xE9 => "Natsume",
            0xEA => "King Records",
            0xEB => "Atlus",
            0xEC => "Epic/Sony Records",
            0xEE => "IGS",
            0xF0 => "A Wave",
            0xF3 => "Extreme Entertainment",
            0xFF => "LJN",
            _ => "",
        }.to_string()
    }
    pub fn get_new_licensee<T: AsRef<[u8]>>(value: &T) -> eyre::Result<String> {
        let v = str::from_utf8(value.as_ref())?.to_string();
        let result = match v.as_str() {
            "01" => "Nintendo Research & Development 1",
            "08" => "Capcom",
            "13" => "EA (Electronic Arts)",
            "18" => "Hudson Soft",
            "19" => "B-AI",
            "20" => "KSS",
            "22" => "Planning Office WADA",
            "24" => "PCM Complete",
            "25" => "San-X",
            "28" => "Kemco",
            "29" => "SETA Corporation",
            "30" => "Viacom",
            "31" => "Nintendo",
            "32" => "Bandai",
            "33" => "Ocean Software/Acclaim Entertainment",
            "34" => "Konami",
            "35" => "HectorSoft",
            "37" => "Taito",
            "38" => "Hudson Soft",
            "39" => "Banpresto",
            "41" => "Ubi Soft1",
            "42" => "Atlus",
            "44" => "Malibu Interactive",
            "46" => "Angel",
            "47" => "Bullet-Proof Software2",
            "49" => "Irem",
            "50" => "Absolute",
            "51" => "Acclaim Entertainment",
            "52" => "Activision",
            "53" => "Sammy USA Corporation",
            "54" => "Konami",
            "55" => "Hi Tech Expressions",
            "56" => "LJN",
            "57" => "Matchbox",
            "58" => "Mattel",
            "59" => "Milton Bradley Company",
            "60" => "Titus Interactive",
            "61" => "Virgin Games Ltd.3",
            "64" => "Lucasfilm Games4",
            "67" => "Ocean Software",
            "69" => "EA (Electronic Arts)",
            "70" => "Infogrames5",
            "71" => "Interplay Entertainment",
            "72" => "Broderbund",
            "73" => "Sculptured Software6",
            "75" => "The Sales Curve Limited7",
            "78" => "THQ",
            "79" => "Accolade8",
            "80" => "Misawa Entertainment",
            "83" => "LOZC G.",
            "86" => "Tokuma Shoten",
            "87" => "Tsukuda Original",
            "91" => "Chunsoft Co.9",
            "92" => "Video System",
            "93" => "Ocean Software/Acclaim Entertainment",
            "95" => "Varie",
            "96" => "Yonezawa10/S’Pal",
            "97" => "Kaneko",
            "99" => "Pack-In-Video",
            "9H" => "Bottom Up",
            "A4" => "Konami (Yu-Gi-Oh!)",
            "BL" => "MTO",
            "DK" => "Kodansha",
            _ => "",
        };

        Ok(result.to_string())
    }
    pub fn get_licensee<T: AsRef<[u8]>>(old: u8, new: &T) -> eyre::Result<String> {
        Ok(
            if old == 0x33 { 
                Self::get_new_licensee(new)?
            }
            else { 
                let result = Self::get_old_licensee(old);
                if result.as_str() == "" { "Unknown".to_string()  } else { result }
            }
        )
    }
    pub fn get_header_checksum(value: u8, rom: &[u8]) -> bool {
        let mut checksum: u8 = 0;

        #[allow(clippy::needless_range_loop)]
        for addr in HEADER_BEGIN..=HEADER_END {
            checksum = checksum.wrapping_sub(rom[addr]).wrapping_sub(1);
        }

        value == checksum
    }
    pub fn get_global_checksum(value: &[u8; 2], rom: &[u8]) -> bool {
        let val = (value[0] as u16) << 8 | value[1] as u16;
        
        let mut checksum: u16 = 0;
        for  (addr, _item) in rom.iter().enumerate()  {
            if addr == 0x014E || addr == 0x014F { continue; } // Skip the checksum itself
            checksum = checksum.wrapping_add(rom[addr] as u16);
        }

        val == checksum
    }
}

pub struct RawHeader {
    pub entry:              [u8; 4],            // 0x0100..=0x0103
    pub logo:               [u8; LOGO_SIZE],    // 0x0104..=0x0133
    pub title:              [u8; 16],           // 0x0134..=0x0143
    pub manufacturer:       [u8; 4],            // 0x013F..=0x0142
    pub cgb:                u8,                 // 0x0143
    pub new_licensee:       [u8; 2],            // 0x0144..=0x0145
    pub sgb:                u8,                 // 0x0146
    pub cartridge_type:     u8,                 // 0x0147
    pub rom_size:           u8,                 // 0x0148
    pub ram_size:           u8,                 // 0x0149
    pub destination:        u8,                 // 0x014A
    pub old_licensee:       u8,                 // 0x014B
    pub version_number:     u8,                 // 0x014C
    pub header_checksum:    u8,                 // 0x014D
    pub global_checksum:    [u8; 2],            // 0x014E..=0x014F
}
impl RawHeader {
    pub fn new(buffer: &[u8]) -> eyre::Result<Self> {
        Ok(
            Self {
                entry:              Self::get_entry(            buffer)?,
                logo:               Self::get_logo(             buffer)?,
                title:              Self::get_title(            buffer)?,
                manufacturer:       Self::get_manufacturer(     buffer)?,
                cgb:                Self::get_cgb(              buffer)?,
                new_licensee:       Self::get_new_licensee(     buffer)?,
                sgb:                Self::get_sgb(              buffer)?,
                cartridge_type:     Self::get_cartridge_type(   buffer)?,
                rom_size:           Self::get_rom_size(         buffer)?,
                ram_size:           Self::get_ram_size(         buffer)?,
                destination:        Self::get_destination(      buffer)?,
                old_licensee:       Self::get_old_licensee(     buffer)?,
                version_number:     Self::get_version_number(   buffer)?,
                header_checksum:    Self::get_header_checksum(  buffer)?,
                global_checksum:    Self::get_global_checksum(  buffer)?,
            }
        )
    }

    // It's a bit funky, but I think this is easier to read
    pub fn get_entry(           buffer: &[u8]) -> eyre::Result<[u8; 4]>         { Ok( buffer.get(ENTRY_POINT..LOGO_BEGIN).ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?.try_into()?) }
    pub fn get_logo(            buffer: &[u8]) -> eyre::Result<[u8; LOGO_SIZE]> { Ok( buffer.get(LOGO_BEGIN..=LOGO_END)  .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?.try_into()?) }
    pub fn get_title(           buffer: &[u8]) -> eyre::Result<[u8; 16]>        { Ok( buffer.get(HEADER_BEGIN..=0x0143)  .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?.try_into()?) }
    pub fn get_manufacturer(    buffer: &[u8]) -> eyre::Result<[u8; 4]>         { Ok( buffer.get(0x013F..=0x0142)        .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?.try_into()?) }
    pub fn get_cgb(             buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x0143)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_new_licensee(    buffer: &[u8]) -> eyre::Result<[u8; 2]>         { Ok( buffer.get(0x0144..=0x0145)        .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?.try_into()?) }
    pub fn get_sgb(             buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x0146)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_cartridge_type(  buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x0147)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_rom_size(        buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x0148)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_ram_size(        buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x0149)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_destination(     buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x014A)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_old_licensee(    buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x014B)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_version_number(  buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(HEADER_END)             .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_header_checksum( buffer: &[u8]) -> eyre::Result<u8>              { Ok(*buffer.get(0x014D)                 .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?)             }
    pub fn get_global_checksum( buffer: &[u8]) -> eyre::Result<[u8; 2]>         { Ok( buffer.get(0x014E..=0x014F)        .ok_or_else(|| eyre::eyre!(ROM_SHORT_ERROR))?.try_into()?) }
}