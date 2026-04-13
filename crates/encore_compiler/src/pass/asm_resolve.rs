use std::collections::{HashMap, HashSet};
use crate::ir::{asm, cps};

#[derive(Clone)]
struct Env {
    bindings: HashMap<String, asm::Loc>,
    local_count: u8,
}

impl Env {
    fn new() -> Self {
        Self { bindings: HashMap::new(), local_count: 0 }
    }

    fn lookup(&self, name: &str) -> asm::Loc {
        self.bindings[name]
    }

    fn bind_local(&mut self, name: String) {
        self.bindings.insert(name, asm::Loc::Local(self.local_count));
        self.local_count += 1;
    }

    fn globals(&self) -> HashMap<String, u8> {
        self.bindings.iter()
            .filter_map(|(name, loc)| match loc {
                asm::Loc::Global(idx) => Some((name.clone(), *idx)),
                _ => None,
            })
            .collect()
    }
}

pub fn resolve_module(module: &cps::Module) -> asm::Module {
    let globals: HashMap<String, u8> = module.defines.iter()
        .enumerate()
        .map(|(i, d)| (d.name.clone(), i as u8))
        .collect();

    let defines = module.defines.iter()
        .enumerate()
        .map(|(i, define)| {
            let mut env = Env::new();
            for (name, idx) in &globals {
                env.bindings.insert(name.clone(), asm::Loc::Global(*idx));
            }
            asm::Define {
                global: i as u8,
                body: resolve_expr(&mut env, &define.body),
            }
        })
        .collect();

    asm::Module { defines }
}

fn resolve_expr(env: &mut Env, expr: &cps::Expr) -> asm::Expr {
    match expr {
        cps::Expr::Let(name, val, body) => {
            let ir_val = resolve_val(env, val);
            env.bind_local(name.clone());
            asm::Expr::Let(ir_val, Box::new(resolve_expr(env, body)))
        }

        cps::Expr::Letrec(name, fun, body) => {
            let ir_fun = resolve_fun(env, fun, Some(name));
            env.bind_local(name.clone());
            asm::Expr::Letrec(ir_fun, Box::new(resolve_expr(env, body)))
        }

        cps::Expr::Encore(f, x, k) => {
            asm::Expr::Encore(env.lookup(f), env.lookup(x), env.lookup(k))
        }

        cps::Expr::Match(name, base, cases) => {
            let loc = env.lookup(name);

            let ir_cases = cases.iter().map(|case| {
                let mut case_env = env.clone();
                for bind in &case.binds {
                    case_env.bind_local(bind.clone());
                }
                asm::Case {
                    arity: case.binds.len() as u8,
                    body: resolve_expr(&mut case_env, &case.body),
                }
            }).collect();

            asm::Expr::Match(loc, *base, ir_cases)
        }

        cps::Expr::Fin(name) => {
            asm::Expr::Fin(env.lookup(name))
        }
    }
}

fn resolve_val(env: &Env, val: &cps::Val) -> asm::Val {
    match val {
        cps::Val::Var(name) => {
            asm::Val::Loc(env.lookup(name))
        }

        cps::Val::Cont(cont) => {
            asm::Val::ContLam(resolve_cont(env, cont))
        }

        cps::Val::Ctor(tag, fields) => {
            asm::Val::Ctor(*tag, fields.iter().map(|n| env.lookup(n)).collect())
        }

        cps::Val::Field(name, idx) => {
            asm::Val::Field(env.lookup(name), *idx)
        }

        cps::Val::Int(n) => {
            asm::Val::Int(*n)
        }

        cps::Val::Prim(op, names) => {
            asm::Val::Prim(*op, names.iter().map(|n| env.lookup(n)).collect())
        }

        cps::Val::Extern(slot) => asm::Val::Extern(*slot),

        cps::Val::NullCont => asm::Val::Loc(asm::Loc::NullCont),
    }
}

