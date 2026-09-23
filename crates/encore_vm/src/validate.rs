//! Load-time bytecode validation.
//!
//! The interpreter reads the code stream with unchecked accesses, so it must
//! only ever run code that has passed [`validate`]. A validated program
//! guarantees:
//!
//! - every instruction reachable by falling through from an instruction
//!   boundary decodes to a known opcode whose operands lie inside the code;
//! - the last instruction never falls through past the end of the code
//!   (it is `FIN`, `ENCORE`, `MATCH` or `BRANCH`);
//! - every static jump target (`MATCH` table entries, `BRANCH`,
//!   `CLOSURE`/`FUNCTION` code pointers, global entry points) is inside the
//!   code and lands on an instruction boundary;
//! - `PACK`/`UNPACK` tags index the arity table, and `UNPACK` never writes
//!   past the register file;
//! - `GLOBAL`/`GLOBAL_W` indices are `< n_globals`, and `EXTERN` slots are
//!   `< MAX_EXTERN`.
//!
//! Together with the check in `ENCORE` that a dynamic call target lies inside
//! the code, this means the program counter only ever sits on an
//! instruction boundary of validated code.
//!
//! The pass is `no_std` and allocation-free. Boundary checks collect jump
//! targets into a fixed buffer, sort it, and merge it against a walk over
//! the instruction starts, one walk per full buffer.

use crate::error::VmError;
use crate::opcode;
use crate::program::Program;
use crate::vm::MAX_EXTERN;

/// Largest code the VM can address: code pointers are 16-bit, and 0xFFFF is
/// the `NULL` continuation sentinel, which must never be a valid target.
pub const MAX_CODE_LEN: usize = 0xFFFF;

/// Jump targets checked per walk over the instruction starts.
const PENDING: usize = 128;

fn invalid(pc: usize, reason: &'static str) -> VmError {
    VmError::Invalid { pc: pc as u16, reason }
}

/// Validate `prog`; see the module docs for the guarantees.
pub fn validate(prog: &Program) -> Result<(), VmError> {
    let v = Validator { code: prog.code, arity_table: prog.arity_table, n_globals: prog.n_globals() };
    v.check_header()?;
    v.check_instructions()?;
    v.check_boundaries(prog)
}

struct Validator<'p> {
    code: &'p [u8],
    arity_table: &'p [u8],
    n_globals: usize,
}

/// Decoded shape of one instruction.
struct Insn {
    len: usize,
    /// `false` when execution never continues at `pc + len`.
    falls_through: bool,
}

