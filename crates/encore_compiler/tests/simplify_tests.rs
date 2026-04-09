use encore_compiler::ir::cps::*;
use encore_compiler::ir::prim::PrimOp;
use encore_compiler::pass::rewrite;
use encore_compiler::pass::simplify::*;

fn n(s: &str) -> String {
    s.to_string()
}

fn var(s: &str) -> Val {
    Val::Var(n(s))
}

fn int(v: i32) -> Val {
    Val::Int(v)
}

fn cont_(param: &str, body: Expr) -> Val {
    Val::Cont(Cont { param: n(param), body: Box::new(body) })
}

fn fun_(arg: &str, cont: &str, body: Expr) -> Fun {
    Fun { arg: n(arg), cont: n(cont), body: Box::new(body) }
}

fn let_(name: &str, val: Val, body: Expr) -> Expr {
    Expr::Let(n(name), val, Box::new(body))
}

fn letrec(name: &str, fun: Fun, body: Expr) -> Expr {
    Expr::Letrec(n(name), fun, Box::new(body))
}

fn return_(k: &str, x: &str) -> Expr {
    Expr::Return(n(k), n(x))
}

fn encore(f: &str, x: &str, k: &str) -> Expr {
    Expr::Encore(n(f), n(x), n(k))
}

fn fin(name: &str) -> Expr {
    Expr::Fin(n(name))
}

fn match_(name: &str, base: Tag, cases: Vec<Case>) -> Expr {
    Expr::Match(n(name), base, cases)
}

fn case(binds: &[&str], body: Expr) -> Case {
    Case { binds: binds.iter().map(|s| n(s)).collect(), body }
}

fn ctor(tag: Tag, fields: &[&str]) -> Val {
    Val::Ctor(tag, fields.iter().map(|s| n(s)).collect())
}

fn field(name: &str, idx: u8) -> Val {
    Val::Field(n(name), idx)
}

fn prim(op: PrimOp, args: &[&str]) -> Val {
    Val::Prim(op, args.iter().map(|s| n(s)).collect())
}

// ── Dead code elimination ──────────────────────────────────────────────────

#[test]
fn test_dead_code_unused_let() {
    // let x = 42 in fin y  ──►  fin y
    let expr = let_("x", int(42), fin("y"));
    assert_eq!(dead_code(expr), fin("y"));
}

#[test]
fn test_dead_code_used_let() {
    // let x = 42 in fin x  ──►  unchanged
    let expr = let_("x", int(42), fin("x"));
    assert_eq!(dead_code(expr.clone()), expr);
}

#[test]
fn test_dead_code_unused_letrec() {
    // letrec f = fun(x, k). return k x in fin y  ──►  fin y
    let expr = letrec("f", fun_("x", "k", return_("k", "x")), fin("y"));
    assert_eq!(dead_code(expr), fin("y"));
}

#[test]
fn test_dead_code_used_letrec() {
    // letrec f = fun(x, k). fin x in fin f  ──►  unchanged
    let expr = letrec("f", fun_("x", "k", fin("x")), fin("f"));
    assert_eq!(dead_code(expr.clone()), expr);
}

#[test]
fn test_dead_code_nested() {
    // let a = 1 in let b = 2 in fin a  ──►  let a = 1 in fin a
    let expr = let_("a", int(1), let_("b", int(2), fin("a")));
    assert_eq!(dead_code(expr), let_("a", int(1), fin("a")));
}

#[test]
fn test_dead_code_inside_cont() {
    // let k = cont(x). let dead = 1 in fin x in fin k
    let inner = cont_("x", let_("dead", int(1), fin("x")));
    let expr = let_("k", inner, fin("k"));
    let expected = let_("k", cont_("x", fin("x")), fin("k"));
    assert_eq!(dead_code(expr), expected);
}

#[test]
fn test_dead_code_inside_match() {
    // match s 0 [ fin x | let dead = 1 in fin y ]
    let expr = match_("s", 0, vec![
        case(&[], fin("x")),
        case(&[], let_("dead", int(1), fin("y"))),
    ]);
    let expected = match_("s", 0, vec![
        case(&[], fin("x")),
        case(&[], fin("y")),
    ]);
    assert_eq!(dead_code(expr), expected);
}

// ── Copy propagation ──────────────────────────────────────────────────────

