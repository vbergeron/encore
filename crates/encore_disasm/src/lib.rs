pub mod tui;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use encore_vm::error::VmError;
use encore_vm::opcode;
use encore_vm::program::Program;
use encore_vm::value::Value;

// --- Intermediate representation ---

pub enum Op {
    Fin,
    Global(u8),
    Capture(u8),
    Local(u8),
    Arg,
    SelfRef,
    Cont,
    Closure { target: u16, ncap: u8 },
    Function { target: u16 },
    Pack { tag: u8 },
    Field(u8),
    Unpack { tag: u8 },
    Match { table: Vec<(u8, u16)> },
    Encore,
    Return,
    Int(i32),
    Int0,
    Int1,
    Int2,
    IntAdd,
    IntSub,
    IntMul,
    IntEq,
    IntLt,
    Unknown(u8),
}

pub struct Instr {
    pub addr: u16,
    pub label: Option<String>,
    pub op: Op,
    pub comment: Option<String>,
}

pub struct Disasm {
    pub arity_table: Vec<(u8, u8)>,
    pub globals: Vec<(usize, String)>,
    pub instructions: Vec<Instr>,
    pub code_len: usize,
    pub ctor_names: BTreeMap<u8, String>,
    pub global_names: BTreeMap<u8, String>,
}

// --- Public API ---

pub fn disasm(bytes: &[u8]) -> Result<String, VmError> {
    Ok(decode(bytes)?.to_string())
}

pub fn disasm_program(prog: &Program) -> String {
    decode_program(prog).to_string()
}

pub fn decode(bytes: &[u8]) -> Result<Disasm, VmError> {
    let prog = Program::parse(bytes)?;
    Ok(decode_program(&prog))
}

pub fn decode_program(prog: &Program) -> Disasm {
    let arity_table: Vec<(u8, u8)> = prog
        .arity_table
        .iter()
        .enumerate()
        .map(|(tag, &arity)| (tag as u8, arity))
        .collect();

    let ctor_names: BTreeMap<u8, String> = prog
        .ctor_names()
        .map(|(tag, name)| (tag, name.to_owned()))
        .collect();

    let global_names: BTreeMap<u8, String> = prog
        .global_names()
        .map(|(idx, name)| (idx, name.to_owned()))
        .collect();

    let globals: Vec<(usize, String)> = (0..prog.n_globals())
        .map(|i| {
            let val_desc = format_value(prog.global(i));
            match global_names.get(&(i as u8)) {
                Some(name) => (i, format!("{name} = {val_desc}")),
                None => (i, val_desc),
            }
        })
        .collect();

    let fn_targets = collect_fn_targets(prog.code);
    let mut instructions = decode_instructions(prog.code);

    for instr in &mut instructions {
        if fn_targets.contains(&instr.addr) {
            instr.label = Some(format!("fn_{:04x}", instr.addr));
        }
        match &instr.op {
            Op::Global(idx) => {
                let idx = *idx;
                if let Some(name) = global_names.get(&idx) {
                    instr.comment = Some(name.clone());
                } else if (idx as usize) < globals.len() {
                    instr.comment = Some(globals[idx as usize].1.clone());
                }
            }
            Op::Pack { tag } => {
                let tag = *tag;
                let mut parts = Vec::new();
                if let Some(name) = ctor_names.get(&tag) {
                    parts.push(name.clone());
                }
                if (tag as usize) < arity_table.len() {
                    parts.push(format!("arity={}", arity_table[tag as usize].1));
                }
                if !parts.is_empty() {
                    instr.comment = Some(parts.join(", "));
                }
            }
            Op::Unpack { tag } => {
                if let Some(name) = ctor_names.get(tag) {
                    instr.comment = Some(name.clone());
                }
            }
            Op::Match { table } => {
                let named: Vec<String> = table.iter()
                    .filter_map(|(tag, _)| ctor_names.get(tag).map(|n| n.clone()))
                    .collect();
                if !named.is_empty() {
                    instr.comment = Some(named.join(" | "));
                }
            }
            _ => {}
        }
    }

    Disasm {
        arity_table,
        globals,
        instructions,
        code_len: prog.code.len(),
        ctor_names,
        global_names,
    }
}

