//! Semantics of the integer opcodes on 24-bit payloads.
//!
//! Integers are 24-bit two's-complement values held in an `i32` (as returned
//! by [`Value::int_value`](crate::value::Value::int_value)). Every function
//! here returns a value already wrapped back into 24-bit range, so the VM and
//! the compiler's constant folder agree bit for bit.

/// Number of payload bits in an integer value.
pub const BITS: u32 = 24;

const MASK: u32 = (1 << BITS) - 1;

/// Wrap `n` into the signed 24-bit range (sign-extend the low 24 bits).
#[inline(always)]
pub fn wrap(n: i32) -> i32 {
    (n << 8) >> 8
}

/// Truncating division, Rocq `Nat.div` convention: `x / 0 = 0`.
#[inline(always)]
pub fn div(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { wrap(a.wrapping_div(b)) }
}

/// Truncating remainder, Rocq `Nat.modulo` convention: `x mod 0 = x`.
#[inline(always)]
pub fn rem(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { wrap(a.wrapping_rem(b)) }
}

/// `nat` truncated subtraction: `a - b`, or `0` when `a <= b`.
#[inline(always)]
pub fn sub_sat(a: i32, b: i32) -> i32 {
    if a <= b { 0 } else { wrap(a.wrapping_sub(b)) }
}

#[inline(always)]
pub fn and(a: i32, b: i32) -> i32 { a & b }

#[inline(always)]
pub fn or(a: i32, b: i32) -> i32 { a | b }

#[inline(always)]
pub fn xor(a: i32, b: i32) -> i32 { a ^ b }

/// Left shift; bits shifted past bit 23 are lost. A shift amount outside
/// `0..24` gives `0`.
#[inline(always)]
pub fn shl(a: i32, b: i32) -> i32 {
    if (b as u32) >= BITS { 0 } else { wrap(a << b) }
}

/// Logical right shift of the 24-bit payload (zero fill). A shift amount
/// outside `0..24` gives `0`.
#[inline(always)]
pub fn shr(a: i32, b: i32) -> i32 {
    if (b as u32) >= BITS { 0 } else { (((a as u32) & MASK) >> b) as i32 }
}
