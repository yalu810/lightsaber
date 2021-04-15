#![no_std]

pub mod logger;
pub mod paging;

#[repr(C)]
pub struct BootInformation {
    pub framebuffer_information: FramebufferInformation
}

#[derive(Clone)]
#[repr(C)]
pub struct FramebufferInformation {
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub pixel_colour_format: PixelColourFormat,
    pub stride: usize,
    pub bytes_per_pixel: usize,
    pub len_bytes: usize
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PixelColourFormat {
    Rgb,

    Bgr,

    U8
}
