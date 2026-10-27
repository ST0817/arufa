use chumsky::span::Spanned;

use crate::{
    ast::Item,
    error::{CheckError, CheckErrorKind::AlreadyDefined},
    symbols::{FuncSig, Symbols},
};

type Result<T> = std::result::Result<T, CheckError>;

#[derive(Debug)]
pub struct Resolver<'src> {
    symbols: Symbols<'src>,
}

impl<'src> Resolver<'src> {
    fn new() -> Self {
        Self {
            symbols: Symbols::new(),
        }
    }

    fn check_func_defined(&self, name: &Spanned<&str>) -> Result<()> {
        if self.symbols.get(name).is_some() {
            return Err(CheckError::new(AlreadyDefined, name.span));
        }
        Ok(())
    }

    fn resolve_item(&mut self, item: &Item<'src>) -> Result<()> {
        use Item::*;
        match item {
            Func(name, params, ret_type, _) => {
                self.check_func_defined(name)?;
                let param_types = params.iter().map(|param| param.ty().clone()).collect();
                let sig = FuncSig::new(param_types, ret_type.clone());
                self.symbols.insert(name, sig);
            }
            Extern(name, param_types, ret_type) => {
                self.check_func_defined(name)?;
                let sig = FuncSig::new(param_types.clone(), ret_type.clone());
                self.symbols.insert(name, sig);
            }
        }
        Ok(())
    }
}

pub fn resolve<'src>(items: &[Item<'src>]) -> Result<Symbols<'src>> {
    let mut resolver = Resolver::new();
    for item in items {
        resolver.resolve_item(item)?;
    }
    Ok(resolver.symbols)
}
