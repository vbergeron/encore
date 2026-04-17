use encore_vm::error::ExternError;
use encore_vm::ffi::{AsVmBytes, AsVmList, VmBytes, VmList};
use encore_vm::opcode::*;
use encore_vm::program::Program;
use encore_vm::value::{CodeAddress, GlobalAddress, Value};
use encore_vm::vm::Vm;

const X01: u8 = 10;

/// Bytecode for an identity function: the global slot returns a function
/// pointer to the body at offset 6; the body is `FIN A1`.
const IDENTITY_CODE: [u8; 8] = [FUNCTION, X01, 6, 0, FIN, X01, FIN, 2];

fn make_vm<'a>(mem: &'a mut [Value], code: &'a [u8], arity_table: &'a [u8]) -> Vm<'a> {
    let prog = Program::new(code, arity_table, &[CodeAddress::new(0)]);
    let mut vm = Vm::init(mem);
    vm.load(&prog).unwrap();
    vm
}

/// Walk a `VmList<i32>` to a `Vec<i32>` for convenient assertions.
fn collect_list(vm: &Vm, list: VmList<i32>) -> Vec<i32> {
    let mut out = Vec::new();
    let mut cur = list;
    while let Some((head, tail)) = cur.next(vm) {
        out.push(head);
        cur = tail;
    }
    out
}

// -- bool encode --

#[test]
fn test_bool_encode_true() {
    // function: return A1 unchanged, we pass an encoded bool
    let code = [FUNCTION, X01, 6, 0, FIN, X01, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let result: bool = vm.call_global(GlobalAddress::new(0), (true,)).unwrap();
    assert!(result);
}

#[test]
fn test_bool_encode_false() {
    let code = [FUNCTION, X01, 6, 0, FIN, X01, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let result: bool = vm.call_global(GlobalAddress::new(0), (false,)).unwrap();
    assert!(!result);
}

// -- bool from VM comparison result --

#[test]
fn test_bool_decode_from_int_eq() {
    let code = [
        FUNCTION, X01, 6, 0, FIN, X01,
        // body: A1 == A2
        INT_EQ, X01, 2, 3,
        FIN, X01,
    ];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let eq: bool = vm.call_global(GlobalAddress::new(0), (7i32, 7i32)).unwrap();
    assert!(eq);

    let neq: bool = vm.call_global(GlobalAddress::new(0), (3i32, 5i32)).unwrap();
    assert!(!neq);
}

#[test]
fn test_bool_decode_from_int_lt() {
    let code = [
        FUNCTION, X01, 6, 0, FIN, X01,
        INT_LT, X01, 2, 3,
        FIN, X01,
    ];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let lt: bool = vm.call_global(GlobalAddress::new(0), (1i32, 5i32)).unwrap();
    assert!(lt);

    let not_lt: bool = vm.call_global(GlobalAddress::new(0), (5i32, 1i32)).unwrap();
    assert!(!not_lt);
}

// -- bytes encode --

#[test]
fn test_bytes_encode_empty() {
    let code = [FUNCTION, X01, 6, 0, FIN, X01, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let result: VmBytes = vm.call_global(GlobalAddress::new(0), (VmBytes::view(b""),)).unwrap();
    assert_eq!(result.len(&vm), 0);
    assert!(result.is_empty(&vm));
}

#[test]
fn test_bytes_encode_passthrough() {
    let code = [FUNCTION, X01, 6, 0, FIN, X01, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let result: VmBytes = vm.call_global(GlobalAddress::new(0), (VmBytes::view(b"hello"),)).unwrap();
    assert_eq!(result.len(&vm), 5);
    assert_eq!(result.get(&vm, 0), b'h');
    assert_eq!(result.get(&vm, 1), b'e');
    assert_eq!(result.get(&vm, 4), b'o');
}

// -- bytes decode via VM concat --

#[test]
fn test_bytes_decode_concat() {
    let code = [
        FUNCTION, X01, 6, 0, FIN, X01,
        // body: concat(A1, A2)
        BYTES_CONCAT, X01, 2, 3,
        FIN, X01,
    ];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let result: VmBytes = vm
        .call_global(GlobalAddress::new(0), (VmBytes::view(b"foo"), VmBytes::view(b"bar")))
        .unwrap();
    assert_eq!(result.len(&vm), 6);

    let mut buf = [0u8; 6];
    let out = result.materialize(&vm, &mut buf);
    assert_eq!(out, b"foobar");
}

// -- bytes decode eq --

#[test]
fn test_bytes_eq_typed() {
    let code = [
        FUNCTION, X01, 6, 0, FIN, X01,
        BYTES_EQ, X01, 2, 3,
        FIN, X01,
    ];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let eq: bool = vm
        .call_global(GlobalAddress::new(0), (VmBytes::view(b"abc"), VmBytes::view(b"abc")))
        .unwrap();
    assert!(eq);

    let neq: bool = vm
        .call_global(GlobalAddress::new(0), (VmBytes::view(b"abc"), VmBytes::view(b"xyz")))
        .unwrap();
    assert!(!neq);
}

// -- type mismatch errors --

#[test]
fn test_bool_decode_type_mismatch() {
    use encore_vm::error::ExternError;
    use encore_vm::ffi::DecodeError;

    let code = [FUNCTION, X01, 6, 0, FIN, X01, INT, 2, 42, 0, 0, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let err = vm.call_global::<_, bool>(GlobalAddress::new(0), ()).unwrap_err();
    assert!(matches!(
        err,
        ExternError::Decode(DecodeError::TypeMismatch { expected: "bool (ctor tag 0 or 1)", .. })
    ));
}

#[test]
fn test_bytes_decode_type_mismatch() {
    use encore_vm::error::ExternError;
    use encore_vm::ffi::DecodeError;

    let code = [FUNCTION, X01, 6, 0, FIN, X01, INT, 2, 1, 0, 0, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let err = vm.call_global::<_, VmBytes>(GlobalAddress::new(0), ()).unwrap_err();
    assert!(matches!(
        err,
        ExternError::Decode(DecodeError::TypeMismatch { expected: "bytes", .. })
    ));
}

// ── VmList: build / iterate / round-trip ───────────────────────────────────

#[test]
fn test_vmlist_build_owned_roundtrip() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let input = VmList::<i32>::build(&mut vm, [1, 2, 3]).unwrap();
    let returned: VmList<i32> = vm.call_global(GlobalAddress::new(0), (input,)).unwrap();

    assert_eq!(collect_list(&vm, returned), vec![1, 2, 3]);
}

#[test]
fn test_vmlist_nil_roundtrip() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let input = VmList::<i32>::nil();
    let returned: VmList<i32> = vm.call_global(GlobalAddress::new(0), (input,)).unwrap();

    assert!(returned.is_nil());
    assert!(returned.next(&vm).is_none());
}

#[test]
fn test_vmlist_cons_chain_roundtrip() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let mut input = VmList::<i32>::nil();
    input = VmList::cons(&mut vm, 3, input).unwrap();
    input = VmList::cons(&mut vm, 2, input).unwrap();
    input = VmList::cons(&mut vm, 1, input).unwrap();

    let returned: VmList<i32> = vm.call_global(GlobalAddress::new(0), (input,)).unwrap();
    assert_eq!(collect_list(&vm, returned), vec![1, 2, 3]);
}

#[test]
fn test_vmlist_encode_passthrough_is_stable() {
    // A decoded VmList re-encoded via `ValueEncode for VmList<T>` must refer
    // to the same heap object — it's a handle, not a recipe.
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let first: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList(&[7, 8, 9]),)).unwrap();
    let second: VmList<i32> = vm.call_global(GlobalAddress::new(0), (first,)).unwrap();

    assert_eq!(first.as_value().to_u32(), second.as_value().to_u32());
    assert_eq!(collect_list(&vm, second), vec![7, 8, 9]);
}

#[test]
fn test_vmlist_decode_type_mismatch() {
    use encore_vm::error::ExternError;
    use encore_vm::ffi::DecodeError;

    // Function returns INT 1, which is not a ctor → decode-as-list fails.
    let code = [FUNCTION, X01, 6, 0, FIN, X01, INT, 2, 1, 0, 0, FIN, 2];
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &code, &[]);

    let err = vm.call_global::<_, VmList<i32>>(GlobalAddress::new(0), ()).unwrap_err();
    assert!(matches!(
        err,
        ExternError::Decode(DecodeError::TypeMismatch { expected: "List", .. })
    ));
}

