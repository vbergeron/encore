use crate::ir::asm::{self, Expr, Val, Fun, Case, Reg};

pub fn optimize_module(module: asm::Module) -> asm::Module {
    asm::Module {
        defines: module.defines.into_iter().map(|d| asm::Define {
            global: d.global,
            body: optimize_expr(d.body),
        }).collect(),
    }
}

fn optimize_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Let(rd, val, body) => {
            let body = optimize_expr(*body);
            if let Some(target) = sink_target(rd, &val, &body) {
                let val = rewrite_val_reg(val, rd, target);
                return Expr::Let(target, val, Box::new(rewrite_expr_reg(body, rd, target)));
            }
            Expr::Let(rd, val, Box::new(body))
        }
        Expr::Letrec(rd, fun, body) => {
            let fun = optimize_fun(fun);
            let body = optimize_expr(*body);
            Expr::Letrec(rd, fun, Box::new(body))
        }
        Expr::Match(rs, base, cases) => {
            let cases = cases.into_iter().map(|c| Case {
                arity: c.arity,
                unpack_base: c.unpack_base,
                body: optimize_expr(c.body),
            }).collect();
            Expr::Match(rs, base, cases)
        }
        other => other,
    }
}

fn optimize_fun(fun: Fun) -> Fun {
    Fun {
        captures: fun.captures,
        body: Box::new(optimize_expr(*fun.body)),
    }
}

/// Determine if `rd` can be sunk into an argument register.
///
/// For `Let(rd, val, Encore(rf, args, rk))`: if rd appears exactly once in
/// args at position i, is not rf/rk, and val doesn't read A(i+1), we can
/// redirect rd to A(i+1).
///
/// For `Let(rd, val, Fin(rd))`: redirect rd to A1.
fn sink_target(rd: Reg, val: &Val, body: &Expr) -> Option<Reg> {
    if rd < asm::X01 {
        return None;
    }

    match body {
        Expr::Fin(r) if *r == rd => {
            if !val_reads(val, asm::A1) {
                return Some(asm::A1);
            }
        }
        Expr::Encore(rf, args, rk) => {
            if *rf == rd || *rk == rd {
                return None;
            }
            let positions: Vec<usize> = args.iter()
                .enumerate()
                .filter(|&(_, a)| *a == rd)
                .map(|(i, _)| i)
                .collect();
            if positions.len() != 1 {
                return None;
            }
            let target = asm::A1 + positions[0] as u8;
            if val_reads(val, target) {
                return None;
            }
            // Ensure no other arg that comes after would be clobbered.
            // Since we're redirecting rd -> target, any arg before position
            // that reads target would be a problem. But args are loaded
            // left-to-right by the emitter, so the only conflict is if
            // another arg IS target (meaning target is live in args).
            for (i, &a) in args.iter().enumerate() {
                if i != positions[0] && a == target {
                    return None;
                }
            }
            return Some(target);
        }
        _ => {}
    }
    None
}

fn val_reads(val: &Val, reg: Reg) -> bool {
    match val {
        Val::Reg(r) => *r == reg,
        Val::Capture(_) | Val::Global(_) | Val::Int(_) | Val::Bytes(_) | Val::Extern(_) => false,
        Val::ContLam(cont) => expr_reads(&cont.body, reg),
        Val::Ctor(_, fields) => fields.iter().any(|&f| f == reg),
        Val::Field(r, _) => *r == reg,
        Val::Prim(_, regs) => regs.iter().any(|&r| r == reg),
    }
}

fn expr_reads(expr: &Expr, reg: Reg) -> bool {
    match expr {
        Expr::Let(rd, val, body) => {
            val_reads(val, reg) || (*rd != reg && expr_reads(body, reg))
        }
        Expr::Letrec(rd, fun, body) => {
            expr_reads(&fun.body, reg) || (*rd != reg && expr_reads(body, reg))
        }
        Expr::Encore(rf, args, rk) => {
            *rf == reg || *rk == reg || args.iter().any(|&a| a == reg)
        }
        Expr::Match(rs, _, cases) => {
            *rs == reg || cases.iter().any(|c| expr_reads(&c.body, reg))
        }
        Expr::Fin(r) => *r == reg,
    }
}

fn rewrite_expr_reg(expr: Expr, from: Reg, to: Reg) -> Expr {
    match expr {
        Expr::Encore(rf, args, rk) => {
            let rf = if rf == from { to } else { rf };
            let rk = if rk == from { to } else { rk };
            let args = args.into_iter().map(|a| if a == from { to } else { a }).collect();
            Expr::Encore(rf, args, rk)
        }
        Expr::Fin(r) => Expr::Fin(if r == from { to } else { r }),
        other => other,
    }
}

fn rewrite_val_reg(val: Val, from: Reg, to: Reg) -> Val {
    match val {
        Val::Reg(r) => Val::Reg(if r == from { to } else { r }),
        Val::Ctor(tag, fields) => Val::Ctor(tag, fields.into_iter().map(|f| if f == from { to } else { f }).collect()),
        Val::Field(r, idx) => Val::Field(if r == from { to } else { r }, idx),
        Val::Prim(op, regs) => Val::Prim(op, regs.into_iter().map(|r| if r == from { to } else { r }).collect()),
        other => other,
    }
}
