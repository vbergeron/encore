//! Semantics of the integer opcodes on 24-bit payloads.
//!
//! Integers are 24-bit two's-complement values in `INT_MIN..=INT_MAX`, held
//! in an `i32` (as returned by [`Value::int_value`](crate::value::Value::int_value)).
//! Operations whose exact result can leave that range return `None`, which
//! the VM reports as [`VmError::IntOverflow`](crate::error::VmError::IntOverflow)
//! and the compiler's constant folder leaves unfolded, so both agree.

use crate::value::int_in_range;

/// Number of payload bits in an integer value.
pub const BITS: u32 = 24;

const MASK: u32 = (1 << BITS) - 1;

fn checked(n: Option<i32>) -> Option<i32> {
    n.filter(|&n| int_in_range(n))
}

/// Truncating division, Rocq `Nat.div` convention: `x / 0 = 0`.
/// Overflows only for `INT_MIN / -1`.
#[inline(always)]
pub fn div(a: i32, b: i32) -> Option<i32> {
    if b == 0 { Some(0) } else { checked(a.checked_div(b)) }
}

/// Truncating remainder, Rocq `Nat.modulo` convention: `x mod 0 = x`.
#[inline(always)]
pub fn rem(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { a.wrapping_rem(b) }
}

/// `nat` truncated subtraction: `a - b`, or `0` when `a <= b`.
#[inline(always)]
pub fn sub_sat(a: i32, b: i32) -> Option<i32> {
    if a <= b { Some(0) } else { checked(a.checked_sub(b)) }
}

#[inline(always)]
pub fn and(a: i32, b: i32) -> i32 { a & b }

#[inline(always)]
pub fn or(a: i32, b: i32) -> i32 { a | b }

#[inline(always)]
pub fn xor(a: i32, b: i32) -> i32 { a ^ b }

/// Left shift, `a * 2^b`; overflows when that leaves the 24-bit range.
/// A negative shift amount gives `0`.
#[inline(always)]
pub fn shl(a: i32, b: i32) -> Option<i32> {
    if b < 0 || a == 0 {
        Some(0)
    } else if b >= BITS as i32 {
        None
    } else {
        checked((a as i64 * (1i64 << b)).try_into().ok())
    }
}

/// Logical right shift of the 24-bit payload (zero fill). A shift amount
/// outside `0..24` gives `0`.
#[inline(always)]
pub fn shr(a: i32, b: i32) -> i32 {
    if (b as u32) >= BITS { 0 } else { (((a as u32) & MASK) >> b) as i32 }
}