// --- Display ---

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Op::Fin => write!(f, "FIN"),
            Op::Global(i) => write!(f, "GLOBAL g{i}"),
            Op::Capture(i) => write!(f, "CAPTURE {i}"),
            Op::Local(i) => write!(f, "LOCAL {i}"),
            Op::Arg => write!(f, "ARG"),
            Op::SelfRef => write!(f, "SELF"),
            Op::Cont => write!(f, "CONT"),
            Op::Closure { target, ncap } => write!(f, "CLOSURE @{target:04x} ncap={ncap}"),
            Op::Function { target } => write!(f, "FUNCTION @{target:04x}"),
            Op::Pack { tag } => write!(f, "PACK tag={tag}"),
            Op::Field(i) => write!(f, "FIELD {i}"),
            Op::Unpack { tag } => write!(f, "UNPACK tag={tag}"),
            Op::Match { table } => {
                write!(f, "MATCH [")?;
                for (j, &(tag, target)) in table.iter().enumerate() {
                    if j > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{tag}:@{target:04x}")?;
                }
                write!(f, "]")
            }
            Op::Encore => write!(f, "ENCORE"),
            Op::Return => write!(f, "RETURN"),
            Op::Int(n) => write!(f, "INT {n}"),
            Op::Int0 => write!(f, "INT_0"),
            Op::Int1 => write!(f, "INT_1"),
            Op::Int2 => write!(f, "INT_2"),
            Op::IntAdd => write!(f, "INT_ADD"),
            Op::IntSub => write!(f, "INT_SUB"),
            Op::IntMul => write!(f, "INT_MUL"),
            Op::IntEq => write!(f, "INT_EQ"),
            Op::IntLt => write!(f, "INT_LT"),
            Op::Unknown(op) => write!(f, "??? (0x{op:02x})"),
        }
    }
}

impl fmt::Display for Disasm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "--- Arity table ({} entries) ---", self.arity_table.len())?;
        for &(tag, arity) in &self.arity_table {
            if let Some(name) = self.ctor_names.get(&tag) {
                writeln!(f, "  tag {tag}: arity {arity}  ({name})")?;
            } else {
                writeln!(f, "  tag {tag}: arity {arity}")?;
            }
        }

        writeln!(f)?;
        writeln!(f, "--- Globals ({} entries) ---", self.globals.len())?;
        for (idx, desc) in &self.globals {
            writeln!(f, "  g{idx} = {desc}")?;
        }

        writeln!(f)?;
        writeln!(f, "--- Code ({} bytes) ---", self.code_len)?;
        for instr in &self.instructions {
            if let Some(label) = &instr.label {
                writeln!(f)?;
                writeln!(f, "<{label}>:")?;
            }
            let op_str = instr.op.to_string();
            if let Some(comment) = &instr.comment {
                writeln!(f, "{:04x}:  {:<30} ; {comment}", instr.addr, op_str)?;
            } else {
                writeln!(f, "{:04x}:  {}", instr.addr, op_str)?;
            }
        }

        Ok(())
    }
}

// --- Internal helpers ---

fn format_value(val: Value) -> String {
    if val.is_int() {
        format!("int({})", val.int_value())
    } else if val.is_ctor() {
        format!(
            "ctor(tag={}, addr=0x{:04x})",
            val.ctor_tag(),
            val.ctor_addr().raw()
        )
    } else if val.is_closure() && val.closure_ncap() == 0 {
        format!("function(code=@{:04x})", val.closure_addr().raw())
    } else if val.is_closure() {
        format!(
            "closure(ncap={}, addr=0x{:04x})",
            val.closure_ncap(),
            val.closure_addr().raw()
        )
    } else {
        format!("0x{:08x}", val.to_u32())
    }
}

