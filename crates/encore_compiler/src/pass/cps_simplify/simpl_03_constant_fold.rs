// Evaluate primitive operations on known constants, resolve field
// accesses on known constructors, and eliminate matches on values
// whose constructor tag is statically known.
//
//   let x = 3 in let y = 4 in builtin add x y     ──►   7
//
//   let r = eq(3, 3) in match r                    ──►   <true branch>
//     | False -> ...
//     | True  -> e
//
//   let p = Ctor(0, [a, b]) in field 0 of p        ──►   a
//
//   let s = bytes [68 69] in bytes_len s            ──►   2
//

use std::collections::HashMap;

use crate::ir::cps::{Case, Cont, Expr, Fun, Tag, Val};
use crate::ir::cps_traversal::CPSTransformer;
use crate::ir::prim::{BytesOp, IntOp, PrimOp};
use crate::pass::cps_subst::subst_expr;

#[derive(Clone)]
enum Known {
    Int(i32),
    Bytes(Vec<u8>),
    Ctor(Tag, Vec<String>),
}

type Env = HashMap<String, Known>;

pub fn constant_fold(expr: Expr) -> Expr {
    ConstantFold.transform_expr(&mut Env::new(), expr)
}

struct ConstantFold;

fn record(env: &mut Env, name: &str, val: &Val) {
    match val {
        Val::Int(n) => {
            env.insert(name.to_string(), Known::Int(*n));
        }
        Val::Bytes(data) => {
            env.insert(name.to_string(), Known::Bytes(data.clone()));
        }
        Val::Ctor(tag, fields) => {
            env.insert(name.to_string(), Known::Ctor(*tag, fields.clone()));
        }
        _ => {}
    }
}

impl CPSTransformer for ConstantFold {
    type Ctx = Env;

    fn transform_let(&self, env: &mut Env, name: String, val: Val, body: Expr) -> Expr {
        match val {
            Val::Int(n) => {
                env.insert(name.clone(), Known::Int(n));
                Expr::Let(name, Val::Int(n), Box::new(self.transform_expr(env, body)))
            }
            Val::Bytes(data) => {
                env.insert(name.clone(), Known::Bytes(data.clone()));
                Expr::Let(name, Val::Bytes(data), Box::new(self.transform_expr(env, body)))
            }
            Val::Ctor(tag, fields) => {
                env.insert(name.clone(), Known::Ctor(tag, fields.clone()));
                Expr::Let(
                    name,
                    Val::Ctor(tag, fields),
                    Box::new(self.transform_expr(env, body)),
                )
            }
            Val::Field(x, idx) => {
                if let Some(Known::Ctor(_, fields)) = env.get(&x).cloned() {
                    if (idx as usize) < fields.len() {
                        let target = fields[idx as usize].clone();
                        return Expr::Let(
                            name,
                            Val::Var(target),
                            Box::new(self.transform_expr(env, body)),
                        );
                    }
                }
                Expr::Let(name, Val::Field(x, idx), Box::new(self.transform_expr(env, body)))
            }
            Val::Prim(op, args) => {
                if let Some(new_val) = try_fold_prim(op, &args, env) {
                    record(env, &name, &new_val);
                    Expr::Let(name, new_val, Box::new(self.transform_expr(env, body)))
                } else {
                    Expr::Let(
                        name,
                        Val::Prim(op, args),
                        Box::new(self.transform_expr(env, body)),
                    )
                }
            }
            other => Expr::Let(
                name,
                self.transform_val(env, other),
                Box::new(self.transform_expr(env, body)),
            ),
        }
    }

    fn transform_fun(&self, env: &mut Env, fun: Fun) -> Fun {
        let mut local = env.clone();
        Fun {
            args: fun.args,
            cont: fun.cont,
            body: Box::new(self.transform_expr(&mut local, *fun.body)),
        }
    }

