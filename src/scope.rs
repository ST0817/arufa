use inkwell::values::PointerValue;

#[derive(Debug)]
pub struct Var<'src, 'ctx> {
    name: &'src str,
    value: PointerValue<'ctx>,
}

impl<'src, 'ctx> Var<'src, 'ctx> {
    pub fn new(name: &'src str, value: PointerValue<'ctx>) -> Self {
        Self { name, value }
    }

    pub fn value(&self) -> PointerValue<'ctx> {
        self.value
    }
}

#[derive(Debug)]
pub struct Scope<'src, 'ctx> {
    vars: Vec<Var<'src, 'ctx>>,
}

impl<'src, 'ctx> Scope<'src, 'ctx> {
    pub fn new() -> Self {
        Self { vars: Vec::new() }
    }

    pub fn push(&mut self, var: Var<'src, 'ctx>) {
        self.vars.push(var);
    }

    pub fn find(&self, name: &str) -> Option<&Var<'src, 'ctx>> {
        self.vars.iter().rev().find(|var| var.name == name)
    }
}
