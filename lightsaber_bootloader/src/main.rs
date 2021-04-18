#![no_std]
#![no_main]

#![feature(abi_efiapi)]
#![feature(asm)]
#![feature(maybe_uninit_extra)]
#![feature(maybe_uninit_slice)]
#![feature(never_type)]

extern crate rlibc;

use core::{
    mem,
    slice
};

use uefi::{
    prelude::{
        Boot,
        entry,
        Handle,
        Status,
        SystemTable
    },
    proto::console::gop::{
        GraphicsOutput,
        PixelFormat
    },
    table::{
        cfg,
        boot::{
            MemoryDescriptor,
            MemoryType
        }
    },
    ResultExt
};

use x86_64::PhysAddr;

use lightsaber_graphics::{
    debug::renderer::DebugRenderer,
    FramebufferInformation
};

mod load;
mod logger;
mod paging;
mod unwind;

use crate::{
    load::SystemInformation,
    logger::MutexedLogger,
    paging::BootFrameAllocator
};

pub const PROJECT_LIGHTSABER_SYSTEM_KERNEL_ELF_PATH: &'static str = r"\efi\kernel\lightsaber.elf";

fn lightsaber_initialize_display(system_table: &SystemTable<Boot>) -> (PhysAddr, FramebufferInformation) {
    let graphics_output_protocol = system_table
        .boot_services()
        .locate_protocol::<GraphicsOutput>()
        .expect_success("Failed to locate the Graphics Output Protocol.");

    let graphics_output_protocol = unsafe {
        &mut *graphics_output_protocol.get()
    };

    let mode_information = graphics_output_protocol.current_mode_info();
    let mut framebuffer = graphics_output_protocol.frame_buffer();

    let slice = unsafe {
        slice::from_raw_parts_mut(framebuffer.as_mut_ptr(), framebuffer.size())
    };

    let framebuffer_information = FramebufferInformation {
        len_bytes: framebuffer.size(),
        horiz_resolution: mode_information.resolution().0,
        vert_resolution: mode_information.resolution().1,
        pixel_colour_format: match mode_information.pixel_format() {
            PixelFormat::Rgb => lightsaber_graphics::PixelColourFormat::Rgb,
            PixelFormat::Bgr => lightsaber_graphics::PixelColourFormat::Bgr,
            PixelFormat::Bitmask | PixelFormat::BltOnly => panic!("Bitmask and BitOnly framebuffers are not supported.")
        },
        bytes_per_pixel: 4,
        stride: mode_information.stride()
    };

    let global_logger = MutexedLogger::new(DebugRenderer::new(slice, framebuffer_information));
    let mutexed_logger = logger::LOGGER.call_once(|| global_logger);

    log::set_logger(mutexed_logger).expect("Failed to set global logger.");
    log::set_max_level(log::LevelFilter::Info);

    (PhysAddr::new(framebuffer.as_mut_ptr() as u64), framebuffer_information)
}

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    let (framebuffer_address, framebuffer_info) = lightsaber_initialize_display(&system_table);
    log::info!("Initialized Graphics Output Protocol.");
    log::info!("Using framebuffer at address {:#x}.", framebuffer_address);

    let kernel_bytes= load::lightsaber_load_file(system_table.boot_services(), PROJECT_LIGHTSABER_SYSTEM_KERNEL_ELF_PATH);

    let mmap_storage = {
        let max_mmap_size =
            system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();

        let ptr = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)?
            .log();

        unsafe {
            slice::from_raw_parts_mut(ptr, max_mmap_size)
        }
    };

    log::info!("Exiting boot services.");

    let (system_table, memory_map) = system_table
        .exit_boot_services(image, mmap_storage)
        .expect_success("Failed to exit boot services");

    let mut frame_allocator = BootFrameAllocator::new(memory_map.copied());
    let page_tables = paging::lightsaber_initialize_paging(&mut frame_allocator);

    let mut config_entries = system_table.config_table().iter();

    let rsdp_address = config_entries
        .find(|entry| matches!(entry.guid, cfg::ACPI_GUID | cfg::ACPI2_GUID))
        .map(|entry| PhysAddr::new(entry.address as u64));

    let system_info = SystemInformation {
        framebuffer_address,
        framebuffer_information: framebuffer_info,
        rsdp_address,
    };

    load::lightsaber_load_and_switch_to_system_kernel(frame_allocator, page_tables, kernel_bytes, system_info);
}
