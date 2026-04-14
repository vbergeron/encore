pub mod tui;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use encore_vm::error::VmError;
use encore_vm::opcode;
use encore_vm::program::Program;

fn reg_name(r: u8) -> String {
    match r {
        0 => "SELF".to_string(),
        1 => "CONT".to_string(),
        r if r >= 2 && r <= 9 => format!("A{}", r - 1),
        r if r >= 10 && r <= 19 => format!("X{:02}", r - 9),
        0xFF => "NULL".to_string(),
        r => format!("r{r}"),
    }
}

// --- Intermediate representation ---

pub enum Op {
    Fin { rs: u8 },
    Mov { rd: u8, rs: u8 },
    Capture { rd: u8, idx: u8 },
    Global { rd: u8, idx: u8 },
    Closure { rd: u8, target: u16, ncap: u8, caps: Vec<u8> },
    Function { rd: u8, target: u16 },
    Pack { rd: u8, tag: u8, fields: Vec<u8> },
    Field { rd: u8, rs: u8, idx: u8 },
    Unpack { rd: u8, tag: u8, rs: u8 },
    Match { rs: u8, table: Vec<(u8, u16)> },
    Encore { rf: u8, rk: u8 },
    Int { rd: u8, val: i32 },
    Int0 { rd: u8 },
    Int1 { rd: u8 },
    Int2 { rd: u8 },
    IntAdd { rd: u8, ra: u8, rb: u8 },
    IntSub { rd: u8, ra: u8, rb: u8 },
    IntMul { rd: u8, ra: u8, rb: u8 },
    IntEq { rd: u8, ra: u8, rb: u8 },
    IntLt { rd: u8, ra: u8, rb: u8 },
    Extern { rd: u8, slot: u16, ra: u8 },
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
            let addr = prog.global(i);
            let addr_desc = format!("@{:04x}", addr.raw());
            match global_names.get(&(i as u8)) {
                Some(name) => (i, format!("{name} = {addr_desc}")),
                None => (i, addr_desc),
            }
        })
        .collect();

    let fn_targets = collect_fn_targets(prog.code, &arity_table);
    let match_targets = collect_match_targets(prog.code, &arity_table);

    let mut labels: BTreeMap<u16, String> = BTreeMap::new();
    for i in 0..prog.n_globals() {
        let addr = prog.global(i).raw();
        let name = match global_names.get(&(i as u8)) {
            Some(n) => n.clone(),
            None => format!("g{i}"),
        };
        labels.insert(addr, name);
    }
    for &addr in &fn_targets {
        labels.entry(addr).or_insert_with(|| format!("fn_{addr:04x}"));
    }
    for &addr in &match_targets {
        labels.entry(addr).or_insert_with(|| format!("case_{addr:04x}"));
    }

    let mut instructions = decode_instructions(prog.code, &arity_table);

    for instr in &mut instructions {
        if let Some(label) = labels.get(&instr.addr) {
            instr.label = Some(label.clone());
        }
        match &instr.op {
            Op::Global { idx, .. } => {
                let idx = *idx;
                if let Some(name) = global_names.get(&idx) {
                    instr.comment = Some(name.clone());
                } else if (idx as usize) < globals.len() {
                    instr.comment = Some(globals[idx as usize].1.clone());
                }
            }
            Op::Pack { tag, .. } => {
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
            Op::Unpack { tag, .. } => {
                if let Some(name) = ctor_names.get(tag) {
                    instr.comment = Some(name.clone());
                }
            }
            Op::Match { table, .. } => {
                let branches: Vec<String> = table.iter()
                    .map(|(tag, addr)| {
                        let label = labels.get(addr)
                            .cloned()
                            .unwrap_or_else(|| format!("{addr:04x}"));
                        match ctor_names.get(tag) {
                            Some(name) => format!("{name} -> {label}"),
                            None => format!("{tag} -> {label}"),
                        }
                    })
                    .collect();
                if !branches.is_empty() {
                    instr.comment = Some(branches.join(" | "));
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
            Op::Fin { rs } => write!(f, "FIN {}", reg_name(*rs)),
            Op::Mov { rd, rs } => write!(f, "MOV {}, {}", reg_name(*rd), reg_name(*rs)),
            Op::Capture { rd, idx } => write!(f, "CAPTURE {}, {idx}", reg_name(*rd)),
            Op::Global { rd, idx } => write!(f, "GLOBAL {}, g{idx}", reg_name(*rd)),
            Op::Closure { rd, target, ncap, caps } => {
                write!(f, "CLOSURE {}, @{target:04x}, {ncap}", reg_name(*rd))?;
                for c in caps {
                    write!(f, ", {}", reg_name(*c))?;
                }
                Ok(())
            }
            Op::Function { rd, target } => write!(f, "FUNCTION {}, @{target:04x}", reg_name(*rd)),
            Op::Pack { rd, tag, fields } => {
                write!(f, "PACK {}, tag={tag}", reg_name(*rd))?;
                for fl in fields {
                    write!(f, ", {}", reg_name(*fl))?;
                }
                Ok(())
            }
            Op::Field { rd, rs, idx } => write!(f, "FIELD {}, {}, {idx}", reg_name(*rd), reg_name(*rs)),
            Op::Unpack { rd, tag, rs } => write!(f, "UNPACK {}, tag={tag}, {}", reg_name(*rd), reg_name(*rs)),
            Op::Match { rs, table } => {
                write!(f, "MATCH {} [", reg_name(*rs))?;
                for (j, &(tag, target)) in table.iter().enumerate() {
                    if j > 0 { write!(f, ", ")?; }
                    write!(f, "{tag}:@{target:04x}")?;
                }
                write!(f, "]")
            }
            Op::Encore { rf, rk } => write!(f, "ENCORE {}, {}", reg_name(*rf), reg_name(*rk)),
            Op::Int { rd, val } => write!(f, "INT {}, {val}", reg_name(*rd)),
            Op::Int0 { rd } => write!(f, "INT_0 {}", reg_name(*rd)),
            Op::Int1 { rd } => write!(f, "INT_1 {}", reg_name(*rd)),
            Op::Int2 { rd } => write!(f, "INT_2 {}", reg_name(*rd)),
            Op::IntAdd { rd, ra, rb } => write!(f, "ADD {}, {}, {}", reg_name(*rd), reg_name(*ra), reg_name(*rb)),
            Op::IntSub { rd, ra, rb } => write!(f, "SUB {}, {}, {}", reg_name(*rd), reg_name(*ra), reg_name(*rb)),
            Op::IntMul { rd, ra, rb } => write!(f, "MUL {}, {}, {}", reg_name(*rd), reg_name(*ra), reg_name(*rb)),
            Op::IntEq { rd, ra, rb } => write!(f, "EQ {}, {}, {}", reg_name(*rd), reg_name(*ra), reg_name(*rb)),
            Op::IntLt { rd, ra, rb } => write!(f, "LT {}, {}, {}", reg_name(*rd), reg_name(*ra), reg_name(*rb)),
            Op::Extern { rd, slot, ra } => write!(f, "EXTERN {}, {slot}, {}", reg_name(*rd), reg_name(*ra)),
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
        for (i, instr) in self.instructions.iter().enumerate() {
            if let Some(label) = &instr.label {
                writeln!(f)?;
                writeln!(f, "<{label}>:")?;
            }
            let op_str = instr.op.to_string();
            if let Some(comment) = &instr.comment {
                writeln!(f, "{:04x}:  {:<40} ; {comment}", instr.addr, op_str)?;
            } else {
                writeln!(f, "{:04x}:  {}", instr.addr, op_str)?;
            }
            let is_terminator = matches!(instr.op, Op::Encore { .. } | Op::Fin { .. });
            let next_has_label = self.instructions.get(i + 1).is_some_and(|n| n.label.is_some());
            if is_terminator && !next_has_label && i + 1 < self.instructions.len() {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

// --- Internal helpers ---

fn collect_fn_targets(code: &[u8], arity_table: &[(u8, u8)]) -> BTreeSet<u16> {
    let mut targets = BTreeSet::new();
    let mut pc = 0;
    while pc < code.len() {
        let op = code[pc];
        pc += 1;
        match op {
            opcode::FIN => { pc += 1; }
            opcode::MOV => { pc += 2; }
            opcode::CAPTURE | opcode::GLOBAL => { pc += 2; }
            opcode::CLOSURE => {
                let _rd = code[pc]; pc += 1;
                let lo = code[pc] as u16;
                let hi = code[pc + 1] as u16;
                targets.insert(lo | (hi << 8));
                pc += 2;
                let ncap = code[pc] as usize; pc += 1;
                pc += ncap;
            }
            opcode::FUNCTION => {
                let _rd = code[pc]; pc += 1;
                let lo = code[pc] as u16;
                let hi = code[pc + 1] as u16;
                targets.insert(lo | (hi << 8));
                pc += 2;
            }
            opcode::PACK => {
                let _rd = code[pc]; pc += 1;
                let tag = code[pc]; pc += 1;
                let arity = arity_table.get(tag as usize).map(|a| a.1).unwrap_or(0) as usize;
                pc += arity;
            }
            opcode::FIELD => { pc += 3; }
            opcode::UNPACK => { pc += 3; }
            opcode::MATCH => {
                pc += 1; // rs
                let _base = code[pc]; pc += 1;
                let n = code[pc] as usize; pc += 1;
                pc += n * 2;
            }
            opcode::ENCORE => { pc += 2; }
            opcode::INT => { pc += 4; }
            opcode::INT_0 | opcode::INT_1 | opcode::INT_2 => { pc += 1; }
            opcode::INT_ADD | opcode::INT_SUB | opcode::INT_MUL
            | opcode::INT_EQ | opcode::INT_LT => { pc += 3; }
            opcode::EXTERN => { pc += 4; }
            _ => {}
        }
    }
    targets
}

fn collect_match_targets(code: &[u8], arity_table: &[(u8, u8)]) -> BTreeSet<u16> {
    let mut targets = BTreeSet::new();
    let mut pc = 0;
    while pc < code.len() {
        let op = code[pc];
        pc += 1;
        match op {
            opcode::FIN => { pc += 1; }
            opcode::MOV => { pc += 2; }
            opcode::CAPTURE | opcode::GLOBAL => { pc += 2; }
            opcode::CLOSURE => {
                pc += 1;
                pc += 2;
                let ncap = code[pc] as usize; pc += 1;
                pc += ncap;
            }
            opcode::FUNCTION => { pc += 3; }
            opcode::PACK => {
                pc += 1;
                let tag = code[pc]; pc += 1;
                let arity = arity_table.get(tag as usize).map(|a| a.1).unwrap_or(0) as usize;
                pc += arity;
            }
            opcode::FIELD => { pc += 3; }
            opcode::UNPACK => { pc += 3; }
            opcode::MATCH => {
                pc += 1;
                let _base = code[pc]; pc += 1;
                let n = code[pc] as usize; pc += 1;
                for _ in 0..n {
                    let lo = code[pc] as u16;
                    let hi = code[pc + 1] as u16;
                    targets.insert(lo | (hi << 8));
                    pc += 2;
                }
            }
            opcode::ENCORE => { pc += 2; }
            opcode::INT => { pc += 4; }
            opcode::INT_0 | opcode::INT_1 | opcode::INT_2 => { pc += 1; }
            opcode::INT_ADD | opcode::INT_SUB | opcode::INT_MUL
            | opcode::INT_EQ | opcode::INT_LT => { pc += 3; }
            opcode::EXTERN => { pc += 4; }
            _ => {}
        }
    }
    targets
}

fn decode_instructions(code: &[u8], arity_table: &[(u8, u8)]) -> Vec<Instr> {
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
            opcode::FIN => Op::Fin { rs: read_u8(&mut pc) },
            opcode::MOV => {
                let rd = read_u8(&mut pc);
                let rs = read_u8(&mut pc);
                Op::Mov { rd, rs }
            }
            opcode::CAPTURE => {
                let rd = read_u8(&mut pc);
                let idx = read_u8(&mut pc);
                Op::Capture { rd, idx }
            }
            opcode::GLOBAL => {
                let rd = read_u8(&mut pc);
                let idx = read_u8(&mut pc);
                Op::Global { rd, idx }
            }
            opcode::CLOSURE => {
                let rd = read_u8(&mut pc);
                let target = read_u16(&mut pc);
                let ncap = read_u8(&mut pc);
                let mut caps = Vec::with_capacity(ncap as usize);
                for _ in 0..ncap {
                    caps.push(read_u8(&mut pc));
                }
                Op::Closure { rd, target, ncap, caps }
            }
            opcode::FUNCTION => {
                let rd = read_u8(&mut pc);
                let target = read_u16(&mut pc);
                Op::Function { rd, target }
            }
            opcode::PACK => {
                let rd = read_u8(&mut pc);
                let tag = read_u8(&mut pc);
                let arity = arity_table.get(tag as usize).map(|a| a.1).unwrap_or(0) as usize;
                let mut fields = Vec::with_capacity(arity);
                for _ in 0..arity {
                    fields.push(read_u8(&mut pc));
                }
                Op::Pack { rd, tag, fields }
            }
            opcode::FIELD => {
                let rd = read_u8(&mut pc);
                let rs = read_u8(&mut pc);
                let idx = read_u8(&mut pc);
                Op::Field { rd, rs, idx }
            }
            opcode::UNPACK => {
                let rd = read_u8(&mut pc);
                let tag = read_u8(&mut pc);
                let rs = read_u8(&mut pc);
                Op::Unpack { rd, tag, rs }
            }
            opcode::MATCH => {
                let rs = read_u8(&mut pc);
                let base = read_u8(&mut pc);
                let n = read_u8(&mut pc);
                let mut table = Vec::with_capacity(n as usize);
                for j in 0..n {
                    table.push((base + j, read_u16(&mut pc)));
                }
                Op::Match { rs, table }
            }
            opcode::ENCORE => {
                let rf = read_u8(&mut pc);
                let rk = read_u8(&mut pc);
                Op::Encore { rf, rk }
            }
            opcode::INT => {
                let rd = read_u8(&mut pc);
                let b0 = read_u8(&mut pc) as u32;
                let b1 = read_u8(&mut pc) as u32;
                let b2 = read_u8(&mut pc) as u32;
                let raw = b0 | (b1 << 8) | (b2 << 16);
                Op::Int { rd, val: ((raw as i32) << 8) >> 8 }
            }
            opcode::INT_0 => Op::Int0 { rd: read_u8(&mut pc) },
            opcode::INT_1 => Op::Int1 { rd: read_u8(&mut pc) },
            opcode::INT_2 => Op::Int2 { rd: read_u8(&mut pc) },
            opcode::INT_ADD => {
                let rd = read_u8(&mut pc);
                let ra = read_u8(&mut pc);
                let rb = read_u8(&mut pc);
                Op::IntAdd { rd, ra, rb }
            }
            opcode::INT_SUB => {
                let rd = read_u8(&mut pc);
                let ra = read_u8(&mut pc);
                let rb = read_u8(&mut pc);
                Op::IntSub { rd, ra, rb }
            }
            opcode::INT_MUL => {
                let rd = read_u8(&mut pc);
                let ra = read_u8(&mut pc);
                let rb = read_u8(&mut pc);
                Op::IntMul { rd, ra, rb }
            }
            opcode::INT_EQ => {
                let rd = read_u8(&mut pc);
                let ra = read_u8(&mut pc);
                let rb = read_u8(&mut pc);
                Op::IntEq { rd, ra, rb }
            }
            opcode::INT_LT => {
                let rd = read_u8(&mut pc);
                let ra = read_u8(&mut pc);
                let rb = read_u8(&mut pc);
                Op::IntLt { rd, ra, rb }
            }
            opcode::EXTERN => {
                let rd = read_u8(&mut pc);
                let slot = read_u16(&mut pc);
                let ra = read_u8(&mut pc);
                Op::Extern { rd, slot, ra }
            }
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
