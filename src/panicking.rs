use core::panic::PanicInfo;

#[panic_handler]
pub extern "C" fn rust_begin_unwind(_info: &PanicInfo<'_>) -> ! {
    loop { }
}

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() { }

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop { }
}
