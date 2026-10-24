use std::{
    fmt::{self, Display, Formatter},
    io,
};

use chumsky::{
    error::Rich,
    span::{SimpleSpan, SpanWrap, Spanned},
};
use inkwell::builder::BuilderError;
use thiserror::Error;

use crate::{ast::Type, token::Token};

#[derive(Debug, Clone, Copy)]
pub enum CheckErrorKind {
    UndefinedVariable,
    TypeMismatch { expected: Type, found: Type },
    UndefinedFunction,
    ArgNumMismatch,
    AlreadyDefined,
    NoMain,
}

impl Display for CheckErrorKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use CheckErrorKind::*;
        match self {
            UndefinedVariable => f.write_str("undefined variable"),
            TypeMismatch { expected, found } => {
                write!(f, "type mismatch: expected {expected}, found {found}")
            }
            UndefinedFunction => f.write_str("undefined function"),
            ArgNumMismatch => f.write_str("the number of arguments does not match"),
            AlreadyDefined => f.write_str("already defined"),
            NoMain => f.write_str("no main function"),
        }
    }
}

#[derive(Debug)]
pub struct CheckError {
    kind: CheckErrorKind,
    span: SimpleSpan,
}

impl CheckError {
    pub fn new(kind: CheckErrorKind, span: SimpleSpan) -> Self {
        Self { kind, span }
    }

    pub fn kind(&self) -> CheckErrorKind {
        self.kind
    }

    pub fn span(&self) -> &SimpleSpan {
        &self.span
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
    #[error("Parse error")]
    Parse(Vec<Spanned<String>>),
    #[error("Check error")]
    Check(CheckError),
    #[error("IR builder error: {0}")]
    Builder(#[from] BuilderError),
    #[error("IR verification error: {0}")]
    Verify(String),
    #[error("target `{triple}` is not available: {message}")]
    Target { triple: String, message: String },
}

impl From<CheckError> for Error {
    fn from(value: CheckError) -> Self {
        Self::Check(value)
    }
}

impl<'src> From<Vec<Rich<'src, char>>> for Error {
    fn from(value: Vec<Rich<'src, char>>) -> Self {
        Self::Parse(
            value
                .into_iter()
                .map(|error| error.reason().to_string().with_span(*error.span()))
                .collect(),
        )
    }
}

impl<'tokens, 'src> From<Vec<Rich<'tokens, Token<'src>>>> for Error {
    fn from(value: Vec<Rich<'tokens, Token<'src>>>) -> Self {
        Self::Parse(
            value
                .into_iter()
                .map(|error| error.reason().to_string().with_span(*error.span()))
                .collect(),
        )
    }
}

impl Error {
    pub fn is_internal(&self) -> bool {
        use Error::*;
        matches!(self, Builder(_) | Verify(_))
    }
}
