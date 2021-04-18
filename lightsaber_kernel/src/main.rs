#![no_std]
#![no_main]

#![feature(abi_x86_interrupt)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(lang_items)]
#![feature(panic_info_message)]

extern crate rlibc;

use lightsaber_bootloader::BootInformation;

mod architecture;
mod logger;
mod unwind;
mod renderer;

#[export_name = "_start"]
extern "C" fn lightsaber_kernel_main(boot_information: &'static mut BootInformation) -> ! {
    let framebuffer = &mut boot_information.framebuffer;
    renderer::lightsaber_kernel_initialize_renderer(framebuffer);
    logger::lightsaber_kernel_initialize_logger();

    log::info!("Initialized kernel debug renderer and logger.");

    unsafe {
        architecture::interrupts::lightsaber_kernel_disable_interrupts();

        loop {
            architecture::interrupts::lightsaber_kernel_halt();
        }
    }
}
