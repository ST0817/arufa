use std::iter::zip;

use chumsky::span::{SimpleSpan, Spanned};

use crate::{
    ast::{
        ArithBinOp, Block, CastType, CmpBinOp, EqCmpBinOp, Expr, Item, LogicBinOp, Param,
        ShiftBinOp, Stmt, Type,
    },
    checked_ast::{
        CastRule, CheckedBlock, CheckedExpr, CheckedExprKind, CheckedFunc, CheckedLValueExpr,
        CheckedLValueExprKind, CheckedParam, CheckedStmt,
    },
    error::{CheckError, CheckErrorKind::*},
    scope::{Scope, Var},
    symbols::{FuncSig, Symbols},
};

fn find_var<'src, 'scope>(
    name: &Spanned<&str>,
    scope: &'scope Scope<'src>,
) -> Result<&'scope Var<'src>> {
    scope
        .find(name)
        .ok_or_else(|| CheckError::new(UndefinedVariable, name.span))
}

type Result<T> = std::result::Result<T, CheckError>;

fn check_type_match(expected: &Type, found: &Type, span: &SimpleSpan) -> Result<()> {
    if expected != found {
        return Err(CheckError::new(
            TypeMismatch {
                expected: expected.clone(),
                found: found.clone(),
            },
            *span,
        ));
    }
    Ok(())
}

fn check_bool_type(ty: &Type, span: &SimpleSpan) -> Result<()> {
    check_type_match(&Type::Bool, ty, span)
}

fn check_int_type(ty: &Type, span: &SimpleSpan) -> Result<()> {
    if !ty.is_int() {
        return Err(CheckError::new(NotIntType(ty.clone()), *span));
    };
    Ok(())
}

#[derive(Debug)]
struct Checker<'src, 'symbols> {
    symbols: &'symbols Symbols<'src>,
}

impl<'src, 'symbols> Checker<'src, 'symbols> {
    fn new(symbols: &'symbols Symbols<'src>) -> Self {
        Self { symbols }
    }

    fn check_var_lvalue(
        &self,
        name: &Spanned<&str>,
        scope: &Scope<'src>,
    ) -> Result<CheckedLValueExpr> {
        let var = find_var(name, scope)?;
        Ok(CheckedLValueExpr::new(
            CheckedLValueExprKind::Var(var.id()),
            var.ty().clone(),
        ))
    }

