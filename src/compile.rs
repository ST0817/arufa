use std::{collections::HashMap, ops::Not, path::Path};

use inkwell::{
    AddressSpace, IntPredicate,
    builder::Builder,
    context::Context,
    module::Module,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue},
};

use crate::{
    ast::{ArithBinOp, CmpBinOp, EqCmpBinOp, LogicBinOp, ShiftBinOp, Type},
    checked_ast::{
        CastRule, CheckedBlock, CheckedExpr, CheckedExprKind, CheckedFunc, CheckedLValueExpr,
        CheckedLValueExprKind, CheckedStmt,
    },
    error::Error,
    scope::VarId,
    symbols::{FuncId, Symbols},
};

type Result<T> = std::result::Result<T, Error>;

type Locals<'ctx> = HashMap<VarId, PointerValue<'ctx>>;

#[derive(Debug)]
struct Compiler<'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    func_values: HashMap<FuncId, FunctionValue<'ctx>>,
    func_value: Option<FunctionValue<'ctx>>,
}

impl<'ctx> Compiler<'ctx> {
    fn new(context: &'ctx Context, src_path: &Path) -> Self {
        Self {
            context,
            builder: context.create_builder(),
            module: context.create_module(&src_path.to_string_lossy()),
            func_values: HashMap::new(),
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
            Ptr(_) | ArrayPtr(_) => self
                .context
                .ptr_type(AddressSpace::default())
                .as_basic_type_enum(),
            Unit => self.context.struct_type(&[], false).as_basic_type_enum(),
            Array(ty, len) => self
                .llvm_type_of(ty)
                .array_type(*len as u32)
                .as_basic_type_enum(),
        }
    }

