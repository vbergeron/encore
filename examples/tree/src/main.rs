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

fn leaf(vm: &mut Vm) -> Value {
    vm.alloc_ctor(ctors::LEAF, &[]).unwrap_or_else(|e| vm_exit_err(e))
}

fn node(vm: &mut Vm, left: Value, right: Value) -> Value {
    vm.alloc_ctor(ctors::NODE, &[left, right]).unwrap_or_else(|e| vm_exit_err(e))
}

#[entry]
fn main() -> ! {
    let buf = HEAP();
    let prog = Program::parse(BYTECODE).unwrap_or_else(|e| vm_exit_err(e));
    let mut vm = Vm::init(buf);
    vm.load(&prog).unwrap_or_else(|e| vm_exit_err(e));

    //       *
    //      / \
    //     *   *
    //    / \ / \
    //   .  . .  *
    //          / \
    //         .   .
    let l1 = leaf(&mut vm);
    let l2 = leaf(&mut vm);
    let l3 = leaf(&mut vm);
    let l4 = leaf(&mut vm);
    let l5 = leaf(&mut vm);
    let left = node(&mut vm, l1, l2);
    let rr = node(&mut vm, l4, l5);
    let right = node(&mut vm, l3, rr);
    let tree = node(&mut vm, left, right);

    let n = vm.call(funcs::COUNT, tree).unwrap_or_else(|e| vm_exit_err(e));
    let _ = hprintln!("{}", n.int_value().unwrap());

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
