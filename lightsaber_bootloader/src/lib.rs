#![no_std]

use core::{
    ops,
    slice
};

use lightsaber_graphics::Framebuffer;

#[derive(Debug)]
#[repr(C)]
pub struct BootInformation {
    pub rsdp_address: u64,
    pub phys_memory_offset: u64,
    pub framebuffer: Framebuffer,
    pub memory_regions: MemoryRegions
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
#[repr(C)]
pub enum MemoryRegionType {
    Usable,

    Bootloader,

    UnknownBios(u32),

    UnknownUefi(u32)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub r#type: MemoryRegionType
}

impl From<MemoryRegions> for &'static mut [MemoryRegion] {
    fn from(regions: MemoryRegions) -> Self {
        unsafe {
            slice::from_raw_parts_mut(regions.ptr, regions.len)
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MemoryRegions {
    pub(in crate) ptr: *mut MemoryRegion,
    pub(in crate) len: usize
}

impl From<&'static mut [MemoryRegion]> for MemoryRegions {
    fn from(regions_slice: &'static mut [MemoryRegion]) -> Self {
        Self {
            ptr: regions_slice.as_mut_ptr(),
            len: regions_slice.len()
        }
    }
}

impl ops::Deref for MemoryRegions {
    type Target = [MemoryRegion];

    fn deref(&self) -> &Self::Target {
        unsafe {
            slice::from_raw_parts(self.ptr, self.len)
        }
    }
}

impl ops::DerefMut for MemoryRegions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            slice::from_raw_parts_mut(self.ptr, self.len)
        }
    }
}
