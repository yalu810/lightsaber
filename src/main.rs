#![no_std]
#![no_main]

#![feature(lang_items)]

use lightsaber_bootloader::BootInformation;

mod panicking;

#[export_name = "_start"]
extern "C" fn lightsaber_system_kernel_main(_boot_info: &'static mut BootInformation) -> ! {
    loop {}
}
