use std::fmt::{self, Display, Formatter};

use chumsky::span::Spanned;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    I32,
    Bool,
    Str,
    Unit,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Type::*;
        match self {
            I32 => f.write_str("i32"),
            Bool => f.write_str("bool"),
            Str => f.write_str("str"),
            Unit => f.write_str("()"),
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
    LShift,
    RShift,
    And,
    Or,
    Xor,
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
pub enum LogicBinOp {
    And,
    Or,
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
    Str(String),
    Unit,
    Var(Spanned<&'src str>),
    ArithBin(Spanned<Box<Self>>, ArithBinOp, Spanned<Box<Self>>),
    CmpBin(Spanned<Box<Self>>, CmpBinOp, Spanned<Box<Self>>),
    LogicBin(Spanned<Box<Self>>, LogicBinOp, Spanned<Box<Self>>),
    Unary(Spanned<Box<Self>>, UnaryOp),
    If(
        Spanned<Box<Self>>,
        Spanned<Block<'src>>,
        Option<Spanned<Block<'src>>>,
    ),
    Call(Spanned<&'src str>, Spanned<Vec<Spanned<Self>>>),
}

#[derive(Debug)]
pub enum Stmt<'src> {
    Var(&'src str, Type),
    Assign(Spanned<&'src str>, Spanned<Expr<'src>>),
    Expr(Expr<'src>),
}

#[derive(Debug)]
pub struct Block<'src> {
    stmts: Vec<Stmt<'src>>,
    tail: Option<Box<Expr<'src>>>,
}

impl<'src> Block<'src> {
    pub fn new(stmts: Vec<Stmt<'src>>, tail: Option<Box<Expr<'src>>>) -> Self {
        Self { stmts, tail }
    }

    pub fn stmts(&self) -> &[Stmt<'src>] {
        &self.stmts
    }

    pub fn tail(&self) -> Option<&Expr<'src>> {
        self.tail.as_deref()
    }
}

#[derive(Debug)]
pub struct Param<'src> {
    name: &'src str,
    ty: Type,
}

impl<'src> Param<'src> {
    pub fn new(name: &'src str, ty: Type) -> Self {
        Self { name, ty }
    }

    pub fn name(&self) -> &'src str {
        self.name
    }

    pub fn ty(&self) -> Type {
        self.ty
    }
}

#[derive(Debug)]
pub enum Item<'src> {
    Func(Spanned<&'src str>, Vec<Param<'src>>, Type, Block<'src>),
    Extern(Spanned<&'src str>, Vec<Type>, Type),
}