#[test]
fn test_copy_prop_simple() {
    // let y = x in fin y  ──►  fin x
    let expr = let_("y", var("x"), fin("y"));
    assert_eq!(copy_propagation(expr), fin("x"));
}

#[test]
fn test_copy_prop_chain() {
    // let y = x in let z = y in fin z  ──►  fin x
    let expr = let_("y", var("x"), let_("z", var("y"), fin("z")));
    assert_eq!(copy_propagation(expr), fin("x"));
}

#[test]
fn test_copy_prop_in_return() {
    // let y = x in return f y  ──►  return f x
    let expr = let_("y", var("x"), return_("f", "y"));
    assert_eq!(copy_propagation(expr), return_("f", "x"));
}

#[test]
fn test_copy_prop_non_var_untouched() {
    // let y = 42 in fin y  ──►  unchanged (not a Var)
    let expr = let_("y", int(42), fin("y"));
    assert_eq!(copy_propagation(expr.clone()), expr);
}

#[test]
fn test_copy_prop_inside_cont() {
    // let k = cont(x). let y = x in fin y in fin k
    let inner = cont_("x", let_("y", var("x"), fin("y")));
    let expr = let_("k", inner, fin("k"));
    let expected = let_("k", cont_("x", fin("x")), fin("k"));
    assert_eq!(copy_propagation(expr), expected);
}

// ── Constant folding ───────────────────────────────────────────────────────

#[test]
fn test_const_fold_add() {
    // let a = 3 in let b = 4 in let c = add(a, b) in fin c
    let expr = let_("a", int(3),
        let_("b", int(4),
            let_("c", prim(PrimOp::Add, &["a", "b"]),
                fin("c"))));
    let result = constant_fold(expr);
    let expected = let_("a", int(3),
        let_("b", int(4),
            let_("c", int(7),
                fin("c"))));
    assert_eq!(result, expected);
}

#[test]
fn test_const_fold_sub() {
    let expr = let_("a", int(10),
        let_("b", int(3),
            let_("c", prim(PrimOp::Sub, &["a", "b"]),
                fin("c"))));
    let result = constant_fold(expr);
    let expected = let_("a", int(10),
        let_("b", int(3),
            let_("c", int(7),
                fin("c"))));
    assert_eq!(result, expected);
}

#[test]
fn test_const_fold_mul() {
    let expr = let_("a", int(6),
        let_("b", int(7),
            let_("c", prim(PrimOp::Mul, &["a", "b"]),
                fin("c"))));
    let result = constant_fold(expr);
    let expected = let_("a", int(6),
        let_("b", int(7),
            let_("c", int(42),
                fin("c"))));
    assert_eq!(result, expected);
}

#[test]
fn test_const_fold_chained() {
    // let a = 2 in let b = 3 in let c = add(a, b) in let d = mul(c, a) in fin d
    let expr = let_("a", int(2),
        let_("b", int(3),
            let_("c", prim(PrimOp::Add, &["a", "b"]),
                let_("d", prim(PrimOp::Mul, &["c", "a"]),
                    fin("d")))));
    let result = constant_fold(expr);
    let expected = let_("a", int(2),
        let_("b", int(3),
            let_("c", int(5),
                let_("d", int(10),
                    fin("d")))));
    assert_eq!(result, expected);
}

#[test]
fn test_const_fold_unknown_operand() {
    // let c = add(a, b) in fin c  ──►  unchanged (a, b not known)
    let expr = let_("c", prim(PrimOp::Add, &["a", "b"]), fin("c"));
    assert_eq!(constant_fold(expr.clone()), expr);
}

#[test]
fn test_const_fold_eq_not_folded() {
    // Eq/Lt produce ctors, not ints — should not be folded
    let expr = let_("a", int(3),
        let_("b", int(3),
            let_("c", prim(PrimOp::Eq, &["a", "b"]),
                fin("c"))));
    assert_eq!(constant_fold(expr.clone()), expr);
}

#[test]
fn test_const_fold_lt_not_folded() {
    let expr = let_("a", int(1),
        let_("b", int(2),
            let_("c", prim(PrimOp::Lt, &["a", "b"]),
                fin("c"))));
    assert_eq!(constant_fold(expr.clone()), expr);
}

// ── Beta contraction ───────────────────────────────────────────────────────

#[test]
fn test_beta_simple() {
    // let k = cont(x). fin x in return k arg  ──►  fin arg
    let expr = let_("k", cont_("x", fin("x")), return_("k", "arg"));
    assert_eq!(beta_contraction(expr), fin("arg"));
}

