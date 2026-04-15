use encore_compiler::ir::cps::*;
use encore_compiler::ir::prim::{PrimOp, IntOp};
use encore_compiler::pass::cps_rewrite;
use encore_compiler::pass::cps_simplify::*;

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
    Val::Cont(Cont { params: vec![n(param)], body: Box::new(body) })
}

fn fun_(arg: &str, cont: &str, body: Expr) -> Fun {
    Fun { args: vec![n(arg)], cont: n(cont), body: Box::new(body) }
}

fn let_(name: &str, val: Val, body: Expr) -> Expr {
    Expr::Let(n(name), val, Box::new(body))
}

fn letrec(name: &str, fun: Fun, body: Expr) -> Expr {
    Expr::Letrec(n(name), fun, Box::new(body))
}

fn return_(k: &str, x: &str) -> Expr {
    Expr::Let("_nc".into(), Val::NullCont, Box::new(Expr::Encore(n(k), vec![n(x)], "_nc".into())))
}

fn encore(f: &str, x: &str, k: &str) -> Expr {
    Expr::Encore(n(f), vec![n(x)], n(k))
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
            let_("c", prim(PrimOp::Int(IntOp::Add), &["a", "b"]),
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
            let_("c", prim(PrimOp::Int(IntOp::Sub), &["a", "b"]),
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
            let_("c", prim(PrimOp::Int(IntOp::Mul), &["a", "b"]),
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
            let_("c", prim(PrimOp::Int(IntOp::Add), &["a", "b"]),
                let_("d", prim(PrimOp::Int(IntOp::Mul), &["c", "a"]),
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
    let expr = let_("c", prim(PrimOp::Int(IntOp::Add), &["a", "b"]), fin("c"));
    assert_eq!(constant_fold(expr.clone()), expr);
}

#[test]
fn test_const_fold_eq_true() {
    // eq(3, 3) → Ctor(1, []) (True)
    let expr = let_("a", int(3),
        let_("b", int(3),
            let_("c", prim(PrimOp::Int(IntOp::Eq), &["a", "b"]),
                fin("c"))));
    let expected = let_("a", int(3),
        let_("b", int(3),
            let_("c", ctor(1, &[]),
                fin("c"))));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_const_fold_eq_false() {
    // eq(3, 4) → Ctor(0, []) (False)
    let expr = let_("a", int(3),
        let_("b", int(4),
            let_("c", prim(PrimOp::Int(IntOp::Eq), &["a", "b"]),
                fin("c"))));
    let expected = let_("a", int(3),
        let_("b", int(4),
            let_("c", ctor(0, &[]),
                fin("c"))));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_const_fold_lt_true() {
    // lt(1, 2) → Ctor(1, []) (True)
    let expr = let_("a", int(1),
        let_("b", int(2),
            let_("c", prim(PrimOp::Int(IntOp::Lt), &["a", "b"]),
                fin("c"))));
    let expected = let_("a", int(1),
        let_("b", int(2),
            let_("c", ctor(1, &[]),
                fin("c"))));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_const_fold_lt_false() {
    // lt(5, 3) → Ctor(0, []) (False)
    let expr = let_("a", int(5),
        let_("b", int(3),
            let_("c", prim(PrimOp::Int(IntOp::Lt), &["a", "b"]),
                fin("c"))));
    let expected = let_("a", int(5),
        let_("b", int(3),
            let_("c", ctor(0, &[]),
                fin("c"))));
    assert_eq!(constant_fold(expr), expected);
}

// ── Known-case elimination ────────────────────────────────────────────────

#[test]
fn test_known_case_nullary() {
    // let x = Ctor(1, []) in match x base=0 [ fin a | fin b ]
    // tag=1, branch=1 → fin b
    let expr = let_("x", ctor(1, &[]),
        match_("x", 0, vec![
            case(&[], fin("a")),
            case(&[], fin("b")),
        ]));
    let expected = let_("x", ctor(1, &[]), fin("b"));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_known_case_with_fields() {
    // let p = Ctor(0, [x, y]) in match p base=0 [ case(a,b) -> fin a ]
    // branch=0, substitute a→x, b→y → fin x
    let expr = let_("p", ctor(0, &["x", "y"]),
        match_("p", 0, vec![
            case(&["a", "b"], fin("a")),
        ]));
    let expected = let_("p", ctor(0, &["x", "y"]), fin("x"));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_known_case_with_base_offset() {
    // let x = Ctor(3, []) in match x base=2 [ fin a | fin b ]
    // branch = 3 - 2 = 1 → fin b
    let expr = let_("x", ctor(3, &[]),
        match_("x", 2, vec![
            case(&[], fin("a")),
            case(&[], fin("b")),
        ]));
    let expected = let_("x", ctor(3, &[]), fin("b"));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_fold_comparison_then_match() {
    // let a=3 in let b=3 in let r=eq(a,b) in match r [ fin no | fin yes ]
    // eq folds to Ctor(1,[]), match selects branch 1 → fin yes
    let expr = let_("a", int(3),
        let_("b", int(3),
            let_("r", prim(PrimOp::Int(IntOp::Eq), &["a", "b"]),
                match_("r", 0, vec![
                    case(&[], fin("no")),
                    case(&[], fin("yes")),
                ]))));
    let expected = let_("a", int(3),
        let_("b", int(3),
            let_("r", ctor(1, &[]),
                fin("yes"))));
    assert_eq!(constant_fold(expr), expected);
}

// ── Known-field projection ────────────────────────────────────────────────

#[test]
fn test_known_field_projection() {
    // let p = Ctor(0, [x, y]) in let a = field 0 of p in fin a
    // → let p = Ctor(0, [x, y]) in let a = x in fin a
    let expr = let_("p", ctor(0, &["x", "y"]),
        let_("a", field("p", 0), fin("a")));
    let expected = let_("p", ctor(0, &["x", "y"]),
        let_("a", var("x"), fin("a")));
    assert_eq!(constant_fold(expr), expected);
}

#[test]
fn test_known_field_second() {
    // field 1 of Ctor(0, [x, y]) → y
    let expr = let_("p", ctor(0, &["x", "y"]),
        let_("a", field("p", 1), fin("a")));
    let expected = let_("p", ctor(0, &["x", "y"]),
        let_("a", var("y"), fin("a")));
    assert_eq!(constant_fold(expr), expected);
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
            let_("c", prim(PrimOp::Int(IntOp::Add), &["a", "b"]),
                fin("c"))));
    let after_fold = constant_fold(expr);
    let after_dead = dead_code(after_fold);
    assert_eq!(after_dead, let_("c", int(7), fin("c")));
}

// ── Inlining ───────────────────────────────────────────────────────────────

#[test]
fn test_inline_small_cont() {
    // let f = cont(x). fin x in let _nc = nullcont in encore f arg _nc
    // ──►  let f = cont(x). fin x in let _nc = nullcont in fin arg
    let expr = let_("f", cont_("x", fin("x")), return_("f", "arg"));
    let result = cps_rewrite::inlining(expr, 20, &Default::default());
    let expected = let_("f", cont_("x", fin("x")),
        let_("_nc", Val::NullCont, fin("arg")));
    assert_eq!(result, expected);
}

#[test]
fn test_inline_too_large() {
    // Body size > threshold → not inlined
    let big_body = let_("a", int(1),
        let_("b", int(2),
            let_("c", int(3), fin("c"))));
    let expr = let_("f", cont_("x", big_body.clone()), return_("f", "arg"));
    let result = cps_rewrite::inlining(expr.clone(), 2, &Default::default());
    assert_eq!(result, expr);
}

#[test]
fn test_inline_letrec_not_inlined() {
    // Recursive functions should not be inlined by cont inlining
    let expr = letrec("f", fun_("x", "k", encore("f", "x", "k")),
        let_("k0", cont_("r", fin("r")), encore("f", "arg", "k0")));
    let result = cps_rewrite::inlining(expr.clone(), 100, &Default::default());
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
            case(&[], let_("_nc", Val::NullCont, fin("x"))),
            case(&[], let_("_nc", Val::NullCont, fin("y"))),
        ]));
    let result = cps_rewrite::inlining(expr, 20, &Default::default());
    assert_eq!(result, expected);
}

#[test]
fn test_inline_with_substitution() {
    // let f = cont(x). let r = field 0 of x in fin r in return f arg
    // ──►  let f = (...) in let _nc = nullcont in let r = field 0 of arg in fin r
    let expr = let_("f",
        cont_("x", let_("r", field("x", 0), fin("r"))),
        return_("f", "arg"));
    let expected = let_("f",
        cont_("x", let_("r", field("x", 0), fin("r"))),
        let_("_nc", Val::NullCont, let_("r", field("arg", 0), fin("r"))));
    let result = cps_rewrite::inlining(expr, 20, &Default::default());
    assert_eq!(result, expected);
}

#[test]
fn test_inline_nested_let() {
    // let f = cont(x). fin x in let y = 1 in return f arg
    // ──►  let f = (...) in let y = 1 in let _nc = nullcont in fin arg
    let expr = let_("f", cont_("x", fin("x")),
        let_("y", int(1), return_("f", "arg")));
    let expected = let_("f", cont_("x", fin("x")),
        let_("y", int(1), let_("_nc", Val::NullCont, fin("arg"))));
    let result = cps_rewrite::inlining(expr, 20, &Default::default());
    assert_eq!(result, expected);
}

#[test]
fn test_inline_then_dead_code() {
    // After inlining all call sites, dead_code removes the binding
    // let f = cont(x). fin x in return f arg  →inline→  let f = (...) in fin arg  →dead→  fin arg
    let expr = let_("f", cont_("x", fin("x")), return_("f", "arg"));
    let after_inline = cps_rewrite::inlining(expr, 20, &Default::default());
    let after_dead = dead_code(after_inline);
    assert_eq!(after_dead, fin("arg"));
}

// ── Hoisting ────────────────────────────────────────────────────────────────

#[test]
fn test_hoist_int_constant() {
    // letrec f(n, k) = let one = 1 in return k one in fin f
    // ──►  let one = 1 in let _nc = nullcont in letrec f(n, k) = encore k one _nc in fin f
    let expr = letrec("f",
        fun_("n", "k", let_("one", int(1), return_("k", "one"))),
        fin("f"));
    let expected = let_("one", int(1),
        let_("_nc", Val::NullCont,
            letrec("f",
                fun_("n", "k", encore("k", "one", "_nc")),
                fin("f"))));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_nothing_variant() {
    // letrec f(n, k) = let r = field 0 of n in return k r in fin f
    // r depends on n → r stays, but _nc is hoisted
    let expr = letrec("f",
        fun_("n", "k", let_("r", field("n", 0), return_("k", "r"))),
        fin("f"));
    let expected = let_("_nc", Val::NullCont,
        letrec("f",
            fun_("n", "k", let_("r", field("n", 0), encore("k", "r", "_nc"))),
            fin("f")));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_chain() {
    // letrec f(n, k) = let a = 1 in let b = 2 in let c = add(a,b) in return k c in fin f
    // a, b, c, _nc are all invariant → all hoisted
    let expr = letrec("f",
        fun_("n", "k",
            let_("a", int(1),
                let_("b", int(2),
                    let_("c", prim(PrimOp::Int(IntOp::Add), &["a", "b"]),
                        return_("k", "c"))))),
        fin("f"));
    let expected =
        let_("a", int(1),
            let_("b", int(2),
                let_("c", prim(PrimOp::Int(IntOp::Add), &["a", "b"]),
                    let_("_nc", Val::NullCont,
                        letrec("f",
                            fun_("n", "k", encore("k", "c", "_nc")),
                            fin("f"))))));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_interleaved() {
    // letrec f(n, k) = let a = 1 in let b = n in let c = 2 in return k c in fin f
    // a hoistable, b variant (uses n), c hoistable, _nc hoistable
    let expr = letrec("f",
        fun_("n", "k",
            let_("a", int(1),
                let_("b", var("n"),
                    let_("c", int(2),
                        return_("k", "c"))))),
        fin("f"));
    let expected =
        let_("a", int(1),
            let_("c", int(2),
                let_("_nc", Val::NullCont,
                    letrec("f",
                        fun_("n", "k",
                            let_("b", var("n"),
                                encore("k", "c", "_nc"))),
                        fin("f")))));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_transitive_variant() {
    // letrec f(n, k) = let a = n in let b = add(a, x) in return k b in fin f
    // a is variant (uses n), b is variant (uses a), but _nc is hoisted
    let expr = letrec("f",
        fun_("n", "k",
            let_("a", var("n"),
                let_("b", prim(PrimOp::Int(IntOp::Add), &["a", "x"]),
                    return_("k", "b")))),
        fin("f"));
    let expected = let_("_nc", Val::NullCont,
        letrec("f",
            fun_("n", "k",
                let_("a", var("n"),
                    let_("b", prim(PrimOp::Int(IntOp::Add), &["a", "x"]),
                        encore("k", "b", "_nc")))),
            fin("f")));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_does_not_hoist_self_ref() {
    // letrec f(n, k) = let g = f in return k g in fin f
    // g references f (the letrec name) → not hoistable, but _nc is hoisted
    let expr = letrec("f",
        fun_("n", "k", let_("g", var("f"), return_("k", "g"))),
        fin("f"));
    let expected = let_("_nc", Val::NullCont,
        letrec("f",
            fun_("n", "k", let_("g", var("f"), encore("k", "g", "_nc"))),
            fin("f")));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_recurses_into_nested_letrec() {
    // let x = 1 in letrec f(n, k) = let a = 2 in return k a in fin f
    // The inner letrec should be processed, _nc is also hoisted
    let expr = let_("x", int(1),
        letrec("f",
            fun_("n", "k", let_("a", int(2), return_("k", "a"))),
            fin("f")));
    let expected = let_("x", int(1),
        let_("a", int(2),
            let_("_nc", Val::NullCont,
                letrec("f",
                    fun_("n", "k", encore("k", "a", "_nc")),
                    fin("f")))));
    assert_eq!(cps_rewrite::hoisting(expr), expected);
}

#[test]
fn test_hoist_no_letrec_unchanged() {
    // No letrec → nothing to hoist
    let expr = let_("a", int(1), fin("a"));
    assert_eq!(cps_rewrite::hoisting(expr.clone()), expr);
}

// ── CSE ─────────────────────────────────────────────────────────────────────

#[test]
fn test_cse_field() {
    // let a = field 0 of x in let b = field 0 of x in fin b
    // ──►  let a = field 0 of x in fin a
    let expr = let_("a", field("x", 0),
        let_("b", field("x", 0),
            fin("b")));
    let expected = let_("a", field("x", 0), fin("a"));
    assert_eq!(cps_rewrite::cse(expr), expected);
}

#[test]
fn test_cse_prim() {
    // let a = add(x,y) in let b = add(x,y) in fin b
    // ──►  let a = add(x,y) in fin a
    let expr = let_("a", prim(PrimOp::Int(IntOp::Add), &["x", "y"]),
        let_("b", prim(PrimOp::Int(IntOp::Add), &["x", "y"]),
            fin("b")));
    let expected = let_("a", prim(PrimOp::Int(IntOp::Add), &["x", "y"]), fin("a"));
    assert_eq!(cps_rewrite::cse(expr), expected);
}

#[test]
fn test_cse_ctor() {
    // let a = Ctor(0, [x, y]) in let b = Ctor(0, [x, y]) in fin b
    // ──►  let a = Ctor(0, [x, y]) in fin a
    let expr = let_("a", ctor(0, &["x", "y"]),
        let_("b", ctor(0, &["x", "y"]),
            fin("b")));
    let expected = let_("a", ctor(0, &["x", "y"]), fin("a"));
    assert_eq!(cps_rewrite::cse(expr), expected);
}

#[test]
fn test_cse_different_vals() {
    // Different field indices → no CSE
    let expr = let_("a", field("x", 0),
        let_("b", field("x", 1),
            fin("b")));
    assert_eq!(cps_rewrite::cse(expr.clone()), expr);
}

#[test]
fn test_cse_var_not_eliminated() {
    // Var is not a CSE candidate (copy propagation handles it)
    let expr = let_("a", var("x"), let_("b", var("x"), fin("b")));
    assert_eq!(cps_rewrite::cse(expr.clone()), expr);
}

#[test]
fn test_cse_chain() {
    // Three identical bindings → all collapse to the first
    let expr = let_("a", field("x", 0),
        let_("b", field("x", 0),
            let_("c", field("x", 0),
                fin("c"))));
    let expected = let_("a", field("x", 0), fin("a"));
    assert_eq!(cps_rewrite::cse(expr), expected);
}

#[test]
fn test_cse_inside_letrec() {
    // CSE works within a letrec body
    let expr = letrec("f",
        fun_("n", "k",
            let_("a", field("n", 0),
                let_("b", field("n", 0),
                    return_("k", "b")))),
        fin("f"));
    let expected = letrec("f",
        fun_("n", "k",
            let_("a", field("n", 0),
                return_("k", "a"))),
        fin("f"));
    assert_eq!(cps_rewrite::cse(expr), expected);
}

#[test]
fn test_cse_does_not_cross_letrec() {
    // Available expressions don't leak into letrec fun bodies
    let expr = let_("a", field("x", 0),
        letrec("f",
            fun_("n", "k",
                let_("b", field("x", 0), return_("k", "b"))),
            fin("f")));
    assert_eq!(cps_rewrite::cse(expr.clone()), expr);
}

#[test]
fn test_cse_match_bind_invalidates() {
    // match bind shadows 'x', so field 0 of x inside the case is NOT the same
    let expr = let_("a", field("x", 0),
        match_("s", 0, vec![
            case(&["x"], let_("b", field("x", 0), fin("b"))),
        ]));
    assert_eq!(cps_rewrite::cse(expr.clone()), expr);
}

#[test]
fn test_cse_across_match_branches() {
    // Available expressions flow into branches (no shadowing)
    let expr = let_("a", field("x", 0),
        match_("s", 0, vec![
            case(&[], let_("b", field("x", 0), fin("b"))),
            case(&[], fin("a")),
        ]));
    let expected = let_("a", field("x", 0),
        match_("s", 0, vec![
            case(&[], fin("a")),
            case(&[], fin("a")),
        ]));
    assert_eq!(cps_rewrite::cse(expr), expected);
}

// ── Contification ────────────────────────────────────────────────────────────

#[test]
fn test_contify_single_use_inline() {
    // letrec f = fun(x, k). return k x in encore f arg k0
    // f is non-recursive, used once → inline: return k0 arg
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        encore("f", "arg", "k0"));
    assert_eq!(cps_rewrite::contification(expr), return_("k0", "arg"));
}

#[test]
fn test_contify_single_use_with_body() {
    // letrec f = fun(x, k). let r = field 0 of x in return k r
    // in encore f arg k0
    // ──►  let r = field 0 of arg in return k0 r
    let expr = letrec("f",
        fun_("x", "k", let_("r", field("x", 0), return_("k", "r"))),
        encore("f", "arg", "k0"));
    let expected = let_("r", field("arg", 0), return_("k0", "r"));
    assert_eq!(cps_rewrite::contification(expr), expected);
}

#[test]
fn test_contify_single_use_nested_in_let() {
    // letrec f = fun(x, k). return k x in let a = 1 in encore f arg k0
    // ──►  let a = 1 in return k0 arg
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        let_("a", int(1), encore("f", "arg", "k0")));
    let expected = let_("a", int(1), return_("k0", "arg"));
    assert_eq!(cps_rewrite::contification(expr), expected);
}

#[test]
fn test_contify_single_use_in_match() {
    // letrec f = fun(x, k). return k x in match s 0 [ encore f a k0 | fin b ]
    // ──►  match s 0 [ return k0 a | fin b ]
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        match_("s", 0, vec![
            case(&[], encore("f", "a", "k0")),
            case(&[], fin("b")),
        ]));
    let expected = match_("s", 0, vec![
        case(&[], return_("k0", "a")),
        case(&[], fin("b")),
    ]);
    assert_eq!(cps_rewrite::contification(expr), expected);
}

#[test]
fn test_contify_recursive_not_touched() {
    // letrec f = fun(x, k). encore f x k in encore f arg k0
    // f is recursive → unchanged
    let expr = letrec("f",
        fun_("x", "k", encore("f", "x", "k")),
        encore("f", "arg", "k0"));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_escapes_as_value() {
    // letrec f = fun(x, k). return k x in fin f
    // f is used as a value (in Fin) → escapes, unchanged
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        fin("f"));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_escapes_as_arg() {
    // letrec f = fun(x, k). return k x in encore g f k0
    // f is passed as an argument → escapes, unchanged
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        encore("g", "f", "k0"));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_escapes_as_cont() {
    // letrec f = fun(x, k). return k x in encore g arg f
    // f is passed as continuation → escapes, unchanged
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        encore("g", "arg", "f"));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_escapes_in_ctor() {
    // letrec f = fun(x, k). return k x in let p = Ctor(0, [f]) in fin p
    // f captured in a Ctor → escapes
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        let_("p", ctor(0, &["f"]), fin("p")));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_multi_use_same_cont_in_scope() {
    // letrec f = fun(x, k). return k x in match s 0 [ encore f a k0 | encore f b k0 ]
    // f is non-recursive, used twice, both with k0, and k0 is NOT bound inside outer
    // ──►  let f = cont(x). return k0 x in match s 0 [ return f a | return f b ]
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        match_("s", 0, vec![
            case(&[], encore("f", "a", "k0")),
            case(&[], encore("f", "b", "k0")),
        ]));
    let expected = let_("f",
        cont_("x", return_("k0", "x")),
        match_("s", 0, vec![
            case(&[], return_("f", "a")),
            case(&[], return_("f", "b")),
        ]));
    assert_eq!(cps_rewrite::contification(expr), expected);
}

#[test]
fn test_contify_multi_use_different_conts() {
    // Two call sites with different continuations → cannot contify
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        let_("a", int(1),
            match_("s", 0, vec![
                case(&[], encore("f", "a", "k1")),
                case(&[], encore("f", "b", "k2")),
            ])));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_multi_use_cont_bound_inside() {
    // k0 is bound inside outer → not in scope at Letrec, skip
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        let_("k0", cont_("r", fin("r")),
            match_("s", 0, vec![
                case(&[], encore("f", "a", "k0")),
                case(&[], encore("f", "b", "k0")),
            ])));
    assert_eq!(cps_rewrite::contification(expr.clone()), expr);
}

#[test]
fn test_contify_cascading() {
    // Two nested non-recursive letrecs: inner f is contified first (bottom-up),
    // which exposes g as single-use, so g is contified in the same pass.
    let expr = letrec("g",
        fun_("y", "kg", return_("kg", "y")),
        letrec("f",
            fun_("x", "kf", encore("g", "x", "kf")),
            encore("f", "arg", "k0")));
    assert_eq!(cps_rewrite::contification(expr), return_("k0", "arg"));
}

#[test]
fn test_contify_recurses_into_let_body() {
    // Contification should find letrecs nested inside Let
    let expr = let_("a", int(1),
        letrec("f",
            fun_("x", "k", return_("k", "x")),
            encore("f", "a", "k0")));
    let expected = let_("a", int(1), return_("k0", "a"));
    assert_eq!(cps_rewrite::contification(expr), expected);
}

#[test]
fn test_contify_then_beta() {
    // After contification creates Let(Cont), beta contraction can inline it.
    let expr = letrec("f",
        fun_("x", "k", return_("k", "x")),
        match_("s", 0, vec![
            case(&[], encore("f", "a", "k0")),
            case(&[], encore("f", "b", "k0")),
        ]));
    let after_contify = cps_rewrite::contification(expr);
    let expected = let_("f",
        cont_("x", return_("k0", "x")),
        match_("s", 0, vec![
            case(&[], return_("f", "a")),
            case(&[], return_("f", "b")),
        ]));
    assert_eq!(after_contify, expected);
}

#[test]
fn test_contify_curried_lambda() {
    // Reproduces the map_filter bug pattern:
    //   letrec f = fun(x, kf). letrec g = fun(y, kg). return kg y in return kf g
    //   in let k = cont(r). fin r
    //      in encore f arg k
    //
    // f is non-recursive, single-use. After contification of f:
    //   let k = cont(r). fin r
    //   in letrec g = fun(y, kg). return kg y
    //      in return k g
    let expr = letrec("f",
        fun_("x", "kf",
            letrec("g",
                fun_("y", "kg", return_("kg", "y")),
                return_("kf", "g"))),
        let_("k", cont_("r", fin("r")),
            encore("f", "arg", "k")));
    let result = cps_rewrite::contification(expr);
    let expected = let_("k", cont_("r", fin("r")),
        letrec("g",
            fun_("y", "kg", return_("kg", "y")),
            return_("k", "g")));
    assert_eq!(result, expected);
}

#[test]
fn test_contify_two_curried_wrappers() {
    // Two curried wrappers with distinct inner names (as DSI would produce).
    // Both map_f and filt_f are single-use non-recursive: both get contified.
    // After contification + simplification, only the inner letrecs remain.
    let expr =
        letrec("map_f",
            fun_("f", "km",
                letrec("go_m",
                    fun_("y", "ky", return_("ky", "y")),
                    return_("km", "go_m"))),
        letrec("filt_f",
            fun_("p", "kf",
                letrec("go_f",
                    fun_("z", "kz", return_("kz", "z")),
                    return_("kf", "go_f"))),
        let_("k1", cont_("pm",
            let_("k2", cont_("pf",
                let_("k3", cont_("mapped",
                    encore("pf", "mapped", "k_halt")),
                    encore("pm", "list", "k3"))),
                encore("filt_f", "is_big", "k2"))),
            encore("map_f", "double", "k1"))));

    let after = cps_rewrite::contification(expr);
    let after = beta_contraction(after);
    let after = copy_propagation(after);
    let after = dead_code(after);

    // Both wrappers should be eliminated. The result should contain
    // go_m and go_f as distinct letrecs, not shadowing each other.
    fn collect_letrec_names(e: &encore_compiler::ir::cps::Expr, names: &mut Vec<String>) {
        use encore_compiler::ir::cps::{Expr, Val};
        match e {
            Expr::Letrec(name, fun, body) => {
                names.push(name.clone());
                collect_letrec_names(&fun.body, names);
                collect_letrec_names(body, names);
            }
            Expr::Let(_, val, body) => {
                if let Val::Cont(c) = val {
                    collect_letrec_names(&c.body, names);
                }
                collect_letrec_names(body, names);
            }
            Expr::Match(_, _, cases) => {
                for c in cases {
                    collect_letrec_names(&c.body, names);
                }
            }
            _ => {}
        }
    }
    let mut names = Vec::new();
    collect_letrec_names(&after, &mut names);
    assert!(names.contains(&"go_m".to_string()), "go_m should survive: {after:#?}");
    assert!(names.contains(&"go_f".to_string()), "go_f should survive: {after:#?}");
    assert!(!names.contains(&"map_f".to_string()), "map_f should be inlined: {after:#?}");
    assert!(!names.contains(&"filt_f".to_string()), "filt_f should be inlined: {after:#?}");
}
