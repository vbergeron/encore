#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::VmError;
use encore_vm::program::Program;
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

    let result = vm.global(funcs::MAIN);
    let len = vm.bytes_len(result);
    let _ = hprintln!("bytes_of_list_nat result: {} bytes", len);
    for i in 0..len {
        let _ = hprintln!("  [{}] = {}", i, vm.bytes_read(result, i));
    }

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