#[test]
fn test_beta_with_body() {
    // let k = cont(x). let r = field 0 of x in fin r in return k arg
    // ──►  let r = field 0 of arg in fin r
    let expr = let_("k",
        cont_("x", let_("r", field("x", 0), fin("r"))),
        return_("k", "arg"));
    let expected = let_("r", field("arg", 0), fin("r"));
    assert_eq!(beta_contraction(expr), expected);
}

#[test]
fn test_beta_nested_in_let() {
    // let k = cont(x). fin x in let y = 42 in return k arg
    // ──►  let y = 42 in fin arg
    let expr = let_("k",
        cont_("x", fin("x")),
        let_("y", int(42), return_("k", "arg")));
    let expected = let_("y", int(42), fin("arg"));
    assert_eq!(beta_contraction(expr), expected);
}

#[test]
fn test_beta_multi_use_not_inlined() {
    // let k = cont(x). fin x in let a = Ctor(0, [k]) in return k arg
    // k is used twice (in Ctor and in Return), should not inline
    let expr = let_("k",
        cont_("x", fin("x")),
        let_("a", ctor(0, &["k"]), return_("k", "arg")));
    assert_eq!(beta_contraction(expr.clone()), expr);
}

#[test]
fn test_beta_in_match_branch() {
    // let k = cont(x). fin x in match s 0 [ return k a | fin b ]
    // ──►  match s 0 [ fin a | fin b ]
    let expr = let_("k",
        cont_("x", fin("x")),
        match_("s", 0, vec![
            case(&[], return_("k", "a")),
            case(&[], fin("b")),
        ]));
    let expected = match_("s", 0, vec![
        case(&[], fin("a")),
        case(&[], fin("b")),
    ]);
    assert_eq!(beta_contraction(expr), expected);
}

#[test]
fn test_beta_inside_cont() {
    // let outer = cont(z). let k = cont(x). fin x in return k z in fin outer
    let inner = cont_("z", let_("k", cont_("x", fin("x")), return_("k", "z")));
    let expr = let_("outer", inner, fin("outer"));
    let expected = let_("outer", cont_("z", fin("z")), fin("outer"));
    assert_eq!(beta_contraction(expr), expected);
}

// ── Eta reduction ──────────────────────────────────────────────────────────

#[test]
fn test_eta_simple() {
    // let g = cont(x). return f x in fin g  ──►  let g = f in fin g
    let expr = let_("g", cont_("x", return_("f", "x")), fin("g"));
    let expected = let_("g", var("f"), fin("g"));
    assert_eq!(eta_reduction(expr), expected);
}

