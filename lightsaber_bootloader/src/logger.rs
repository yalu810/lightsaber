use core::fmt::Write;

use log::{
    Level,
    Metadata,
    Record
};

use spin::{
    Mutex,
    Once
};

use lightsaber_graphics::debug::{
    colour::{
        Colour,
        ColourCode
    },
    renderer::DebugRenderer
};

pub static LOGGER: Once<MutexedLogger> = Once::new();

pub struct MutexedLogger<'buffer>(Mutex<DebugRenderer<'buffer>>);

impl<'buffer> MutexedLogger<'buffer> {
    #[inline(always)]
    pub fn new(mut inner: DebugRenderer<'buffer>) -> Self {
        inner.clear_screen();

        Self(Mutex::new(inner))
    }
}

impl<'buffer> log::Log for MutexedLogger<'buffer> {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let this = &mut *self.0.lock();
            this.set_colour_code(ColourCode::new(Colour::WHITE, Colour::BLACK));

            write!(this, "[ ").expect("Failed to write to the framebuffer.");

            match record.level() {
                Level::Error => {
                    this.set_colour_code(ColourCode::new(Colour::from_hex(0xFF0000), Colour::BLACK));
                }
                Level::Warn => {
                    this.set_colour_code(ColourCode::new(Colour::from_hex(0xDEDB18), Colour::BLACK));
                }
                Level::Info => {
                    this.set_colour_code(ColourCode::new(Colour::from_hex(0x21AD11), Colour::BLACK));
                }
                Level::Debug => {
                    this.set_colour_code(ColourCode::new(Colour::from_hex(0x116AAD), Colour::BLACK));
                }
                Level::Trace => {
                    this.set_colour_code(ColourCode::new(Colour::from_hex(0x4F524E), Colour::BLACK));
                }
            }

            write!(this, "{}", record.level()).expect("Failed to write to the framebuffer.");

            this.set_colour_code(ColourCode::new(Colour::WHITE, Colour::BLACK));

            writeln!(this, " ]    - {}", record.args()).expect("Failed to write to the framebuffer.");
        }
    }

    fn flush(&self) { }
}
