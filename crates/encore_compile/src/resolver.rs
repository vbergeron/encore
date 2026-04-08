use std::collections::HashMap;
use crate::{cps, ir};

#[derive(Clone)]
struct Env {
    bindings: HashMap<String, ir::Loc>,
    local_count: u8,
}

impl Env {
    fn new() -> Self {
        Self { bindings: HashMap::new(), local_count: 0 }
    }

    fn lookup(&self, name: &str) -> ir::Loc {
        self.bindings[name]
    }

    fn bind_local(&mut self, name: String) {
        self.bindings.insert(name, ir::Loc::Local(self.local_count));
        self.local_count += 1;
    }
}

pub fn resolve_module(module: &cps::Module) -> ir::Module {
    let globals: HashMap<String, u8> = module.defines.iter()
        .enumerate()
        .map(|(i, d)| (d.name.clone(), i as u8))
        .collect();

    let defines = module.defines.iter()
        .enumerate()
        .map(|(i, define)| {
            let mut env = Env::new();
            for (name, idx) in &globals {
                env.bindings.insert(name.clone(), ir::Loc::Global(*idx));
            }
            ir::Define {
                global: i as u8,
                body: resolve_expr(&mut env, &define.body),
            }
        })
        .collect();

    ir::Module { defines }
}

fn resolve_expr(env: &mut Env, expr: &cps::Expr) -> ir::Expr {
    match expr {
        cps::Expr::Let(name, val, body) => {
            let ir_val = resolve_val(env, val);
            env.bind_local(name.clone());
            ir::Expr::Let(ir_val, Box::new(resolve_expr(env, body)))
        }

        cps::Expr::Letrec(name, lam, body) => {
            let ir_lam = resolve_lambda(env, lam, Some(name));
            env.bind_local(name.clone());
            ir::Expr::Letrec(ir_lam, Box::new(resolve_expr(env, body)))
        }

        cps::Expr::App(f, x) => {
            ir::Expr::App(env.lookup(f), env.lookup(x))
        }

        cps::Expr::Match(name, base, cases) => {
            let loc = env.lookup(name);
            // emit_loc pushes the scrutinee copy onto the stack
            env.local_count += 1;

            let ir_cases = cases.iter().map(|case| {
                let mut case_env = env.clone();
                for bind in &case.binds {
                    case_env.bind_local(bind.clone());
                }
                ir::Case {
                    arity: case.binds.len() as u8,
                    body: resolve_expr(&mut case_env, &case.body),
                }
            }).collect();

            ir::Expr::Match(loc, *base, ir_cases)
        }

        cps::Expr::Halt(name) => {
            ir::Expr::Halt(env.lookup(name))
        }
    }
}

fn resolve_val(env: &Env, val: &cps::Val) -> ir::Val {
    match val {
        cps::Val::Var(name) => {
            ir::Val::Loc(env.lookup(name))
        }

        cps::Val::Lambda(lam) => {
            ir::Val::Lambda(resolve_lambda(env, lam, None))
        }

        cps::Val::Ctor(tag, fields) => {
            ir::Val::Ctor(*tag, fields.iter().map(|n| env.lookup(n)).collect())
        }

        cps::Val::Field(name, idx) => {
            ir::Val::Field(env.lookup(name), *idx)
        }
    }
}

fn resolve_lambda(env: &Env, lam: &cps::Lambda, rec_name: Option<&str>) -> ir::Lambda {
    // TODO: free variable analysis
    //
    // 1. Compute free variables of lam.body (names used but not bound inside)
    // 2. Exclude lam.param (→ Loc::Arg) and globals (→ Loc::Global)
    // 3. If rec_name is Some, exclude it (→ Loc::SelfRef inside the body)
    // 4. Remaining free vars become captures:
    //    - In the outer env, look up their Loc → that's what goes in ir::Lambda.captures
    //    - In the inner env, assign them Loc::Capture(0), Loc::Capture(1), ...
    // 5. Build inner env: param→Arg, captures→Capture(i), globals→Global(i),
    //    rec_name→SelfRef
    // 6. Resolve lam.body in the inner env
    let _ = (env, lam, rec_name);
    todo!("lambda capture analysis")
}
