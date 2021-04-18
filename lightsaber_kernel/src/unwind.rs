use core::panic::PanicInfo;

use crate::architecture::interrupts;

#[panic_handler]
pub extern "C" fn rust_begin_unwind(panic_info: &PanicInfo<'_>) -> ! {
    let default_panic_message = &format_args!("");
    let panic_message = panic_info.message().unwrap_or(default_panic_message);

    log::error!("Unexpected Kernel Panic");
    log::error!("{}", panic_info.location().unwrap());
    log::error!("{}", panic_message);

    unsafe {
        interrupts::lightsaber_kernel_disable_interrupts();

        loop {
            interrupts::lightsaber_kernel_halt();
        }
    }

}

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() { }

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {
        unsafe {
            interrupts::lightsaber_kernel_halt();
        }
    }
}
