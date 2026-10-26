use std::{iter::zip, path::Path, rc::Rc};

use chumsky::span::{SimpleSpan, SpanWrap, Spanned};
use inkwell::{
    AddressSpace, IntPredicate,
    builder::Builder,
    context::Context,
    module::Module,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue},
};

use crate::{
    ast::{
        ArithBinOp, Block, CmpBinOp, Expr, Item, LogicBinOp, Param, ShiftBinOp, Stmt, Type,
        TypeKind,
    },
    error::{CheckError, CheckErrorKind::*, Error},
    func_defs::{FuncDef, FuncDefs, FuncSig},
    scope::{Scope, Var},
};

fn find_var<'src, 'ctx>(
    name: &Spanned<&'src str>,
    scope: &Scope<'src, 'ctx>,
) -> Result<Rc<Var<'src, 'ctx>>, Error> {
    scope
        .find(name)
        .ok_or_else(|| CheckError::new(UndefinedVariable, name.span).into())
}

fn check_type_match(expected: &Type, found: &Type, span: &SimpleSpan) -> Result<(), Error> {
    if expected != found {
        return Err(CheckError::new(
            TypeMismatch {
                expected: expected.clone(),
                found: found.clone(),
            },
            *span,
        )
        .into());
    }
    Ok(())
}

fn check_bool_type(ty: &Type, span: &SimpleSpan) -> Result<(), Error> {
    check_type_match(&Type::Bool, ty, span)
}

fn check_int_type(ty: &Type, span: &SimpleSpan) -> Result<(), Error> {
    let TypeKind::Int = ty.kind() else {
        return Err(CheckError::new(NotIntType(ty.clone()), *span).into());
    };
    Ok(())
}

#[derive(Debug)]
struct Value<'ctx> {
    basic_value: BasicValueEnum<'ctx>,
    ty: Type,
}

impl<'ctx> Value<'ctx> {
    fn new(basic_value: BasicValueEnum<'ctx>, ty: Type) -> Self {
        Self { basic_value, ty }
    }
}

#[derive(Debug)]
struct AddrValue<'ctx> {
    ptr_value: PointerValue<'ctx>,
    ptr_of: Type,
}

impl<'ctx> AddrValue<'ctx> {
    fn new(ptr_value: PointerValue<'ctx>, ptr_of: Type) -> Self {
        Self { ptr_value, ptr_of }
    }
}

#[derive(Debug)]
struct Compiler<'src, 'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    func_defs: FuncDefs<'src, 'ctx>,
    func_value: Option<FunctionValue<'ctx>>,
}

impl<'src, 'ctx> Compiler<'src, 'ctx> {
    fn new(context: &'ctx Context, src_path: &Path) -> Self {
        Self {
            context,
            builder: context.create_builder(),
            module: context.create_module(&src_path.to_string_lossy()),
            func_defs: FuncDefs::new(),
            func_value: None,
        }
    }

