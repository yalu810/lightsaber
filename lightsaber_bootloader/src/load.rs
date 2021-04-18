use core::{
    mem::{
        self,
        MaybeUninit
    },
    slice
};

use uefi::{
    prelude::BootServices,
    proto::{
        media::{
            file::{
                FileAttribute,
                FileInfo,
                FileMode,
                FileSystemVolumeLabel,
                RegularFile
            },
            fs::SimpleFileSystem
        },
    },
    table::boot::{
        AllocateType,
        MemoryType
    },
    ResultExt
};

use x86_64::{
    registers,
    structures::paging::{
        FrameAllocator,
        OffsetPageTable,
        Page,
        PageSize,
        PageTableFlags,
        PageTableIndex,
        PhysFrame,
        Size2MiB,
        Size4KiB
    },
    align_up,
    PhysAddr,
    VirtAddr
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

use lightsaber_bootloader::{
    BootInformation,
    MemoryRegion
};

use lightsaber_graphics::{
    Framebuffer,
    FramebufferInformation
};

use crate::paging::{
    self,
    BootFrameAllocator,
    BootMemoryRegion,
    PageTables,
    ReservedFrames
};
use uefi::proto::media::file::File;
use x86_64::structures::paging::Mapper;

type Size4KiBPageArray = [u64; Size4KiB::SIZE as usize / 8];
const SIZE_4_KIB_ZERO_ARRAY: Size4KiBPageArray = [0; Size4KiB::SIZE as usize / 8];

#[derive(Debug)]
pub struct LevelFourEntries {
    entries: [bool; 512]
}

impl LevelFourEntries {
    fn new<'a>(segments: impl Iterator<Item = ProgramHeader<'a>>) -> Self {
        let mut this = Self {
            entries: [false; 512]
        };

        this.entries[0] = true;

        segments.into_iter().for_each(|segment| {
            let start_page: Page = Page::containing_address(VirtAddr::new(segment.virtual_addr()));
            let end_page: Page = Page::containing_address(VirtAddr::new(
                segment.virtual_addr() + segment.mem_size()
            ));

            (u64::from(start_page.p4_index())..=u64::from(end_page.p4_index())).for_each(|index| {
                this.entries[index as usize] = true;
            })
        });

        this
    }

    fn get_free_entry(&mut self) -> PageTableIndex {
        let (index, entry) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, &mut entry)| !entry)
            .expect("No usable Level Four Entries are found.");

        *entry = true;

        PageTableIndex::new(index as u16)
    }

    fn get_free_address(&mut self) -> VirtAddr {
        Page::from_page_table_indices_1gib(self.get_free_entry(), PageTableIndex::new(0))
            .start_address()
    }
}

pub struct Mappings {
    pub entry_point: VirtAddr,
    pub stack_end: Page,
    pub used_entries: LevelFourEntries,
    pub framebuffer: VirtAddr,
    pub phys_memory_offset: VirtAddr
}

#[derive(Debug, Clone, Copy)]
pub struct SystemInformation {
    pub framebuffer_address: PhysAddr,
    pub framebuffer_information: FramebufferInformation,
    pub rsdp_address: Option<PhysAddr>
}

