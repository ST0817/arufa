use std::io;

use chumsky::error::Rich;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Parse error")]
    Parse(Vec<Rich<'static, char>>),
}

impl<'src> From<Vec<Rich<'src, char>>> for Error {
    fn from(value: Vec<Rich<'src, char>>) -> Self {
        Self::Parse(value.into_iter().map(|error| error.into_owned()).collect())
    }
}
