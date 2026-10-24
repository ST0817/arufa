use std::fmt::{self, Display, Formatter, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'src> {
    Plus,
    Minus,
    Star,
    Slash,
    Perc,
    Eq,
    Colon,
    Semic,
    I32,
    Bool,
    True,
    False,
    Var,
    Int(u32),
    Name(&'src str),
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Token::*;
        match self {
            Plus => f.write_char('+'),
            Minus => f.write_char('-'),
            Star => f.write_char('*'),
            Slash => f.write_char('/'),
            Perc => f.write_char('%'),
            Eq => f.write_char('='),
            Colon => f.write_char(':'),
            Semic => f.write_char(';'),
            I32 => f.write_str("i32"),
            Bool => f.write_str("bool"),
            True => f.write_str("true"),
            False => f.write_str("false"),
            Var => f.write_str("var"),
            Int(value) => write!(f, "{value}"),
            Name(name) => f.write_str(name),
        }
    }
}
