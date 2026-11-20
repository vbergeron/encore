use std::path::Path;

pub use crate::error::CompileError;
use crate::ir::{asm, cps, ds, dsi, text::TextError};
use crate::pass::{asm_emit::{self, Emitter, Metadata}, asm_peephole, asm_resolve, cps_optimize::{self, OptimizeConfig}, cps_transform, ds_uncurry, dsi_resolve};

/// A point in the pipeline, named after the pass that produced it
/// (`Parse` is the frontend output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Parse,
    DsUncurry,
    DsiResolve,
    CpsTransform,
    CpsOptimize,
    AsmResolve,
    AsmPeephole,
}

/// Which IR a stage's output is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrKind {
    Ds,
    Dsi,
    Cps,
    Asm,
}

impl Stage {
    pub const ALL: [Stage; 7] = [
        Stage::Parse,
        Stage::DsUncurry,
        Stage::DsiResolve,
        Stage::CpsTransform,
        Stage::CpsOptimize,
        Stage::AsmResolve,
        Stage::AsmPeephole,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Parse => "parse",
            Stage::DsUncurry => "ds-uncurry",
            Stage::DsiResolve => "dsi-resolve",
            Stage::CpsTransform => "cps-transform",
            Stage::CpsOptimize => "cps-optimize",
            Stage::AsmResolve => "asm-resolve",
            Stage::AsmPeephole => "asm-peephole",
        }
    }

    pub fn ir(self) -> IrKind {
        match self {
            Stage::Parse | Stage::DsUncurry => IrKind::Ds,
            Stage::DsiResolve => IrKind::Dsi,
            Stage::CpsTransform | Stage::CpsOptimize => IrKind::Cps,
            Stage::AsmResolve | Stage::AsmPeephole => IrKind::Asm,
        }
    }
}

impl std::str::FromStr for Stage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Stage::ALL.into_iter().find(|st| st.name() == s).ok_or_else(|| {
            let names: Vec<&str> = Stage::ALL.iter().map(|st| st.name()).collect();
            format!("unknown stage '{s}', expected one of: {}", names.join(", "))
        })
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl IrKind {
    pub fn name(self) -> &'static str {
        match self {
            IrKind::Ds => "ds",
            IrKind::Dsi => "dsi",
            IrKind::Cps => "cps",
            IrKind::Asm => "asm",
        }
    }
}

/// A module in any IR.
pub enum Ir {
    Ds(ds::Module),
    Dsi(dsi::Module),
    Cps(cps::Module),
    Asm(asm::Module),
}

impl Ir {
    pub fn kind(&self) -> IrKind {
        match self {
            Ir::Ds(_) => IrKind::Ds,
            Ir::Dsi(_) => IrKind::Dsi,
            Ir::Cps(_) => IrKind::Cps,
            Ir::Asm(_) => IrKind::Asm,
        }
    }

    pub fn to_text(&self) -> String {
        match self {
            Ir::Ds(m) => m.to_text(),
            Ir::Dsi(m) => m.to_text(),
            Ir::Cps(m) => m.to_text(),
            Ir::Asm(m) => m.to_text(),
        }
    }

    pub fn from_text(kind: IrKind, src: &str) -> Result<Ir, TextError> {
        Ok(match kind {
            IrKind::Ds => Ir::Ds(ds::Module::from_text(src)?),
            IrKind::Dsi => Ir::Dsi(dsi::Module::from_text(src)?),
            IrKind::Cps => Ir::Cps(cps::Module::from_text(src)?),
            IrKind::Asm => Ir::Asm(asm::Module::from_text(src)?),
        })
    }

    /// Global names, in define order, when the IR still carries them (not asm).
    pub fn global_names(&self) -> Option<Vec<(u16, String)>> {
        let names: Vec<String> = match self {
            Ir::Ds(m) => m.defines.iter().map(|d| d.name.clone()).collect(),
            Ir::Dsi(m) => m.defines.iter().map(|d| d.name.clone()).collect(),
            Ir::Cps(m) => m.defines.iter().map(|d| d.name.clone()).collect(),
            Ir::Asm(_) => return None,
        };
        Some(names.into_iter().enumerate().map(|(i, n)| (i as u16, n)).collect())
    }

    fn define_count(&self) -> usize {
        match self {
            Ir::Ds(m) => m.defines.len(),
            Ir::Dsi(m) => m.defines.len(),
            Ir::Cps(m) => m.defines.len(),
            Ir::Asm(m) => m.defines.len(),
        }
    }
}

