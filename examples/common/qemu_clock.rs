//! SysTick-based clock for `Vm::set_clock` on the QEMU `lm3s6965evb` board.
//!
//! QEMU does not emulate the DWT cycle counter, but it does emulate SysTick.
//! SysTick is a 24-bit down-counter; the exception handler below counts
//! wraps to extend it to 64 bits.
//!
//! With the runner's `-icount shift=0`, QEMU advances virtual time by 1 ns
//! per guest instruction, so reported times are deterministic and read as
//! "instructions executed" (at 80-instruction resolution, one SysTick tick).

use core::sync::atomic::{AtomicU32, Ordering};
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::peripheral::SYST;
use cortex_m_rt::exception;

/// QEMU models the reset system clock as 12.5 MHz (RCC SYSDIV = 16 on a
/// 200 MHz base), i.e. 80 ns per SysTick tick. The examples never
/// reprogram RCC.
const NS_PER_TICK: u64 = 80;
const RELOAD: u32 = 0x00FF_FFFF;

static WRAPS: AtomicU32 = AtomicU32::new(0);

#[exception]
fn SysTick() {
    WRAPS.fetch_add(1, Ordering::Relaxed);
}

/// Start SysTick as a free-running counter. Call once, before `boot`.
pub fn start(mut syst: SYST) {
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(RELOAD);
    syst.clear_current();
    syst.enable_interrupt();
    syst.enable_counter();
}

/// Nanoseconds since [`start`]; pass to `Vm::set_clock`.
pub fn now_ns() -> u64 {
    loop {
        let wraps = WRAPS.load(Ordering::Relaxed);
        let elapsed = RELOAD - SYST::get_current();
        // Retry if the wrap interrupt ran between the two reads.
        if WRAPS.load(Ordering::Relaxed) == wraps {
            let ticks = ((wraps as u64) << 24) | elapsed as u64;
            return ticks * NS_PER_TICK;
        }
    }
}
