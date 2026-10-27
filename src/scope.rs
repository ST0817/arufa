use std::ops::Index;

use crate::ast::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(usize);

#[derive(Debug)]
pub struct Var<'src> {
    name: &'src str,
    id: VarId,
    ty: Type,
}

impl<'src> Var<'src> {
    fn new(name: &'src str, id: VarId, ty: Type) -> Self {
        Self { name, id, ty }
    }

    pub fn id(&self) -> VarId {
        self.id
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug)]
pub struct Scope<'src> {
    vars: Vec<Var<'src>>,
}

impl<'src> Scope<'src> {
    pub fn new() -> Self {
        Self { vars: Vec::new() }
    }

    pub fn push(&mut self, name: &'src str, ty: Type) -> VarId {
        let id = VarId(self.vars.len());
        let var = Var::new(name, id, ty);
        self.vars.push(var);
        id
    }

    pub fn find(&self, name: &str) -> Option<&Var<'src>> {
        self.vars.iter().rev().find(|var| var.name == name)
    }
}

impl<'src> Index<VarId> for Scope<'src> {
    type Output = Var<'src>;
    fn index(&self, index: VarId) -> &Self::Output {
        &self.vars[index.0]
    }
}
