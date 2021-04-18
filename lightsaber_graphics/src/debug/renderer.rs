use core::fmt;

use font8x8::UnicodeFonts;

use crate::{
    debug::colour::{
        Colour,
        ColourCode
    },
    FramebufferInformation
};

pub struct DebugRenderer<'buffer> {
    buffer: &'buffer mut [u8],
    information: FramebufferInformation,
    x_position: usize,
    y_position: usize,
    colour: ColourCode
}

impl<'buffer> DebugRenderer<'buffer> {
    #[inline(always)]
    pub fn new(buffer: &'buffer mut [u8], information: FramebufferInformation) -> Self {
        Self {
            buffer,
            information,
            x_position: 0,
            y_position: 0,
            colour: ColourCode::new(Colour::WHITE, Colour::BLACK)
        }
    }

    pub fn clear_screen(&mut self) {
        self.x_position = 0;
        self.y_position = 0;

        self.buffer.fill(self.colour.background().inner() as u8);
    }

    #[inline(always)]
    pub fn height(&self) -> usize {
        self.information.vert_resolution
    }

    pub fn put_bytes(&mut self, bytes: &[u8]) {
        bytes.iter().enumerate().for_each(|(y, byte)| {
            (0..8).enumerate().for_each(|(x, bit)| {
                if *byte & ( 1 << bit) == 0 {
                    // BACKGROUND
                    self.put_pixel(self.x_position + x, self.y_position + y, self.colour.background());
                }
                else {
                    // FOREGROUND
                    self.put_pixel(self.x_position + x, self.y_position + y, self.colour.foreground());
                }
            });
        });

        self.x_position += 8;
    }

    #[inline(always)]
    pub fn set_colour_code(&mut self, colour: ColourCode) {
        // Do not set the color again if its the same color.
        if colour != self.colour {
            self.colour = colour;
        }
    }

    #[inline(always)]
    pub fn width(&self) -> usize {
        self.information.horiz_resolution
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, colour: Colour) {
        let pixel_offset = y * self.information.stride + x;

        let colour = [
            colour.get_r_bit(),
            colour.get_g_bit(),
            colour.get_b_bit(),
            colour.get_a_bit()
        ];

        let bytes_per_pixel = self.information.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.buffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&colour[..bytes_per_pixel]);
    }

    pub fn write_char(&mut self, r#char: char) {
        match r#char {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            _ => {
                let char_from_basic_font = font8x8::BASIC_FONTS.get(r#char).unwrap();

                if self.x_position >= self.width() {
                    self.newline();
                }

                if self.y_position >= (self.height() - 16) {
                    self.clear_screen();
                }

                self.put_bytes(&char_from_basic_font);
            }
        }
    }

    pub fn write_str(&mut self, r#str: &str) {
        r#str.chars().for_each(|r#char| {
            self.write_char(r#char)
        });
    }

    fn carriage_return(&mut self) {
        self.x_position = 0;
    }

    fn newline(&mut self) {
        self.y_position += 16;
        self.carriage_return();
    }
}

impl<'buffer> fmt::Write for DebugRenderer<'buffer> {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_str(string);
        Ok(())
    }
}

unsafe impl<'buffer> Send for DebugRenderer<'buffer> { }
unsafe impl<'buffer> Sync for DebugRenderer<'buffer> { }
