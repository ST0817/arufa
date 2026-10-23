use std::fmt::{self, Display, Formatter, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Plus,
    Minus,
    Star,
    Slash,
    Perc,
    Int(u32),
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Token::*;
        match self {
            Plus => f.write_char('+'),
            Minus => f.write_char('-'),
            Star => f.write_char('*'),
            Slash => f.write_char('/'),
            Perc => f.write_char('%'),
            Int(value) => write!(f, "{value}"),
        }
    }
}
