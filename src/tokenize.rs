use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra::Err,
    primitive::{choice, just},
    span::Spanned,
    text::int,
};

use crate::{
    error::Error,
    token::Token::{self, *},
};

fn tokenizer<'src>() -> impl Parser<'src, &'src str, Vec<Spanned<Token>>, Err<Rich<'src, char>>> {
    choice((
        just('+').to(Plus),
        just('-').to(Minus),
        just('*').to(Star),
        just('/').to(Slash),
        just('%').to(Perc),
        int(10).from_str().unwrapped().map(Int),
    ))
    .spanned()
    .padded()
    .repeated()
    .collect()
}

pub fn tokenize(src: &str) -> Result<Vec<Spanned<Token>>, Error> {
    let tokens = tokenizer().parse(src).into_result()?;
    Ok(tokens)
}
