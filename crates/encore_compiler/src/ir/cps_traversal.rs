use super::cps::{Case, Cont, Define, Expr, Fun, Module, Name, Tag, Val};
use super::prim::PrimOp;

/// Owning transformer over the CPS IR with a threaded context.
///
/// Every callback carries `&mut Self::Ctx`, so state (environments,
/// available expressions, scope stacks, …) is threaded through the
/// traversal rather than living on the transformer struct. Stateless
/// passes set `type Ctx = ();` and ignore the parameter; stateful passes
/// fork their context at scope boundaries by cloning before recursing.
///
/// Every callback has a default implementation that rebuilds the node and
/// recurses into its children. Overriding a specific variant callback lets
/// a pass focus on the cases it cares about while the rest of the tree is
/// reconstructed for free.
pub trait CPSTransformer {
    type Ctx;

    // ── Top-level ───────────────────────────────────────────────────────────

    fn transform_module(&self, ctx: &mut Self::Ctx, module: Module) -> Module {
        Module {
            defines: module
                .defines
                .into_iter()
                .map(|define| self.transform_define(ctx, define))
                .collect(),
        }
    }

    fn transform_define(&self, ctx: &mut Self::Ctx, define: Define) -> Define {
        Define {
            name: define.name,
            body: self.transform_expr(ctx, define.body),
        }
    }

    // ── Expr dispatch ───────────────────────────────────────────────────────

    fn transform_expr(&self, ctx: &mut Self::Ctx, expr: Expr) -> Expr {
        match expr {
            Expr::Let(name, val, body) => self.transform_let(ctx, name, val, *body),
            Expr::Letrec(name, fun, body) => self.transform_letrec(ctx, name, fun, *body),
            Expr::Encore(f, args, k) => self.transform_encore(ctx, f, args, k),
            Expr::Match(scrutinee, base, cases) => {
                self.transform_match_expr(ctx, scrutinee, base, cases)
            }
            Expr::Fin(name) => self.transform_fin(ctx, name),
        }
    }

    fn transform_let(&self, ctx: &mut Self::Ctx, name: Name, val: Val, body: Expr) -> Expr {
        Expr::Let(
            name,
            self.transform_val(ctx, val),
            Box::new(self.transform_expr(ctx, body)),
        )
    }

    fn transform_letrec(&self, ctx: &mut Self::Ctx, name: Name, fun: Fun, body: Expr) -> Expr {
        Expr::Letrec(
            name,
            self.transform_fun(ctx, fun),
            Box::new(self.transform_expr(ctx, body)),
        )
    }

    fn transform_encore(
        &self,
        _ctx: &mut Self::Ctx,
        f: Name,
        args: Vec<Name>,
        k: Name,
    ) -> Expr {
        Expr::Encore(f, args, k)
    }

    fn transform_match_expr(
        &self,
        ctx: &mut Self::Ctx,
        scrutinee: Name,
        base: Tag,
        cases: Vec<Case>,
    ) -> Expr {
        Expr::Match(
            scrutinee,
            base,
            cases
                .into_iter()
                .map(|case| self.transform_case(ctx, case))
                .collect(),
        )
    }

    fn transform_fin(&self, _ctx: &mut Self::Ctx, name: Name) -> Expr {
        Expr::Fin(name)
    }

    // ── Val dispatch ────────────────────────────────────────────────────────

    fn transform_val(&self, ctx: &mut Self::Ctx, val: Val) -> Val {
        match val {
            Val::Var(name) => self.transform_var(ctx, name),
            Val::Cont(cont) => self.transform_cont_val(ctx, cont),
            Val::NullCont => self.transform_null_cont(ctx),
            Val::Ctor(tag, fields) => self.transform_ctor(ctx, tag, fields),
            Val::Field(name, idx) => self.transform_field(ctx, name, idx),
            Val::Int(n) => self.transform_int(ctx, n),
            Val::Bytes(data) => self.transform_bytes(ctx, data),
            Val::Prim(op, args) => self.transform_prim(ctx, op, args),
            Val::Extern(slot) => self.transform_extern(ctx, slot),
        }
    }

    fn transform_var(&self, _ctx: &mut Self::Ctx, name: Name) -> Val {
        Val::Var(name)
    }

    fn transform_cont_val(&self, ctx: &mut Self::Ctx, cont: Cont) -> Val {
        Val::Cont(self.transform_cont(ctx, cont))
    }

    fn transform_null_cont(&self, _ctx: &mut Self::Ctx) -> Val {
        Val::NullCont
    }

    fn transform_ctor(&self, _ctx: &mut Self::Ctx, tag: Tag, fields: Vec<Name>) -> Val {
        Val::Ctor(tag, fields)
    }

    fn transform_field(&self, _ctx: &mut Self::Ctx, name: Name, idx: u8) -> Val {
        Val::Field(name, idx)
    }

    fn transform_int(&self, _ctx: &mut Self::Ctx, n: i32) -> Val {
        Val::Int(n)
    }

    fn transform_bytes(&self, _ctx: &mut Self::Ctx, data: Vec<u8>) -> Val {
        Val::Bytes(data)
    }

    fn transform_prim(&self, _ctx: &mut Self::Ctx, op: PrimOp, args: Vec<Name>) -> Val {
        Val::Prim(op, args)
    }

    fn transform_extern(&self, _ctx: &mut Self::Ctx, slot: u16) -> Val {
        Val::Extern(slot)
    }

    // ── Nested structures ───────────────────────────────────────────────────

    fn transform_fun(&self, ctx: &mut Self::Ctx, fun: Fun) -> Fun {
        Fun {
            args: fun.args,
            cont: fun.cont,
            body: Box::new(self.transform_expr(ctx, *fun.body)),
        }
    }

    fn transform_cont(&self, ctx: &mut Self::Ctx, cont: Cont) -> Cont {
        Cont {
            params: cont.params,
            body: Box::new(self.transform_expr(ctx, *cont.body)),
        }
    }

    fn transform_case(&self, ctx: &mut Self::Ctx, case: Case) -> Case {
        Case {
            binds: case.binds,
            body: self.transform_expr(ctx, case.body),
        }
    }
}
