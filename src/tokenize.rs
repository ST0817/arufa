use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra::Err,
    primitive::{any, choice, just},
    span::Spanned,
    text::int,
};

use crate::{
    error::Error,
    token::Token::{self, *},
};

fn tokenizer<'src>()
-> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, Err<Rich<'src, char>>> {
    let int = int(10).from_str().unwrapped();
    let name = any()
        .filter(char::is_ascii_alphabetic)
        .ignore_then(any().filter(char::is_ascii_alphanumeric).repeated())
        .to_slice();
    choice((
        just('+').to(Plus),
        just('-').to(Minus),
        just('*').to(Star),
        just('/').to(Slash),
        just('%').to(Perc),
        just('=').to(Eq),
        just(';').to(Semic),
        just("var").to(Var),
        int.map(Int),
        name.map(Name),
    ))
    .spanned()
    .padded()
    .repeated()
    .collect()
}

pub fn tokenize<'src>(src: &'src str) -> Result<Vec<Spanned<Token<'src>>>, Error> {
    let tokens = tokenizer().parse(src).into_result()?;
    Ok(tokens)
}
