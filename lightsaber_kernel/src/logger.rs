use log::{
    Level,
    LevelFilter,
    Metadata,
    Record
};

use lightsaber_graphics::debug::colour::{
    Colour,
    ColourCode
};

use crate::renderer::{
    self,
    print,
    println
};

pub static LOGGER: LightsaberKernelLogger = LightsaberKernelLogger;

pub struct LightsaberKernelLogger;

impl log::Log for LightsaberKernelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::WHITE, Colour::BLACK));
            print!("[ ");

            match record.level() {
                Level::Error => {
                    renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::from_hex(0xFF0000), Colour::BLACK));
                }
                Level::Warn => {
                    renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::from_hex(0xDEDB18), Colour::BLACK));
                }
                Level::Info => {
                    renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::from_hex(0x21AD11), Colour::BLACK));
                }
                Level::Debug => {
                    renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::from_hex(0x116AAD), Colour::BLACK));
                }
                Level::Trace => {
                    renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::from_hex(0x4F524E), Colour::BLACK));
                }
            }

            print!("{}", record.level());

            renderer::lightsaber_kernel_set_colour_code(ColourCode::new(Colour::WHITE, Colour::BLACK));

            println!(" ]    - {}", record.args());
        }
    }

    fn flush(&self) { }
}

pub fn lightsaber_kernel_initialize_logger() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}
