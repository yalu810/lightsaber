pub mod gdt;
pub mod interrupts;
pub mod processor;

pub mod elf {
    pub use goblin::elf64::*;
}

