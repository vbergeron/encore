use std::fs;
use std::process;

use clap::{Parser, Subcommand};
use encore_compiler::pass::asm_emit::Metadata;
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
        /// Entrypoint: define name or index (0-based)
        #[arg(short, long, default_value = "0")]
        entry: String,
        /// Heap size in 32-bit words
        #[arg(long, default_value_t = DEFAULT_HEAP_SIZE)]
        heap_size: usize,
    },
    /// Compile source to a binary
    Compile {
        #[command(subcommand)]
        frontend: Frontend,
    },
    /// Disassemble a compiled .bin program
    Disasm {
        /// Path to the compiled binary
        file: String,
        /// Launch interactive TUI
        #[arg(short, long)]
        interactive: bool,
    },
}

#[derive(Subcommand)]
enum Frontend {
    /// Compile a .fleche source file
    Fleche {
        /// Path to the .fleche source file
        file: String,
        /// Output directory
        #[arg(short, long, default_value = "out")]
        out: String,
        /// Include debug metadata (constructor names) in the binary
        #[arg(long)]
        include_metadata: bool,
        /// Generate Rust bindings (funcs/ctors constants) in the output directory
        #[arg(long)]
        include_bindings: bool,
        #[command(flatten)]
        opt: OptimizeFlags,
    },
    /// Compile a Rocq-extracted .scm source file
    Scheme {
        /// Path to the .scm source file
        file: String,
        /// Output directory
        #[arg(short, long, default_value = "out")]
        out: String,
        /// Include debug metadata (constructor names) in the binary
        #[arg(long)]
        include_metadata: bool,
        /// Generate Rust bindings (funcs/ctors constants) in the output directory
        #[arg(long)]
        include_bindings: bool,
        #[command(flatten)]
        opt: OptimizeFlags,
    },
}

#[derive(Parser)]
struct OptimizeFlags {
    /// Enable/disable the CPS optimizer entirely
    #[arg(long, default_value = "on")]
    cps_optimize: Flag,

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

    #[arg(long, default_value = "on")]
    cps_optimize_rewrite_contification: Flag,
}

impl From<OptimizeFlags> for Option<OptimizeConfig> {
    fn from(f: OptimizeFlags) -> Self {
        if matches!(f.cps_optimize, Flag::Off) {
            return None;
        }
        Some(OptimizeConfig {
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
            rewrite_contification: f.cps_optimize_rewrite_contification.into(),
        })
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { file, entry, heap_size } => cmd_run(&file, &entry, heap_size),
        Command::Compile { frontend } => match frontend {
            Frontend::Fleche { file, out, include_metadata, include_bindings, opt } => {
                let config: Option<OptimizeConfig> = opt.into();
                cmd_compile_fleche(&file, &out, config, include_metadata, include_bindings);
            }
            Frontend::Scheme { file, out, include_metadata, include_bindings, opt } => {
                let config: Option<OptimizeConfig> = opt.into();
                cmd_compile_scheme(&file, &out, config, include_metadata, include_bindings);
            }
        },
        Command::Disasm { file, interactive } => cmd_disasm(&file, interactive),
    }
}

fn cmd_run(path: &str, entry: &str, heap_size: usize) {
    let bin_path = if std::path::Path::new(path).is_dir() {
        std::path::Path::new(path).join("bytecode.bin").to_string_lossy().into_owned()
    } else {
        path.to_string()
    };
    let bytes = fs::read(&bin_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {bin_path}: {e}");
        process::exit(1);
    });

    let prog = Program::parse(&bytes).unwrap_or_else(|e| {
        eprintln!("error: invalid binary: {e:?}");
        process::exit(1);
    });

    let entry_idx = resolve_entry(entry, &prog);

    let mut heap = vec![Value::from_u32(0); heap_size];
    let mut vm = Vm::init(&mut heap);

    vm.load(&prog).unwrap_or_else(|e| {
        eprintln!("runtime error: {e:?}");
        process::exit(2);
    });

    #[cfg(feature = "stats")]
    eprintln!("{}", vm.stats());

    print_value(vm.global(entry_idx));
}

fn resolve_entry(entry: &str, prog: &Program) -> usize {
    if let Ok(idx) = entry.parse::<usize>() {
        if idx >= prog.n_globals() {
            eprintln!(
                "error: entrypoint {idx} out of range (module has {} defines)",
                prog.n_globals()
            );
            process::exit(1);
        }
        return idx;
    }

    let global_names: Vec<(u8, &str)> = prog.global_names().collect();
    for &(idx, name) in &global_names {
        if name == entry {
            return idx as usize;
        }
    }

    if global_names.is_empty() {
        eprintln!("error: no define named '{entry}' (binary has no metadata; use --include-metadata when compiling, or pass a numeric index)");
    } else {
        let available: Vec<&str> = global_names.iter().map(|(_, n)| *n).collect();
        eprintln!("error: no define named '{entry}'; available: {}", available.join(", "));
    }
    process::exit(1);
}

