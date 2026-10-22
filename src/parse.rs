use chumsky::{Parser, error::Rich, extra::Err, text::int};

pub fn parser<'src>() -> impl Parser<'src, &'src str, u32, Err<Rich<'src, char>>> {
    int(10).from_str().unwrapped().padded()
}
