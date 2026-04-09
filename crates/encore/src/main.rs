use std::fs;
use std::process;

use clap::{Parser, Subcommand};
use encore_compiler::pass::cps_optimize::OptimizeConfig;
use encore_vm::program::Program;
use encore_vm::value::Value;
use encore_vm::vm::Vm;

const DEFAULT_HEAP_SIZE: usize = 1 << 16;

#[derive(Clone, Copy)]
enum Flag {
    On,
    Off,
}

impl From<Flag> for bool {
    fn from(f: Flag) -> bool {
        matches!(f, Flag::On)
    }
}

impl std::str::FromStr for Flag {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "on" => Ok(Flag::On),
            "off" => Ok(Flag::Off),
            _ => Err(format!("expected on/off, got '{s}'")),
        }
    }
}

#[derive(Parser)]
#[command(name = "encore", about = "The Encore VM toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Execute a compiled .bin program
    Run {
        /// Path to the compiled binary
        file: String,
        /// Entrypoint define index (0-based)
        #[arg(short, long, default_value_t = 0)]
        entry: usize,
        /// Heap size in 32-bit words
        #[arg(long, default_value_t = DEFAULT_HEAP_SIZE)]
        heap_size: usize,
    },
    /// Compile source to a binary
    Compile {
        #[command(subcommand)]
        frontend: Frontend,
    },
}

#[derive(Subcommand)]
enum Frontend {
    /// Compile a .fleche source file
    Fleche {
        /// Path to the .fleche source file
        file: String,
        /// Output binary path
        #[arg(short, long, default_value = "out.bin")]
        out: String,
        #[command(flatten)]
        opt: OptimizeFlags,
    },
}

#[derive(Parser)]
struct OptimizeFlags {
    /// Optimizer fuel (max iterations)
    #[arg(long, default_value_t = 100)]
    cps_optimize_fuel: usize,

    /// Inlining size threshold
    #[arg(long, default_value_t = 20)]
    cps_optimize_inline_threshold: usize,

    #[arg(long, default_value = "on")]
    cps_optimize_simplify_dead_code: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_simplify_copy_propagation: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_simplify_constant_fold: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_simplify_beta_contraction: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_simplify_eta_reduction: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_rewrite_inlining: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_rewrite_hoisting: Flag,

    #[arg(long, default_value = "on")]
    cps_optimize_rewrite_cse: Flag,
}

impl From<OptimizeFlags> for OptimizeConfig {
    fn from(f: OptimizeFlags) -> Self {
        Self {
            fuel: f.cps_optimize_fuel,
            inline_threshold: f.cps_optimize_inline_threshold,
            simplify_dead_code: f.cps_optimize_simplify_dead_code.into(),
            simplify_copy_propagation: f.cps_optimize_simplify_copy_propagation.into(),
            simplify_constant_fold: f.cps_optimize_simplify_constant_fold.into(),
            simplify_beta_contraction: f.cps_optimize_simplify_beta_contraction.into(),
            simplify_eta_reduction: f.cps_optimize_simplify_eta_reduction.into(),
            rewrite_inlining: f.cps_optimize_rewrite_inlining.into(),
            rewrite_hoisting: f.cps_optimize_rewrite_hoisting.into(),
            rewrite_cse: f.cps_optimize_rewrite_cse.into(),
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { file, entry, heap_size } => cmd_run(&file, entry, heap_size),
        Command::Compile { frontend } => match frontend {
            Frontend::Fleche { file, out, opt } => cmd_compile_fleche(&file, &out, opt.into()),
        },
    }
}

fn cmd_run(path: &str, entry: usize, heap_size: usize) {
    let bytes = fs::read(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {path}: {e}");
        process::exit(1);
    });

    let prog = Program::parse(&bytes).unwrap_or_else(|e| {
        eprintln!("error: invalid binary: {e:?}");
        process::exit(1);
    });

    if entry >= prog.n_globals() {
        eprintln!(
            "error: entrypoint {entry} out of range (module has {} defines)",
            prog.n_globals()
        );
        process::exit(1);
    }

    let mut heap = vec![Value::from_u32(0); heap_size];
    let mut globals = vec![Value::from_u32(0); prog.n_globals()];
    prog.load_globals(&mut globals);

    globals.swap(0, entry);

    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut heap);
    match vm.run() {
        Ok(val) => print_value(val),
        Err(e) => {
            eprintln!("runtime error: {e:?}");
            process::exit(2);
        }
    }
}

fn cmd_compile_fleche(path: &str, out: &str, config: OptimizeConfig) {
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {path}: {e}");
        process::exit(1);
    });

    let module = encore_fleche::parse(&source);
    let binary = encore_compiler::pipeline::compile_module_with_config(module, config);

    fs::write(out, &binary).unwrap_or_else(|e| {
        eprintln!("error: cannot write {out}: {e}");
        process::exit(1);
    });

    eprintln!("compiled {path} -> {out} ({} bytes)", binary.len());
}

fn print_value(val: Value) {
    if val.is_int() {
        println!("{}", val.int_value());
    } else if val.is_ctor() {
        println!("ctor(tag={})", val.ctor_tag());
    } else if val.is_closure() {
        println!("closure(ncap={})", val.closure_ncap());
    } else {
        println!("value(0x{:08x})", val.to_u32());
    }
}