    fn unit_value(&self) -> BasicValueEnum<'ctx> {
        self.context.const_struct(&[], false).as_basic_value_enum()
    }

    fn compile_var_lvalue(&self, id: &VarId, locals: &Locals<'ctx>) -> Result<PointerValue<'ctx>> {
        Ok(locals[id])
    }

    fn compile_deref_lvalue(
        &self,
        expr: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<PointerValue<'ctx>> {
        let value = self.compile_expr(expr, locals)?;
        Ok(value.into_pointer_value())
    }

    fn compile_index_lvalue(
        &self,
        expr: &CheckedLValueExpr,
        is_ptr: bool,
        index: &CheckedExpr,
        elem_type: &Type,
        locals: &mut Locals<'ctx>,
    ) -> Result<PointerValue<'ctx>> {
        let expr_ptr = self.compile_lvalue(expr, locals)?;
        let expr_ptr = if is_ptr {
            self.builder
                .build_load(self.context.ptr_type(AddressSpace::default()), expr_ptr, "")?
                .into_pointer_value()
        } else {
            expr_ptr
        };
        let index_value = self.compile_expr(index, locals)?;
        let elem_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                self.llvm_type_of(elem_type),
                expr_ptr,
                &[index_value.into_int_value()],
                "",
            )?
        };
        Ok(elem_ptr)
    }

    fn compile_lvalue(
        &self,
        expr: &CheckedLValueExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<PointerValue<'ctx>> {
        use CheckedLValueExprKind::*;
        match expr.kind() {
            Var(id) => self.compile_var_lvalue(id, locals),
            Deref(expr) => self.compile_deref_lvalue(expr, locals),
            Index(array, is_ptr, index) => {
                self.compile_index_lvalue(array, *is_ptr, index, expr.ty(), locals)
            }
        }
    }

    fn compile_int_expr(&self, value: u64) -> Result<BasicValueEnum<'ctx>> {
        let value = self
            .context
            .i32_type()
            .const_int(value, false)
            .as_basic_value_enum();
        Ok(value)
    }

    fn compile_char_expr(&self, value: char) -> Result<BasicValueEnum<'ctx>> {
        let value = self
            .context
            .i8_type()
            .const_int(value as u64, false)
            .as_basic_value_enum();
        Ok(value)
    }

    fn compile_bool_expr(&self, value: bool) -> Result<BasicValueEnum<'ctx>> {
        let value = self
            .context
            .bool_type()
            .const_int(value as u64, false)
            .as_basic_value_enum();
        Ok(value)
    }

    fn compile_str_expr(&self, value: &str) -> Result<BasicValueEnum<'ctx>> {
        let value = self.builder.build_global_string_ptr(value, "")?;
        Ok(value.as_basic_value_enum())
    }

    fn compile_unit_expr(&self) -> Result<BasicValueEnum<'ctx>> {
        Ok(self.unit_value())
    }

    fn compile_arith_bin_expr(
        &self,
        lhs: &CheckedExpr,
        op: ArithBinOp,
        rhs: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use ArithBinOp::*;
        let lhs_value = self.compile_expr(lhs, locals)?.into_int_value();
        let rhs_value = self.compile_expr(rhs, locals)?.into_int_value();
        let value = match op {
            Add => self.builder.build_int_add(lhs_value, rhs_value, "")?,
            Sub => self.builder.build_int_sub(lhs_value, rhs_value, "")?,
            Mul => self.builder.build_int_mul(lhs_value, rhs_value, "")?,
            Div => self
                .builder
                .build_int_signed_div(lhs_value, rhs_value, "")?,
            Mod => self
                .builder
                .build_int_signed_rem(lhs_value, rhs_value, "")?,
            And => self.builder.build_and(lhs_value, rhs_value, "")?,
            Or => self.builder.build_or(lhs_value, rhs_value, "")?,
            Xor => self.builder.build_xor(lhs_value, rhs_value, "")?,
        };
        Ok(value.as_basic_value_enum())
    }

    fn compile_shift_bin_expr(
        &self,
        lhs: &CheckedExpr,
        op: ShiftBinOp,
        rhs: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use ShiftBinOp::*;
        let lhs_value = self.compile_expr(lhs, locals)?.into_int_value();
        let rhs_value = self.compile_expr(rhs, locals)?.into_int_value();
        let value = match op {
            LShift => self.builder.build_left_shift(lhs_value, rhs_value, "")?,
            RShift => self
                .builder
                .build_right_shift(lhs_value, rhs_value, true, "")?,
        };
        Ok(value.as_basic_value_enum())
    }

    fn compile_eq_cmp_bin_expr(
        &self,
        lhs: &CheckedExpr,
        op: EqCmpBinOp,
        rhs: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use EqCmpBinOp::*;
        let lhs_value = self.compile_expr(lhs, locals)?.into_int_value();
        let rhs_value = self.compile_expr(rhs, locals)?.into_int_value();
        let value = self.builder.build_int_compare(
            match op {
                Eq => IntPredicate::EQ,
                NotEq => IntPredicate::NE,
            },
            lhs_value,
            rhs_value,
            "cmp",
        )?;
        Ok(value.as_basic_value_enum())
    }

    fn compile_cmp_bin_expr(
        &self,
        lhs: &CheckedExpr,
        op: CmpBinOp,
        rhs: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use {CmpBinOp::*, IntPredicate::*};
        let lhs_value = self.compile_expr(lhs, locals)?.into_int_value();
        let rhs_value = self.compile_expr(rhs, locals)?.into_int_value();
        let value = self.builder.build_int_compare(
            if lhs.ty().is_signed() {
                match op {
                    LT => SLT,
                    LE => SLE,
                    GT => SGT,
                    GE => SGE,
                }
            } else {
                match op {
                    LT => ULT,
                    LE => ULE,
                    GT => UGT,
                    GE => UGE,
                }
            },
            lhs_value,
            rhs_value,
            "cmp",
        )?;
        Ok(value.as_basic_value_enum())
    }

    fn compile_logic_bin_expr(
        &self,
        lhs: &CheckedExpr,
        op: LogicBinOp,
        rhs: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use LogicBinOp::*;
        let lhs_value = self.compile_expr(lhs, locals)?.into_int_value();
        let rhs_value = self.compile_expr(rhs, locals)?.into_int_value();
        let value = match op {
            And => self.builder.build_and(lhs_value, rhs_value, "")?,
            Or => self.builder.build_or(lhs_value, rhs_value, "")?,
        };
        Ok(value.as_basic_value_enum())
    }

    fn compile_neg_expr(
        &self,
        expr: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let expr_value = self.compile_expr(expr, locals)?.into_int_value();
        let value = self.builder.build_int_neg(expr_value, "")?;
        Ok(value.as_basic_value_enum())
    }

    fn compile_not_expr(
        &self,
        expr: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let expr_value = self.compile_expr(expr, locals)?.into_int_value();
        let value = self.builder.build_not(expr_value, "")?;
        Ok(value.as_basic_value_enum())
    }

    fn compile_ref_expr(
        &self,
        expr: &CheckedLValueExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let ptr = self.compile_lvalue(expr, locals)?;
        Ok(ptr.as_basic_value_enum())
    }

    fn compile_lvalue_expr(
        &self,
        expr: &CheckedLValueExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let ptr = self.compile_lvalue(expr, locals)?;
        let value = if expr.ty().is_aggregate() {
            ptr.as_basic_value_enum()
        } else {
            self.builder
                .build_load(self.llvm_type_of(expr.ty()), ptr, "")?
                .as_basic_value_enum()
        };
        Ok(value)
    }

    fn compile_if_expr(
        &self,
        cond: &CheckedExpr,
        then: &CheckedBlock,
        els: Option<&CheckedBlock>,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        if let Some(els) = els {
            let func_value = self.func_value.unwrap();
            let then_block = self.context.append_basic_block(func_value, "then");
            let else_block = self.context.append_basic_block(func_value, "else");
            let merge_block = self.context.append_basic_block(func_value, "merge");

            let cond_value = self.compile_expr(cond, locals)?.into_int_value();
            self.builder
                .build_conditional_branch(cond_value, then_block, else_block)?;

            self.builder.position_at_end(then_block);
            let then_value = self.compile_block(then, locals)?;
            self.builder.build_unconditional_branch(merge_block)?;
            let then_end = self.builder.get_insert_block().unwrap();

            self.builder.position_at_end(else_block);
            let else_value = self.compile_block(els, locals)?;
            self.builder.build_unconditional_branch(merge_block)?;
            let else_end = self.builder.get_insert_block().unwrap();

            self.builder.position_at_end(merge_block);
            let phi = self
                .builder
                .build_phi(self.llvm_type_of(then.tail_type()), "")?;
            phi.add_incoming(&[(&then_value, then_end), (&else_value, else_end)]);
            Ok(phi.as_basic_value())
        } else {
            let func_value = self.func_value.unwrap();
            let then_block = self.context.append_basic_block(func_value, "then");
            let merge_block = self.context.append_basic_block(func_value, "merge");

            let cond_value = self.compile_expr(cond, locals)?.into_int_value();
            self.builder
                .build_conditional_branch(cond_value, then_block, merge_block)?;

            self.builder.position_at_end(then_block);
            let then_value = self.compile_block(then, locals)?;
            self.builder.build_unconditional_branch(merge_block)?;

            self.builder.position_at_end(merge_block);
            Ok(then_value)
        }
    }

    fn compile_call_expr(
        &self,
        func_id: &FuncId,
        args: &[CheckedExpr],
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let arg_values = args
            .iter()
            .map(|arg| Ok(self.compile_expr(arg, locals)?.into()))
            .collect::<Result<Vec<_>>>()?;
        let call_value = self
            .builder
            .build_call(self.func_values[func_id], &arg_values, "")?;
        Ok(call_value
            .try_as_basic_value()
            .basic()
            .unwrap_or(self.unit_value()))
    }

    fn compile_as_expr(
        &self,
        expr: &CheckedExpr,
        ty: &Type,
        rule: CastRule,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use CastRule::*;
        let expr_value = self.compile_expr(expr, locals)?;
        let value = match rule {
            IntToInt(is_signed) => self
                .builder
                .build_int_cast_sign_flag(
                    expr_value.into_int_value(),
                    self.llvm_type_of(ty).into_int_type(),
                    is_signed,
                    "",
                )?
                .as_basic_value_enum(),
            PtrToInt => self
                .builder
                .build_ptr_to_int(
                    expr_value.into_pointer_value(),
                    self.llvm_type_of(ty).into_int_type(),
                    "",
                )?
                .as_basic_value_enum(),
            IntToPtr => self
                .builder
                .build_int_to_ptr(
                    expr_value.into_int_value(),
                    self.llvm_type_of(ty).into_pointer_type(),
                    "",
                )?
                .as_basic_value_enum(),
            PtrToPtr => expr_value,
        };
        Ok(value)
    }

    fn compile_expr(
        &self,
        expr: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        use CheckedExprKind::*;
        match expr.kind() {
            Int(value) => self.compile_int_expr(*value),
            Bool(value) => self.compile_bool_expr(*value),
            Char(value) => self.compile_char_expr(*value),
            Str(value) => self.compile_str_expr(value),
            Unit => self.compile_unit_expr(),
            LValue(expr) => self.compile_lvalue_expr(expr, locals),
            ArithBin(lhs, op, rhs) => self.compile_arith_bin_expr(lhs, *op, rhs, locals),
            ShiftBin(lhs, op, rhs) => self.compile_shift_bin_expr(lhs, *op, rhs, locals),
            EqCmpBin(lhs, op, rhs) => self.compile_eq_cmp_bin_expr(lhs, *op, rhs, locals),
            CmpBin(lhs, op, rhs) => self.compile_cmp_bin_expr(lhs, *op, rhs, locals),
            LogicBin(lhs, op, rhs) => self.compile_logic_bin_expr(lhs, *op, rhs, locals),
            Neg(expr) => self.compile_neg_expr(expr, locals),
            Not(expr) => self.compile_not_expr(expr, locals),
            Ref(expr) => self.compile_ref_expr(expr, locals),
            If(cond, then, els) => self.compile_if_expr(cond, then, els.as_ref(), locals),
            Call(func_id, args) => self.compile_call_expr(func_id, args, locals),
            As(expr, ty, rule) => self.compile_as_expr(expr, ty, *rule, locals),
        }
    }

    fn compile_var_stmt(&self, id: VarId, ty: &Type, locals: &mut Locals<'ctx>) -> Result<()> {
        let ptr = self.builder.build_alloca(self.llvm_type_of(ty), "")?;
        locals.insert(id, ptr);
        Ok(())
    }

    fn compile_assign_stmt(
        &self,
        target: &CheckedLValueExpr,
        expr: &CheckedExpr,
        locals: &mut Locals<'ctx>,
    ) -> Result<()> {
        let ptr = self.compile_lvalue(target, locals)?;
        let value = self.compile_expr(expr, locals)?;
        let value = if expr.ty().is_aggregate() {
            self.builder
                .build_load(self.llvm_type_of(expr.ty()), value.into_pointer_value(), "")?
        } else {
            value
        };
        self.builder.build_store(ptr, value)?;
        Ok(())
    }

    fn compile_expr_stmt(&self, expr: &CheckedExpr, locals: &mut Locals<'ctx>) -> Result<()> {
        self.compile_expr(expr, locals)?;
        Ok(())
    }

    fn compile_stmt(&self, stmt: &CheckedStmt, locals: &mut Locals<'ctx>) -> Result<()> {
        use CheckedStmt::*;
        match stmt {
            Var(id, ty) => self.compile_var_stmt(*id, ty, locals),
            Assign(name, expr) => self.compile_assign_stmt(name, expr, locals),
            Expr(expr) => self.compile_expr_stmt(expr, locals),
        }
    }

    fn compile_block(
        &self,
        block: &CheckedBlock,
        locals: &mut Locals<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        for stmt in block.stmts() {
            self.compile_stmt(stmt, locals)?;
        }
        if let Some(tail) = block.tail() {
            self.compile_expr(tail, locals)
        } else {
            Ok(self.unit_value())
        }
    }

    fn declare_funcs(&mut self, symbols: &Symbols) -> Result<()> {
        for func_decl in symbols {
            let llvm_param_types = func_decl
                .sig()
                .params()
                .iter()
                .map(|param_type| {
                    if param_type.is_aggregate() {
                        self.context.ptr_type(AddressSpace::default()).into()
                    } else {
                        self.llvm_type_of(param_type).into()
                    }
                })
                .collect::<Vec<_>>();
            let llvm_func_type = if let Type::Unit = func_decl.sig().ret() {
                self.context.void_type().fn_type(&llvm_param_types, false)
            } else {
                self.llvm_type_of(func_decl.sig().ret())
                    .fn_type(&llvm_param_types, false)
            };
            let func_value = self
                .module
                .add_function(func_decl.name(), llvm_func_type, None);
            self.func_values.insert(func_decl.id(), func_value);
        }
        Ok(())
    }

    fn compile_func(&mut self, func: &CheckedFunc) -> Result<()> {
        let func_value = self.func_values[&func.id()];
        self.func_value = Some(func_value);

        let mut locals = Locals::new();
        let entry = self.context.append_basic_block(func_value, "entry");
        self.builder.position_at_end(entry);

        for (i, param) in func.params().iter().enumerate() {
            let ptr = self
                .builder
                .build_alloca(self.llvm_type_of(param.ty()), "")?;
            let value = func_value.get_nth_param(i as u32).unwrap();
            let value = if param.ty().is_aggregate() {
                self.builder.build_load(
                    self.llvm_type_of(param.ty()),
                    value.into_pointer_value(),
                    "",
                )?
            } else {
                value
            };
            self.builder.build_store(ptr, value)?;
            locals.insert(param.id(), ptr);
        }

        let ret_value = self.compile_block(func.body(), &mut locals)?;
        self.builder.build_return(
            matches!(func.body().tail_type(), Type::Unit)
                .not()
                .then_some(&ret_value),
        )?;
        if !func_value.verify(true) {
            return Err(Error::FuncVerify);
        }
        Ok(())
    }
}

pub fn compile<'src, 'ctx>(
    context: &'ctx Context,
    symbols: &Symbols<'src>,
    src_path: &Path,
    funcs: &[CheckedFunc],
) -> Result<Module<'ctx>> {
    let mut compiler = Compiler::new(context, src_path);
    compiler.declare_funcs(symbols)?;
    for func in funcs {
        compiler.compile_func(func)?;
    }
    compiler
        .module
        .verify()
        .map_err(|error| Error::ModuleVerify(error.to_string()))?;
    Ok(compiler.module)
}
