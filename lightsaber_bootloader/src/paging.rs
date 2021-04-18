use core::{
    fmt,
    mem::MaybeUninit
};

use uefi::table::boot::{
    MemoryDescriptor,
    MemoryType
};

use x86_64::{
    registers::{
        control::{
            Cr0,
            Cr0Flags,
            Cr3,
            Cr3Flags,
        },
        model_specific::{
            Efer,
            EferFlags
        }
    },
    structures::paging::{
        FrameAllocator,
        OffsetPageTable,
        PageSize,
        PageTable,
        PhysFrame,
        Size4KiB
    },
    PhysAddr,
    VirtAddr
};

use lightsaber_bootloader::{
    MemoryRegion,
    MemoryRegionType
};

pub struct BootFrameAllocator<I, D> {
    original: I,
    memory_map: I,
    current_descriptor: Option<D>,
    next_frame: PhysFrame
}

impl<I, D> BootFrameAllocator<I, D>
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion {
    pub fn new(memory_map: I) -> Self {
        let start_frame = PhysFrame::containing_address(PhysAddr::new(0x1000));

        Self {
            original: memory_map.clone(),
            memory_map,
            current_descriptor: None,
            next_frame: start_frame
        }
   }

    pub fn allocate_frame_from_descriptor(&mut self, descriptor: I::Item) -> Option<PhysFrame> {
        let start_address = descriptor.start();
        let start_frame = PhysFrame::containing_address(start_address);
        let end_address = start_address + descriptor.len();
        let end_frame = PhysFrame::containing_address(end_address - 1u64);

        if self.next_frame < start_frame {
            self.next_frame = start_frame;
        }

        if self.next_frame < end_frame {
            let frame = self.next_frame;
            self.next_frame += 1;

            return Some(frame);
        }

        None
    }

    pub fn construct_memory_map(self, regions: &mut [MaybeUninit<MemoryRegion>]) -> &mut [MemoryRegion] {
        let mut next_index = 0;

        for descriptor in self.original {
            let mut start = descriptor.start();
            let end = start + descriptor.len();
            let next_free = self.next_frame.start_address();
            let r#type = match descriptor.region_type() {
                MemoryRegionType::Usable => {
                    if end <= next_free {
                        MemoryRegionType::Bootloader
                    }
                    else if descriptor.start() >= next_free {
                        MemoryRegionType::Usable
                    }
                    else {
                        let used_region = MemoryRegion {
                            start: descriptor.start().as_u64(),
                            end: next_free.as_u64(),
                            r#type: MemoryRegionType::Bootloader
                        };

                        Self::add_region(used_region, regions, &mut next_index)
                            .expect("Failed to add memory region.");

                        start = next_free;
                        MemoryRegionType::Usable
                    }
                }
                MemoryRegionType::UnknownUefi(other) => {
                    match MemoryType(other) {
                        MemoryType::LOADER_CODE
                            | MemoryType::LOADER_DATA
                            | MemoryType::BOOT_SERVICES_CODE
                            | MemoryType::BOOT_SERVICES_DATA
                            | MemoryType::RUNTIME_SERVICES_CODE
                            | MemoryType::RUNTIME_SERVICES_DATA => {
                            MemoryRegionType::Usable
                        }
                        other => MemoryRegionType::UnknownUefi(other.0)
                    }
                }
                other => other
            };

            let region = MemoryRegion {
                start: start.as_u64(),
                end: end.as_u64(),
                r#type
            };

            Self::add_region(region, regions, &mut next_index).unwrap();
        }

        let initialized = &mut regions[..next_index];

        unsafe {
            MaybeUninit::slice_assume_init_mut(initialized)
        }
    }

    pub fn len(&self) -> usize {
        self.original.len()
    }

    pub fn max_physical_address(&self) -> PhysAddr {
        self
            .original
            .clone()
            .map(|r| r.start() + r.len())
            .max()
            .unwrap()
    }

    fn add_region(region: MemoryRegion, regions: &mut [MaybeUninit<MemoryRegion>], next_index: &mut usize) -> Result<(), ()> {
        unsafe {
            regions
                .get_mut(*next_index)
                .ok_or(())?
                .as_mut_ptr()
                .write(region)
        };

        *next_index += 1;
        Ok(())
    }
}

