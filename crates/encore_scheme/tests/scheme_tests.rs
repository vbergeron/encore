use encore_compiler::pipeline;
use encore_scheme;
use encore_vm::program::Program;
use encore_vm::value::{Value, GlobalAddress};
use encore_vm::vm::Vm;

// -- Scheme: fold_left with match+call init (inliner bug reproducer) --

fn run_scheme_int(source: &str) -> i32 {
    let module = encore_scheme::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 8192];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    vm.global_raw(GlobalAddress::new(last as u16)).int_value().unwrap()
}

#[test]
fn test_scheme_fold_left_match_init() {
    let result = run_scheme_int("
        (define fold_left (lambdas (f l a0)
          (match l
             ((Nil) a0)
             ((Cons b l0) (@ fold_left f l0 (@ f a0 b))))))

        (define go (lambdas (ops n0 lst)
          (@ fold_left
            (lambdas (acc n) (+ acc n))
            lst
            (match ops
              ((Box h) (h n0))))))

        (define id (lambda (x) x))

        (define main (@ go `(Box ,id) 42 `(Cons ,1 ,`(Nil))))
    ");
    assert_eq!(result, 43);
}

// -- Scheme: bytes_of_list_nat --

fn run_scheme_bytes(source: &str) -> Vec<u8> {
    let module = encore_scheme::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 8192];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let result = vm.global_raw(GlobalAddress::new(last as u16));
    assert!(result.is_bytes(), "expected bytes, got {}", result.type_name());
    let len = vm.bytes_len(result);
    (0..len).map(|i| vm.bytes_read(result, i)).collect()
}

const BYTES_OF_LIST_NAT_PRELUDE: &str = "
(define fold_left (lambdas (f l a0)
  (match l
     ((Nil) a0)
     ((Cons b l0) (@ fold_left f l0 (@ f a0 b))))))

(define bytes_of_list_nat (lambdas (bytesOps n0 lst)
  (@ fold_left (lambdas (acc n)
    (match bytesOps
       ((Build_BytesOps _ _ _ bytes_concat0)
         (@ bytes_concat0 acc
           (match bytesOps
              ((Build_BytesOps _ _ bytes_of_nat0 _) (bytes_of_nat0 n)))))))
    lst
    (match bytesOps
       ((Build_BytesOps _ _ bytes_of_nat0 _) (bytes_of_nat0 n0))))))

(define bytes_len (lambda (b) (bytes-len b)))
(define bytes_get (lambdas (b i) (bytes-get b i)))
(define bytes_of_nat (lambda (i) (int->byte i)))
(define bytes_concat (lambdas (l r) (bytes-concat l r)))
(define bytes_ops `(Build_BytesOps ,bytes_len ,bytes_get ,bytes_of_nat
  ,bytes_concat))
";

#[test]
fn test_scheme_bytes_of_list_nat_singleton() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 42 `(Nil)))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![42]);
}

#[test]
fn test_scheme_bytes_of_list_nat_two_elements() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 1
          `(Cons ,2 ,`(Nil))))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![1, 2]);
}

#[test]
fn test_scheme_bytes_of_list_nat_multi() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 1
          `(Cons ,2 ,`(Cons ,3 ,`(Nil)))))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![1, 2, 3]);
}

#[test]
fn test_scheme_bytes_of_list_nat_boundary_values() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 0
          `(Cons ,127 ,`(Cons ,255 ,`(Nil)))))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![0, 127, 255]);
}