/// Run the passes after `from` up to and including `until`.
///
/// `ir` must be the output of stage `from`. With `until <= from` the module is
/// returned unchanged. A disabled optimizer (`config: None`) makes
/// `cps-optimize` the identity.
pub fn run_stages(
    ir: Ir,
    from: Stage,
    until: Stage,
    config: Option<OptimizeConfig>,
) -> Result<Ir, CompileError> {
    if ir.kind() != from.ir() {
        return Err(CompileError::StageMismatch { stage: from.name(), expected: from.ir().name(), found: ir.kind().name() });
    }
    if ir.define_count() > asm_emit::MAX_GLOBALS {
        return Err(CompileError::TooManyGlobals { count: ir.define_count(), max: asm_emit::MAX_GLOBALS });
    }
    let mut ir = ir;
    let mut config = config;
    for &stage in Stage::ALL.iter().filter(|&&s| s > from && s <= until) {
        ir = match (stage, ir) {
            (Stage::DsUncurry, Ir::Ds(m)) => Ir::Ds(ds_uncurry::resolve_module(m)),
            (Stage::DsiResolve, Ir::Ds(m)) => Ir::Dsi(dsi_resolve::resolve_module(m)),
            (Stage::CpsTransform, Ir::Dsi(m)) => Ir::Cps(cps_transform::transform_module(m)),
            (Stage::CpsOptimize, Ir::Cps(m)) => Ir::Cps(match config.take() {
                Some(config) => cps_optimize::optimize_module(m, config),
                None => m,
            }),
            (Stage::AsmResolve, Ir::Cps(m)) => Ir::Asm(asm_resolve::resolve_module(&m)),
            (Stage::AsmPeephole, Ir::Asm(m)) => Ir::Asm(asm_peephole::optimize_module(m)),
            _ => unreachable!("stage {stage} applied to the wrong IR"),
        };
    }
    Ok(ir)
}

/// Run the remaining passes from stage `from` and emit bytecode.
pub fn compile_from(
    ir: Ir,
    from: Stage,
    config: Option<OptimizeConfig>,
    metadata: Option<&Metadata>,
) -> Result<Vec<u8>, CompileError> {
    match run_stages(ir, from, Stage::AsmPeephole, config)? {
        Ir::Asm(m) => Emitter::emit_module(&m, metadata),
        _ => unreachable!(),
    }
}

pub fn compile_module(
    module: ds::Module,
    config: Option<OptimizeConfig>,
    metadata: Option<&Metadata>,
) -> Result<Vec<u8>, CompileError> {
    compile_from(Ir::Ds(module), Stage::Parse, config, metadata)
}

pub fn compile_to_dir(
    module: &ds::Module,
    config: Option<OptimizeConfig>,
    include_bindings: bool,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    compile_to_dir_with_ctors(module, config, include_bindings, dir, &[])
}

pub fn compile_to_dir_with_ctors(
    module: &ds::Module,
    config: Option<OptimizeConfig>,
    include_bindings: bool,
    dir: &Path,
    ctor_names: &[(u8, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;

    let global_names: Vec<(u16, String)> = module.defines.iter()
        .enumerate()
        .map(|(i, d)| (i as u16, d.name.clone()))
        .collect();

    let metadata = Metadata {
        ctor_names: ctor_names.to_vec(),
        global_names: global_names.clone(),
    };

    let binary = compile_module(module.clone(), config, Some(&metadata))?;
    std::fs::write(dir.join("bytecode.bin"), &binary)?;

    if include_bindings {
        write_bindings(dir, &global_names, ctor_names)?;
    }

    Ok(())
}

/// Write `bindings.rs` (funcs/ctors constants) into `dir`.
pub fn write_bindings(
    dir: &Path,
    global_names: &[(u16, String)],
    ctor_names: &[(u8, String)],
) -> std::io::Result<()> {
    let mut s = String::new();
    s.push_str("pub mod funcs {\n");
    s.push_str("    use encore_vm::value::GlobalAddress;\n");
    for (idx, name) in global_names {
        s.push_str(&format!(
            "    #[allow(dead_code)]\n    pub const {}: GlobalAddress = GlobalAddress::new({});\n",
            rust_const_name(name), idx,
        ));
    }
    s.push_str("}\n\n");

    s.push_str("pub mod ctors {\n");
    for (tag, name) in ctor_names {
        s.push_str(&format!(
            "    #[allow(dead_code)]\n    pub const {}: u8 = {};\n",
            rust_const_name(name), tag,
        ));
    }
    s.push_str("}\n");

    std::fs::write(dir.join("bindings.rs"), &s)
}

fn rust_const_name(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}
