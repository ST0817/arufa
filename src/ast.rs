use std::fmt::{self, Display, Formatter};

use chumsky::span::Spanned;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    I32,
    Bool,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Type::*;
        match self {
            I32 => f.write_str("i32"),
            Bool => f.write_str("bool"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ArithBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, Copy)]
pub enum CmpBinOp {
    Eq,
    NotEq,
    LT,
    LE,
    GT,
    GE,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug)]
pub enum Expr<'src> {
    Int(u32),
    Bool(bool),
    Var(Spanned<&'src str>),
    ArithBin(Spanned<Box<Self>>, ArithBinOp, Spanned<Box<Self>>),
    CmpBin(Spanned<Box<Self>>, CmpBinOp, Spanned<Box<Self>>),
    Unary(Spanned<Box<Self>>, UnaryOp),
    If(Spanned<Box<Self>>, Block<'src>, Spanned<Block<'src>>),
}

#[derive(Debug)]
pub enum Stmt<'src> {
    Var(&'src str, Type),
    Assign(Spanned<&'src str>, Spanned<Expr<'src>>),
}

#[derive(Debug)]
pub struct Block<'src> {
    stmts: Vec<Stmt<'src>>,
    tail: Box<Expr<'src>>,
}

impl<'src> Block<'src> {
    pub fn new(stmts: Vec<Stmt<'src>>, tail: Box<Expr<'src>>) -> Self {
        Self { stmts, tail }
    }

    pub fn stmts(&self) -> &[Stmt<'src>] {
        &self.stmts
    }

    pub fn tail(&self) -> &Expr<'src> {
        &self.tail
    }
}
