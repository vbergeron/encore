pub const FIN: u8 = 0x00;
pub const MOV: u8 = 0x01;
pub const CAPTURE: u8 = 0x02;
pub const GLOBAL: u8 = 0x03;
pub const GLOBAL_W: u8 = 0x04;
pub const CLOSURE: u8 = 0x06;
pub const PACK: u8 = 0x07;
pub const FIELD: u8 = 0x08;
pub const MATCH: u8 = 0x09;
pub const ENCORE: u8 = 0x0A;
pub const BRANCH: u8 = 0x0B;
pub const FUNCTION: u8 = 0x0D;
pub const UNPACK: u8 = 0x0E;

pub const INT: u8 = 0x10;
pub const INT_ADD: u8 = 0x11;
pub const INT_SUB: u8 = 0x12;
pub const INT_MUL: u8 = 0x13;
pub const INT_EQ: u8 = 0x14;
pub const INT_LT: u8 = 0x15;
pub const INT_BYTE: u8 = 0x16;
pub const INT_LE: u8 = 0x17;

pub const INT_0: u8 = 0x18;
pub const INT_1: u8 = 0x19;
pub const INT_2: u8 = 0x1A;
pub const INT_DIV: u8 = 0x1B;
pub const INT_MOD: u8 = 0x1C;
pub const INT_SUB_SAT: u8 = 0x1D;

pub const EXTERN: u8 = 0x20;

pub const INT_AND: u8 = 0x24;
pub const INT_OR: u8 = 0x25;
pub const INT_XOR: u8 = 0x26;
pub const INT_SHL: u8 = 0x27;
pub const INT_SHR: u8 = 0x28;

pub const BYTES: u8 = 0x30;
pub const BYTES_LEN: u8 = 0x31;
pub const BYTES_GET: u8 = 0x32;
pub const BYTES_CONCAT: u8 = 0x33;
pub const BYTES_SLICE: u8 = 0x34;
pub const BYTES_EQ: u8 = 0x35;

pub const NULL: u8 = 0xFF;

/// Opcodes occupy `0x00..OPCODE_SLOTS` (`NULL` is a register operand, not an opcode).
pub const OPCODE_SLOTS: usize = 0x40;

#[cfg(feature = "stats")]
pub fn name(op: u8) -> &'static str {
    match op {
        FIN => "FIN",
        MOV => "MOV",
        CAPTURE => "CAPTURE",
        GLOBAL => "GLOBAL",
        GLOBAL_W => "GLOBAL_W",
        CLOSURE => "CLOSURE",
        PACK => "PACK",
        FIELD => "FIELD",
        MATCH => "MATCH",
        ENCORE => "ENCORE",
        BRANCH => "BRANCH",
        FUNCTION => "FUNCTION",
        UNPACK => "UNPACK",
        INT => "INT",
        INT_ADD => "INT_ADD",
        INT_SUB => "INT_SUB",
        INT_MUL => "INT_MUL",
        INT_EQ => "INT_EQ",
        INT_LT => "INT_LT",
        INT_BYTE => "INT_BYTE",
        INT_LE => "INT_LE",
        INT_0 => "INT_0",
        INT_1 => "INT_1",
        INT_2 => "INT_2",
        INT_DIV => "INT_DIV",
        INT_MOD => "INT_MOD",
        INT_SUB_SAT => "INT_SUB_SAT",
        EXTERN => "EXTERN",
        INT_AND => "INT_AND",
        INT_OR => "INT_OR",
        INT_XOR => "INT_XOR",
        INT_SHL => "INT_SHL",
        INT_SHR => "INT_SHR",
        BYTES => "BYTES",
        BYTES_LEN => "BYTES_LEN",
        BYTES_GET => "BYTES_GET",
        BYTES_CONCAT => "BYTES_CONCAT",
        BYTES_SLICE => "BYTES_SLICE",
        BYTES_EQ => "BYTES_EQ",
        NULL => "NULL",
        _ => "?",
    }
}