fn resolve_fun(env: &Env, fun: &cps::Fun, rec_name: Option<&str>) -> asm::Fun {
    let mut free = HashSet::new();
    free_vars_expr(&fun.body, &mut HashSet::new(), &mut free);
    free.remove(&fun.arg);
    free.remove(&fun.cont);
    if let Some(name) = rec_name {
        free.remove(name);
    }

    let globals = env.globals();
    let mut capture_names: Vec<String> = free.into_iter()
        .filter(|n| !globals.contains_key(n))
        .collect();
    capture_names.sort();

    let captures: Vec<asm::Loc> = capture_names.iter()
        .map(|n| env.lookup(n))
        .collect();

    let mut inner = Env::new();
    inner.bindings.insert(fun.arg.clone(), asm::Loc::Arg);
    inner.bindings.insert(fun.cont.clone(), asm::Loc::Cont);
    for (name, idx) in &globals {
        inner.bindings.insert(name.clone(), asm::Loc::Global(*idx));
    }
    for (i, name) in capture_names.iter().enumerate() {
        inner.bindings.insert(name.clone(), asm::Loc::Capture(i as u8));
    }
    if let Some(name) = rec_name {
        inner.bindings.insert(name.to_string(), asm::Loc::SelfRef);
    }

    let body = resolve_expr(&mut inner, &fun.body);
    asm::Fun { captures, body: Box::new(body) }
}

fn resolve_cont(env: &Env, cont: &cps::Cont) -> asm::ContLam {
    let mut free = HashSet::new();
    free_vars_expr(&cont.body, &mut HashSet::new(), &mut free);
    free.remove(&cont.param);

    let globals = env.globals();
    let mut capture_names: Vec<String> = free.into_iter()
        .filter(|n| !globals.contains_key(n))
        .collect();
    capture_names.sort();

    let captures: Vec<asm::Loc> = capture_names.iter()
        .map(|n| env.lookup(n))
        .collect();

    let mut inner = Env::new();
    inner.bindings.insert(cont.param.clone(), asm::Loc::Arg);
    for (name, idx) in &globals {
        inner.bindings.insert(name.clone(), asm::Loc::Global(*idx));
    }
    for (i, name) in capture_names.iter().enumerate() {
        inner.bindings.insert(name.clone(), asm::Loc::Capture(i as u8));
    }

    let body = resolve_expr(&mut inner, &cont.body);
    asm::ContLam { captures, body: Box::new(body) }
}

fn free_vars_expr(expr: &cps::Expr, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match expr {
        cps::Expr::Let(name, val, body) => {
            free_vars_val(val, bound, free);
            bound.insert(name.clone());
            free_vars_expr(body, bound, free);
        }
        cps::Expr::Letrec(name, fun, body) => {
            bound.insert(name.clone());
            free_vars_fun(fun, bound, free);
            free_vars_expr(body, bound, free);
        }
        cps::Expr::Encore(f, x, k) => {
            use_name(f, bound, free);
            use_name(x, bound, free);
            use_name(k, bound, free);
        }
        cps::Expr::Match(name, _, cases) => {
            use_name(name, bound, free);
            for case in cases {
                let mut case_bound = bound.clone();
                for bind in &case.binds {
                    case_bound.insert(bind.clone());
                }
                free_vars_expr(&case.body, &mut case_bound, free);
            }
        }
        cps::Expr::Fin(name) => {
            use_name(name, bound, free);
        }
    }
}

fn free_vars_val(val: &cps::Val, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match val {
        cps::Val::Var(name) => use_name(name, bound, free),
        cps::Val::Cont(cont) => free_vars_cont(cont, bound, free),
        cps::Val::Ctor(_, fields) => {
            for name in fields {
                use_name(name, bound, free);
            }
        }
        cps::Val::Field(name, _) => use_name(name, bound, free),
        cps::Val::Int(_) | cps::Val::NullCont => {}
        cps::Val::Prim(_, names) => {
            for name in names {
                use_name(name, bound, free);
            }
        }
        cps::Val::Extern(_) => {}
    }
}

fn free_vars_fun(fun: &cps::Fun, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    let mut inner_bound = bound.clone();
    inner_bound.insert(fun.arg.clone());
    inner_bound.insert(fun.cont.clone());
    free_vars_expr(&fun.body, &mut inner_bound, free);
}

fn free_vars_cont(cont: &cps::Cont, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    let mut inner_bound = bound.clone();
    inner_bound.insert(cont.param.clone());
    free_vars_expr(&cont.body, &mut inner_bound, free);
}

fn use_name(name: &str, bound: &HashSet<String>, free: &mut HashSet<String>) {
    if !bound.contains(name) {
        free.insert(name.to_string());
    }
}
