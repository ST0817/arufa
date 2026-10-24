use chumsky::span::Spanned;
use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    values::{BasicValue, BasicValueEnum},
};

use crate::{
    ast::{BinOp, Block, Expr, Stmt},
    error::{CheckError, CheckErrorKind::UndefinedVariable, Error},
    scope::{Scope, Var},
};

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

    fn find_var<'src, 'scope>(
        &self,
        name: &Spanned<&'src str>,
        scope: &'scope mut Scope<'src, 'ctx>,
    ) -> Result<&'scope Var<'src, 'ctx>, Error> {
        scope
            .find(name)
            .ok_or_else(|| CheckError::new(UndefinedVariable, name.span).into())
    }

    fn compile_int_expr(&self, value: u32) -> Result<BasicValueEnum<'ctx>, Error> {
        Ok(self
            .context
            .i32_type()
            .const_int(value as u64, false)
            .as_basic_value_enum())
    }

    fn compile_bin_expr<'src>(
        &self,
        lhs: &Expr<'src>,
        op: BinOp,
        rhs: &Expr<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, Error> {
        use BinOp::*;
        let lhs_value = self.compile_expr(lhs, scope)?.into_int_value();
        let rhs_value = self.compile_expr(rhs, scope)?.into_int_value();
        let value = match op {
            Add => self.builder.build_int_add(lhs_value, rhs_value, "add")?,
            Sub => self.builder.build_int_sub(lhs_value, rhs_value, "sub")?,
            Mul => self.builder.build_int_mul(lhs_value, rhs_value, "mul")?,
            Div => self
                .builder
                .build_int_signed_div(lhs_value, rhs_value, "sdiv")?,
            Mod => self
                .builder
                .build_int_signed_rem(lhs_value, rhs_value, "srem")?,
        };
        Ok(value.as_basic_value_enum())
    }

    fn compile_var_expr<'src>(
        &self,
        name: &Spanned<&'src str>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, Error> {
        let var = self.find_var(name, scope)?;
        let value = self
            .builder
            .build_load(self.context.i32_type(), var.value(), "deref")?;
        Ok(value)
    }

    fn compile_expr<'src>(
        &self,
        expr: &Expr<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, Error> {
        use Expr::*;
        match expr {
            Int(value) => self.compile_int_expr(*value),
            Bin(lhs, op, rhs) => self.compile_bin_expr(lhs, *op, rhs, scope),
            Var(name) => self.compile_var_expr(name, scope),
        }
    }

    fn compile_var_stmt<'src>(
        &self,
        name: &'src str,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let value = self.builder.build_alloca(self.context.i32_type(), name)?;
        scope.push(Var::new(name, value));
        Ok(())
    }

    fn compile_assign_stmt<'src>(
        &self,
        name: &Spanned<&'src str>,
        value: &Expr<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        let var = self.find_var(name, scope)?.value();
        let value = self.compile_expr(value, scope)?;
        self.builder.build_store(var, value)?;
        Ok(())
    }

    fn compile_stmt<'src>(
        &self,
        stmt: &Stmt<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<(), Error> {
        use Stmt::*;
        match stmt {
            Var(name) => self.compile_var_stmt(name, scope),
            Assign(name, value) => self.compile_assign_stmt(name, value, scope),
        }
    }

    fn compile_block<'src>(
        &self,
        block: &Block<'src>,
        scope: &mut Scope<'src, 'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, Error> {
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
        self.builder.build_return(Some(&value))?;
        module
            .verify()
            .map_err(|error| Error::Verify(error.to_string()))?;
        Ok(module)
    }
}
