use std::collections::HashMap;

use inkwell::values::FunctionValue;

use crate::ast::Type;

#[derive(Debug)]
pub struct FuncDef<'ctx> {
    func_value: FunctionValue<'ctx>,
    param_types: Vec<Type>,
    ret_type: Type,
}

impl<'ctx> FuncDef<'ctx> {
    pub fn new(func_value: FunctionValue<'ctx>, param_types: Vec<Type>, ret_type: Type) -> Self {
        Self {
            func_value,
            param_types,
            ret_type,
        }
    }

    pub fn func_value(&self) -> FunctionValue<'ctx> {
        self.func_value
    }

    pub fn param_types(&self) -> &[Type] {
        &self.param_types
    }

    pub fn ret_type(&self) -> Type {
        self.ret_type
    }
}

#[derive(Debug)]
pub struct FuncDefs<'src, 'ctx> {
    func_defs: HashMap<&'src str, FuncDef<'ctx>>,
}

impl<'src, 'ctx> FuncDefs<'src, 'ctx> {
    pub fn new() -> Self {
        Self {
            func_defs: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: &'src str, func_def: FuncDef<'ctx>) {
        self.func_defs.insert(name, func_def);
    }

    pub fn get(&self, name: &str) -> Option<&FuncDef<'ctx>> {
        self.func_defs.get(name)
    }
}
