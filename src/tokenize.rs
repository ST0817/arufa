use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra::Err,
    primitive::{any, choice, just, none_of},
    span::Spanned,
    text::{self, ascii::keyword},
};

use crate::{
    error::Error,
    token::Token::{self, *},
};

trait Tokenizer<'src, T> = Parser<'src, &'src str, T, Err<Rich<'src, char>>>;

fn int<'src>() -> impl Tokenizer<'src, Token<'src>> {
    text::int(10).from_str().unwrapped().map(Int)
}

fn escape<'src>() -> impl Tokenizer<'src, char> {
    just('\\').ignore_then(choice((
        just('\\').to('\\'),
        just('n').to('\n'),
        just('r').to('\n'),
        just('t').to('\n'),
        just('0').to('\n'),
        just('\'').to('\''),
        just('"').to('"'),
    )))
}

fn str<'src>() -> impl Tokenizer<'src, Token<'src>> {
    choice((escape(), none_of(r#"""#).filter(char::is_ascii)))
        .repeated()
        .to_slice()
        .delimited_by(just('"'), just('"'))
        .map(String)
}

fn name<'src>() -> impl Tokenizer<'src, Token<'src>> {
    let head = any().filter(char::is_ascii_lowercase);
    let tail = any()
        .filter(|char: &char| char.is_ascii_lowercase() || char.is_ascii_digit() || *char == '_');
    head.ignore_then(tail).to_slice().map(Name)
}

fn symbols<'src>() -> impl Tokenizer<'src, Token<'src>> {
    choice((
        just('+').to(Plus),
        just("->").to(Arrow),
        just('-').to(Minus),
        just('*').to(Star),
        just('/').to(Slash),
        just('%').to(Perc),
        just("==").to(Eq),
        just('=').to(Assign),
        just("<<").to(LShift),
        just("<=").to(LE),
        just('<').to(LT),
        just(">>").to(RShift),
        just(">=").to(GE),
        just('>').to(GT),
        just("!=").to(NotEq),
        just('!').to(Not),
        just('&').to(Amp),
        just('|').to(Pipe),
        just('^').to(Hat),
        just(':').to(Colon),
        just(';').to(Semic),
        just(',').to(Comma),
        just('(').to(LParen),
        just(')').to(RParen),
        just('{').to(LBrace),
        just('}').to(RBrace),
    ))
}

fn keywords<'src>() -> impl Tokenizer<'src, Token<'src>> {
    choice((
        keyword("i32").to(I32),
        keyword("bool").to(Bool),
        keyword("str").to(Str),
        keyword("true").to(True),
        keyword("false").to(False),
        keyword("and").to(And),
        keyword("or").to(Or),
        keyword("if").to(If),
        keyword("else").to(Else),
        keyword("var").to(Var),
        keyword("fn").to(Fn),
        keyword("extern").to(Extern),
    ))
}

fn tokenizer<'src>() -> impl Tokenizer<'src, Vec<Spanned<Token<'src>>>> {
    choice((symbols(), keywords(), int(), str(), name()))
        .spanned()
        .padded()
        .repeated()
        .collect()
}

pub fn tokenize<'src>(src: &'src str) -> Result<Vec<Spanned<Token<'src>>>, Error> {
    let tokens = tokenizer().parse(src).into_result()?;
    Ok(tokens)
}
