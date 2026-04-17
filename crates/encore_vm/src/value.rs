use crate::error::VmError;

const TYP_CLOS: u32 = 0;
const TYP_CTOR: u32 = 1;
const TYP_HDR: u32 = 2;
const TYP_GC: u32 = 3;
const TYP_INT: u32 = 4;
const TYP_FUNC: u32 = 5;
const TYP_BYTES: u32 = 6;
const TYP_BYTES_HDR: u32 = 7;

const GC_MARK_BIT: u32 = 0x80 << 8;

#[derive(Clone, Copy)]
pub struct HeapAddress(u16);

impl HeapAddress {
    pub const NULL: Self = Self(u16::MAX);

    pub fn new(raw: u16) -> Self { Self(raw) }
    pub fn is_null(self) -> bool { self.0 == u16::MAX }
    pub fn raw(self) -> u16 { self.0 }
}

impl core::ops::Add<usize> for HeapAddress {
    type Output = usize;
    #[inline(always)]
    fn add(self, rhs: usize) -> usize { self.0 as usize + rhs }
}

#[derive(Clone, Copy)]
pub struct Reg(u8);

impl Reg {
    pub const fn new(raw: u8) -> Self { Self(raw) }
    pub fn idx(self) -> usize { self.0 as usize }
    pub fn raw(self) -> u8 { self.0 }
}

impl core::ops::Add<usize> for Reg {
    type Output = Reg;
    #[inline(always)]
    fn add(self, rhs: usize) -> Reg { Reg(self.0 + rhs as u8) }
}

#[derive(Clone, Copy, Debug)]
pub struct CodeAddress(u16);

impl CodeAddress {
    pub fn new(raw: u16) -> Self { Self(raw) }
    pub fn raw(self) -> u16 { self.0 }
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalAddress(u16);

impl GlobalAddress {
    pub const fn new(raw: u16) -> Self { Self(raw) }
    pub fn raw(self) -> u16 { self.0 }
}

/// Packed 32-bit value: [typ:8 | meta:8 | payload:16]
#[derive(Clone, Copy, Debug)]
pub struct Value(u32);

impl Value {
    pub const ZERO: Self = Self(0);

    // -- Constructors --

    pub fn function(code_ptr: CodeAddress) -> Self {
        Self(TYP_FUNC | (code_ptr.raw() as u32) << 16)
    }

    pub const fn function_const(code_ptr_raw: u16) -> Self {
        Self(TYP_FUNC | (code_ptr_raw as u32) << 16)
    }

    pub fn closure(addr: HeapAddress) -> Self {
        Self(TYP_CLOS | (addr.raw() as u32) << 16)
    }

    pub fn ctor(tag: u8, addr: HeapAddress) -> Self {
        Self(TYP_CTOR | (tag as u32) << 8 | (addr.raw() as u32) << 16)
    }

    /// Closure header on heap: [TYP_HDR | env_len | code_ptr]
    pub fn closure_header(env_len: u8, code_ptr: CodeAddress) -> Self {
        Self(TYP_HDR | (env_len as u32) << 8 | (code_ptr.raw() as u32) << 16)
    }

    pub fn int(n: i32) -> Self {
        Self(TYP_INT | ((n as u32 & 0x00FF_FFFF) << 8))
    }

    pub fn bytes(addr: HeapAddress) -> Self {
        Self(TYP_BYTES | (addr.raw() as u32) << 16)
    }

    /// Bytes header on heap: [TYP_BYTES_HDR:8 | byte_len:24]
    pub fn bytes_header(byte_len: usize) -> Self {
        Self(TYP_BYTES_HDR | (byte_len as u32) << 8)
    }

    // -- Type discrimination --

    pub fn is_function(self) -> bool { self.0 & 0xFF == TYP_FUNC }
    pub fn is_closure(self) -> bool { self.0 & 0xFF == TYP_CLOS }
    pub fn is_ctor(self) -> bool { self.0 & 0xFF == TYP_CTOR }
    pub fn is_header(self) -> bool { self.0 & 0xFF == TYP_HDR }
    pub fn is_int(self) -> bool { self.0 & 0xFF == TYP_INT }
    pub fn is_bytes(self) -> bool { self.0 & 0xFF == TYP_BYTES }
    pub fn is_bytes_hdr(self) -> bool { self.0 & 0xFF == TYP_BYTES_HDR }

    // -- Function / closure accessors --

    pub fn code_ptr(self) -> CodeAddress { CodeAddress((self.0 >> 16) as u16) }
    pub fn closure_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    // -- Constructor accessors --

    pub fn ctor_tag(self) -> u8 { (self.0 >> 8) as u8 }
    pub fn ctor_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    // -- Bytes accessors --

    pub fn bytes_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }
    pub fn bytes_hdr_len(self) -> usize { (self.0 >> 8) as usize }

    // -- Type name --

    pub fn type_name(self) -> &'static str {
        match self.0 & 0xFF {
            TYP_CLOS => "closure",
            TYP_CTOR => "constructor",
            TYP_HDR => "header",
            TYP_GC => "gc",
            TYP_INT => "int",
            TYP_FUNC => "function",
            TYP_BYTES => "bytes",
            TYP_BYTES_HDR => "bytes_header",
            _ => "unknown",
        }
    }

    // -- Integer accessors --

    pub fn int_value(self) -> Result<i32, VmError> {
        if self.is_int() {
            Ok((self.0 as i32) >> 8)
        } else {
            Err(VmError::TypeError { expected: "int", got: self.type_name() })
        }
    }

    // -- Header accessors --

    pub fn header_code_ptr(self) -> CodeAddress { CodeAddress((self.0 >> 16) as u16) }
    pub fn header_env_len(self) -> u8 { (self.0 >> 8) as u8 }

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
        const HEAP_BITS: u8 = (1 << TYP_CLOS) | (1 << TYP_CTOR) | (1 << TYP_BYTES);
        let tag = self.0 as u8;
        (1u8 << tag) & HEAP_BITS != 0 && (tag != TYP_CTOR as u8 || !self.heap_addr().is_null())
    }

    pub fn heap_addr(self) -> HeapAddress { HeapAddress((self.0 >> 16) as u16) }

    // -- Raw conversions --

    pub fn to_u32(self) -> u32 { self.0 }
    pub fn from_u32(bits: u32) -> Self { Self(bits) }
}
