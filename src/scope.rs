use std::rc::Rc;

use inkwell::values::PointerValue;

use crate::ast::Type;

#[derive(Debug)]
pub struct Var<'src, 'ctx> {
    name: &'src str,
    ptr_value: PointerValue<'ctx>,
    ty: Type,
}

impl<'src, 'ctx> Var<'src, 'ctx> {
    pub fn new(name: &'src str, ptr_value: PointerValue<'ctx>, ty: Type) -> Self {
        Self {
            name,
            ptr_value,
            ty,
        }
    }

    pub fn ptr_value(&self) -> PointerValue<'ctx> {
        self.ptr_value
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug)]
pub struct Scope<'src, 'ctx> {
    vars: Vec<Rc<Var<'src, 'ctx>>>,
}

impl<'src, 'ctx> Scope<'src, 'ctx> {
    pub fn new() -> Self {
        Self { vars: Vec::new() }
    }

    pub fn push(&mut self, var: Var<'src, 'ctx>) {
        self.vars.push(Rc::new(var));
    }

    pub fn find(&self, name: &str) -> Option<Rc<Var<'src, 'ctx>>> {
        self.vars.iter().rev().find(|var| var.name == name).cloned()
    }
}
