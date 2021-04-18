use crate::architecture::interrupts::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn lightsaber_kernel_x86_interrupt_division_by_zero(_stack_frame: InterruptStackFrame) {
    panic!("Division by zero. (`DIVISION_BY_ZERO`)");
}

pub extern "x86-interrupt" fn lightsaber_kernel_x86_interrupt_debug(_stack_frame: InterruptStackFrame) {
    panic!("Debug. (`DEBUG`)");
}

pub extern "x86-interrupt" fn lightsaber_kernel_x86_interrupt_non_maskable_interrupts(_stack_frame: InterruptStackFrame) {
    panic!("Non-maskable interrupt. (`NONMASKABLE_INTERRUPT`)");
}

pub extern "x86-interrupt" fn lightsaber_kernel_x86_interrupt_breakpoint(_stack_frame: InterruptStackFrame) {
    panic!("Breakpoint. (`BREAKPOINT`)");
}

pub extern "x86-interrupt" fn lightsaber_kernel_x86_interrupt_overflow(_stack_frame: InterruptStackFrame) {
    panic!("Overflow. (`OVERFLOW`)");
}
