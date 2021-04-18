use core::mem;

const GLOBAL_DESCRIPTOR_TABLE_ENTRIES: usize = 6;
static mut GLOBAL_DESCRIPTOR_TABLE: [GdtEntry; GLOBAL_DESCRIPTOR_TABLE_ENTRIES] = [GdtEntry::null(); GLOBAL_DESCRIPTOR_TABLE_ENTRIES];

#[repr(C, packed)]
struct GdtDescriptor {
    size: u16,
    offset: u64
}

impl GdtDescriptor {
    #[inline]
    pub const fn new(size: u16, offset: u64) -> Self {
        Self {
            size,
            offset
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    limit_high_flags: u8,
    base_high: u8
}

impl GdtEntry {
    #[inline]
    pub const fn new(limit_low: u16, base_low: u16, base_middle: u8, access_byte: u8, limit_high_flags: u8, base_high: u8) -> Self {
        Self {
            limit_low,
            base_low,
            base_middle,
            access_byte,
            limit_high_flags,
            base_high
        }
    }

    #[inline]
    const fn null() -> Self {
        Self::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TssEntry {
    previous_tss: u64,
    esp0: u64,
    ss0: u64,
    esp1: u64,
    ss1: u64,
    esp2: u64,
    ss2: u64,
    cr3: u64,
    eip: u64,
    eflags: u64,
    eax: u64,
    ecx: u64,
    edx: u64,
    ebx: u64,
    esp: u64,
    ebp: u64,
    esi: u64,
    edi: u64,
    es: u64,
    cs: u64,
    ss: u64,
    ds: u64,
    fs: u64,
    gs: u64,
    ldt: u64,
    trap: u16,
    iomap_base: u16
}

impl TssEntry {
    pub fn new() -> GdtEntry {
        let this = Self::null();
        let base = (&this as *const Self) as usize;
        let limit = base + mem::size_of::<Self>();

        GdtEntry::new(0, 0, base as u8, limit as u8, 0xE9, 0x00)
    }

    #[inline]
    pub const fn null() -> Self {
        Self {
            previous_tss: 0,
            esp0: 0,
            ss0: 0,
            esp1: 0,
            ss1: 0,
            esp2: 0,
            ss2: 0,
            cr3: 0,
            eip: 0,
            eflags: 0,
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            es: 0,
            cs: 0,
            ss: 0,
            ds: 0,
            fs: 0,
            gs: 0,
            ldt: 0,
            trap: 0,
            iomap_base: mem::size_of::<Self>() as u16
        }
    }
}

pub fn lightsaber_kernel_initialize_global_descriptor_table() {
    unsafe {
        let task_state_segment = TssEntry::new();

        GLOBAL_DESCRIPTOR_TABLE[0] = GdtEntry::new(0, 0, 0, 0x00, 0x00, 0);
        GLOBAL_DESCRIPTOR_TABLE[1] = GdtEntry::new(0, 0, 0, 0x9A, 0xA0, 0);
        GLOBAL_DESCRIPTOR_TABLE[2] = GdtEntry::new(0, 0, 0, 0x92, 0xA0, 0);
        GLOBAL_DESCRIPTOR_TABLE[3] = GdtEntry::new(0, 0, 0, 0xFA, 0xA0, 0);
        GLOBAL_DESCRIPTOR_TABLE[4] = GdtEntry::new(0, 0, 0, 0xF2, 0xA0, 0);
        GLOBAL_DESCRIPTOR_TABLE[5] = task_state_segment;

        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; GLOBAL_DESCRIPTOR_TABLE_ENTRIES]>() - 1) as u16,
            &GLOBAL_DESCRIPTOR_TABLE as *const _ as u64
        );

        lightsaber_kernel_load_global_descriptor_table(&gdt_descriptor as *const _);
        lightsaber_kernel_load_task_state_segment(&task_state_segment as *const _);
    };
}

unsafe fn lightsaber_kernel_load_global_descriptor_table(gdt_descriptor: *const GdtDescriptor) {
    asm!("
        lgdt [rdi]

        mov ax, 0x10

        mov ds, ax
        mov es, ax
        mov fs, ax
        mov gs, ax
        mov ss, ax

        pop rdi

        mov rax, 0x08

        push rax
        push rdi
        ",
        in("rdi") gdt_descriptor
    )
}

unsafe fn lightsaber_kernel_load_task_state_segment(gdt_entry: *const GdtEntry) {
    asm!(
        "ltr [rdi]",
        in("rdi") gdt_entry
    )
}
