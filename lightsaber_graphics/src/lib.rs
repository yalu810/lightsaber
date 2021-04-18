#![no_std]

#![feature(const_fn)]

use core::slice;

pub mod debug;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Framebuffer {
    pub buffer_start: u64,
    pub buffer_len_bytes: usize,
    pub information: FramebufferInformation
}

impl Framebuffer {
    pub fn buffer(&self) -> &[u8] {
        unsafe {
            self.create_buffer()
        }
    }

    pub fn buffer_mut(&self) -> &mut [u8] {
        unsafe {
            self.create_buffer()
        }
    }

    pub(in self) unsafe fn create_buffer<'a>(&self) -> &'a mut [u8] {
        slice::from_raw_parts_mut(self.buffer_start as *mut u8, self.buffer_len_bytes)
    }

    pub fn information(&self) -> FramebufferInformation {
        self.information
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FramebufferInformation {
    pub len_bytes: usize,
    pub horiz_resolution: usize,
    pub vert_resolution: usize,
    pub pixel_colour_format: PixelColourFormat,
    pub bytes_per_pixel: usize,
    pub stride: usize
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub enum PixelColourFormat {
    Rgb,

    Bgr,

    U8
}