fn collect_fn_targets(code: &[u8]) -> BTreeSet<u16> {
    let mut targets = BTreeSet::new();
    let mut pc = 0;
    while pc < code.len() {
        let op = code[pc];
        pc += 1;
        match op {
            opcode::GLOBAL | opcode::CAPTURE | opcode::LOCAL | opcode::PACK | opcode::FIELD
            | opcode::UNPACK => {
                pc += 1;
            }
            opcode::CLOSURE => {
                let lo = code[pc] as u16;
                let hi = code[pc + 1] as u16;
                targets.insert(lo | (hi << 8));
                pc += 3;
            }
            opcode::FUNCTION => {
                let lo = code[pc] as u16;
                let hi = code[pc + 1] as u16;
                targets.insert(lo | (hi << 8));
                pc += 2;
            }
            opcode::MATCH => {
                let _base = code[pc];
                let n = code[pc + 1];
                pc += 2 + n as usize * 2;
            }
            opcode::INT => {
                pc += 3;
            }
            _ => {}
        }
    }
    targets
}

fn decode_instructions(code: &[u8]) -> Vec<Instr> {
    let mut instrs = Vec::new();
    let mut pc = 0;

    let read_u8 = |pc: &mut usize| -> u8 {
        let b = code[*pc];
        *pc += 1;
        b
    };

    let read_u16 = |pc: &mut usize| -> u16 {
        let lo = code[*pc] as u16;
        let hi = code[*pc + 1] as u16;
        *pc += 2;
        lo | (hi << 8)
    };

    while pc < code.len() {
        let addr = pc as u16;
        let op_byte = read_u8(&mut pc);
        let op = match op_byte {
            opcode::FIN => Op::Fin,
            opcode::GLOBAL => Op::Global(read_u8(&mut pc)),
            opcode::CAPTURE => Op::Capture(read_u8(&mut pc)),
            opcode::LOCAL => Op::Local(read_u8(&mut pc)),
            opcode::ARG => Op::Arg,
            opcode::SELF => Op::SelfRef,
            opcode::CONT => Op::Cont,
            opcode::CLOSURE => {
                let target = read_u16(&mut pc);
                let ncap = read_u8(&mut pc);
                Op::Closure { target, ncap }
            }
            opcode::FUNCTION => Op::Function {
                target: read_u16(&mut pc),
            },
            opcode::PACK => Op::Pack {
                tag: read_u8(&mut pc),
            },
            opcode::FIELD => Op::Field(read_u8(&mut pc)),
            opcode::UNPACK => Op::Unpack { tag: read_u8(&mut pc) },
            opcode::MATCH => {
                let base = read_u8(&mut pc);
                let n = read_u8(&mut pc);
                let mut table = Vec::with_capacity(n as usize);
                for j in 0..n {
                    table.push((base + j, read_u16(&mut pc)));
                }
                Op::Match { table }
            }
            opcode::ENCORE => Op::Encore,
            opcode::RETURN => Op::Return,
            opcode::INT => {
                let b0 = read_u8(&mut pc) as u32;
                let b1 = read_u8(&mut pc) as u32;
                let b2 = read_u8(&mut pc) as u32;
                let raw = b0 | (b1 << 8) | (b2 << 16);
                Op::Int(((raw as i32) << 8) >> 8)
            }
            opcode::INT_0 => Op::Int0,
            opcode::INT_1 => Op::Int1,
            opcode::INT_2 => Op::Int2,
            opcode::INT_ADD => Op::IntAdd,
            opcode::INT_SUB => Op::IntSub,
            opcode::INT_MUL => Op::IntMul,
            opcode::INT_EQ => Op::IntEq,
            opcode::INT_LT => Op::IntLt,
            _ => Op::Unknown(op_byte),
        };
        instrs.push(Instr {
            addr,
            label: None,
            op,
            comment: None,
        });
    }

    instrs
}
