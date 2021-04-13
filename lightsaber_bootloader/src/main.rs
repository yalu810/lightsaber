#![no_main]
#![no_std]

#![feature(abi_efiapi)]
#![feature(asm)]
#![feature(maybe_uninit_extra)]
#![feature(never_type)]

extern crate rlibc;

use core::{
    mem::{
        self,
        MaybeUninit
    },
    slice
};

use uefi::{
    prelude::{
        Boot,
        BootServices,
        entry,
        Handle,
        Status,
        SystemTable
    },
    proto::{
        console::gop::GraphicsOutput,
        media::{
            file::{
                File,
                FileAttribute,
                FileMode,
                FileInfo,
                FileSystemVolumeLabel,
                RegularFile
            },
            fs::SimpleFileSystem,
        }
    },
    table::boot::{
        AllocateType,
        MemoryDescriptor,
        MemoryType
    },
    ResultExt
};

use xmas_elf::{
    header,
    program::{
        self,
        ProgramHeader,
        Type
    },
    ElfFile
};

use x86_64::{
    registers,
    structures::paging::{
        page::PageSize,
        FrameAllocator,
        Mapper,
        OffsetPageTable,
        Page,
        PageTableFlags,
        PhysFrame,
        Size4KiB,
        Size2MiB,
    },
    align_up,
    PhysAddr,
    VirtAddr
};

use lightsaber_bootloader::{
    paging::{
        self,
        BootFrameAllocator,
        LevelFourEntries,
        PageTables
    },
    BootInformation,
    FramebufferInformation,
    MemoryRegion
};

const LIGHTSABER_SYSTEM_KERNEL_ELF_PATH: &str = r"\efi\kernel\lightsaber.elf";
const ZERO_PAGE_ARRAY_SIZE_FOUR_KIB: PageArraySize4KiB = [0; Size4KiB::SIZE as usize / 8];

struct LightsaberSystemKernelInformation {
    entry_point: VirtAddr,
    stack_top: VirtAddr
}

type PageArraySize4KiB = [u64; Size4KiB::SIZE as usize / 8];

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize UEFI services.");

    system_table
        .stdout()
        .reset(false)
        .expect_success("Failed to reset stdout.");

    let framebuffer_information = lightsaber_initialize_graphics_output_protocol(&system_table);
    let kernel_bin = lightsaber_load_file(system_table.boot_services(), LIGHTSABER_SYSTEM_KERNEL_ELF_PATH);

    log::info!("Exiting boot services.");

    let buffer_size = system_table.boot_services().memory_map_size() * 2;
    let buffer_ptr = system_table
        .boot_services()
        .allocate_pool(MemoryType::LOADER_DATA, buffer_size)
        .expect_success("Failed to allocate pool.");

    let mmap_buffer = unsafe {
        slice::from_raw_parts_mut(buffer_ptr, buffer_size)
    };

    let (_, mmap) =
        system_table
            .exit_boot_services(image, mmap_buffer)
            .expect_success("Failed to exit boot services.");

    let mut boot_frame_allocator = BootFrameAllocator::new(mmap.copied());
    let mut page_tables = paging::lightsaber_initialize_page_tables(&mut boot_frame_allocator);

    let (kernel_information, mut used_entries) = lightsaber_load_system_kernel(
        kernel_bin,
        &mut boot_frame_allocator,
        &mut page_tables.kernel_page_table,
    );

    let boot_information = BootInformation {
        framebuffer_information
    };
    let new_boot_information = lightsaber_create_boot_information(
        &mut used_entries,
        &mut boot_frame_allocator,
        &mut page_tables,
        boot_information,
    );

    lightsaber_switch_to_system_kernel(kernel_information, new_boot_information, &mut boot_frame_allocator, &mut page_tables);
}

fn lightsaber_create_boot_information<I>(used_entries: &mut LevelFourEntries,
                                         frame_allocator: &mut BootFrameAllocator<I>,
                                         page_tables: &mut PageTables,
                                         boot_info: BootInformation) -> &'static mut BootInformation
    where
        I: ExactSizeIterator<Item = MemoryDescriptor> + Clone {

    let boot_info_start = used_entries.get_free_address();
    let boot_info_end = boot_info_start + mem::size_of::<BootInformation>();

    let mmap_regions_start = boot_info_end.align_up(mem::align_of::<MemoryRegion>() as u64);
    let mmap_regions_end =
        mmap_regions_start + (frame_allocator.len() + 1) * mem::size_of::<MemoryRegion>();

    let start_page = Page::containing_address(boot_info_start);
    let end_page = Page::containing_address(mmap_regions_end - 1u64);

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator.allocate_frame().unwrap();

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();

            page_tables
                .boot_page_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();
        }
    }

    unsafe {
        let boot_info_uninit: &'static mut MaybeUninit<BootInformation> =
            &mut *boot_info_start.as_mut_ptr();

        let memory_regions: &'static mut [MaybeUninit<MemoryRegion>] =
            slice::from_raw_parts_mut(
                mmap_regions_start.as_mut_ptr(),
                frame_allocator.len() + 1,
            );

        let boot_info = boot_info_uninit.write(boot_info);

        boot_info
    }
}

