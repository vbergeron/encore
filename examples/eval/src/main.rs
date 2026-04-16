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

fn read_option_nat(vm: &Vm, v: Value) -> Option<i32> {
    if v.is_ctor() && v.ctor_tag() == ctors::SOME {
        Some(vm.ctor_field(v, 0).int_value().unwrap())
    } else {
        None
    }
}

fn call3(vm: &mut Vm, global: usize, a: Value, b: Value, c: Value) -> Result<Value, VmError> {
    let f1 = vm.call(global, a)?;
    let f2 = vm.call_value(f1, b)?;
    vm.call_value(f2, c)
}

#[entry]
fn main() -> ! {
    let buf = HEAP();
    let prog = Program::parse(BYTECODE).unwrap_or_else(|e| vm_exit_err(e));
    let mut vm = Vm::init(buf);
    vm.load(&prog).unwrap_or_else(|e| vm_exit_err(e));

    for &(a, b, f) in &[(1i32, 1i32, 500i32), (1, 2, 1000), (2, 2, 3000)] {
        let r = call3(&mut vm, funcs::TEST_ADD, Value::int(a), Value::int(b), Value::int(f))
            .unwrap_or_else(|e| vm_exit_err(e));
        match read_option_nat(&vm, r) {
            Some(n) => { let _ = hprintln!("church {} + {} (fuel={}) = {}", a, b, f, n); }
            None =>    { let _ = hprintln!("church {} + {} (fuel={}) = timeout", a, b, f); }
        }
    }

    let _ = hprintln!("{}", vm.stats());

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
