use std::collections::{HashMap, HashSet};
use crate::ir::{asm, cps};

#[derive(Clone)]
struct Env {
    bindings: HashMap<String, asm::Reg>,
    local_count: u8,
}

impl Env {
    fn new() -> Self {
        Self { bindings: HashMap::new(), local_count: 0 }
    }

    fn lookup(&self, name: &str) -> asm::Reg {
        self.bindings[name]
    }

    fn bind(&mut self, name: String, reg: asm::Reg) {
        self.bindings.insert(name, reg);
    }

    fn bind_local(&mut self, name: String) -> asm::Reg {
        assert!(self.local_count < 17, "register overflow: more than 17 locals needed");
        let reg = asm::X01 + self.local_count;
        self.local_count += 1;
        self.bindings.insert(name, reg);
        reg
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

            let mut free = HashSet::new();
            free_vars_expr(&define.body, &mut HashSet::new(), &mut free);
            let mut used_globals: Vec<(String, u8)> = free.iter()
                .filter_map(|n| globals.get(n).map(|idx| (n.clone(), *idx)))
                .collect();
            used_globals.sort_by_key(|(_, idx)| *idx);

            let global_regs: Vec<(asm::Reg, u8)> = used_globals.iter()
                .map(|(name, idx)| {
                    let reg = env.bind_local(name.clone());
                    (reg, *idx)
                })
                .collect();

            let mut body = resolve_expr(&mut env, &define.body, &globals);

            for (reg, idx) in global_regs.into_iter().rev() {
                body = asm::Expr::Let(reg, asm::Val::Global(idx), Box::new(body));
            }

            asm::Define {
                global: i as u8,
                body,
            }
        })
        .collect();

    asm::Module { defines }
}

fn resolve_expr(env: &mut Env, expr: &cps::Expr, globals: &HashMap<String, u8>) -> asm::Expr {
    match expr {
        cps::Expr::Let(name, val, body) => {
            if matches!(val, cps::Val::NullCont) {
                env.bind(name.clone(), asm::NULL);
                return resolve_expr(env, body, globals);
            }

            let ir_val = resolve_val(env, val, globals);
            let dest = env.bind_local(name.clone());
            asm::Expr::Let(dest, ir_val, Box::new(resolve_expr(env, body, globals)))
        }

        cps::Expr::Letrec(name, fun, body) => {
            let ir_fun = resolve_fun(env, fun, Some(name), globals);
            let dest = env.bind_local(name.clone());
            asm::Expr::Letrec(dest, ir_fun, Box::new(resolve_expr(env, body, globals)))
        }

        cps::Expr::Encore(f, x, k) => {
            asm::Expr::Encore(env.lookup(f), env.lookup(x), env.lookup(k))
        }

        cps::Expr::Match(name, base, cases) => {
            let reg = env.lookup(name);

            let ir_cases = cases.iter().map(|case| {
                let mut case_env = env.clone();
                let unpack_base = asm::X01 + case_env.local_count;
                for bind in &case.binds {
                    case_env.bind_local(bind.clone());
                }
                asm::Case {
                    arity: case.binds.len() as u8,
                    unpack_base,
                    body: resolve_expr(&mut case_env, &case.body, globals),
                }
            }).collect();

            asm::Expr::Match(reg, *base, ir_cases)
        }

        cps::Expr::Fin(name) => {
            asm::Expr::Fin(env.lookup(name))
        }
    }
}

