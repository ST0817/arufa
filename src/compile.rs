use std::{iter::zip, rc::Rc};

use chumsky::span::{SimpleSpan, SpanWrap, Spanned};
use inkwell::{
    AddressSpace, IntPredicate,
    builder::Builder,
    context::Context,
    module::Module,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, FunctionValue},
};

use crate::{
    ast::{ArithBinOp, Block, CmpBinOp, Expr, Stmt, Type, UnaryOp},
    error::{CheckError, CheckErrorKind::*, Error},
    func_defs::{FuncDef, FuncDefs},
    scope::{Scope, Var},
};

fn find_var<'src, 'ctx>(
    name: &Spanned<&'src str>,
    scope: &mut Scope<'src, 'ctx>,
) -> Result<Rc<Var<'src, 'ctx>>, Error> {
    scope
        .find(name)
        .ok_or_else(|| CheckError::new(UndefinedVariable, name.span).into())
}

fn check_type_match(expected: Type, found: Type, span: &SimpleSpan) -> Result<(), Error> {
    if expected != found {
        return Err(CheckError::new(TypeMismatch { expected, found }, *span).into());
    }
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

struct Compiler<'src, 'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    main_func: FunctionValue<'ctx>,
    decls: FuncDefs<'src, 'ctx>,
}

impl<'src, 'ctx> Compiler<'src, 'ctx> {
    fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let main_type = context.i32_type().fn_type(&[], false);
        let main_func = module.add_function("main", main_type, None);
        Self {
            context,
            builder: context.create_builder(),
            module,
            main_func,
            decls: FuncDefs::new(),
        }
    }

    fn llvm_type_of(&self, ty: Type) -> BasicTypeEnum<'ctx> {
        use Type::*;
        match ty {
            I32 => self.context.i32_type().as_basic_type_enum(),
            Bool => self.context.bool_type().as_basic_type_enum(),
        }
    }

    fn compile_int_expr(&self, value: u32) -> Result<Value<'ctx>, Error> {
        let basic_value = self
            .context
            .i32_type()
            .const_int(value as u64, false)
            .as_basic_value_enum();
        Ok(Value::new(basic_value, Type::I32))
    }

    fn compile_bool_expr(&self, value: bool) -> Result<Value<'ctx>, Error> {
        let basic_value = self
            .context
            .bool_type()
            .const_int(value as u64, false)
            .as_basic_value_enum();
        Ok(Value::new(basic_value, Type::Bool))
    }

    fn compile_var_expr(
        &self,
        name: &Spanned<&'src str>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let var = find_var(name, scope)?;
        let basic_value =
            self.builder
                .build_load(self.context.i32_type(), var.ptr_value(), "deref")?;
        Ok(Value::new(basic_value, var.ty()))
    }

    fn compile_arith_bin_expr(
        &self,
        lhs: &Spanned<Box<Expr<'src>>>,
        op: ArithBinOp,
        rhs: &Spanned<Box<Expr<'src>>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use ArithBinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?;
        let lhs_int_value = lhs_value.basic_value.into_int_value();
        check_type_match(Type::I32, lhs_value.ty, &lhs.span)?;
        let rhs_value = self.compile_expr(rhs, scope)?;
        check_type_match(Type::I32, rhs_value.ty, &rhs.span)?;
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
        };
        Ok(Value::new(int_value.into(), Type::I32))
    }

    fn compile_cmp_bin_expr(
        &self,
        lhs: &Spanned<Box<Expr<'src>>>,
        op: CmpBinOp,
        rhs: &Spanned<Box<Expr<'src>>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use CmpBinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?;
        let lhs_int_value = lhs_value.basic_value.into_int_value();
        check_type_match(Type::I32, lhs_value.ty, &lhs.span)?;
        let rhs_value = self.compile_expr(rhs, scope)?;
        check_type_match(Type::I32, rhs_value.ty, &rhs.span)?;
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

    fn compile_unary_expr(
        &self,
        expr: &Spanned<Box<Expr<'src>>>,
        op: UnaryOp,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use UnaryOp::*;
        let expr_value = self.compile_expr(expr, scope)?;
        let expr_int_value = expr_value.basic_value.into_int_value();
        match op {
            Neg => {
                check_type_match(Type::I32, expr_value.ty, &expr.span)?;
                let int_value = self.builder.build_int_neg(expr_int_value, "neg")?;
                Ok(Value::new(int_value.into(), Type::I32))
            }
            Not => {
                check_type_match(Type::Bool, expr_value.ty, &expr.span)?;
                let int_value = self.builder.build_not(expr_int_value, "not")?;
                Ok(Value::new(int_value.into(), Type::Bool))
            }
        }
    }

    fn compile_if_expr(
        &self,
        cond: &Spanned<Box<Expr<'src>>>,
        then: &Block<'src>,
        els: &Spanned<Block<'src>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let then_block = self.context.append_basic_block(self.main_func, "then");
        let else_block = self.context.append_basic_block(self.main_func, "else");
        let merge_block = self.context.append_basic_block(self.main_func, "merge");

        let cond_value = self.compile_expr(cond, scope)?;
        check_type_match(Type::Bool, cond_value.ty, &cond.span)?;
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
        check_type_match(then_value.ty, else_value.ty, &els.span)?;
        self.builder.build_unconditional_branch(merge_block)?;
        let else_end = self.builder.get_insert_block().unwrap();

        self.builder.position_at_end(merge_block);
        let ty = then_value.ty;
        let phi = self.builder.build_phi(self.llvm_type_of(ty), "if")?;
        phi.add_incoming(&[
            (&then_value.basic_value, then_end),
            (&else_value.basic_value, else_end),
        ]);

        Ok(Value::new(phi.as_basic_value(), ty))
    }

    fn compile_call_expr(
        &self,
        name: &Spanned<&'src str>,
        args: &Spanned<Vec<Spanned<Expr<'src>>>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        let Some(func_def) = self.decls.get(name) else {
            return Err(CheckError::new(UndefinedFunction, name.span).into());
        };
        if args.len() != func_def.param_types().len() {
            return Err(CheckError::new(ArgNumMismatch, args.span).into());
        }
        let arg_values = args
            .iter()
            .map(|arg| Ok(self.compile_expr(arg, scope)?.with_span(arg.span)))
            .collect::<Result<Vec<Spanned<Value>>, Error>>()?;
        for (arg_value, param_type) in zip(&arg_values, func_def.param_types()) {
            check_type_match(*param_type, arg_value.ty, &arg_value.span)?;
        }
        let call_value = self.builder.build_call(
            func_def.func_value(),
            &arg_values
                .iter()
                .map(|arg_value| arg_value.basic_value.into())
                .collect::<Vec<_>>(),
            "call",
        )?;
        Ok(Value::new(
            call_value.try_as_basic_value().unwrap_basic(),
            func_def.ret_type(),
        ))
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
            Var(name) => self.compile_var_expr(name, scope),
            ArithBin(lhs, op, rhs) => self.compile_arith_bin_expr(lhs, *op, rhs, scope),
            CmpBin(lhs, op, rhs) => self.compile_cmp_bin_expr(lhs, *op, rhs, scope),
            Unary(expr, op) => self.compile_unary_expr(expr, *op, scope),
            If(cond, then, els) => self.compile_if_expr(cond, then, els, scope),
            Call(name, args) => self.compile_call_expr(name, args, scope),
        }
    }

    fn compile_var_stmt(
        &self,
        name: &'src str,
        ty: Type,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let value = self.builder.build_alloca(self.context.i32_type(), name)?;
        scope.push(Var::new(name, value, ty));
        Ok(())
    }

    fn compile_assign_stmt(
        &self,
        name: &Spanned<&'src str>,
        expr: &Spanned<Expr<'src>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let var = find_var(name, scope)?;
        let value = self.compile_expr(expr, scope)?;
        check_type_match(var.ty(), value.ty, &expr.span)?;
        self.builder
            .build_store(var.ptr_value(), value.basic_value)?;
        Ok(())
    }

    fn compile_stmt(&self, stmt: &Stmt<'src>, scope: &mut Scope<'src, 'ctx>) -> Result<(), Error> {
        use Stmt::*;
        match stmt {
            Var(name, ty) => self.compile_var_stmt(name, *ty, scope),
            Assign(name, expr) => self.compile_assign_stmt(name, expr, scope),
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
        self.compile_expr(block.tail(), scope)
    }

    fn def_iprint(&mut self) -> Result<(), Error> {
        let printf_type = self.context.void_type().fn_type(
            &[self.context.ptr_type(AddressSpace::default()).into()],
            true,
        );
        let printf = self.module.add_function("printf", printf_type, None);

        let iprint_type = self
            .context
            .i32_type()
            .fn_type(&[self.context.i32_type().into()], false);
        let iprint = self.module.add_function("iprint", iprint_type, None);
        self.decls
            .insert("iprint", FuncDef::new(iprint, vec![Type::I32], Type::I32));
        let iprint_entry = self.context.append_basic_block(iprint, "entry");
        self.builder.position_at_end(iprint_entry);
        let param = iprint.get_nth_param(0).unwrap().into_int_value();
        let fmt = self.builder.build_global_string_ptr("%d\n", "fmt")?;
        self.builder.build_call(
            printf,
            &[fmt.as_pointer_value().into(), param.into()],
            "printf_call",
        )?;
        self.builder.build_return(Some(
            &self
                .context
                .i32_type()
                .const_int(0, false)
                .as_basic_value_enum(),
        ))?;
        Ok(())
    }
}

pub fn compile<'ctx>(context: &'ctx Context, block: &Block) -> Result<Module<'ctx>, Error> {
    let mut compiler = Compiler::new(context);
    compiler.def_iprint()?;
    let entry = compiler
        .context
        .append_basic_block(compiler.main_func, "entry");
    compiler.builder.position_at_end(entry);
    let mut scope = Scope::new();
    let value = compiler.compile_block(block, &mut scope)?;
    compiler.builder.build_return(Some(&value.basic_value))?;
    compiler
        .module
        .verify()
        .map_err(|error| Error::Verify(error.to_string()))?;
    Ok(compiler.module)
}
