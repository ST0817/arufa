use inkwell::{builder::Builder, context::Context, module::Module, values::BasicValue};

use crate::error::Error;

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

    pub fn compile(&self, value: u32) -> Result<Module<'ctx>, Error> {
        let module = self.context.create_module("main");
        let main_type = self.context.i32_type().fn_type(&[], false);
        let main = module.add_function("main", main_type, None);
        let entry = self.context.append_basic_block(main, "entry");
        self.builder.position_at_end(entry);
        self.builder.build_return(Some(
            &self
                .context
                .i32_type()
                .const_int(value as u64, false)
                .as_basic_value_enum()
                .into_int_value(),
        ))?;
        module
            .verify()
            .map_err(|error| Error::Verify(error.to_string()))?;
        Ok(module)
    }
}
