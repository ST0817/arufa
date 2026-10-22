use std::io;

use chumsky::error::Rich;
use inkwell::builder::BuilderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
    #[error("Parse error")]
    Parse(Vec<Rich<'static, char>>),
    #[error("IR builder error: {0}")]
    Builder(#[from] BuilderError),
    #[error("IR verification error: {0}")]
    Verify(String),
    #[error("target `{triple}` is not available: {message}")]
    Target { triple: String, message: String },
}

impl<'src> From<Vec<Rich<'src, char>>> for Error {
    fn from(value: Vec<Rich<'src, char>>) -> Self {
        Self::Parse(value.into_iter().map(|error| error.into_owned()).collect())
    }
}

impl Error {
    pub fn is_internal(&self) -> bool {
        use Error::*;
        matches!(self, Builder(_) | Verify(_))
    }
}
