use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    values::{BasicValue, BasicValueEnum},
};

use crate::{
    ast::{BinOp, Expr},
    error::Error,
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

    fn compile_int_expr(&self, value: u32) -> Result<BasicValueEnum<'ctx>, Error> {
        Ok(self
            .context
            .i32_type()
            .const_int(value as u64, false)
            .as_basic_value_enum())
    }

    fn compile_bin_expr(
        &self,
        lhs: &Expr,
        op: BinOp,
        rhs: &Expr,
    ) -> Result<BasicValueEnum<'ctx>, Error> {
        use BinOp::*;
        let lhs_value = self.compile_expr(lhs)?.into_int_value();
        let rhs_value = self.compile_expr(rhs)?.into_int_value();
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

    fn compile_expr(&self, expr: &Expr) -> Result<BasicValueEnum<'ctx>, Error> {
        use Expr::*;
        match expr {
            Int(value) => self.compile_int_expr(*value),
            Bin(lhs, op, rhs) => self.compile_bin_expr(lhs, *op, rhs),
        }
    }

    pub fn compile(&self, expr: &Expr) -> Result<Module<'ctx>, Error> {
        let module = self.context.create_module("main");
        let main_type = self.context.i32_type().fn_type(&[], false);
        let main = module.add_function("main", main_type, None);
        let entry = self.context.append_basic_block(main, "entry");
        self.builder.position_at_end(entry);
        let value = self.compile_expr(expr)?;
        self.builder.build_return(Some(&value))?;
        module
            .verify()
            .map_err(|error| Error::Verify(error.to_string()))?;
        Ok(module)
    }
}
