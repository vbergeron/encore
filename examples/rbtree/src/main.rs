#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;
use encore_vm::value::Value;

encore_vm::encore_program!(env!("OUT_DIR"));
encore_vm::encore_heap!(HEAP, 40_000);

fn vm_exit_err(e: ExternError) -> ! {
    let _ = hprintln!("VM error: {:?}", e);
    debug::exit(debug::EXIT_FAILURE);
    loop {}
}

#[entry]
fn main() -> ! {
    let mut vm = boot(HEAP()).unwrap_or_else(|e| vm_exit_err(e));

    let n = 30i32;

    let tree: Value = vm.call_global(funcs::BUILD_TREE, (n,))
        .unwrap_or_else(|e| vm_exit_err(e));

    let depth: i32 = vm.call_global(funcs::DEPTH, (tree,))
        .unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!("build_tree({}) depth = {}", n, depth);

    let size: i32 = vm.call_global(funcs::SIZE, (tree,))
        .unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!("build_tree({}) size  = {}", n, size);

    // BUILD_AND_CHECK returns True (tag 1) or False (tag 0),
    // which maps directly to Rust's bool via ValueDecode.
    let ok: bool = vm.call_global(funcs::BUILD_AND_CHECK, (n,))
        .unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!("build_and_check({}) = {}", n, ok);

    let _ = hprintln!("{}", vm.stats());

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
