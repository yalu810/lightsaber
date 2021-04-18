pub struct ProcessorState {
    pub ax: usize,
    pub bx: usize,
    pub cx: usize,
    pub dx: usize
}

impl ProcessorState {
    pub fn new() -> Self {
        let ax;
        let bx;
        let cx;
        let dx;

        unsafe {
            asm!("
                mov {}, rax
                mov {}, rbx
                mov {}, rcx
                mov {}, rdx
                ",
                out(reg) ax,
                out(reg) bx,
                out(reg) cx,
                out(reg) dx
            )
        }

        Self {
            ax,
            bx,
            cx,
            dx
        }
    }
}
