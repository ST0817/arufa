use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra::Err,
    primitive::{any, choice, just},
    span::Spanned,
    text::{ascii::keyword, int},
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
        just('=').to(Assign),
        just("==").to(Eq),
        just('<').to(LT),
        just("<=").to(LE),
        just('>').to(GT),
        just(">=").to(GE),
        just('!').to(Not),
        just("!=").to(NotEq),
        just(':').to(Colon),
        just(';').to(Semic),
        keyword("i32").to(I32),
        keyword("bool").to(Bool),
        keyword("true").to(True),
        keyword("false").to(False),
        keyword("var").to(Var),
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