    fn check_deref_lvalue(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedLValueExpr> {
        let texpr = self.check_expr(expr, scope)?;
        let Type::Ptr(ty) = texpr.ty().clone() else {
            return Err(CheckError::new(NotPtrType(texpr.ty().clone()), *expr_span));
        };
        Ok(CheckedLValueExpr::new(
            CheckedLValueExprKind::Deref(texpr),
            *ty,
        ))
    }

    fn check_index_lvalue(
        &self,
        array: &Expr<'src>,
        array_span: &SimpleSpan,
        index: &Expr<'src>,
        index_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedLValueExpr> {
        let checked_expr_lvalue = self.check_lvalue(array, array_span, scope)?;
        let (elem_type, is_ptr) = match checked_expr_lvalue.ty() {
            Type::Array(elem_type, _) => (elem_type.clone(), false),
            Type::ArrayPtr(elem_type) => (elem_type.clone(), true),
            _ => {
                return Err(CheckError::new(
                    CannotIndex(checked_expr_lvalue.ty().clone()),
                    *array_span,
                ));
            }
        };
        let checked_index = self.check_expr(index, scope)?;
        check_int_type(checked_index.ty(), index_span)?;
        let ty = elem_type.as_ref().clone();
        Ok(CheckedLValueExpr::new(
            CheckedLValueExprKind::Index(checked_expr_lvalue, is_ptr, checked_index),
            ty,
        ))
    }

    fn check_lvalue(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedLValueExpr> {
        use Expr::*;
        match expr {
            Var(name) => self.check_var_lvalue(name, scope),
            Deref(expr) => self.check_deref_lvalue(expr, &expr.span, scope),
            Index(array, index) => {
                self.check_index_lvalue(array, &array.span, index, &index.span, scope)
            }
            _ => Err(CheckError::new(CannotGetAddress, *expr_span)),
        }
    }

    fn check_int_expr(&self, value: u64) -> Result<CheckedExpr> {
        Ok(CheckedExpr::new(CheckedExprKind::Int(value), Type::I32))
    }

    fn check_char_expr(&self, value: char) -> Result<CheckedExpr> {
        Ok(CheckedExpr::new(CheckedExprKind::Char(value), Type::Char))
    }

    fn check_bool_expr(&self, value: bool) -> Result<CheckedExpr> {
        Ok(CheckedExpr::new(CheckedExprKind::Bool(value), Type::Bool))
    }

    fn check_str_expr(&self, value: String) -> Result<CheckedExpr> {
        Ok(CheckedExpr::new(
            CheckedExprKind::Str(value),
            Type::ArrayPtr(Box::new(Type::Char)),
        ))
    }

    fn check_unit_expr(&self) -> Result<CheckedExpr> {
        Ok(CheckedExpr::new(CheckedExprKind::Unit, Type::Unit))
    }

    fn check_var_expr(
        &self,
        name: &Spanned<&'src str>,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lvalue_expr = self.check_var_lvalue(name, scope)?;
        let ty = checked_lvalue_expr.ty().clone();
        Ok(CheckedExpr::new(
            CheckedExprKind::LValue(checked_lvalue_expr),
            ty,
        ))
    }

    fn check_arith_bin_expr(
        &self,
        lhs: &Expr<'src>,
        lhs_span: &SimpleSpan,
        op: ArithBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lhs = self.check_expr(lhs, scope)?;
        check_int_type(checked_lhs.ty(), lhs_span)?;
        let checked_rhs = self.check_expr(rhs, scope)?;
        check_type_match(checked_lhs.ty(), checked_rhs.ty(), rhs_span)?;
        let ty = checked_lhs.ty().clone();
        Ok(CheckedExpr::new(
            CheckedExprKind::ArithBin(checked_lhs, op, checked_rhs),
            ty,
        ))
    }

    fn check_shift_bin_expr(
        &self,
        lhs: &Expr<'src>,
        lhs_span: &SimpleSpan,
        op: ShiftBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lhs = self.check_expr(lhs, scope)?;
        check_int_type(checked_lhs.ty(), lhs_span)?;
        let checked_rhs = self.check_expr(rhs, scope)?;
        check_int_type(checked_rhs.ty(), rhs_span)?;
        let ty = checked_lhs.ty().clone();
        Ok(CheckedExpr::new(
            CheckedExprKind::ShiftBin(checked_lhs, op, checked_rhs),
            ty,
        ))
    }

    fn check_eq_cmp_bin_expr(
        &self,
        lhs: &Expr<'src>,
        op: EqCmpBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lhs = self.check_expr(lhs, scope)?;
        let checked_rhs = self.check_expr(rhs, scope)?;
        if !checked_lhs.ty().is_eq_comparable_to(checked_rhs.ty()) {
            return Err(CheckError::new(
                NotEqComparable(checked_lhs.ty().clone(), checked_rhs.ty().clone()),
                *rhs_span,
            ));
        }
        Ok(CheckedExpr::new(
            CheckedExprKind::EqCmpBin(checked_lhs, op, checked_rhs),
            Type::Bool,
        ))
    }

    fn check_cmp_bin_expr(
        &self,
        lhs: &Expr<'src>,
        op: CmpBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lhs = self.check_expr(lhs, scope)?;
        let checked_rhs = self.check_expr(rhs, scope)?;
        if !checked_lhs.ty().is_comparable_to(checked_rhs.ty()) {
            return Err(CheckError::new(
                NotComparable(checked_lhs.ty().clone(), checked_rhs.ty().clone()),
                *rhs_span,
            ));
        }
        Ok(CheckedExpr::new(
            CheckedExprKind::CmpBin(checked_lhs, op, checked_rhs),
            Type::Bool,
        ))
    }

    fn check_logic_bin_expr(
        &self,
        lhs: &Expr<'src>,
        lhs_span: &SimpleSpan,
        op: LogicBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lhs = self.check_expr(lhs, scope)?;
        check_bool_type(checked_lhs.ty(), lhs_span)?;
        let checked_rhs = self.check_expr(rhs, scope)?;
        check_bool_type(checked_rhs.ty(), rhs_span)?;
        Ok(CheckedExpr::new(
            CheckedExprKind::LogicBin(checked_lhs, op, checked_rhs),
            Type::Bool,
        ))
    }

    fn check_neg_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_expr = self.check_expr(expr, scope)?;
        check_int_type(checked_expr.ty(), expr_span)?;
        Ok(CheckedExpr::new(
            CheckedExprKind::Neg(checked_expr),
            Type::I32,
        ))
    }

    fn check_not_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_expr = self.check_expr(expr, scope)?;
        check_bool_type(checked_expr.ty(), expr_span)?;
        Ok(CheckedExpr::new(
            CheckedExprKind::Not(checked_expr),
            Type::Bool,
        ))
    }

    fn check_ref_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lvalue_expr = self.check_lvalue(expr, expr_span, scope)?;
        let ty = Type::Ptr(Box::new(checked_lvalue_expr.ty().clone()));
        Ok(CheckedExpr::new(
            CheckedExprKind::Ref(checked_lvalue_expr),
            ty,
        ))
    }