fn lightsaber_create_boot_information<I, D>(mut frame_allocator: BootFrameAllocator<I, D>, page_tables: &mut PageTables, mappings: &mut Mappings, system_information: SystemInformation) -> (&'static mut BootInformation, ReservedFrames)
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion {
    let (boot_information, memory_regions_) = {
        let boot_info_address = mappings.used_entries.get_free_address();
        let boot_info_end = boot_info_address + mem::size_of::<BootInformation>();

        let memory_map_regions_address = boot_info_end.align_up(mem::align_of::<MemoryRegion>() as u64);
        let regions = frame_allocator.len() + 1;
        let memory_map_regions_end = memory_map_regions_address + regions * mem::size_of::<MemoryRegion>();

        let start_page = Page::containing_address(boot_info_address);
        let end_page = Page::containing_address(memory_map_regions_end - 1u64);

        Page::range_inclusive(start_page, end_page).for_each(|page| {
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            let frame = frame_allocator.allocate_frame().expect("Failed to allocate frames for boot information.");

            unsafe {
                page_tables
                    .kernel_page_table
                    .map_to(page, frame, flags, &mut frame_allocator)
                    .unwrap()
                    .flush();
            };

            unsafe {
                page_tables
                    .boot_page_table
                    .map_to(page, frame, flags, &mut frame_allocator)
                    .unwrap()
                    .flush();
            }
        });

        let boot_info: &'static mut MaybeUninit<BootInformation> = unsafe {
            &mut *boot_info_address.as_mut_ptr()
        };

        let memory_regions: &'static mut [MaybeUninit<MemoryRegion>] = unsafe {
            slice::from_raw_parts_mut(memory_map_regions_address.as_mut_ptr(), regions)
        };

        (boot_info, memory_regions)
    };

    let reserved_frames = ReservedFrames::new(&mut frame_allocator);

    log::info!("Constructing memory map.");
    let memory_regions = frame_allocator.construct_memory_map(memory_regions_);

    log::info!("Creating boot information.");
    let framebuffer = Framebuffer {
        buffer_start: mappings.framebuffer.as_u64(),
        buffer_len_bytes: system_information.framebuffer_information.len_bytes,
        information: system_information.framebuffer_information
    };

    (
        boot_information.write(BootInformation {
            rsdp_address: system_information.rsdp_address.unwrap().as_u64(),
            phys_memory_offset: mappings.phys_memory_offset.as_u64(),
            framebuffer,
            memory_regions: memory_regions.into()
        }),
        reserved_frames
    )
}

pub fn lightsaber_load_and_switch_to_system_kernel<I, D>(mut frame_allocator: BootFrameAllocator<I, D>, mut page_tables: PageTables, kernel_bytes: &[u8], system_information: SystemInformation) -> !
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion {
    let (kernel_entry, used_entries) = lightsaber_load_system_kernel(&mut frame_allocator, &mut page_tables, kernel_bytes);

    let mut mappings = lightsaber_set_up_mappings(
        &mut frame_allocator,
        &mut page_tables,
        system_information,
        kernel_entry,
        used_entries
    );

    let (boot_information, mut reserved_frames) = lightsaber_create_boot_information(
        frame_allocator,
        &mut page_tables,
        &mut mappings,
        system_information
    );

    let current_address = PhysAddr::new(registers::read_rip().as_u64());
    let current_frame: PhysFrame = PhysFrame::containing_address(current_address);

    PhysFrame::range_inclusive(current_frame, current_frame + 1).for_each(|frame| {
        unsafe {
            page_tables
                .kernel_page_table
                .identity_map(frame, PageTableFlags::PRESENT,&mut reserved_frames)
                .unwrap()
                .flush();
        }
    });

    drop(page_tables.kernel_page_table);

    log::info!("Jumping to the Lightsaber System Kernel entry point at {:#x}.", mappings.entry_point);

    unsafe {
        let kernel_level_four_start = page_tables.kernel_level_four_frame.start_address().as_u64();
        let stack_top = mappings.stack_end.start_address().as_u64();
        let entry_point = mappings.entry_point.as_u64();

        asm!("
            mov cr3, {}
            mov rsp, {}
            push 0
            jmp {}",
            in(reg) kernel_level_four_start,
            in(reg) stack_top,
            in(reg) entry_point,
            in("rdi") boot_information as *const _ as usize
        )
    };

    unreachable!()
}

