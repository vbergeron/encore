#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;
use encore_vm::ffi::VmList;

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

    let n = 256i32;

    let sorted: VmList<i32> = vm.call_global(funcs::SORT_SEQ, (n,))
        .unwrap_or_else(|e| vm_exit_err(e));

    let mut len = 0i32;
    let mut list = sorted;
    while let Some((_, tail)) = list.next(&vm) {
        len += 1;
        list = tail;
    }

    let _ = hprintln!("merge_sort(rev_range({})) -> {} elements", n, len);

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
