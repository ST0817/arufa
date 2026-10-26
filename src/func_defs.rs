use std::collections::HashMap;

use inkwell::values::FunctionValue;

use crate::ast::Type;

#[derive(Debug, PartialEq)]
pub struct FuncSig {
    params: Vec<Type>,
    ret: Type,
}

impl FuncSig {
    pub fn new(params: Vec<Type>, ret: Type) -> Self {
        Self { params, ret }
    }

    pub fn params(&self) -> &[Type] {
        &self.params
    }

    pub fn ret(&self) -> &Type {
        &self.ret
    }
}

#[derive(Debug)]
pub struct FuncDef<'ctx> {
    value: FunctionValue<'ctx>,
    sig: FuncSig,
}

impl<'ctx> FuncDef<'ctx> {
    pub fn new(value: FunctionValue<'ctx>, sig: FuncSig) -> Self {
        Self { value, sig }
    }

    pub fn value(&self) -> FunctionValue<'ctx> {
        self.value
    }

    pub fn sig(&self) -> &FuncSig {
        &self.sig
    }
}

#[derive(Debug)]
pub struct FuncDefs<'src, 'ctx> {
    defs: HashMap<&'src str, FuncDef<'ctx>>,
}

impl<'src, 'ctx> FuncDefs<'src, 'ctx> {
    pub fn new() -> Self {
        Self {
            defs: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: &'src str, def: FuncDef<'ctx>) {
        self.defs.insert(name, def);
    }

    pub fn get(&self, name: &str) -> Option<&FuncDef<'ctx>> {
        self.defs.get(name)
    }
}
