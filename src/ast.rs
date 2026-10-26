use std::fmt::{self, Display, Formatter};

use chumsky::span::{SimpleSpan, Spanned};

#[derive(Debug, Clone, Copy)]
pub enum TypeClass {
    Int,
    Bool,
    Ptr,
    Unit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Bool,
    Str,
    Unit,
    Ptr(Box<Self>),
}

impl Type {
    pub fn is_signed(&self) -> bool {
        use Type::*;
        matches!(self, I8 | I16 | I32 | I64)
    }

    pub fn class(&self) -> TypeClass {
        use Type::*;
        match self {
            U8 | U16 | U32 | U64 | I8 | I16 | I32 | I64 => TypeClass::Int,
            Bool => TypeClass::Bool,
            Str | Ptr(_) => TypeClass::Ptr,
            Unit => TypeClass::Unit,
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Type::*;
        match self {
            U8 => f.write_str("u8"),
            U16 => f.write_str("u16"),
            U32 => f.write_str("u32"),
            U64 => f.write_str("u64"),
            I8 => f.write_str("i8"),
            I16 => f.write_str("i16"),
            I32 => f.write_str("i32"),
            I64 => f.write_str("i64"),
            Bool => f.write_str("bool"),
            Str => f.write_str("str"),
            Unit => f.write_str("()"),
            Ptr(ty) => write!(f, "{ty}*"),
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
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone, Copy)]
pub enum ShiftBinOp {
    LShift,
    RShift,
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

#[derive(Debug)]
pub enum Expr<'src> {
    Int(u64),
    Bool(bool),
    Str(String),
    Unit,
    Var(Spanned<&'src str>),
    ArithBin(Spanned<Box<Self>>, ArithBinOp, Spanned<Box<Self>>),
    ShiftBin(Spanned<Box<Self>>, ShiftBinOp, Spanned<Box<Self>>),
    CmpBin(Spanned<Box<Self>>, CmpBinOp, Spanned<Box<Self>>),
    LogicBin(Spanned<Box<Self>>, LogicBinOp, Spanned<Box<Self>>),
    Neg(Spanned<Box<Self>>),
    Not(Spanned<Box<Self>>),
    Ref(Spanned<Box<Self>>),
    Deref(Spanned<Box<Self>>),
    If(
        Spanned<Box<Self>>,
        Spanned<Block<'src>>,
        Option<Spanned<Block<'src>>>,
    ),
    Call(Spanned<&'src str>, Spanned<Vec<Spanned<Self>>>),
    As(Box<Self>, Type, SimpleSpan),
}

#[derive(Debug)]
pub enum Stmt<'src> {
    Var(&'src str, Type),
    Assign(Spanned<Expr<'src>>, Spanned<Expr<'src>>),
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

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug)]
pub enum Item<'src> {
    Func(Spanned<&'src str>, Vec<Param<'src>>, Type, Block<'src>),
    Extern(Spanned<&'src str>, Vec<Type>, Type),
}
