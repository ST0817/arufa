use chumsky::span::Spanned;

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug)]
pub enum Expr<'src> {
    Int(u32),
    Bin(Box<Self>, BinOp, Box<Self>),
    Var(Spanned<&'src str>),
}

#[derive(Debug)]
pub enum Stmt<'src> {
    Var(&'src str),
    Assign(Spanned<&'src str>, Expr<'src>),
}

#[derive(Debug)]
pub struct Block<'src> {
    stmts: Vec<Stmt<'src>>,
    tail: Expr<'src>,
}

impl<'src> Block<'src> {
    pub fn new(stmts: Vec<Stmt<'src>>, tail: Expr<'src>) -> Self {
        Self { stmts, tail }
    }

    pub fn stmts(&self) -> &[Stmt<'src>] {
        &self.stmts
    }

    pub fn tail(&self) -> &Expr<'src> {
        &self.tail
    }
}
