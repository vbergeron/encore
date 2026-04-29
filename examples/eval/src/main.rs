#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;
use encore_vm::ffi::VmCallable;
use encore_vm::value::GlobalAddress;
use encore_vm::vm::Vm;

encore_vm::encore_program!(env!("OUT_DIR"));
encore_vm::encore_heap!(HEAP, 40_000);

fn vm_exit_err(e: ExternError) -> ! {
    let _ = hprintln!("VM error: {:?}", e);
    debug::exit(debug::EXIT_FAILURE);
    loop {}
}

// Option<i32> as returned by the church numeral evaluator.
// None: evaluation timed out. Some(n): result is n.

#[derive(encore_vm::ValueDecode)]
enum OptionNat {
    #[ctor(ctors::NONE)] None,
    #[ctor(ctors::SOME)] Some(i32),
}

// step : curried 3-argument call (TEST_ADD is a -> b -> fuel -> Option nat)
fn call3<O: encore_vm::ffi::ValueDecode>(
    vm: &mut Vm,
    global: GlobalAddress,
    a: i32,
    b: i32,
    fuel: i32,
) -> Result<O, ExternError> {
    let f1: VmCallable = vm.call_global(global, (a,))?;
    let f2: VmCallable = vm.call_closure(&f1, (b,))?;
    vm.call_closure(&f2, (fuel,))
}

#[entry]
fn main() -> ! {
    let mut vm = boot(HEAP()).unwrap_or_else(|e| vm_exit_err(e));

    for &(a, b, fuel) in &[(1i32, 1i32, 500i32), (1, 2, 1000), (2, 2, 3000)] {
        let result: OptionNat = call3(&mut vm, funcs::TEST_ADD, a, b, fuel)
            .unwrap_or_else(|e| vm_exit_err(e));

        match result {
            OptionNat::Some(n) => { let _ = hprintln!("church {} + {} (fuel={}) = {}", a, b, fuel, n); }
            OptionNat::None    => { let _ = hprintln!("church {} + {} (fuel={}) = timeout", a, b, fuel); }
        }
    }

    let _ = hprintln!("{}", vm.stats());

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
