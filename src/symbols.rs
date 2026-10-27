use core::slice;
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
pub struct FuncDecl<'src> {
    name: &'src str,
    id: FuncId,
    sig: FuncSig,
}

impl<'src> FuncDecl<'src> {
    pub fn new(name: &'src str, id: FuncId, sig: FuncSig) -> Self {
        Self { name, id, sig }
    }

    pub fn name(&self) -> &'src str {
        self.name
    }

    pub fn id(&self) -> FuncId {
        self.id
    }

    pub fn sig(&self) -> &FuncSig {
        &self.sig
    }
}

#[derive(Debug)]
pub struct Symbols<'src> {
    funcs: Vec<FuncDecl<'src>>,
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

    pub fn insert(&mut self, name: &'src str, sig: FuncSig) -> FuncId {
        let id = FuncId(self.funcs.len());
        let decl = FuncDecl::new(name, id, sig);
        self.funcs.push(decl);
        self.func_ids.insert(name, id);
        id
    }
}

impl<'src> Index<FuncId> for Symbols<'src> {
    type Output = FuncDecl<'src>;
    fn index(&self, index: FuncId) -> &Self::Output {
        &self.funcs[index.0]
    }
}

impl<'src> Index<&str> for Symbols<'src> {
    type Output = FuncId;
    fn index(&self, index: &str) -> &Self::Output {
        &self.func_ids[index]
    }
}

impl<'src, 'a> IntoIterator for &'a Symbols<'src> {
    type Item = &'a FuncDecl<'src>;
    type IntoIter = slice::Iter<'a, FuncDecl<'src>>;
    fn into_iter(self) -> Self::IntoIter {
        self.funcs.iter()
    }
}
