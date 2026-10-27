use std::{collections::HashMap, ops::Index};

use crate::ast::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncId(usize);

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct FuncDef<'src> {
    name: &'src str,
    sig: FuncSig,
}

impl<'src> FuncDef<'src> {
    pub fn new(name: &'src str, sig: FuncSig) -> Self {
        Self { name, sig }
    }

    pub fn name(&self) -> &'src str {
        self.name
    }

    pub fn sig(&self) -> &FuncSig {
        &self.sig
    }
}

#[derive(Debug)]
pub struct Symbols<'src> {
    funcs: Vec<FuncDef<'src>>,
    func_ids: HashMap<&'src str, FuncId>,
}

impl<'src> Symbols<'src> {
    pub fn new() -> Self {
        Self {
            funcs: Vec::new(),
            func_ids: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<FuncId> {
        self.func_ids.get(name).copied()
    }

    pub fn insert(&mut self, name: &'src str, func_def: FuncDef<'src>) -> FuncId {
        let id = FuncId(self.funcs.len());
        self.funcs.push(func_def);
        self.func_ids.insert(name, id);
        id
    }
}

impl<'src> Index<FuncId> for Symbols<'src> {
    type Output = FuncDef<'src>;
    fn index(&self, index: FuncId) -> &Self::Output {
        &self.funcs[index.0]
    }
}