// ── VmList::materialize ────────────────────────────────────────────────────

#[test]
fn test_vmlist_materialize_exact_fit() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let list: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList(&[10, 20, 30]),)).unwrap();
    let mut buf = [0i32; 3];
    let out = list.materialize(&vm, &mut buf);
    assert_eq!(out, &[10, 20, 30]);
}

#[test]
fn test_vmlist_materialize_buffer_shorter_than_list() {
    // Buffer caps the traversal: we only see the prefix that fits.
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let list: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList(&[1, 2, 3, 4, 5]),)).unwrap();
    let mut buf = [0i32; 3];
    let out = list.materialize(&vm, &mut buf);
    assert_eq!(out, &[1, 2, 3]);
}

#[test]
fn test_vmlist_materialize_buffer_longer_than_list() {
    // List ends before buffer fills: returned slice reflects the real length,
    // trailing capacity is left at its prior value.
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let list: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList(&[7, 8]),)).unwrap();
    let mut buf = [-1i32; 5];
    let out = list.materialize(&vm, &mut buf);
    assert_eq!(out, &[7, 8]);
    assert_eq!(&buf[2..], &[-1, -1, -1]);
}

#[test]
fn test_vmlist_materialize_nil_yields_empty_slice() {
    let vm_mem = [Value::from_u32(0); 32];
    let code = IDENTITY_CODE;
    let mut mem = vm_mem;
    let vm = make_vm(&mut mem, &code, &[]);

    let nil = VmList::<i32>::nil();
    let mut buf = [0i32; 4];
    let out = nil.materialize(&vm, &mut buf);
    assert!(out.is_empty());
}