#[test]
fn test_eta_not_forwarding() {
    // let g = cont(x). let r = 1 in fin x in fin g  ──►  unchanged
    let expr = let_("g", cont_("x", let_("r", int(1), fin("x"))), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

#[test]
fn test_eta_self_reference() {
    // let g = cont(x). return x x in fin g  ──►  unchanged (f == param)
    let expr = let_("g", cont_("x", return_("x", "x")), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

#[test]
fn test_eta_wrong_arg() {
    // let g = cont(x). return f y in fin g  ──►  unchanged (arg != param)
    let expr = let_("g", cont_("x", return_("f", "y")), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

#[test]
fn test_eta_inside_cont() {
    // let outer = cont(z). let g = cont(x). return f x in return g z in fin outer
    let inner = cont_("z", let_("g", cont_("x", return_("f", "x")), return_("g", "z")));
    let expr = let_("outer", inner, fin("outer"));
    let expected = let_("outer", cont_("z", let_("g", var("f"), return_("g", "z"))), fin("outer"));
    assert_eq!(eta_reduction(expr), expected);
}

#[test]
fn test_eta_letrec_not_reduced() {
    // letrec should not be eta-reduced (only Let with Cont)
    let expr = letrec("g", fun_("x", "k", encore("f", "x", "k")), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

// ── Interaction between passes ─────────────────────────────────────────────

#[test]
fn test_eta_then_copy_prop() {
    // let g = cont(x). return f x in return g arg
    // eta:  let g = f in return g arg
    // copy: return f arg
    let expr = let_("g", cont_("x", return_("f", "x")), return_("g", "arg"));
    let after_eta = eta_reduction(expr);
    let after_copy = copy_propagation(after_eta);
    assert_eq!(after_copy, return_("f", "arg"));
}

#[test]
fn test_beta_then_dead_code() {
    // let k = cont(x). fin x in let unused = 99 in return k arg
    // beta: let unused = 99 in fin arg
    // dead: fin arg
    let expr = let_("k",
        cont_("x", fin("x")),
        let_("unused", int(99), return_("k", "arg")));
    let after_beta = beta_contraction(expr);
    let after_dead = dead_code(after_beta);
    assert_eq!(after_dead, fin("arg"));
}

#[test]
fn test_const_fold_then_dead_code() {
    // let a = 3 in let b = 4 in let c = add(a, b) in fin c
    // fold: let a = 3 in let b = 4 in let c = 7 in fin c
    // dead: let c = 7 in fin c  (a, b unused)
    let expr = let_("a", int(3),
        let_("b", int(4),
            let_("c", prim(PrimOp::Add, &["a", "b"]),
                fin("c"))));
    let after_fold = constant_fold(expr);
    let after_dead = dead_code(after_fold);
    assert_eq!(after_dead, let_("c", int(7), fin("c")));
}

// ── Inlining ───────────────────────────────────────────────────────────────

#[test]
fn test_inline_small_cont() {
    // let f = cont(x). fin x in return f arg  ──►  let f = cont(x). fin x in fin arg
    let expr = let_("f", cont_("x", fin("x")), return_("f", "arg"));
    let result = rewrite::inlining(expr, 20);
    let expected = let_("f", cont_("x", fin("x")), fin("arg"));
    assert_eq!(result, expected);
}

#[test]
fn test_inline_too_large() {
    // Body size > threshold → not inlined
    let big_body = let_("a", int(1),
        let_("b", int(2),
            let_("c", int(3), fin("c"))));
    let expr = let_("f", cont_("x", big_body.clone()), return_("f", "arg"));
    let result = rewrite::inlining(expr.clone(), 2);
    assert_eq!(result, expr);
}

#[test]
fn test_inline_letrec_not_inlined() {
    // Recursive functions should not be inlined by cont inlining
    let expr = letrec("f", fun_("x", "k", encore("f", "x", "k")),
        let_("k0", cont_("r", fin("r")), encore("f", "arg", "k0")));
    let result = rewrite::inlining(expr.clone(), 100);
    assert_eq!(result, expr);
}

#[test]
fn test_inline_multiple_call_sites() {
    // let f = cont(x). fin x in match a 0 [ return f x | return f y ]
    // Both call sites get inlined
    let expr = let_("f", cont_("x", fin("x")),
        match_("a", 0, vec![
            case(&[], return_("f", "x")),
            case(&[], return_("f", "y")),
        ]));
    let expected = let_("f", cont_("x", fin("x")),
        match_("a", 0, vec![
            case(&[], fin("x")),
            case(&[], fin("y")),
        ]));
    let result = rewrite::inlining(expr, 20);
    assert_eq!(result, expected);
}

#[test]
fn test_inline_with_substitution() {
    // let f = cont(x). let r = field 0 of x in fin r in return f arg
    // ──►  let f = (...) in let r = field 0 of arg in fin r
    let expr = let_("f",
        cont_("x", let_("r", field("x", 0), fin("r"))),
        return_("f", "arg"));
    let expected = let_("f",
        cont_("x", let_("r", field("x", 0), fin("r"))),
        let_("r", field("arg", 0), fin("r")));
    let result = rewrite::inlining(expr, 20);
    assert_eq!(result, expected);
}

#[test]
fn test_inline_nested_let() {
    // let f = cont(x). fin x in let y = 1 in return f arg
    // ──►  let f = (...) in let y = 1 in fin arg
    let expr = let_("f", cont_("x", fin("x")),
        let_("y", int(1), return_("f", "arg")));
    let expected = let_("f", cont_("x", fin("x")),
        let_("y", int(1), fin("arg")));
    let result = rewrite::inlining(expr, 20);
    assert_eq!(result, expected);
}

#[test]
fn test_inline_then_dead_code() {
    // After inlining all call sites, dead_code removes the binding
    // let f = cont(x). fin x in return f arg  →inline→  let f = (...) in fin arg  →dead→  fin arg
    let expr = let_("f", cont_("x", fin("x")), return_("f", "arg"));
    let after_inline = rewrite::inlining(expr, 20);
    let after_dead = dead_code(after_inline);
    assert_eq!(after_dead, fin("arg"));
}