    fn llvm_type_of(&self, ty: &Type) -> BasicTypeEnum<'ctx> {
        use Type::*;
        match ty {
            U8 | I8 | Char => self.context.i8_type().as_basic_type_enum(),
            U16 | I16 => self.context.i16_type().as_basic_type_enum(),
            U32 | I32 => self.context.i32_type().as_basic_type_enum(),
            U64 | I64 => self.context.i64_type().as_basic_type_enum(),
            Bool => self.context.bool_type().as_basic_type_enum(),
            Ptr(_) | Str => self
                .context
                .ptr_type(AddressSpace::default())
                .as_basic_type_enum(),
            Unit => self.context.struct_type(&[], false).as_basic_type_enum(),
        }
    }

    fn unit_value(&self) -> Value<'ctx> {
        let basic_value = self.context.const_struct(&[], false).as_basic_value_enum();
        Value::new(basic_value, Type::Unit)
    }

    fn compile_var_addr(
        &self,
        name: &Spanned<&str>,
        scope: &Scope<'src, 'ctx>,
    ) -> Result<AddrValue<'ctx>, Error> {
        let var = find_var(name, scope)?;
        Ok(AddrValue::new(var.ptr_value(), var.ty().clone()))
    }

    fn compile_deref_addr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<AddrValue<'ctx>, Error> {
        let value = self.compile_expr(expr, scope)?;
        let Type::Ptr(ty) = value.ty else {
            return Err(CheckError::new(NotPtrType(value.ty), *expr_span).into());
        };
        Ok(AddrValue::new(value.basic_value.into_pointer_value(), *ty))
    }

    fn compile_addr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<AddrValue<'ctx>, Error> {
        use Expr::*;
        match expr {
            Var(name) => self.compile_var_addr(name, scope),
            Deref(expr) => self.compile_deref_addr(expr, &expr.span, scope),
            _ => Err(CheckError::new(CannotGetAddress, *expr_span).into()),
        }
    }

    fn compile_int_expr(&self, value: u64) -> Result<Value<'ctx>, Error> {
        let basic_value = self
            .context
            .i32_type()
            .const_int(value, false)
            .as_basic_value_enum();
        Ok(Value::new(basic_value, Type::I32))
    }

    fn compile_char_expr(&self, value: char) -> Result<Value<'ctx>, Error> {
        let basic_value = self
            .context
            .i8_type()
            .const_int(value as u64, false)
            .as_basic_value_enum();
        Ok(Value::new(basic_value, Type::Char))
    }

    fn compile_bool_expr(&self, value: bool) -> Result<Value<'ctx>, Error> {
        let basic_value = self
            .context
            .bool_type()
            .const_int(value as u64, false)
            .as_basic_value_enum();
        Ok(Value::new(basic_value, Type::Bool))
    }

    fn compile_str_expr(&self, value: &str) -> Result<Value<'ctx>, Error> {
        let global_value = self.builder.build_global_string_ptr(value, "str")?;
        Ok(Value::new(global_value.as_basic_value_enum(), Type::Str))
    }

    fn compile_unit_expr(&self) -> Result<Value<'ctx>, Error> {
        Ok(self.unit_value())
    }

    fn compile_var_expr(
        &self,
        name: &Spanned<&'src str>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let var = find_var(name, scope)?;
        let basic_value =
            self.builder
                .build_load(self.llvm_type_of(var.ty()), var.ptr_value(), "deref")?;
        Ok(Value::new(basic_value, var.ty().clone()))
    }

    fn compile_arith_bin_expr(
        &self,
        lhs: &Expr<'src>,
        lhs_span: &SimpleSpan,
        op: ArithBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use ArithBinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?;
        check_int_type(&lhs_value.ty, lhs_span)?;
        let lhs_int_value = lhs_value.basic_value.into_int_value();
        let rhs_value = self.compile_expr(rhs, scope)?;
        check_type_match(&lhs_value.ty, &rhs_value.ty, rhs_span)?;
        let rhs_int_value = rhs_value.basic_value.into_int_value();
        let int_value = match op {
            Add => self
                .builder
                .build_int_add(lhs_int_value, rhs_int_value, "add")?,
            Sub => self
                .builder
                .build_int_sub(lhs_int_value, rhs_int_value, "sub")?,
            Mul => self
                .builder
                .build_int_mul(lhs_int_value, rhs_int_value, "mul")?,
            Div => self
                .builder
                .build_int_signed_div(lhs_int_value, rhs_int_value, "sdiv")?,
            Mod => self
                .builder
                .build_int_signed_rem(lhs_int_value, rhs_int_value, "srem")?,
            And => self
                .builder
                .build_and(lhs_int_value, rhs_int_value, "and")?,
            Or => self.builder.build_or(lhs_int_value, rhs_int_value, "or")?,
            Xor => self
                .builder
                .build_xor(lhs_int_value, rhs_int_value, "xor")?,
        };
        Ok(Value::new(int_value.into(), lhs_value.ty))
    }

    fn compile_shift_bin_expr(
        &self,
        lhs: &Expr<'src>,
        lhs_span: &SimpleSpan,
        op: ShiftBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use ShiftBinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?;
        check_int_type(&lhs_value.ty, lhs_span)?;
        let lhs_int_value = lhs_value.basic_value.into_int_value();
        let rhs_value = self.compile_expr(rhs, scope)?;
        check_int_type(&rhs_value.ty, rhs_span)?;
        let rhs_int_value = rhs_value.basic_value.into_int_value();
        let int_value = match op {
            LShift => self
                .builder
                .build_left_shift(lhs_int_value, rhs_int_value, "left_shift")?,
            RShift => {
                self.builder
                    .build_right_shift(lhs_int_value, rhs_int_value, true, "right_shift")?
            }
        };
        Ok(Value::new(int_value.into(), lhs_value.ty))
    }

    fn compile_cmp_bin_expr(
        &self,
        lhs: &Expr<'src>,
        op: CmpBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use CmpBinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?;
        let lhs_int_value = lhs_value.basic_value.into_int_value();
        let rhs_value = self.compile_expr(rhs, scope)?;
        if !lhs_value.ty.is_comparable_to(&rhs_value.ty) {
            return Err(
                CheckError::new(CannotBeCompared(lhs_value.ty, rhs_value.ty), *rhs_span).into(),
            );
        }
        let rhs_int_value = rhs_value.basic_value.into_int_value();
        let int_value = self.builder.build_int_compare(
            match op {
                Eq => IntPredicate::EQ,
                NotEq => IntPredicate::NE,
                LT => IntPredicate::SLT,
                LE => IntPredicate::SLE,
                GT => IntPredicate::SGT,
                GE => IntPredicate::SGE,
            },
            lhs_int_value,
            rhs_int_value,
            "cmp",
        )?;
        Ok(Value::new(int_value.into(), Type::Bool))
    }

    fn compile_logic_bin_expr(
        &self,
        lhs: &Expr<'src>,
        lhs_span: &SimpleSpan,
        op: LogicBinOp,
        rhs: &Expr<'src>,
        rhs_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use LogicBinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?;
        let lhs_int_value = lhs_value.basic_value.into_int_value();
        check_bool_type(&lhs_value.ty, lhs_span)?;
        let rhs_value = self.compile_expr(rhs, scope)?;
        check_bool_type(&rhs_value.ty, rhs_span)?;
        let rhs_int_value = rhs_value.basic_value.into_int_value();
        let int_value = match op {
            And => self
                .builder
                .build_and(lhs_int_value, rhs_int_value, "logic_and")?,
            Or => self
                .builder
                .build_or(lhs_int_value, rhs_int_value, "logic_or")?,
        };
        Ok(Value::new(int_value.into(), Type::Bool))
    }

    fn compile_neg_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let expr_value = self.compile_expr(expr, scope)?;
        let expr_int_value = expr_value.basic_value.into_int_value();
        check_int_type(&expr_value.ty, expr_span)?;
        let int_value = self.builder.build_int_neg(expr_int_value, "neg")?;
        Ok(Value::new(int_value.into(), Type::I32))
    }

    fn compile_not_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let expr_value = self.compile_expr(expr, scope)?;
        let expr_int_value = expr_value.basic_value.into_int_value();
        check_bool_type(&expr_value.ty, expr_span)?;
        let int_value = self.builder.build_not(expr_int_value, "not")?;
        Ok(Value::new(int_value.into(), Type::Bool))
    }

    fn compile_ref_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let addr_value = self.compile_addr(expr, expr_span, scope)?;
        Ok(Value::new(
            addr_value.ptr_value.as_basic_value_enum(),
            Type::Ptr(Box::new(addr_value.ptr_of)),
        ))
    }

    fn compile_deref_expr(
        &self,
        expr: &Expr<'src>,
        expr_span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let addr_value = self.compile_deref_addr(expr, expr_span, scope)?;
        let basic_value = self.builder.build_load(
            self.llvm_type_of(&addr_value.ptr_of),
            addr_value.ptr_value,
            "deref",
        )?;
        Ok(Value::new(basic_value, addr_value.ptr_of))
    }

    fn compile_if_expr(
        &self,
        cond: &Expr<'src>,
        cond_span: &SimpleSpan,
        then: &Spanned<Block<'src>>,
        els: Option<&Spanned<Block<'src>>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let func_value = self.func_value.unwrap();

        if let Some(els) = els {
            let then_block = self.context.append_basic_block(func_value, "then");
            let else_block = self.context.append_basic_block(func_value, "else");
            let merge_block = self.context.append_basic_block(func_value, "merge");

            let cond_value = self.compile_expr(cond, scope)?;
            check_bool_type(&cond_value.ty, cond_span)?;
            self.builder.build_conditional_branch(
                cond_value.basic_value.into_int_value(),
                then_block,
                else_block,
            )?;

            self.builder.position_at_end(then_block);
            let then_value = self.compile_block(then, scope)?;
            self.builder.build_unconditional_branch(merge_block)?;
            let then_end = self.builder.get_insert_block().unwrap();

            self.builder.position_at_end(else_block);
            let else_value = self.compile_block(els, scope)?;
            check_type_match(&then_value.ty, &else_value.ty, &els.span)?;
            self.builder.build_unconditional_branch(merge_block)?;
            let else_end = self.builder.get_insert_block().unwrap();

            self.builder.position_at_end(merge_block);
            let ty = then_value.ty;
            let phi = self.builder.build_phi(self.llvm_type_of(&ty), "if")?;
            phi.add_incoming(&[
                (&then_value.basic_value, then_end),
                (&else_value.basic_value, else_end),
            ]);
            Ok(Value::new(phi.as_basic_value(), ty))
        } else {
            let then_block = self.context.append_basic_block(func_value, "then");
            let merge_block = self.context.append_basic_block(func_value, "merge");

            let cond_value = self.compile_expr(cond, scope)?;
            check_bool_type(&cond_value.ty, cond_span)?;
            self.builder.build_conditional_branch(
                cond_value.basic_value.into_int_value(),
                then_block,
                merge_block,
            )?;

            self.builder.position_at_end(then_block);
            let then_value = self.compile_block(then, scope)?;
            check_type_match(&Type::Unit, &then_value.ty, &then.span)?;
            self.builder.build_unconditional_branch(merge_block)?;

            self.builder.position_at_end(merge_block);
            Ok(self.unit_value())
        }
    }

    fn compile_call_expr(
        &self,
        name: &Spanned<&'src str>,
        args: &Spanned<Vec<Spanned<Expr<'src>>>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let Some(func_def) = self.func_defs.get(name) else {
            return Err(CheckError::new(UndefinedFunction, name.span).into());
        };
        if args.len() != func_def.sig().params().len() {
            return Err(CheckError::new(ArgNumMismatch, args.span).into());
        }
        let arg_values = args
            .iter()
            .map(|arg| Ok(self.compile_expr(arg, scope)?.with_span(arg.span)))
            .collect::<Result<Vec<Spanned<Value>>, Error>>()?;
        for (arg_value, param_type) in zip(&arg_values, func_def.sig().params()) {
            check_type_match(param_type, &arg_value.ty, &arg_value.span)?;
        }
        let call_value = self.builder.build_call(
            func_def.value(),
            &arg_values
                .iter()
                .map(|arg_value| arg_value.basic_value.into())
                .collect::<Vec<_>>(),
            "call",
        )?;
        Ok(call_value
            .try_as_basic_value()
            .basic()
            .map(|basic_value| Value::new(basic_value, func_def.sig().ret().clone()))
            .unwrap_or(self.unit_value()))
    }

    fn compile_as_expr(
        &self,
        expr: &Expr<'src>,
        ty: &Type,
        span: &SimpleSpan,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use TypeKind::*;
        let value = self.compile_expr(expr, scope)?;
        let basic_value = match (value.ty.kind(), ty.kind()) {
            (Unit, _) | (_, Unit) => {
                return Err(CheckError::new(
                    CannotBeCast {
                        from: value.ty,
                        to: ty.clone(),
                    },
                    *span,
                )
                .into());
            }
            (Int | Bool | Char, Int | Bool | Char) => self
                .builder
                .build_int_cast_sign_flag(
                    value.basic_value.into_int_value(),
                    self.llvm_type_of(ty).into_int_type(),
                    value.ty.is_signed(),
                    "cast",
                )?
                .as_basic_value_enum(),
            (Ptr, Int | Bool | Char) => self
                .builder
                .build_ptr_to_int(
                    value.basic_value.into_pointer_value(),
                    self.llvm_type_of(ty).into_int_type(),
                    "cast",
                )?
                .as_basic_value_enum(),
            (Int | Bool | Char, Ptr) => self
                .builder
                .build_int_to_ptr(
                    value.basic_value.into_int_value(),
                    self.llvm_type_of(ty).into_pointer_type(),
                    "cast",
                )?
                .as_basic_value_enum(),
            (Ptr, Ptr) => value.basic_value,
        };
        Ok(Value::new(basic_value, ty.clone()))
    }

    fn compile_expr(
        &self,
        expr: &Expr<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use Expr::*;
        match expr {
            Int(value) => self.compile_int_expr(*value),
            Bool(value) => self.compile_bool_expr(*value),
            Char(value) => self.compile_char_expr(*value),
            Str(value) => self.compile_str_expr(value),
            Unit => self.compile_unit_expr(),
            Var(name) => self.compile_var_expr(name, scope),
            ArithBin(lhs, op, rhs) => {
                self.compile_arith_bin_expr(lhs, &lhs.span, *op, rhs, &rhs.span, scope)
            }
            ShiftBin(lhs, op, rhs) => {
                self.compile_shift_bin_expr(lhs, &lhs.span, *op, rhs, &rhs.span, scope)
            }
            CmpBin(lhs, op, rhs) => self.compile_cmp_bin_expr(lhs, *op, rhs, &rhs.span, scope),
            LogicBin(lhs, op, rhs) => {
                self.compile_logic_bin_expr(lhs, &lhs.span, *op, rhs, &rhs.span, scope)
            }
            Neg(expr) => self.compile_neg_expr(expr, &expr.span, scope),
            Not(expr) => self.compile_not_expr(expr, &expr.span, scope),
            Ref(expr) => self.compile_ref_expr(expr, &expr.span, scope),
            Deref(expr) => self.compile_deref_expr(expr, &expr.span, scope),
            If(cond, then, els) => {
                self.compile_if_expr(cond, &cond.span, then, els.as_ref(), scope)
            }
            Call(name, args) => self.compile_call_expr(name, args, scope),
            As(expr, ty, span) => self.compile_as_expr(expr, ty, span, scope),
        }
    }

    fn compile_var_stmt(
        &self,
        name: &'src str,
        ty: &Type,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let value = self.builder.build_alloca(self.llvm_type_of(ty), name)?;
        scope.push(Var::new(name, value, ty.clone()));
        Ok(())
    }

    fn compile_assign_stmt(
        &self,
        target: &Spanned<Expr<'src>>,
        expr: &Spanned<Expr<'src>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let addr_value = self.compile_addr(target, &target.span, scope)?;
        let value = self.compile_expr(expr, scope)?;
        check_type_match(&addr_value.ptr_of, &value.ty, &expr.span)?;
        self.builder
            .build_store(addr_value.ptr_value, value.basic_value)?;
        Ok(())
    }

    fn compile_expr_stmt(
        &self,
        expr: &Expr<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        self.compile_expr(expr, scope)?;
        Ok(())
    }

    fn compile_stmt(&self, stmt: &Stmt<'src>, scope: &mut Scope<'src, 'ctx>) -> Result<(), Error> {
        use Stmt::*;
        match stmt {
            Var(name, ty) => self.compile_var_stmt(name, ty, scope),
            Assign(name, expr) => self.compile_assign_stmt(name, expr, scope),
            Expr(expr) => self.compile_expr_stmt(expr, scope),
        }
    }

    fn compile_block(
        &self,
        block: &Block<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        for stmt in block.stmts() {
            self.compile_stmt(stmt, scope)?;
        }
        if let Some(tail) = block.tail() {
            self.compile_expr(tail, scope)
        } else {
            Ok(self.unit_value())
        }
    }

    fn define_func(
        &mut self,
        name: &Spanned<&'src str>,
        param_types: Vec<Type>,
        ret_type: &Type,
    ) -> Result<FunctionValue<'ctx>, Error> {
        let None = self.func_defs.get(name) else {
            return Err(CheckError::new(AlreadyDefined, name.span).into());
        };
        let llvm_param_types = param_types
            .iter()
            .map(|param_type| self.llvm_type_of(param_type).into())
            .collect::<Vec<_>>();
        let llvm_func_type = if let Type::Unit = ret_type {
            self.context.void_type().fn_type(&llvm_param_types, false)
        } else {
            self.llvm_type_of(ret_type)
                .fn_type(&llvm_param_types, false)
        };
        let func_value = self.module.add_function(name, llvm_func_type, None);
        self.func_defs.insert(
            name,
            FuncDef::new(func_value, FuncSig::new(param_types, ret_type.clone())),
        );
        Ok(func_value)
    }

    fn compile_func_item(
        &mut self,
        name: &Spanned<&'src str>,
        params: &[Param<'src>],
        ret_type: &Type,
        body: &Block<'src>,
    ) -> Result<(), Error> {
        let param_types = params
            .iter()
            .map(|param| param.ty().clone())
            .collect::<Vec<_>>();
        let func_value = self.define_func(name, param_types, ret_type)?;
        self.func_value = Some(func_value);

        let mut scope = Scope::new();
        let entry = self.context.append_basic_block(func_value, "entry");
        self.builder.position_at_end(entry);

        for (i, param) in params.iter().enumerate() {
            let ptr_value = self
                .builder
                .build_alloca(self.llvm_type_of(param.ty()), param.name())?;
            let basic_value = func_value.get_nth_param(i as u32).unwrap();
            self.builder.build_store(ptr_value, basic_value)?;
            scope.push(Var::new(param.name(), ptr_value, param.ty().clone()));
        }

        let ret_value = self.compile_block(body, &mut scope)?;
        self.builder.build_return(if let Type::Unit = ret_type {
            None
        } else {
            Some(&ret_value.basic_value)
        })?;
        Ok(())
    }

    fn compile_extern_item(
        &mut self,
        name: &Spanned<&'src str>,
        param_types: &[Type],
        ret_type: &Type,
    ) -> Result<(), Error> {
        self.define_func(name, param_types.to_vec(), ret_type)?;
        Ok(())
    }

    fn compile_item(&mut self, item: &Item<'src>) -> Result<(), Error> {
        use Item::*;
        match item {
            Func(name, params, ret_type, body) => {
                self.compile_func_item(name, params, ret_type, body)
            }
            Extern(name, param_types, ret_type) => {
                self.compile_extern_item(name, param_types, ret_type)
            }
        }
    }

    fn compile_items(&mut self, items: &[Item<'src>]) -> Result<(), Error> {
        for item in items {
            self.compile_item(item)?;
        }

        if !matches!(self.func_defs.get("main"), Some(def) if *def.sig() == FuncSig::new(Vec::new(), Type::I32))
        {
            return Err(CheckError::new(NoMain, SimpleSpan::default()).into());
        }
        Ok(())
    }
}

pub fn compile<'src, 'ctx>(
    context: &'ctx Context,
    src_path: &Path,
    items: &[Item<'src>],
) -> Result<Module<'ctx>, Error> {
    let mut compiler = Compiler::new(context, src_path);
    compiler.compile_items(items)?;
    compiler
        .module
        .verify()
        .map_err(|error| Error::Verify(error.to_string()))?;
    Ok(compiler.module)
}
