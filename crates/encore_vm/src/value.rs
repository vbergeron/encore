const TYP_CLOS: u32 = 0;
const TYP_CTOR: u32 = 1;
const TYP_HDR: u32 = 2;
const TYP_GC: u32 = 3;

const GC_MARK_BIT: u32 = 0x80 << 8;

#[derive(Clone, Copy)]
pub struct HeapAddress(u16);

impl HeapAddress {
    pub const NULL: Self = Self(u16::MAX);

    pub fn new(raw: u16) -> Self { Self(raw) }
    pub fn is_null(self) -> bool { self.0 == u16::MAX }
    pub fn offset(self, off: usize) -> usize { self.0 as usize + off }
    pub fn raw(self) -> u16 { self.0 }
}

#[derive(Clone, Copy)]
pub struct CodeAddress(u16);

impl CodeAddress {
    pub fn new(raw: u16) -> Self { Self(raw) }
    pub fn raw(self) -> u16 { self.0 }
}

/// Packed 32-bit value: [typ:8 | meta:8 | addr:16]
#[derive(Clone, Copy)]
pub struct Value(u32);

impl Value {
    // -- Constructors --

    pub fn closure(ncap: u8, addr: HeapAddress) -> Self {
        Self(TYP_CLOS | (ncap as u32) << 8 | (addr.raw() as u32) << 16)
    }

    pub fn ctor(tag: u8, addr: HeapAddress) -> Self {
        Self(TYP_CTOR | (tag as u32) << 8 | (addr.raw() as u32) << 16)
    }

    pub fn closure_header(code_ptr: CodeAddress) -> Self {
        Self(TYP_HDR | (code_ptr.raw() as u32) << 16)
    }

    // -- Type discrimination --

    pub fn is_closure(self) -> bool { self.0 & 0xFF == TYP_CLOS }
    pub fn is_ctor(self) -> bool { self.0 & 0xFF == TYP_CTOR }
    pub fn is_header(self) -> bool { self.0 & 0xFF == TYP_HDR }

    // -- Closure accessors --

    pub fn closure_ncap(self) -> u8 { (self.0 >> 8) as u8 }
    pub fn closure_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    // -- Constructor accessors --

    pub fn ctor_tag(self) -> u8 { (self.0 >> 8) as u8 }
    pub fn ctor_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    // -- Header accessors --

    pub fn header_code_ptr(self) -> CodeAddress { CodeAddress((self.0 >> 16) as u16) }

    // -- GC header: [TYP_GC:8 | mark:1+size:7 :8 | fwd:16] --

    pub fn gc_header(size: u8) -> Self {
        debug_assert!(size < 128);
        Self(TYP_GC | (size as u32) << 8)
    }

    pub fn is_gc(self) -> bool { self.0 & 0xFF == TYP_GC }

    pub fn gc_size(self) -> u8 { ((self.0 >> 8) as u8) & 0x7F }

    pub fn gc_is_marked(self) -> bool { self.0 & GC_MARK_BIT != 0 }

    pub fn gc_set_mark(self) -> Self { Self(self.0 | GC_MARK_BIT) }

    pub fn gc_fwd(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    pub fn gc_set_fwd(self, addr: HeapAddress) -> Self {
        Self((self.0 & 0xFFFF) | (addr.raw() as u32) << 16)
    }

    // -- Pointer update (rewrite HeapAddress, keep typ+meta) --

    pub fn with_heap_addr(self, new: HeapAddress) -> Self {
        Self((self.0 & 0xFFFF) | (new.raw() as u32) << 16)
    }

    pub fn has_heap_addr(self) -> bool {
        (self.is_closure() || self.is_ctor()) && !self.heap_addr().is_null()
    }

    pub fn heap_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    // -- Raw conversions --

    pub fn to_u32(self) -> u32 { self.0 }
    pub fn from_u32(bits: u32) -> Self { Self(bits) }
}