    fn check_deref_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lvalue_expr = self.check_deref_lvalue(expr, expr_span, scope)?;
        let ty = checked_lvalue_expr.ty().clone();
        Ok(CheckedExpr::new(
            CheckedExprKind::LValue(checked_lvalue_expr),
            ty,
        ))
    }

    fn check_if_expr(
        &self,
        cond: &Expr<'src>,
        cond_span: &SimpleSpan,
        then: &Spanned<Block<'src>>,
        els: Option<&Spanned<Block<'src>>>,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_cond = self.check_expr(cond, scope)?;
        check_bool_type(checked_cond.ty(), cond_span)?;
        let checked_then = self.check_block(then, &mut scope.clone())?;
        if let Some(els) = els {
            let checked_else = self.check_block(els, &mut scope.clone())?;
            check_type_match(
                checked_then.tail_type(),
                checked_else.tail_type(),
                &els.span,
            )?;
            let ty = checked_then.tail_type().clone();
            Ok(CheckedExpr::new(
                CheckedExprKind::If(checked_cond, checked_then, Some(checked_else)),
                ty,
            ))
        } else {
            check_type_match(&Type::Unit, checked_then.tail_type(), &then.span)?;
            Ok(CheckedExpr::new(
                CheckedExprKind::If(checked_cond, checked_then, None),
                Type::Unit,
            ))
        }
    }

    fn check_call_expr(
        &self,
        name: &Spanned<&'src str>,
        args: &Spanned<Vec<Spanned<Expr<'src>>>>,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let Some(func_id) = self.symbols.get(name) else {
            return Err(CheckError::new(UndefinedFunction, name.span));
        };
        let func_sig = self.symbols[func_id].sig();
        if args.len() != func_sig.params().len() {
            return Err(CheckError::new(ArgNumMismatch, args.span));
        }
        let arg_values = zip(&args.inner, func_sig.params())
            .map(|(arg, param_type)| {
                let checked_arg = self.check_expr(arg, scope)?;
                check_type_match(param_type, checked_arg.ty(), &arg.span)?;
                Ok(checked_arg)
            })
            .collect::<Result<_>>()?;
        Ok(CheckedExpr::new(
            CheckedExprKind::Call(func_id, arg_values),
            func_sig.ret().clone(),
        ))
    }

    fn check_as_expr(
        &self,
        expr: &Expr<'src>,
        ty: &Type,
        span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        use {CastRule::*, CastType::*};
        let checked_expr = self.check_expr(expr, scope)?;
        let rule = match (checked_expr.ty().cast_type(), ty.cast_type()) {
            (Int, Int) => IntToInt(checked_expr.ty().is_signed()),
            (Ptr, Int) => PtrToInt,
            (Int, Ptr) => IntToPtr,
            (Ptr, Ptr) => PtrToPtr,
            _ => {
                let error_kind = CannotBeCast {
                    from: checked_expr.ty().clone(),
                    to: ty.clone(),
                };
                return Err(CheckError::new(error_kind, *span));
            }
        };
        Ok(CheckedExpr::new(
            CheckedExprKind::As(checked_expr, ty.clone(), rule),
            ty.clone(),
        ))
    }

    fn check_index_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        index: &Expr<'src>,
        index_span: &SimpleSpan,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedExpr> {
        let checked_lvalue_expr =
            self.check_index_lvalue(expr, expr_span, index, index_span, scope)?;
        let ty = checked_lvalue_expr.ty().clone();
        Ok(CheckedExpr::new(
            CheckedExprKind::LValue(checked_lvalue_expr),
            ty,
        ))
    }

    fn check_expr(&self, expr: &Expr<'src>, scope: &mut Scope<'src>) -> Result<CheckedExpr> {
        use Expr::*;
        match expr {
            Int(value) => self.check_int_expr(*value),
            Bool(value) => self.check_bool_expr(*value),
            Char(value) => self.check_char_expr(*value),
            Str(value) => self.check_str_expr(value.clone()),
            Unit => self.check_unit_expr(),
            Var(name) => self.check_var_expr(name, scope),
            ArithBin(lhs, op, rhs) => {
                self.check_arith_bin_expr(lhs, &lhs.span, *op, rhs, &rhs.span, scope)
            }
            ShiftBin(lhs, op, rhs) => {
                self.check_shift_bin_expr(lhs, &lhs.span, *op, rhs, &rhs.span, scope)
            }
            EqCmpBin(lhs, op, rhs) => self.check_eq_cmp_bin_expr(lhs, *op, rhs, &rhs.span, scope),
            CmpBin(lhs, op, rhs) => self.check_cmp_bin_expr(lhs, *op, rhs, &rhs.span, scope),
            LogicBin(lhs, op, rhs) => {
                self.check_logic_bin_expr(lhs, &lhs.span, *op, rhs, &rhs.span, scope)
            }
            Neg(expr) => self.check_neg_expr(expr, &expr.span, scope),
            Not(expr) => self.check_not_expr(expr, &expr.span, scope),
            Ref(expr) => self.check_ref_expr(expr, &expr.span, scope),
            Deref(expr) => self.check_deref_expr(expr, &expr.span, scope),
            If(cond, then, els) => self.check_if_expr(cond, &cond.span, then, els.as_ref(), scope),
            Call(name, args) => self.check_call_expr(name, args, scope),
            As(expr, ty, span) => self.check_as_expr(expr, ty, span, scope),
            Index(expr, index) => {
                self.check_index_expr(expr, &expr.span, index, &index.span, scope)
            }
        }
    }

    fn check_var_stmt(
        &self,
        name: &'src str,
        ty: &Type,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedStmt> {
        let id = scope.push(name, ty.clone());
        Ok(CheckedStmt::Var(id, ty.clone()))
    }

    fn check_assign_stmt(
        &self,
        target: &Spanned<Expr<'src>>,
        expr: &Spanned<Expr<'src>>,
        scope: &mut Scope<'src>,
    ) -> Result<CheckedStmt> {
        let addr_value = self.check_lvalue(target, &target.span, scope)?;
        let checked_expr = self.check_expr(expr, scope)?;
        check_type_match(addr_value.ty(), checked_expr.ty(), &expr.span)?;
        Ok(CheckedStmt::Assign(addr_value, checked_expr))
    }

    fn check_expr_stmt(&self, expr: &Expr<'src>, scope: &mut Scope<'src>) -> Result<CheckedStmt> {
        let checked_expr = self.check_expr(expr, scope)?;
        Ok(CheckedStmt::Expr(checked_expr))
    }

    fn check_stmt(&self, stmt: &Stmt<'src>, scope: &mut Scope<'src>) -> Result<CheckedStmt> {
        use Stmt::*;
        match stmt {
            Var(name, ty) => self.check_var_stmt(name, ty, scope),
            Assign(name, expr) => self.check_assign_stmt(name, expr, scope),
            Expr(expr) => self.check_expr_stmt(expr, scope),
        }
    }

    fn check_block(&self, block: &Block<'src>, scope: &mut Scope<'src>) -> Result<CheckedBlock> {
        let checked_stmts = block
            .stmts()
            .iter()
            .map(|stmt| self.check_stmt(stmt, scope))
            .collect::<Result<_>>()?;
        let tail = block
            .tail()
            .map(|tail| self.check_expr(tail, scope))
            .transpose()?;
        Ok(CheckedBlock::new(checked_stmts, tail))
    }

    fn check_func_item(
        &self,
        name: &Spanned<&'src str>,
        params: &[Param<'src>],
        ret_type: &Type,
        body: &Spanned<Block<'src>>,
    ) -> Result<CheckedFunc> {
        let id = self.symbols[name.inner];
        let mut scope = Scope::new();
        let checked_params = params
            .iter()
            .map(|param| {
                let id = scope.push(param.name(), param.ty().clone());
                Ok(CheckedParam::new(id, param.ty().clone()))
            })
            .collect::<Result<_>>()?;
        let checked_body = self.check_block(body, &mut scope)?;
        check_type_match(ret_type, checked_body.tail_type(), &body.span)?;
        Ok(CheckedFunc::new(id, checked_params, checked_body))
    }

    fn check_items(&self, items: &[Item<'src>]) -> Result<Vec<CheckedFunc>> {
        let mut checked_funcs = Vec::new();
        for item in items {
            if let Item::Func(name, params, ret_type, body) = item {
                checked_funcs.push(self.check_func_item(name, params, ret_type, body)?);
            }
        }
        if !matches!(self.symbols.get("main"), Some(id) if *self.symbols[id].sig() == FuncSig::new(Vec::new(), Type::I32))
        {
            return Err(CheckError::new(NoMain, SimpleSpan::default()));
        }
        Ok(checked_funcs)
    }
}

pub fn check<'src>(items: &[Item<'src>], symbols: &Symbols<'src>) -> Result<Vec<CheckedFunc>> {
    Checker::new(symbols).check_items(items)
}
