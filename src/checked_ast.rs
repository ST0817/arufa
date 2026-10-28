use crate::{
    ast::{ArithBinOp, CmpBinOp, LogicBinOp, ShiftBinOp, Type},
    scope::VarId,
    symbols::FuncId,
};

#[derive(Debug)]
pub enum CheckedLValueExprKind {
    Var(VarId),
    Deref(CheckedExpr),
    Index(CheckedLValueExpr, bool, CheckedExpr),
}

#[derive(Debug)]
pub struct CheckedLValueExpr {
    kind: Box<CheckedLValueExprKind>,
    ty: Type,
}

impl CheckedLValueExpr {
    pub fn new(kind: CheckedLValueExprKind, ty: Type) -> Self {
        Self {
            kind: Box::new(kind),
            ty,
        }
    }

    pub fn kind(&self) -> &CheckedLValueExprKind {
        &self.kind
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CastRule {
    IntToInt(bool),
    PtrToInt,
    IntToPtr,
    PtrToPtr,
}

#[derive(Debug)]
pub enum CheckedExprKind {
    Int(u64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
    LValue(CheckedLValueExpr),
    ArithBin(CheckedExpr, ArithBinOp, CheckedExpr),
    ShiftBin(CheckedExpr, ShiftBinOp, CheckedExpr),
    CmpBin(CheckedExpr, CmpBinOp, CheckedExpr),
    LogicBin(CheckedExpr, LogicBinOp, CheckedExpr),
    Neg(CheckedExpr),
    Not(CheckedExpr),
    Ref(CheckedLValueExpr),
    If(CheckedExpr, CheckedBlock, Option<CheckedBlock>),
    Call(FuncId, Vec<CheckedExpr>),
    As(CheckedExpr, Type, CastRule),
}

#[derive(Debug)]
pub struct CheckedExpr {
    kind: Box<CheckedExprKind>,
    ty: Type,
}

impl CheckedExpr {
    pub fn new(kind: CheckedExprKind, ty: Type) -> Self {
        Self {
            kind: Box::new(kind),
            ty,
        }
    }

    pub fn kind(&self) -> &CheckedExprKind {
        &self.kind
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug)]
pub enum CheckedStmt {
    Var(VarId, Type),
    Assign(CheckedLValueExpr, CheckedExpr),
    Expr(CheckedExpr),
}

#[derive(Debug)]
pub struct CheckedBlock {
    stmts: Vec<CheckedStmt>,
    tail: Option<CheckedExpr>,
}

impl CheckedBlock {
    pub fn new(stmts: Vec<CheckedStmt>, tail: Option<CheckedExpr>) -> Self {
        Self { stmts, tail }
    }

    pub fn stmts(&self) -> &[CheckedStmt] {
        &self.stmts
    }

    pub fn tail(&self) -> Option<&CheckedExpr> {
        self.tail.as_ref()
    }

    pub fn tail_type(&self) -> &Type {
        self.tail().map(|tail| tail.ty()).unwrap_or(&Type::Unit)
    }
}

#[derive(Debug)]
pub struct CheckedParam {
    id: VarId,
    ty: Type,
}

impl CheckedParam {
    pub fn new(id: VarId, ty: Type) -> Self {
        Self { id, ty }
    }

    pub fn id(&self) -> VarId {
        self.id
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug)]
pub struct CheckedFunc {
    id: FuncId,
    params: Vec<CheckedParam>,
    body: CheckedBlock,
}

impl CheckedFunc {
    pub fn new(id: FuncId, params: Vec<CheckedParam>, body: CheckedBlock) -> Self {
        Self { id, params, body }
    }

    pub fn id(&self) -> FuncId {
        self.id
    }

    pub fn params(&self) -> &[CheckedParam] {
        &self.params
    }

    pub fn body(&self) -> &CheckedBlock {
        &self.body
    }
}
