#![no_std]

pub mod paging;

#[repr(C)]
pub struct BootInformation {
    pub framebuffer_information: FramebufferInformation
}

#[repr(C)]
pub struct FramebufferInformation {
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub stride: usize
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub memory_type: MemoryRegionType,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
#[repr(C)]
pub enum MemoryRegionType {
    Usable,
    Bootloader,
    UnknownUefi(u32),
    UnknownBios(u32),
}