fn lightsaber_initialize_graphics_output_protocol(system_table: &SystemTable<Boot>) -> FramebufferInformation {
    log::info!("Initializing Graphics Output Protocol.");

    let graphics_output_protocol = unsafe {
        &mut *system_table.boot_services()
            .locate_protocol::<GraphicsOutput>()
            .expect_success("Failed to locate Graphics Output Protocol.")
            .get()
    };

    let mode_information = graphics_output_protocol.current_mode_info();
    let (horizontal_resolution, vertical_resolution) = mode_information.resolution();

    FramebufferInformation {
        horizontal_resolution,
        vertical_resolution,
        stride: mode_information.stride()
    }
}

fn lightsaber_load_file(boot_services: &BootServices, path: &str) -> &'static [u8] {
    let mut information_buffer = [0u8; 0x100];

    let filesystem = unsafe {
        &mut *boot_services
            .locate_protocol::<SimpleFileSystem>()
            .expect_success("Failed to locate filesystem.")
            .get()
    };

    let mut filesystem_root = filesystem
        .open_volume()
        .expect_success("Failed to find the filesystem root.");

    let volume_label = filesystem
        .open_volume()
        .expect_success("Could not open volume.")
        .get_info::<FileSystemVolumeLabel>(&mut information_buffer)
        .expect_success("Failed to get filesystem volume label information.")
        .volume_label();

    log::info!("Found volume label: {}", volume_label);

    let file_handle = filesystem_root
        .open(path, FileMode::Read, FileAttribute::empty())
        .expect_success("Failed to open file.");

    let mut regular_file = unsafe {
        RegularFile::new(file_handle)
    };

    log::info!("Loading {} into memory.", path);

    let file_information = regular_file
        .get_info::<FileInfo>(&mut information_buffer)
        .expect_success("Failed to retrieve file information");

    let pages = file_information.file_size() as usize / 0x1000 + 1;
    let memory_start = boot_services
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)
        .expect_success("Failed to allocate pages.");

    let buffer = unsafe {
        slice::from_raw_parts_mut(memory_start as *mut u8, pages * 0x1000)
    };
    let length = regular_file
        .read(buffer)
        .expect_success("Failed to read file.");

    buffer[..length].as_ref()
}

fn lightsaber_load_system_kernel<I>(kernel_bin: &[u8], frame_allocator: &mut BootFrameAllocator<I>, kernel_page_table: &mut OffsetPageTable)
                                    -> (LightsaberSystemKernelInformation, LevelFourEntries)
    where
        I: ExactSizeIterator<Item = MemoryDescriptor> + Clone {

    let kernel_elf = ElfFile::new(&kernel_bin).expect("The Lightsaber System Kernel file is corrupted.");
    let kernel_offset = PhysAddr::new(&kernel_bin[0] as *const u8 as u64);

    assert!(kernel_offset.is_aligned(Size4KiB::SIZE));
    header::sanity_check(&kernel_elf).expect("The Lightsaber System Kernel file failed the header sanity check.");

    for header in kernel_elf.program_iter() {
        program::sanity_check(header, &kernel_elf).expect("The Lightsaber System Kernel file failed the program header sanity check.");

        match header.get_type().expect("Unable to get the header type.") {
            Type::Load => lightsaber_map_segment(&header, kernel_offset, frame_allocator, kernel_page_table),
            _ => (),
        }
    }

    let mut used_entries = LevelFourEntries::new(kernel_elf.program_iter());

    let stack_start_address = used_entries.get_free_address();
    let stack_end_address = stack_start_address + 20 * Size4KiB::SIZE;

    let stack_start: Page = Page::containing_address(stack_start_address);
    let stack_end: Page = Page::containing_address(stack_end_address - 1u64);

    for page in Page::range_inclusive(stack_start, stack_end) {
        let frame = frame_allocator.allocate_frame().unwrap();

        unsafe {
            kernel_page_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();
        }
    }

    let physical_memory_offset = used_entries.get_free_address();

    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let max_physical = frame_allocator.max_physical_address();

    let end_frame: PhysFrame<Size2MiB> = PhysFrame::containing_address(max_physical - 1u64);

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let page =
            Page::containing_address(physical_memory_offset + frame.start_address().as_u64());

        unsafe {
            kernel_page_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .ignore();
        }
    }

    (
        LightsaberSystemKernelInformation {
            entry_point: VirtAddr::new(kernel_elf.header.pt2.entry_point()),
            stack_top: stack_end.start_address()
        },
        used_entries,
    )
}

