#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_halt as _;

use encore_vm::error::ExternError;
use encore_vm::ffi::{VmCallable, VmList};
use encore_vm::vm::Vm;

encore_vm::encore_program!(env!("OUT_DIR"));
encore_vm::encore_heap!(HEAP, 40_000);

// ── Event ──────────────────────────────────────────────────────────────────
// 3 nullary variants (0 fields each): Inc | Dec | Reset
// Encode only: we send events into the VM.

#[derive(Clone, Copy, encore_vm::ValueEncode)]
enum Event {
    #[ctor(ctors::INC)]   Inc,
    #[ctor(ctors::DEC)]   Dec,
    #[ctor(ctors::RESET)] Reset,
}

// ── Effect ─────────────────────────────────────────────────────────────────
// Beep: 0 fields | Print(i32): 1 field
// Decode only: the VM sends effects out to us.

#[derive(Clone, Copy, encore_vm::ValueDecode)]
enum Effect {
    #[ctor(ctors::BEEP)]  Beep,
    #[ctor(ctors::PRINT)] Print(i32),
}


// ── StepResult ─────────────────────────────────────────────────────────────
// Pair(state: i32, effects: List Effect) — 2 named fields.
// #[ctor] goes on the struct; fields decoded in declaration order.

#[derive(encore_vm::ValueDecode)]
#[ctor(ctors::PAIR)]
struct StepResult { state: i32, effects: VmList<Effect> }

// ── helpers ────────────────────────────────────────────────────────────────

fn vm_exit_err(e: ExternError) -> ! {
    let _ = hprintln!("VM error: {:?}", e);
    debug::exit(debug::EXIT_FAILURE);
    loop {}
}

fn run_step(vm: &mut Vm, state: i32, event: Event) -> StepResult {
    let partial: VmCallable = vm
        .call_global(funcs::STEP, (state,))
        .unwrap_or_else(|e| vm_exit_err(e));
    vm.call_closure(partial, (event,))
        .unwrap_or_else(|e| vm_exit_err(e))
}

#[entry]
fn main() -> ! {
    let mut vm = boot(HEAP()).unwrap_or_else(|e| vm_exit_err(e));

    let mut state: i32 = 0;

    let events: &[(Event, &str)] = &[
        (Event::Inc,   "Inc"),
        (Event::Inc,   "Inc"),
        (Event::Inc,   "Inc"),
        (Event::Dec,   "Dec"),
        (Event::Reset, "Reset"),
        (Event::Inc,   "Inc"),
    ];

    // Scratch buffer for pulling each step's effect list out of the VM in one
    // go via `VmList::materialize`. Sized generously: the FSM emits at most
    // two effects per step (Print + optional Beep).
    let mut effect_buf = [Effect::Beep; 4];

    for &(event, name) in events {
        let StepResult { state: next_state, effects } = run_step(&mut vm, state, event);
        state = next_state;

        let _ = hprintln!("event: {} -> state={}", name, state);

        for &effect in effects.materialize(&vm, &mut effect_buf) {
            match effect {
                Effect::Print(n) => { let _ = hprintln!("  [print] {}", n); }
                Effect::Beep     => { let _ = hprintln!("  [beep!]"); }
            }
        }
    }

    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}
