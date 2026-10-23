use chumsky::{
    Parser as ChumskyParser,
    error::Rich,
    extra::Err,
    input::{Input, ValueInput},
    pratt::{infix, left},
    primitive::just,
    recursive::Recursive,
    select,
    span::{SimpleSpan, Spanned},
    text::TextExpected,
};

use crate::{
    ast::{BinOp, Expr},
    error::Error,
    token::Token::{self, *},
};

trait InputBound<'tokens> = ValueInput<'tokens, Token = Token, Span = SimpleSpan>;

trait Parser<'tokens, I: InputBound<'tokens>, T> =
    ChumskyParser<'tokens, I, T, Err<Rich<'tokens, Token>>> + Clone;

fn int<'tokens, I: InputBound<'tokens>>() -> impl Parser<'tokens, I, Expr> {
    select! { Int(value) => Expr::Int(value) }.labelled(TextExpected::<()>::Int)
}

fn bin_expr<'tokens, I: InputBound<'tokens>>(
    expr: impl Parser<'tokens, I, Expr>,
) -> impl Parser<'tokens, I, Expr> {
    expr.pratt((
        infix(left(1), just(Star).to(BinOp::Mul), |lhs, op, rhs, _| {
            Expr::Bin(Box::new(lhs), op, Box::new(rhs))
        }),
        infix(left(1), just(Slash).to(BinOp::Div), |lhs, op, rhs, _| {
            Expr::Bin(Box::new(lhs), op, Box::new(rhs))
        }),
        infix(left(1), just(Perc).to(BinOp::Mod), |lhs, op, rhs, _| {
            Expr::Bin(Box::new(lhs), op, Box::new(rhs))
        }),
        infix(left(0), just(Plus).to(BinOp::Add), |lhs, op, rhs, _| {
            Expr::Bin(Box::new(lhs), op, Box::new(rhs))
        }),
        infix(left(0), just(Minus).to(BinOp::Sub), |lhs, op, rhs, _| {
            Expr::Bin(Box::new(lhs), op, Box::new(rhs))
        }),
    ))
}

fn expr<'token, I: InputBound<'token>>() -> impl Parser<'token, I, Expr> {
    let mut expr = Recursive::declare();
    expr.define({
        let atom = int();
        bin_expr(atom)
    });
    expr
}

pub fn parse(tokens: &[Spanned<Token>], src: &str) -> Result<Expr, Error> {
    let expr = expr()
        .parse(tokens.split_spanned((src.len()..src.len()).into()))
        .into_result()?;
    Ok(expr)
}
