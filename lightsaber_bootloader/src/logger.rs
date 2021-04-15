use core::fmt::{
    self,
    Write
};

use crate::{
    FramebufferInformation,
    PixelColourFormat
};

use font8x8::UnicodeFonts;

use spin::{
    Mutex,
    Once
};

pub static LOGGER: Once<MutexedLogger> = Once::new();

pub struct MutexedLogger(Mutex<Logger>);

impl MutexedLogger {
    #[inline(always)]
    pub fn new(inner: Logger) -> Self {
        Self(Mutex::new(inner))
    }
}

pub struct Logger {
    framebuffer: &'static mut [u8],
    information: FramebufferInformation,
    x_pos: usize,
    y_pos: usize
}

impl Logger {
    pub fn new(framebuffer: &'static mut [u8], information: FramebufferInformation) -> Self {
        let this = Self {
            framebuffer,
            information,
            x_pos: 0,
            y_pos: 2,
        };

        this.framebuffer.fill(0);

        this
    }

    pub fn write_str(&mut self, string: &str) {
        for character in string.chars() {
            self.write_character(character);
        }
    }

    pub fn write_character(&mut self, character: char) {
        match character {
            '\n' => self.new_line(),
            '\r' => self.carriage_return(),
            _ => {
                if self.x_pos >= self.width() {
                    self.new_line();
                }

                if self.y_pos >= (self.height() - 8) {
                    self.clear_screen();
                }

                let bytes = font8x8::BASIC_FONTS
                    .get(character)
                    .expect("Character not found in basic font,");

                self.write_bytes(bytes);
            }
        }
    }

    pub fn write_bytes(&mut self, bytes: [u8; 8]) {
        for (y, byte) in bytes.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };

                self.put_pixel(self.x_pos + x, self.y_pos + y, alpha);
            }
        }

        self.x_pos += 8;
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.information.stride + x;

        let colour = match self.information.pixel_colour_format {
            PixelColourFormat::Rgb => [intensity, intensity, intensity / 2, 0],
            PixelColourFormat::Bgr => [intensity / 2, intensity, intensity, 0],
            PixelColourFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
        };

        let bytes_per_pixel = self.information.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&colour[..bytes_per_pixel]);
    }

    #[inline(always)]
    fn width(&self) -> usize {
        self.information.horizontal_resolution
    }

    #[inline(always)]
    fn height(&self) -> usize {
        self.information.vertical_resolution
    }

    #[inline(always)]
    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    fn new_line(&mut self) {
        self.y_pos += 8;

        self.carriage_return();
    }

    fn clear_screen(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;

        self.framebuffer.fill(0);
    }
}

impl log::Log for MutexedLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut this = self.0.lock();

        writeln!(this, "[{}]:      {}", record.level(), record.args())
            .expect("Failed to write to the framebuffer");
    }

    fn flush(&self) { }
}

unsafe impl Send for Logger { }
unsafe impl Sync for Logger { }

impl fmt::Write for Logger {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_str(string);

        Ok(())
    }
}
