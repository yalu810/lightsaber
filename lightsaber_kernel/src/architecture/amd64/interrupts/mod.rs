pub mod exceptions;
pub mod idt;

pub unsafe fn lightsaber_kernel_disable_interrupts() {
    asm!("cli");
}

pub unsafe fn lightsaber_kernel_halt() {
    asm!("hlt", options(nostack, nomem))
}
