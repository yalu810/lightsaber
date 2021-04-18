use bit_field::BitField;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct Colour(u32);

impl Colour {
    pub const BLACK: Colour = Self::from_hex(0x000000);
    pub const WHITE: Colour = Self::from_hex(0xFFFFFF);

    #[inline(always)]
    pub const fn from_hex(value: u32) -> Self {
        Self(value)
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let mut hex_value = 0u32;

        hex_value
            .set_bits(0..8, r as u32)
            .set_bits(8..16, g as u32)
            .set_bits(16..24, b as u32)
            .set_bits(24..32, a as u32);

        Self::from_hex(hex_value)
    }

    #[inline(always)]
    pub const fn inner(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub fn get_r_bit(&self) -> u8 {
        (self.0.get_bits(0..8) & 255) as u8
    }

    #[inline(always)]
    pub fn get_g_bit(&self) -> u8 {
        (self.0.get_bits(8..16) & 255) as u8
    }

    #[inline(always)]
    pub fn get_b_bit(&self) -> u8 {
        (self.0.get_bits(16..24) & 255) as u8
    }

    #[inline(always)]
    pub fn get_a_bit(&self) -> u8 {
        (self.0.get_bits(24..32) & 255) as u8
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ColourCode(Colour, Colour);

impl ColourCode {
    #[inline(always)]
    pub fn new(foreground: Colour, background: Colour) -> Self {
        Self(foreground, background)
    }

    #[inline(always)]
    pub fn foreground(&self) -> Colour {
        self.0
    }

    #[inline(always)]
    pub fn background(&self) -> Colour {
        self.1
    }
}