impl Validator<'_> {
    fn check_header(&self) -> Result<(), VmError> {
        if self.code.len() > MAX_CODE_LEN {
            return Err(invalid(0, "code too long"));
        }
        Ok(())
    }

    /// Decode every instruction once: opcodes, operand extents, operand
    /// ranges, and that the code does not fall off its end.
    fn check_instructions(&self) -> Result<(), VmError> {
        let len = self.code.len();
        let mut pc = 0;
        let mut falls_through = false;
        while pc < len {
            let insn = self.decode(pc, &mut |from, target| {
                if (target as usize) < len { Ok(()) } else { Err(invalid(from, "jump target outside code")) }
            })?;
            falls_through = insn.falls_through;
            pc += insn.len;
        }
        if falls_through {
            return Err(invalid(len, "execution falls off the end of code"));
        }
        Ok(())
    }

    /// Check that every static jump target is an instruction start.
    /// Assumes `check_instructions` succeeded.
    fn check_boundaries(&self, prog: &Program) -> Result<(), VmError> {
        let mut pending = [(0u16, 0u16); PENDING];
        let mut n = 0;
        let mut push = |from: usize, target: u16| -> Result<(), VmError> {
            pending[n] = (target, from as u16);
            n += 1;
            if n == PENDING {
                self.flush(&mut pending)?;
                n = 0;
            }
            Ok(())
        };
        for i in 0..self.n_globals {
            let entry = prog.global(i).raw();
            if entry as usize >= self.code.len() {
                return Err(invalid(entry as usize, "global entry point outside code"));
            }
            push(entry as usize, entry)?;
        }
        let mut pc = 0;
        while pc < self.code.len() {
            pc += self.decode(pc, &mut push)?.len;
        }
        self.flush(&mut pending[..n])
    }

    /// Merge sorted `targets` against the instruction starts.
    fn flush(&self, targets: &mut [(u16, u16)]) -> Result<(), VmError> {
        targets.sort_unstable_by_key(|&(target, _)| target);
        let mut pc = 0;
        for &(target, from) in targets.iter() {
            let target = target as usize;
            while pc < target {
                pc += self.decode(pc, &mut |_, _| Ok(()))?.len;
            }
            if pc != target {
                return Err(invalid(from as usize, "jump target not on an instruction boundary"));
            }
        }
        Ok(())
    }

    fn tag_arity(&self, pc: usize, tag: u8) -> Result<usize, VmError> {
        match self.arity_table.get(tag as usize) {
            Some(&arity) => Ok(arity as usize),
            None => Err(invalid(pc, "constructor tag outside the arity table")),
        }
    }

    /// Decode the instruction at `pc`, reporting each static jump target to
    /// `on_target(pc, target)`.
    fn decode(
        &self,
        pc: usize,
        on_target: &mut dyn FnMut(usize, u16) -> Result<(), VmError>,
    ) -> Result<Insn, VmError> {
        let code = self.code;
        let need = |n: usize| -> Result<(), VmError> {
            if pc + n <= code.len() { Ok(()) } else { Err(invalid(pc, "instruction runs past end of code")) }
        };
        let addr = |off: usize| u16::from_le_bytes([code[pc + off], code[pc + off + 1]]);
        let next = |len: usize| Ok(Insn { len, falls_through: true });
        let last = |len: usize| Ok(Insn { len, falls_through: false });

        let op = code[pc];
        match op {
            opcode::FIN => { need(2)?; last(2) }
            opcode::ENCORE => { need(3)?; last(3) }

            opcode::MOV | opcode::CAPTURE | opcode::INT_BYTE
            | opcode::BYTES_LEN => { need(3)?; next(3) }

            opcode::INT_0 | opcode::INT_1 | opcode::INT_2 => { need(2)?; next(2) }

            opcode::FIELD | opcode::INT_ADD | opcode::INT_SUB | opcode::INT_MUL
            | opcode::INT_EQ | opcode::INT_LT | opcode::INT_LE | opcode::INT_DIV
            | opcode::INT_MOD | opcode::INT_SUB_SAT | opcode::INT_AND | opcode::INT_OR
            | opcode::INT_XOR | opcode::INT_SHL | opcode::INT_SHR | opcode::BYTES_GET
            | opcode::BYTES_CONCAT | opcode::BYTES_EQ => { need(4)?; next(4) }

            opcode::INT | opcode::BYTES_SLICE => { need(5)?; next(5) }

            opcode::GLOBAL => {
                need(3)?;
                if code[pc + 2] as usize >= self.n_globals {
                    return Err(invalid(pc, "global index out of range"));
                }
                next(3)
            }

            opcode::GLOBAL_W => {
                need(4)?;
                if addr(2) as usize >= self.n_globals {
                    return Err(invalid(pc, "global index out of range"));
                }
                next(4)
            }

            opcode::EXTERN => {
                need(5)?;
                if addr(3) as usize >= MAX_EXTERN {
                    return Err(invalid(pc, "extern slot out of range"));
                }
                next(5)
            }

            opcode::FUNCTION => {
                need(4)?;
                on_target(pc, addr(2))?;
                next(4)
            }

            opcode::CLOSURE => {
                need(5)?;
                let len = 5 + code[pc + 4] as usize;
                need(len)?;
                on_target(pc, addr(2))?;
                next(len)
            }

            opcode::PACK => {
                need(3)?;
                let len = 3 + self.tag_arity(pc, code[pc + 2])?;
                need(len)?;
                next(len)
            }

            opcode::UNPACK => {
                need(4)?;
                let arity = self.tag_arity(pc, code[pc + 2])?;
                if code[pc + 1] as usize + arity > 256 {
                    return Err(invalid(pc, "UNPACK writes past the register file"));
                }
                next(4)
            }

            opcode::BYTES => {
                need(3)?;
                let len = 3 + code[pc + 2] as usize;
                need(len)?;
                next(len)
            }

            opcode::MATCH => {
                need(4)?;
                let n = code[pc + 3] as usize;
                let len = 4 + 2 * n;
                need(len)?;
                for i in 0..n {
                    on_target(pc, addr(4 + 2 * i))?;
                }
                last(len)
            }

            opcode::BRANCH => {
                need(7)?;
                on_target(pc, addr(3))?;
                on_target(pc, addr(5))?;
                last(7)
            }

            _ => Err(VmError::InvalidOpcode { opcode: op, pc: pc as u16 }),
        }
    }
}