    fn transform_cont(&self, env: &mut Env, cont: Cont) -> Cont {
        let mut local = env.clone();
        Cont {
            params: cont.params,
            body: Box::new(self.transform_expr(&mut local, *cont.body)),
        }
    }

    fn transform_match_expr(
        &self,
        env: &mut Env,
        scrutinee: String,
        base: Tag,
        cases: Vec<Case>,
    ) -> Expr {
        if let Some(Known::Ctor(tag, fields)) = env.get(&scrutinee).cloned() {
            let branch = tag.wrapping_sub(base) as usize;
            if branch < cases.len() {
                let case = cases.into_iter().nth(branch).unwrap();
                let mut body = case.body;
                for (bind, field) in case.binds.iter().zip(fields.iter()) {
                    subst_expr(bind, field, &mut body);
                }
                return self.transform_expr(env, body);
            }
        }
        let cases = cases
            .into_iter()
            .map(|c| {
                let mut local = env.clone();
                Case {
                    binds: c.binds,
                    body: self.transform_expr(&mut local, c.body),
                }
            })
            .collect();
        Expr::Match(scrutinee, base, cases)
    }
}

fn try_fold_prim(op: PrimOp, args: &[String], env: &Env) -> Option<Val> {
    match op {
        PrimOp::Int(IntOp::Byte) => {
            let n = get_int(env, &args[0])?;
            (0..=255).contains(&n).then(|| Val::Bytes(vec![n as u8]))
        }
        PrimOp::Int(op) => {
            let a = get_int(env, &args[0])?;
            let b = get_int(env, &args[1])?;
            Some(eval_int_binop(op, a, b))
        }
        PrimOp::Bytes(BytesOp::Len) => {
            let bs = get_bytes(env, &args[0])?;
            Some(Val::Int(bs.len() as i32))
        }
        PrimOp::Bytes(BytesOp::Get) => {
            let bs = get_bytes(env, &args[0])?;
            let idx = usize::try_from(get_int(env, &args[1])?).ok()?;
            bs.get(idx).map(|b| Val::Int(*b as i32))
        }
        PrimOp::Bytes(BytesOp::Concat) => {
            let a = get_bytes(env, &args[0])?;
            let b = get_bytes(env, &args[1])?;
            let mut result = a.to_vec();
            result.extend_from_slice(b);
            Some(Val::Bytes(result))
        }
        PrimOp::Bytes(BytesOp::Slice) => {
            let bs = get_bytes(env, &args[0])?;
            let start = usize::try_from(get_int(env, &args[1])?).ok()?;
            let len = usize::try_from(get_int(env, &args[2])?).ok()?;
            let end = start.checked_add(len)?;
            (end <= bs.len()).then(|| Val::Bytes(bs[start..end].to_vec()))
        }
        PrimOp::Bytes(BytesOp::Eq) => {
            let a = get_bytes(env, &args[0])?;
            let b = get_bytes(env, &args[1])?;
            Some(if a == b { Val::TRUE } else { Val::FALSE })
        }
    }
}

fn eval_int_binop(op: IntOp, a: i32, b: i32) -> Val {
    match op {
        IntOp::Add => Val::Int(a.wrapping_add(b)),
        IntOp::Sub => Val::Int(a.wrapping_sub(b)),
        IntOp::Mul => Val::Int(a.wrapping_mul(b)),
        IntOp::Eq => if a == b { Val::TRUE } else { Val::FALSE },
        IntOp::Lt => if a < b { Val::TRUE } else { Val::FALSE },
        IntOp::Byte => unreachable!("Byte is unary and handled by try_fold_prim"),
    }
}

fn get_int(env: &Env, name: &str) -> Option<i32> {
    if let Some(Known::Int(n)) = env.get(name) { Some(*n) } else { None }
}

fn get_bytes<'a>(env: &'a Env, name: &str) -> Option<&'a [u8]> {
    if let Some(Known::Bytes(bs)) = env.get(name) { Some(bs.as_slice()) } else { None }
}

