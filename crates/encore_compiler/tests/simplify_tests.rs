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

fn lam(param: &str, body: Expr) -> Val {
    Val::Lambda(Lambda { param: n(param), body: Box::new(body) })
}

fn lambda(param: &str, body: Expr) -> Lambda {
    Lambda { param: n(param), body: Box::new(body) }
}

fn let_(name: &str, val: Val, body: Expr) -> Expr {
    Expr::Let(n(name), val, Box::new(body))
}

fn letrec(name: &str, lam: Lambda, body: Expr) -> Expr {
    Expr::Letrec(n(name), lam, Box::new(body))
}

fn app(f: &str, x: &str) -> Expr {
    Expr::App(n(f), n(x))
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
    // letrec f = (x -> x) in fin y  ──►  fin y
    let expr = letrec("f", lambda("x", app("f", "x")), fin("y"));
    assert_eq!(dead_code(expr), fin("y"));
}

#[test]
fn test_dead_code_used_letrec() {
    // letrec f = (x -> x) in f y  ──►  unchanged
    let expr = letrec("f", lambda("x", fin("x")), app("f", "y"));
    assert_eq!(dead_code(expr.clone()), expr);
}

#[test]
fn test_dead_code_nested() {
    // let a = 1 in let b = 2 in fin a  ──►  let a = 1 in fin a
    let expr = let_("a", int(1), let_("b", int(2), fin("a")));
    assert_eq!(dead_code(expr), let_("a", int(1), fin("a")));
}

