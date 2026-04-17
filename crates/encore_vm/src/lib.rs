#![no_std]

#[cfg(feature = "derive")]
pub use encore_derive::{ValueDecode, ValueEncode};

pub mod arena;
pub mod builtins;
pub mod code;
pub mod error;
pub mod ffi;
pub mod gc;
pub mod opcode;
pub mod program;
mod registers;
#[cfg(feature = "stats")]
pub mod stats;
pub mod value;
pub mod vm;

#[macro_export]
macro_rules! encore_heap {
    ($name:ident, $bytes:expr) => {
        #[allow(non_snake_case)]
        fn $name() -> &'static mut [$crate::value::Value] {
            const N: usize = $bytes / core::mem::size_of::<$crate::value::Value>();
            static mut STORAGE: [$crate::value::Value; N] =
                [$crate::value::Value::ZERO; N];
            unsafe { &mut *(&raw mut STORAGE) }
        }
    };
}

#[macro_export]
macro_rules! encore_program {
    ($dir:expr) => {
        static BYTECODE: &[u8] = include_bytes!(concat!($dir, "/bytecode.bin"));

        #[allow(dead_code)]
        mod bindings {
            include!(concat!($dir, "/bindings.rs"));
        }

        #[allow(unused_imports)]
        use bindings::{ctors, funcs};

        /// Parse `BYTECODE`, initialise a VM over `heap`, and run the module's
        /// top-level code to populate globals. Returns a ready-to-use `Vm`.
        #[allow(dead_code)]
        fn boot(
            heap: &'static mut [$crate::value::Value],
        ) -> ::core::result::Result<$crate::vm::Vm<'static>, $crate::error::ExternError> {
            let prog = $crate::program::Program::parse(BYTECODE)?;
            let mut vm = $crate::vm::Vm::init(heap);
            vm.load(&prog)?;
            ::core::result::Result::Ok(vm)
        }
    };
}