unsafe impl<I, D> FrameAllocator<Size4KiB> for BootFrameAllocator<I, D>
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(current_descriptor) = self.current_descriptor {
            match self.allocate_frame_from_descriptor(current_descriptor) {
                Some(frame) => return Some(frame),
                None => {
                    self.current_descriptor = None;
                }
            }
        }

        while let Some(descriptor) = self.memory_map.next() {
            if descriptor.region_type() != MemoryRegionType::Usable {
                continue;
            }

            if let Some(frame) = self.allocate_frame_from_descriptor(descriptor) {
                self.current_descriptor = Some(descriptor);

                return Some(frame)
            }
        }

        None
    }
}

pub trait BootMemoryRegion: Copy + fmt::Debug {
    fn start(&self) -> PhysAddr;

    fn len(&self) -> u64;

    fn region_type(&self) -> MemoryRegionType;
}

impl BootMemoryRegion for MemoryDescriptor {
    fn start(&self) -> PhysAddr {
        PhysAddr::new(self.phys_start)
    }

    fn len(&self) -> u64 {
        self.page_count * Size4KiB::SIZE
    }

    fn region_type(&self) -> MemoryRegionType {
        match self.ty {
            MemoryType::CONVENTIONAL => MemoryRegionType::Usable,
            other => MemoryRegionType::UnknownUefi(other.0)
        }
    }
}

pub struct PageTables {
    pub boot_page_table: OffsetPageTable<'static>,
    pub kernel_page_table: OffsetPageTable<'static>,
    pub kernel_level_four_frame: PhysFrame
}

pub struct ReservedFrames {
    frames: [Option<PhysFrame>; 2]
}

impl ReservedFrames {
    pub fn new(frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> Self {
        Self {
            frames: [
                Some(frame_allocator.allocate_frame().unwrap()),
                Some(frame_allocator.allocate_frame().unwrap())
            ]
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for ReservedFrames {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.frames.iter_mut().find_map(|frame| frame.take())
    }
}

pub fn lightsaber_efer_no_execute_enable() {
    unsafe {
        Efer::update(|efer_flags| *efer_flags |= EferFlags::NO_EXECUTE_ENABLE);
    };
}

pub fn lightsaber_cr0_write_protect() {
    unsafe {
        Cr0::update(|cr0_flags| *cr0_flags |= Cr0Flags::WRITE_PROTECT);
    };
}

pub fn lightsaber_initialize_paging(frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> PageTables {
    let physical_offset = VirtAddr::new(0x00);

    let old_table = {
        let frame = Cr3::read().0;
        let ptr: *const PageTable = (physical_offset + frame.start_address().as_u64()).as_ptr();

        unsafe {
            &*ptr
        }
    };

    let new_frame = frame_allocator.allocate_frame().unwrap();
    let new_table = {
        let ptr: *mut PageTable = (physical_offset + new_frame.start_address().as_u64()).as_mut_ptr();

        unsafe {
            ptr.write(PageTable::new());

            &mut *ptr
        }
    };

    new_table[0] = old_table[0].clone();

    let boot_page_table = unsafe {
        Cr3::write(new_frame, Cr3Flags::empty());
        OffsetPageTable::new(&mut *new_table, physical_offset)
    };

    let (kernel_page_table, kernel_level_four_frame) = {
        let frame = frame_allocator.allocate_frame().expect("No unused frames.");
        log::info!("Created a new page table for the Lightsaber System Kernel at {:#?}.", &frame);

        let address = physical_offset + frame.start_address().as_u64();

        let ptr = address.as_mut_ptr();

        unsafe {
            *ptr = PageTable::new();
        };

        let level_four_table = unsafe {
            &mut *ptr
        };

        (
            unsafe {
                OffsetPageTable::new(level_four_table, physical_offset)
            },
            frame
        )
    };

    PageTables {
        boot_page_table,
        kernel_page_table,
        kernel_level_four_frame
    }
}
