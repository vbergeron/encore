#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;

encore_vm::encore_program!(env!("OUT_DIR"));
encore_vm::encore_heap!(HEAP, 4_000);

fn vm_exit_err(e: ExternError) -> ! {
    let _ = hprintln!("VM error: {:?}", e);
    debug::exit(debug::EXIT_FAILURE);
    loop {}
}

#[entry]
fn main() -> ! {
    let mut vm = boot(HEAP()).unwrap_or_else(|e| vm_exit_err(e));

    let result: i32 = vm
        .call_global(funcs::MAIN, (48i32, 18i32))
        .unwrap_or_else(|e| vm_exit_err(e));

    let _ = hprintln!("gcd(48, 18) = {}", result);

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
