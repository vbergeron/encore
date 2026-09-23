#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;
use encore_vm::vm::Vm;

#[path = "../../common/qemu_clock.rs"]
mod qemu_clock;

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

#[entry]
fn main() -> ! {
    qemu_clock::start(cortex_m::Peripherals::take().unwrap().SYST);

    let mut vm = boot(HEAP()).unwrap_or_else(|e| vm_exit_err(e));
    vm.set_clock(qemu_clock::now_ns);

    for &(a, b, fuel) in &[(1i32, 1i32, 500i32), (1, 2, 1000), (2, 2, 3000)] {
        // TEST_ADD : a -> b -> fuel -> Option nat, uncurried into a 3-ary function.
        let result: OptionNat = vm.call_global(funcs::TEST_ADD, (a, b, fuel))
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
