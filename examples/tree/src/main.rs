#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;
use encore_vm::ffi::ValueEncode;
use encore_vm::value::Value;
use encore_vm::vm::Vm;

encore_vm::encore_program!(env!("OUT_DIR"));
encore_vm::encore_heap!(HEAP, 40_000);

fn vm_exit_err(e: ExternError) -> ! {
    let _ = hprintln!("VM error: {:?}", e);
    debug::exit(debug::EXIT_FAILURE);
    loop {}
}

// Tree constructors: Leaf (0 fields) | Node(left, right) (2 fields).
// Encode only: we build trees on the Rust side and pass them into the VM.

#[derive(Clone, Copy, encore_vm::ValueEncode)]
enum Tree {
    #[ctor(ctors::LEAF)] Leaf,
    #[ctor(ctors::NODE)] Node(Value, Value),
}

fn mk(vm: &mut Vm, tree: Tree) -> Value {
    tree.encode(vm).unwrap_or_else(|e| vm_exit_err(ExternError::from(e)))
}

#[entry]
fn main() -> ! {
    let mut vm = boot(HEAP()).unwrap_or_else(|e| vm_exit_err(e));

    //       *
    //      / \
    //     *   *
    //    / \ / \
    //   .  . .  *
    //          / \
    //         .   .
    let l1 = mk(&mut vm, Tree::Leaf);
    let l2 = mk(&mut vm, Tree::Leaf);
    let l3 = mk(&mut vm, Tree::Leaf);
    let l4 = mk(&mut vm, Tree::Leaf);
    let l5 = mk(&mut vm, Tree::Leaf);
    let left  = mk(&mut vm, Tree::Node(l1, l2));
    let rr    = mk(&mut vm, Tree::Node(l4, l5));
    let right = mk(&mut vm, Tree::Node(l3, rr));
    let tree  = mk(&mut vm, Tree::Node(left, right));

    let n: i32 = vm.call_global(funcs::COUNT, (tree,))
        .unwrap_or_else(|e| vm_exit_err(e));

    let _ = hprintln!("{}", n);

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
