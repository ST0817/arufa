use std::rc::Rc;

use chumsky::span::{SimpleSpan, Spanned};
use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    values::{BasicValue, BasicValueEnum},
};

use crate::{
    ast::{BinOp, Block, Expr, Stmt, Type},
    error::{CheckError, CheckErrorKind::*, Error},
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

pub struct Compiler<'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        Self {
            context,
            builder: context.create_builder(),
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

    fn compile_var_expr<'src>(
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

    fn compile_bin_expr<'src>(
        &self,
        lhs: &Spanned<Box<Expr<'src>>>,
        op: BinOp,
        rhs: &Spanned<Box<Expr<'src>>>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use BinOp::*;
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

    fn compile_expr<'src>(
        &self,
        expr: &Expr<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        use Expr::*;
        match expr {
            Int(value) => self.compile_int_expr(*value),
            Bool(value) => self.compile_bool_expr(*value),
            Var(name) => self.compile_var_expr(name, scope),
            Bin(lhs, op, rhs) => self.compile_bin_expr(lhs, *op, rhs, scope),
        }
    }

    fn compile_var_stmt<'src>(
        &self,
        name: &'src str,
        ty: Type,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let value = self.builder.build_alloca(self.context.i32_type(), name)?;
        scope.push(Var::new(name, value, ty));
        Ok(())
    }

    fn compile_assign_stmt<'src>(
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

    fn compile_stmt<'src>(
        &self,
        stmt: &Stmt<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        use Stmt::*;
        match stmt {
            Var(name, ty) => self.compile_var_stmt(name, *ty, scope),
            Assign(name, expr) => self.compile_assign_stmt(name, expr, scope),
        }
    }

    fn compile_block<'src>(
        &self,
        block: &Block<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<Value<'ctx>, Error> {
        for stmt in block.stmts() {
            self.compile_stmt(stmt, scope)?;
        }
        self.compile_expr(block.tail(), scope)
    }

    pub fn compile(&self, block: &Block) -> Result<Module<'ctx>, Error> {
        let module = self.context.create_module("main");
        let main_type = self.context.i32_type().fn_type(&[], false);
        let main = module.add_function("main", main_type, None);
        let entry = self.context.append_basic_block(main, "entry");
        self.builder.position_at_end(entry);
        let mut scope = Scope::new();
        let value = self.compile_block(block, &mut scope)?;
        self.builder.build_return(Some(&value.basic_value))?;
        module
            .verify()
            .map_err(|error| Error::Verify(error.to_string()))?;
        Ok(module)
    }
}