// ── AsVmList: deferred-encoding writer ─────────────────────────────────────

#[test]
fn test_asvmlist_slice_argument() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let returned: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList(&[10, 20, 30]),)).unwrap();
    assert_eq!(collect_list(&vm, returned), vec![10, 20, 30]);
}

#[test]
fn test_asvmlist_empty_slice_builds_nil() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let returned: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList::<i32>(&[]),)).unwrap();
    assert!(returned.is_nil());
}

#[test]
fn test_asvmlist_and_build_agree() {
    // The slice writer and the owned-iterator constructor must produce
    // identical list contents; the handles differ but the elements match.
    let mut mem = [Value::from_u32(0); 512];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let from_as: VmList<i32> = vm.call_global(GlobalAddress::new(0), (AsVmList(&[4, 5, 6]),)).unwrap();
    let from_build = VmList::<i32>::build(&mut vm, [4, 5, 6]).unwrap();
    let from_build_ret: VmList<i32> = vm.call_global(GlobalAddress::new(0), (from_build,)).unwrap();

    assert_eq!(collect_list(&vm, from_as), collect_list(&vm, from_build_ret));
}

// ── VmBytes: direct constructors + round-trip ──────────────────────────────

#[test]
fn test_vmbytes_build_direct_roundtrip() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let input = VmBytes::build(&mut vm, b"hello").unwrap();
    let returned: VmBytes = vm.call_global(GlobalAddress::new(0), (input,)).unwrap();

    assert_eq!(returned.len(&vm), 5);
    let mut buf = [0u8; 5];
    assert_eq!(returned.materialize(&vm, &mut buf), b"hello");
}

#[test]
fn test_vmbytes_empty_direct_roundtrip() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let input = VmBytes::empty(&mut vm).unwrap();
    let returned: VmBytes = vm.call_global(GlobalAddress::new(0), (input,)).unwrap();

    assert!(returned.is_empty(&vm));
    assert_eq!(returned.len(&vm), 0);
}

#[test]
fn test_vmbytes_encode_passthrough_is_stable() {
    // Same invariant as VmList: round-tripping a decoded VmBytes handle
    // must preserve its heap identity.
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let first: VmBytes = vm.call_global(GlobalAddress::new(0), (VmBytes::view(b"pass"),)).unwrap();
    let second: VmBytes = vm.call_global(GlobalAddress::new(0), (first,)).unwrap();

    assert_eq!(first.as_value().to_u32(), second.as_value().to_u32());
    assert_eq!(second.len(&vm), 4);
    let mut buf = [0u8; 4];
    assert_eq!(second.materialize(&vm, &mut buf), b"pass");
}

// ── AsVmBytes: deferred-encoding writer ────────────────────────────────────

#[test]
fn test_asvmbytes_argument() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let returned: VmBytes = vm.call_global(GlobalAddress::new(0), (AsVmBytes(b"world"),)).unwrap();

    assert_eq!(returned.len(&vm), 5);
    let mut buf = [0u8; 5];
    assert_eq!(returned.materialize(&vm, &mut buf), b"world");
}

#[test]
fn test_asvmbytes_empty_argument() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    let returned: VmBytes = vm.call_global(GlobalAddress::new(0), (AsVmBytes(b""),)).unwrap();
    assert!(returned.is_empty(&vm));
}

// -- Typed extern via `extern_fn!` macro --

/// Typed handler: i32 in, i32 out. Doubles the argument.
fn double_handler(_vm: &mut Vm, n: i32) -> Result<i32, ExternError> {
    Ok(n * 2)
}

#[test]
fn test_call_global_bare_arg() {
    let mut mem = [Value::from_u32(0); 256];
    let mut vm = make_vm(&mut mem, &IDENTITY_CODE, &[]);

    // Bare value instead of (7i32,).
    let out: i32 = vm.call_global(GlobalAddress::new(0), 7i32).unwrap();
    assert_eq!(out, 7);
}

#[test]
fn test_extern_fn_macro_roundtrip() {
    // Global 0: returns extern(0)(A1). Opcodes:
    //   FUNCTION X01 6 0, FIN X01,  <-- trampoline: returns function ptr
    //   EXTERN X01 2 0 0, FIN X01   <-- body at offset 6: X01 = extern(0)(A1); return X01
    let code = [
        FUNCTION, X01, 6, 0, FIN, X01,
        EXTERN, X01, 2, 0, 0, FIN, X01,
    ];
    let mut mem = [Value::from_u32(0); 256];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut vm = Vm::init(&mut mem);
    vm.register_extern(0, encore_vm::extern_fn!(double_handler));
    vm.load(&prog).unwrap();

    let out: i32 = vm.call_global(GlobalAddress::new(0), (21i32,)).unwrap();
    assert_eq!(out, 42);
}
