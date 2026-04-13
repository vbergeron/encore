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

fn list_length(vm: &Vm, mut v: Value) -> i32 {
    let mut n = 0;
    while v.is_ctor() && v.ctor_tag() == ctors::CONS {
        v = vm.ctor_field(v, 1);
        n += 1;
    }
    n
}

#[entry]
fn main() -> ! {
    let buf = HEAP();
    let prog = Program::parse(BYTECODE).unwrap_or_else(|e| vm_exit_err(e));
    let mut vm = Vm::init(buf);
    vm.load(&prog).unwrap_or_else(|e| vm_exit_err(e));

    let n = 256;

    let sorted = vm.call(funcs::SORT_SEQ, Value::int(n)).unwrap_or_else(|e| vm_exit_err(e));
    let len = list_length(&vm, sorted);
    let _ = hprintln!("merge_sort(rev_range({})) -> {} elements", n, len);

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