fn cmd_compile_fleche(path: &str, out: &str, config: Option<OptimizeConfig>, include_metadata: bool, include_bindings: bool) {
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {path}: {e}");
        process::exit(1);
    });

    let (module, ctor_names) = encore_fleche::parse_with_metadata(&source);

    if include_bindings {
        encore_compiler::pipeline::compile_to_dir_with_ctors(
            &module, config, true, std::path::Path::new(out), &ctor_names,
        ).unwrap_or_else(|e| {
            eprintln!("error: compile_to_dir: {e}");
            process::exit(1);
        });
        eprintln!("compiled {path} -> {out}/");
    } else {
        let metadata = if include_metadata {
            let global_names = module.defines.iter()
                .enumerate()
                .map(|(i, d)| (i as u8, d.name.clone()))
                .collect();
            Some(Metadata { ctor_names, global_names })
        } else {
            None
        };
        let binary = encore_compiler::pipeline::compile_module(module, config, metadata.as_ref());
        fs::create_dir_all(out).unwrap_or_else(|e| {
            eprintln!("error: cannot create {out}: {e}");
            process::exit(1);
        });
        let bin_path = std::path::Path::new(out).join("bytecode.bin");
        fs::write(&bin_path, &binary).unwrap_or_else(|e| {
            eprintln!("error: cannot write {}: {e}", bin_path.display());
            process::exit(1);
        });
        eprintln!("compiled {path} -> {} ({} bytes)", bin_path.display(), binary.len());
    }
}

fn cmd_compile_scheme(path: &str, out: &str, config: Option<OptimizeConfig>, include_metadata: bool, include_bindings: bool) {
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {path}: {e}");
        process::exit(1);
    });

    let (module, ctor_names) = encore_scheme::parse_with_metadata(&source);

    if include_bindings {
        encore_compiler::pipeline::compile_to_dir_with_ctors(
            &module, config, true, std::path::Path::new(out), &ctor_names,
        ).unwrap_or_else(|e| {
            eprintln!("error: compile_to_dir: {e}");
            process::exit(1);
        });
        eprintln!("compiled {path} -> {out}/");
    } else {
        let metadata = if include_metadata {
            let global_names = module.defines.iter()
                .enumerate()
                .map(|(i, d)| (i as u8, d.name.clone()))
                .collect();
            Some(Metadata { ctor_names, global_names })
        } else {
            None
        };
        let binary = encore_compiler::pipeline::compile_module(module, config, metadata.as_ref());
        fs::create_dir_all(out).unwrap_or_else(|e| {
            eprintln!("error: cannot create {out}: {e}");
            process::exit(1);
        });
        let bin_path = std::path::Path::new(out).join("bytecode.bin");
        fs::write(&bin_path, &binary).unwrap_or_else(|e| {
            eprintln!("error: cannot write {}: {e}", bin_path.display());
            process::exit(1);
        });
        eprintln!("compiled {path} -> {} ({} bytes)", bin_path.display(), binary.len());
    }
}

fn print_value(val: Value) {
    if val.is_int() {
        println!("{}", val.int_value());
    } else if val.is_ctor() {
        println!("ctor(tag={})", val.ctor_tag());
    } else if val.is_function() {
        println!("function(@{:04x})", val.code_ptr().raw());
    } else if val.is_closure() {
        println!("closure");
    } else {
        println!("value(0x{:08x})", val.to_u32());
    }
}

fn cmd_disasm(path: &str, interactive: bool) {
    let bin_path = if std::path::Path::new(path).is_dir() {
        std::path::Path::new(path).join("bytecode.bin").to_string_lossy().into_owned()
    } else {
        path.to_string()
    };
    let bytes = fs::read(&bin_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {bin_path}: {e}");
        process::exit(1);
    });

    let disasm = match encore_disasm::decode(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: invalid binary: {e:?}");
            process::exit(1);
        }
    };

    if interactive {
        encore_disasm::tui::run(disasm).unwrap_or_else(|e| {
            eprintln!("error: TUI failed: {e}");
            process::exit(1);
        });
    } else {
        print!("{disasm}");
    }
}