#[test]
fn test_dead_code_inside_lambda() {
    // let k = (x -> let dead = 1 in fin x) in fin k
    let inner = lam("x", let_("dead", int(1), fin("x")));
    let expr = let_("k", inner, fin("k"));
    let expected = let_("k", lam("x", fin("x")), fin("k"));
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
fn test_copy_prop_in_app() {
    // let y = x in f y  ──►  f x
    let expr = let_("y", var("x"), app("f", "y"));
    assert_eq!(copy_propagation(expr), app("f", "x"));
}

#[test]
fn test_copy_prop_non_var_untouched() {
    // let y = 42 in fin y  ──►  unchanged (not a Var)
    let expr = let_("y", int(42), fin("y"));
    assert_eq!(copy_propagation(expr.clone()), expr);
}

#[test]
fn test_copy_prop_inside_lambda() {
    // let k = (x -> let y = x in fin y) in fin k
    let inner = lam("x", let_("y", var("x"), fin("y")));
    let expr = let_("k", inner, fin("k"));
    let expected = let_("k", lam("x", fin("x")), fin("k"));
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
    // let k = (x -> fin x) in k arg  ──►  fin arg
    let expr = let_("k", lam("x", fin("x")), app("k", "arg"));
    assert_eq!(beta_contraction(expr), fin("arg"));
}

#[test]
fn test_beta_with_body() {
    // let k = (x -> let r = field 0 of x in fin r) in k arg
    // ──►  let r = field 0 of arg in fin r
    let expr = let_("k",
        lam("x", let_("r", field("x", 0), fin("r"))),
        app("k", "arg"));
    let expected = let_("r", field("arg", 0), fin("r"));
    assert_eq!(beta_contraction(expr), expected);
}

#[test]
fn test_beta_nested_in_let() {
    // let k = (x -> fin x) in let y = 42 in k arg
    // ──►  let y = 42 in fin arg
    let expr = let_("k",
        lam("x", fin("x")),
        let_("y", int(42), app("k", "arg")));
    let expected = let_("y", int(42), fin("arg"));
    assert_eq!(beta_contraction(expr), expected);
}

#[test]
fn test_beta_multi_use_not_inlined() {
    // let k = (x -> fin x) in let a = Ctor(0, [k]) in k arg
    // k is used twice (in Ctor and in App), should not inline
    let expr = let_("k",
        lam("x", fin("x")),
        let_("a", ctor(0, &["k"]), app("k", "arg")));
    assert_eq!(beta_contraction(expr.clone()), expr);
}

#[test]
fn test_beta_in_match_branch() {
    // let k = (x -> fin x) in match s 0 [ k a | fin b ]
    // ──►  match s 0 [ fin a | fin b ]
    let expr = let_("k",
        lam("x", fin("x")),
        match_("s", 0, vec![
            case(&[], app("k", "a")),
            case(&[], fin("b")),
        ]));
    let expected = match_("s", 0, vec![
        case(&[], fin("a")),
        case(&[], fin("b")),
    ]);
    assert_eq!(beta_contraction(expr), expected);
}

#[test]
fn test_beta_inside_lambda() {
    // let outer = (z -> let k = (x -> fin x) in k z) in fin outer
    let inner = lam("z", let_("k", lam("x", fin("x")), app("k", "z")));
    let expr = let_("outer", inner, fin("outer"));
    let expected = let_("outer", lam("z", fin("z")), fin("outer"));
    assert_eq!(beta_contraction(expr), expected);
}

// ── Eta reduction ──────────────────────────────────────────────────────────

#[test]
fn test_eta_simple() {
    // let g = (x -> f x) in fin g  ──►  let g = f in fin g
    let expr = let_("g", lam("x", app("f", "x")), fin("g"));
    let expected = let_("g", var("f"), fin("g"));
    assert_eq!(eta_reduction(expr), expected);
}

#[test]
fn test_eta_not_forwarding() {
    // let g = (x -> h x) where body isn't just App
    // let g = (x -> let r = 1 in fin x) in fin g  ──►  unchanged
    let expr = let_("g", lam("x", let_("r", int(1), fin("x"))), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

#[test]
fn test_eta_self_reference() {
    // let g = (x -> x x) in fin g  ──►  unchanged (f == param)
    let expr = let_("g", lam("x", app("x", "x")), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

#[test]
fn test_eta_wrong_arg() {
    // let g = (x -> f y) in fin g  ──►  unchanged (arg != param)
    let expr = let_("g", lam("x", app("f", "y")), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

#[test]
fn test_eta_inside_lambda() {
    // let outer = (z -> let g = (x -> f x) in g z) in fin outer
    let inner = lam("z", let_("g", lam("x", app("f", "x")), app("g", "z")));
    let expr = let_("outer", inner, fin("outer"));
    let expected = let_("outer", lam("z", let_("g", var("f"), app("g", "z"))), fin("outer"));
    assert_eq!(eta_reduction(expr), expected);
}

#[test]
fn test_eta_letrec_not_reduced() {
    // letrec should not be eta-reduced (only Let with Lambda)
    let expr = letrec("g", lambda("x", app("f", "x")), fin("g"));
    assert_eq!(eta_reduction(expr.clone()), expr);
}

// ── Interaction between passes ─────────────────────────────────────────────

#[test]
fn test_eta_then_copy_prop() {
    // let g = (x -> f x) in g arg
    // eta:  let g = f in g arg
    // copy: f arg
    let expr = let_("g", lam("x", app("f", "x")), app("g", "arg"));
    let after_eta = eta_reduction(expr);
    let after_copy = copy_propagation(after_eta);
    assert_eq!(after_copy, app("f", "arg"));
}

#[test]
fn test_beta_then_dead_code() {
    // let k = (x -> fin x) in let unused = 99 in k arg
    // beta: let unused = 99 in fin arg
    // dead: fin arg
    let expr = let_("k",
        lam("x", fin("x")),
        let_("unused", int(99), app("k", "arg")));
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
fn test_inline_small_function() {
    // let f = (x -> fin x) in f arg  ──►  let f = (x -> fin x) in fin arg
    let expr = let_("f", lam("x", fin("x")), app("f", "arg"));
    let result = rewrite::inlining(expr, 20);
    let expected = let_("f", lam("x", fin("x")), fin("arg"));
    assert_eq!(result, expected);
}

#[test]
fn test_inline_too_large() {
    // Body size > threshold → not inlined
    let big_body = let_("a", int(1),
        let_("b", int(2),
            let_("c", int(3), fin("c"))));
    let expr = let_("f", lam("x", big_body.clone()), app("f", "arg"));
    let result = rewrite::inlining(expr.clone(), 2);
    assert_eq!(result, expr);
}

#[test]
fn test_inline_letrec_not_inlined() {
    // Recursive functions should not be inlined
    let expr = letrec("f", lambda("x", app("f", "x")), app("f", "arg"));
    let result = rewrite::inlining(expr.clone(), 100);
    assert_eq!(result, expr);
}

#[test]
fn test_inline_multiple_call_sites() {
    // let f = (x -> fin x) in let a = Ctor(0, []) in match a 0 [ f x | f y ]
    // Both call sites get inlined
    let expr = let_("f", lam("x", fin("x")),
        match_("a", 0, vec![
            case(&[], app("f", "x")),
            case(&[], app("f", "y")),
        ]));
    let expected = let_("f", lam("x", fin("x")),
        match_("a", 0, vec![
            case(&[], fin("x")),
            case(&[], fin("y")),
        ]));
    let result = rewrite::inlining(expr, 20);
    assert_eq!(result, expected);
}

#[test]
fn test_inline_with_substitution() {
    // let f = (x -> let r = field 0 of x in fin r) in f arg
    // ──►  let f = (...) in let r = field 0 of arg in fin r
    let expr = let_("f",
        lam("x", let_("r", field("x", 0), fin("r"))),
        app("f", "arg"));
    let expected = let_("f",
        lam("x", let_("r", field("x", 0), fin("r"))),
        let_("r", field("arg", 0), fin("r")));
    let result = rewrite::inlining(expr, 20);
    assert_eq!(result, expected);
}

#[test]
fn test_inline_nested_let() {
    // let f = (x -> fin x) in let y = 1 in f arg
    // ──►  let f = (...) in let y = 1 in fin arg
    let expr = let_("f", lam("x", fin("x")),
        let_("y", int(1), app("f", "arg")));
    let expected = let_("f", lam("x", fin("x")),
        let_("y", int(1), fin("arg")));
    let result = rewrite::inlining(expr, 20);
    assert_eq!(result, expected);
}

#[test]
fn test_inline_then_dead_code() {
    // After inlining all call sites, dead_code removes the binding
    // let f = (x -> fin x) in f arg  →inline→  let f = (...) in fin arg  →dead→  fin arg
    let expr = let_("f", lam("x", fin("x")), app("f", "arg"));
    let after_inline = rewrite::inlining(expr, 20);
    let after_dead = dead_code(after_inline);
    assert_eq!(after_dead, fin("arg"));
}
