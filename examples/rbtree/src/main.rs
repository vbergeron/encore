#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::VmError;
use encore_vm::program::Program;
use encore_vm::value::Value;
use encore_vm::vm::Vm;

encore_vm::encore_program!(env!("OUT_DIR"));
encore_vm::encore_heap!(HEAP, 40_000);

fn vm_exit_err(e: VmError) -> ! {
    let _ = hprintln!("VM error: {:?}", e);
    debug::exit(debug::EXIT_FAILURE);
    loop {}
}

#[entry]
fn main() -> ! {
    let buf = HEAP();
    let prog = Program::parse(BYTECODE).unwrap_or_else(|e| vm_exit_err(e));
    let mut vm = Vm::init(buf);
    vm.load(&prog).unwrap_or_else(|e| vm_exit_err(e));

    let n = 50;

    let tree = vm.call(funcs::BUILD_TREE, Value::int(n)).unwrap_or_else(|e| vm_exit_err(e));

    let d = vm.call(funcs::DEPTH, tree).unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!("build_tree({}) depth = {}", n, d.int_value());

    let s = vm.call(funcs::SIZE, tree).unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!("build_tree({}) size  = {}", n, s.int_value());

    let ok = vm.call(funcs::BUILD_AND_CHECK, Value::int(n)).unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!(
        "build_and_check({}) = {}",
        n,
        if ok.is_ctor() && ok.ctor_tag() == ctors::TRUE { "true" } else { "false" }
    );

    let _ = hprintln!("{}", vm.stats());

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