pub fn lightsaber_load_file(boot_services: &BootServices, path: &str) -> &'static [u8] {
    let mut information_buffer = [0u8; 0x100];

    let filesystem = unsafe {
        &mut *boot_services
            .locate_protocol::<SimpleFileSystem>()
            .expect_success("Failed to locate filesystem.")
            .get()
    };

    let mut root = filesystem.open_volume()
        .expect_success("Failed to open volume.");

    let volume_label = filesystem
        .open_volume()
        .expect_success("Failed to open volume.")
        .get_info::<FileSystemVolumeLabel>(&mut information_buffer)
        .expect_success("Failed to get filesystem volume label.")
        .volume_label();

    log::info!("Found volume label: {}.", volume_label);

    let file_handle = root
        .open(path, FileMode::Read, FileAttribute::empty())
        .expect_success("Failed to retrieve file handle.");

    let mut file_handle = unsafe {
        RegularFile::new(file_handle)
    };

    log::info!("Loading file `{}` into memory.", path);

    let file_info = file_handle
        .get_info::<FileInfo>(&mut information_buffer)
        .expect_success("Failed to retrieve file information.");

    let pages = file_info.file_size() as usize / 0x1000 + 1;
    let mem_start = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)
        .expect_success("Failed to allocate pages.");

    let buffer = unsafe {
        slice::from_raw_parts_mut(mem_start as *mut u8, pages * 0x1000)
    };
    let length = file_handle.read(buffer)
        .expect_success("Failed to read file.");

    buffer[..length].as_ref()
}

fn lightsaber_load_system_kernel(frame_allocator: &mut impl FrameAllocator<Size4KiB>, page_tables: &mut PageTables, kernel_bytes: &[u8]) -> (u64, LevelFourEntries) {
    log::info!("Loading the Lightsaber System Kernel.");

    paging::lightsaber_efer_no_execute_enable();
    paging::lightsaber_cr0_write_protect();

    let kernel_elf = ElfFile::new(kernel_bytes).expect("The Lightsaber System Kernel ELF file is corrupted.");
    let kernel_offset = PhysAddr::new(&kernel_bytes[0] as *const u8 as u64);

    assert!(kernel_offset.is_aligned(Size4KiB::SIZE));

    header::sanity_check(&kernel_elf).expect("The Lightsaber System Kernel ELF file failed the header sanity check.");

    let entry_point = kernel_elf.header.pt2.entry_point();
    log::info!("Found the Lightsaber System Kernel entry point at {:#x}.", entry_point);

    kernel_elf.program_iter().for_each(|program_header| {
        program::sanity_check(program_header, &kernel_elf).expect("Failed program header sanity check.");

        match program_header.get_type().expect("Could not get program header type.") {
            Type::Load => lightsaber_map_segment(
                &program_header,
                kernel_offset,
                frame_allocator,
                &mut page_tables.kernel_page_table
            ),
            _ => ()
        }
    });

    let used_entries = LevelFourEntries::new(kernel_elf.program_iter());

    (entry_point, used_entries)
}