fn lightsaber_map_segment(segment: &ProgramHeader, kernel_offset: PhysAddr, frame_allocator: &mut impl FrameAllocator<Size4KiB>, page_table: &mut OffsetPageTable) {
    let physical_address = kernel_offset + segment.offset();
    let start_frame: PhysFrame = PhysFrame::containing_address(physical_address);
    let end_frame: PhysFrame = PhysFrame::containing_address(physical_address + segment.file_size() - 1u64);

    let virtual_start = VirtAddr::new(segment.virtual_addr());
    let start_page = Page::containing_address(virtual_start);

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;

    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE;
    }

    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE;
    }

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let offset = frame - start_frame;
        let page = start_page + offset;

        unsafe {
            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)
                .unwrap()
                .ignore();
        }
    }

    if segment.mem_size() > segment.file_size() {
        let zero_start = virtual_start + segment.file_size();
        let zero_end = virtual_start + segment.mem_size();

        if zero_start.as_u64() & 0xfff != 0 {
            let original_frame: PhysFrame = PhysFrame::containing_address(physical_address + segment.file_size() - 1u64);

            let new_frame = frame_allocator.allocate_frame().unwrap();

            let new_frame_ptr = new_frame.start_address().as_u64() as *mut PageArraySize4KiB;
            unsafe {
                new_frame_ptr.write(ZERO_PAGE_ARRAY_SIZE_FOUR_KIB)
            };

            drop(new_frame_ptr);

            let original_bytes_ptr = original_frame.start_address().as_u64() as *mut u8;
            let new_bytes_ptr = new_frame.start_address().as_u64() as *mut u8;

            for offset in 0..((zero_start.as_u64() & 0xfff) as isize) {
                unsafe {
                    let original_byte = original_bytes_ptr.offset(offset).read();
                    new_bytes_ptr.offset(offset).write(original_byte);
                }
            }

            let last_page = Page::containing_address(virtual_start + segment.file_size() - 1u64);

            unsafe {
                page_table.unmap(last_page).unwrap().1.ignore();
                page_table
                    .map_to(last_page, new_frame, page_table_flags, frame_allocator)
                    .unwrap()
                    .ignore();
            }
        }

        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end);

        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator.allocate_frame().unwrap();

            let frame_ptr = frame.start_address().as_u64() as *mut PageArraySize4KiB;
            unsafe { frame_ptr.write(ZERO_PAGE_ARRAY_SIZE_FOUR_KIB) };

            drop(frame_ptr);

            unsafe {
                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)
                    .unwrap()
                    .ignore();
            }
        }
    }
}

fn lightsaber_switch_to_system_kernel(
    kernel_information: LightsaberSystemKernelInformation,
    boot_information: &'static mut BootInformation,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    page_tables: &mut PageTables) -> ! {
    paging::lightsaber_efer_update_no_execute_enable();
    paging::lightsaber_cr0_update_write_protect();

    let current_address = PhysAddr::new(registers::read_rip().as_u64());
    let current_frame: PhysFrame = PhysFrame::containing_address(current_address);

    for frame in PhysFrame::range_inclusive(current_frame, current_frame + 1) {
        unsafe {
            page_tables
                .kernel_page_table
                .identity_map(frame, PageTableFlags::PRESENT, frame_allocator)
                .unwrap()
                .flush();
        }
    }

    unsafe {
        let kernel_level_four_start = page_tables.kernel_level_four_frame.start_address().as_u64();
        let stack_top = kernel_information.stack_top.as_u64();
        let entry_point = kernel_information.entry_point.as_u64();

        asm!("mov cr3, {}", in(reg) kernel_level_four_start);
        asm!("mov rsp, {}", in(reg) stack_top);
        asm!("push 0");
        asm!("jmp {}", in(reg) entry_point, in("rdi") &boot_information as *const _ as usize);
    }

    unreachable!()
}
