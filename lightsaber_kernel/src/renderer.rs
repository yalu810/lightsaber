use core::fmt::{
    self,
    Write
};

use spin::{
    Mutex,
    Once
};

use lightsaber_graphics::{
    debug::{
        colour::ColourCode,
        renderer::DebugRenderer
    },
    Framebuffer
};

static DEBUG_RENDERER: Once<Mutex<DebugRenderer>> = Once::new();

pub fn lightsaber_kernel_initialize_renderer(framebuffer: &'static mut Framebuffer) {
    let information = framebuffer.information();
    let buffer = framebuffer.buffer_mut();

    let mut renderer = DebugRenderer::new(buffer, information);
    renderer.clear_screen();

    DEBUG_RENDERER.call_once(|| Mutex::new(renderer));
}

pub fn lightsaber_kernel_set_colour_code(colour_code: ColourCode) {
    DEBUG_RENDERER.get().unwrap().lock().set_colour_code(colour_code);
}

pub fn __lightsaber_kernel_print(args: fmt::Arguments) {
    DEBUG_RENDERER.get().unwrap().lock().write_fmt(args).unwrap();
}

pub macro print {
    ($($arg:tt)*) => {
        $crate::renderer::__lightsaber_kernel_print(format_args!($($arg)*));
    }
}

pub macro println {
    ($($arg:tt)*) => {
        $crate::renderer::print!("{}\n", format_args!($($arg)*));
    }
}

pub macro dbg {
    () => {
        $crate::renderer::println!("[{}:{}]", $core::file!(), $core::line!());
    },
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                $crate::renderer::println!("[{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    },
    ($($val:expr),+ $(,)?) => {
        ($($crate::renderer::dbg!($val)),+,)
    }
}