fn lightsaber_map_segment(segment: &ProgramHeader, kernel_offset: PhysAddr, frame_allocator: &mut impl FrameAllocator<Size4KiB>, page_table: &mut OffsetPageTable) {
    let physical_address = kernel_offset + segment.offset();
    let start_frame = PhysFrame::containing_address(physical_address);
    let end_frame = PhysFrame::containing_address(physical_address + segment.file_size() - 1u64);

    let virtual_start = VirtAddr::new(segment.virtual_addr());
    let start_page: Page = Page::containing_address(virtual_start);

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;

    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE;
    }

    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE;
    }

    PhysFrame::range_inclusive(start_frame, end_frame).into_iter().for_each(|frame| {
        let offset = frame - start_frame;
        let page = start_page + offset;

        unsafe {
            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)
                .unwrap()
                .ignore();
        }
    });

    if segment.mem_size() > segment.file_size() {
        let zero_start = virtual_start + segment.file_size();
        let zero_end = virtual_start + segment.mem_size();

        if zero_start.as_u64() & 0xFFF != 0 {
            let original_frame: PhysFrame = PhysFrame::containing_address(physical_address + segment.file_size() - 1u64);
            let new_frame = frame_allocator.allocate_frame().unwrap();

            let new_frame_ptr = new_frame.start_address().as_u64() as *mut Size4KiBPageArray;
            unsafe {
                new_frame_ptr.write(SIZE_4_KIB_ZERO_ARRAY)
            };

            drop(new_frame_ptr);

            let original_bytes_ptr = original_frame.start_address().as_u64() as *mut u8;
            let new_bytes_ptr = new_frame.start_address().as_u64() as *mut u8;

            (0..(zero_start.as_u64() & 0xFFF) as isize).for_each(|offset| {
                unsafe {
                    let original_byte = original_bytes_ptr.offset(offset).read();
                    new_bytes_ptr.offset(offset).write(original_byte);
                }
            });

            let last_page: Page = Page::containing_address(virtual_start + segment.file_size() - 1u64);

            unsafe {
                page_table.unmap(last_page).unwrap().1.ignore();
                page_table.map_to(last_page, new_frame, page_table_flags, frame_allocator)
                    .unwrap()
                    .ignore();
            };
        }

        let start_page: Page = Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page: Page = Page::containing_address(zero_end);

        Page::range_inclusive(start_page, end_page).for_each(|page| {
            let frame = frame_allocator.allocate_frame().unwrap();
            let frame_ptr = frame.start_address().as_u64() as *mut Size4KiBPageArray;

            unsafe {
                frame_ptr.write(SIZE_4_KIB_ZERO_ARRAY)
            };

            drop(frame_ptr);

            unsafe {
                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)
                    .unwrap()
                    .ignore();
            }
        });
    }
}

fn lightsaber_set_up_mappings<I, D>(frame_allocator: &mut BootFrameAllocator<I, D>, page_tables: &mut PageTables, system_information: SystemInformation, kernel_entry: u64, mut used_entries: LevelFourEntries) -> Mappings
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion {
    let entry_point = VirtAddr::new(kernel_entry);

    log::info!("Creating a stack for the Lightsaber System Kernel.");

    let stack_start_address = used_entries.get_free_address();
    let stack_start: Page = Page::containing_address(stack_start_address);

    let stack_end_address = stack_start_address + Size4KiB::SIZE * 20;
    let stack_end: Page = Page::containing_address(stack_end_address - 1u64);

    Page::range_inclusive(stack_start, stack_end).for_each(|page| {
        let frame = frame_allocator.allocate_frame().expect("Failed to allocate frame.");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(page, frame, flags, frame_allocator)
                .unwrap()
                .flush();
        }
    });

    log::info!("Mapping the framebuffer.");

    let framebuffer_start_frame: PhysFrame = PhysFrame::containing_address(system_information.framebuffer_address);
    let framebuffer_end_frame: PhysFrame = PhysFrame::containing_address(system_information.framebuffer_address + system_information.framebuffer_information.len_bytes - 1u64);

    let framebuffer_start_page: Page = Page::containing_address(used_entries.get_free_address());

    PhysFrame::range_inclusive(framebuffer_start_frame, framebuffer_end_frame).enumerate().for_each(|(index, frame)| {
        let page = framebuffer_start_page + index as u64;

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(page, frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE, frame_allocator)
                .unwrap()
                .flush();
        }
    });

    let framebuffer = framebuffer_start_page.start_address();
    let physical_memory_offset = used_entries.get_free_address();

    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let max_physical = frame_allocator.max_physical_address();
    let end_frame: PhysFrame<Size2MiB> = PhysFrame::containing_address(max_physical - 1u64);

    PhysFrame::range_inclusive(start_frame, end_frame).for_each(|frame| {
        let page = Page::containing_address(physical_memory_offset + frame.start_address().as_u64());

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(page, frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE, frame_allocator)
                .unwrap()
                .ignore();
        }
    });

    Mappings {
        entry_point,
        stack_end,
        used_entries,
        framebuffer,
        phys_memory_offset: physical_memory_offset
    }
}
