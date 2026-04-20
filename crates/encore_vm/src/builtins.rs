//! Reserved constructor tags recognised by the VM and used by every
//! front-end. Front-ends must pre-register these names so that user-defined
//! constructors start at `FIRST_USER_TAG`.
//!
//!   0 → False
//!   1 → True
//!   2 → Nil
//!   3 → Cons
//!   4 → Pair

pub const TAG_FALSE: u8 = 0;
pub const TAG_TRUE: u8 = 1;
pub const TAG_NIL: u8 = 2;
pub const TAG_CONS: u8 = 3;
pub const TAG_PAIR: u8 = 4;

pub const FIRST_USER_TAG: u8 = 5;

pub const ARITY_FALSE: u8 = 0;
pub const ARITY_TRUE: u8 = 0;
pub const ARITY_NIL: u8 = 0;
pub const ARITY_CONS: u8 = 2;
pub const ARITY_PAIR: u8 = 2;

#[inline]
pub const fn bool_tag(b: bool) -> u8 {
    if b { TAG_TRUE } else { TAG_FALSE }
}
