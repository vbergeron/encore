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

/// Wrap a typed handler `fn(&mut Vm, Args) -> Result<O, ExternError>` into a
/// plain [`ExternFn`](crate::vm::ExternFn) suitable for
/// [`Vm::register_extern`](crate::vm::Vm::register_extern).
///
/// The macro expands to a named `fn` item that decodes `Args` from the
/// incoming `Value`, calls the handler, and encodes `O` back. Because it is
/// a free function (not a closure), no state is captured and it coerces
/// directly to an `fn` pointer.
///
/// ```ignore
/// fn greet(vm: &mut Vm, name: VmBytes) -> Result<VmBytes, ExternError> { /* … */ }
/// vm.register_extern(0, encore_vm::extern_fn!(greet));
/// ```
#[macro_export]
macro_rules! extern_fn {
    ($handler:path) => {{
        fn __wrapped(
            vm: &mut $crate::vm::Vm,
            arg: $crate::value::Value,
        ) -> ::core::result::Result<$crate::value::Value, $crate::error::ExternError> {
            let args = $crate::ffi::ValueDecode::decode(vm, arg)?;
            let out = $handler(vm, args)?;
            $crate::ffi::ValueEncode::encode(&out, vm)
                .map_err($crate::error::ExternError::from)
        }
        __wrapped as $crate::vm::ExternFn
    }};
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