fn resolve_val(env: &Env, val: &cps::Val, globals: &HashMap<String, u8>) -> asm::Val {
    match val {
        cps::Val::Var(name) => {
            asm::Val::Reg(env.lookup(name))
        }

        cps::Val::Cont(cont) => {
            asm::Val::ContLam(resolve_cont(env, cont, globals))
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

        cps::Val::NullCont => asm::Val::Reg(asm::NULL),
    }
}

fn resolve_fun(env: &Env, fun: &cps::Fun, rec_name: Option<&str>, globals: &HashMap<String, u8>) -> asm::Fun {
    let mut free = HashSet::new();
    free_vars_expr(&fun.body, &mut HashSet::new(), &mut free);
    free.remove(&fun.arg);
    free.remove(&fun.cont);
    if let Some(name) = rec_name {
        free.remove(name);
    }

    let mut capture_names: Vec<String> = Vec::new();
    let mut used_globals: Vec<(String, u8)> = Vec::new();
    for name in &free {
        if let Some(idx) = globals.get(name) {
            used_globals.push((name.clone(), *idx));
        } else {
            capture_names.push(name.clone());
        }
    }
    capture_names.sort();
    used_globals.sort_by_key(|(_, idx)| *idx);

    let captures: Vec<asm::Reg> = capture_names.iter()
        .map(|n| env.lookup(n))
        .collect();

    let mut inner = Env::new();
    inner.bind(fun.cont.clone(), asm::CONT);
    if let Some(name) = rec_name {
        inner.bind(name.to_string(), asm::SELF);
    }
    let arg_reg = inner.bind_local(fun.arg.clone());

    let capture_regs: Vec<(asm::Reg, u8)> = capture_names.iter().enumerate()
        .map(|(i, name)| {
            let reg = inner.bind_local(name.clone());
            (reg, i as u8)
        })
        .collect();

    let global_regs: Vec<(asm::Reg, u8)> = used_globals.iter()
        .map(|(name, idx)| {
            let reg = inner.bind_local(name.clone());
            (reg, *idx)
        })
        .collect();

    let mut body = resolve_expr(&mut inner, &fun.body, globals);

    for (reg, idx) in global_regs.into_iter().rev() {
        body = asm::Expr::Let(reg, asm::Val::Global(idx), Box::new(body));
    }
    for (reg, cap_idx) in capture_regs.into_iter().rev() {
        body = asm::Expr::Let(reg, asm::Val::Capture(cap_idx), Box::new(body));
    }
    body = asm::Expr::Let(arg_reg, asm::Val::Reg(asm::A1), Box::new(body));

    asm::Fun { captures, body: Box::new(body) }
}

fn resolve_cont(env: &Env, cont: &cps::Cont, globals: &HashMap<String, u8>) -> asm::ContLam {
    let mut free = HashSet::new();
    free_vars_expr(&cont.body, &mut HashSet::new(), &mut free);
    free.remove(&cont.param);

    let mut capture_names: Vec<String> = Vec::new();
    let mut used_globals: Vec<(String, u8)> = Vec::new();
    for name in &free {
        if let Some(idx) = globals.get(name) {
            used_globals.push((name.clone(), *idx));
        } else {
            capture_names.push(name.clone());
        }
    }
    capture_names.sort();
    used_globals.sort_by_key(|(_, idx)| *idx);

    let captures: Vec<asm::Reg> = capture_names.iter()
        .map(|n| env.lookup(n))
        .collect();

    let mut inner = Env::new();
    let param_reg = inner.bind_local(cont.param.clone());

    let capture_regs: Vec<(asm::Reg, u8)> = capture_names.iter().enumerate()
        .map(|(i, name)| {
            let reg = inner.bind_local(name.clone());
            (reg, i as u8)
        })
        .collect();

    let global_regs: Vec<(asm::Reg, u8)> = used_globals.iter()
        .map(|(name, idx)| {
            let reg = inner.bind_local(name.clone());
            (reg, *idx)
        })
        .collect();

    let mut body = resolve_expr(&mut inner, &cont.body, globals);

    for (reg, idx) in global_regs.into_iter().rev() {
        body = asm::Expr::Let(reg, asm::Val::Global(idx), Box::new(body));
    }
    for (reg, cap_idx) in capture_regs.into_iter().rev() {
        body = asm::Expr::Let(reg, asm::Val::Capture(cap_idx), Box::new(body));
    }
    body = asm::Expr::Let(param_reg, asm::Val::Reg(asm::A1), Box::new(body));

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
