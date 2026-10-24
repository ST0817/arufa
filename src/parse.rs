use chumsky::{
    IterParser, Parser as ChumskyParser,
    error::Rich,
    extra::Err,
    input::{Input, ValueInput},
    pratt::{infix, left},
    primitive::{choice, just},
    recursive::Recursive,
    select,
    span::{SimpleSpan, Spanned},
    text::TextExpected,
};

use crate::{
    ast::{BinOp, Block, Expr, Stmt},
    error::Error,
    token::Token::{self, *},
};

trait InputBound<'tokens, 'src> = ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>;

trait Parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyParser<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

fn name<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, &'src str> {
    select! { Name(ident) => ident }.labelled(TextExpected::<()>::AnyIdentifier)
}

fn int_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Int(value) => Expr::Int(value) }.labelled(TextExpected::<()>::Int)
}

fn var_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    name().spanned().map(Expr::Var)
}

fn bin_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Expr<'src>> {
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

fn expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    let mut expr = Recursive::declare();
    expr.define({
        let atom = choice((int_expr(), var_expr()));
        bin_expr(atom)
    });
    expr
}

fn var_stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    just(Var)
        .ignore_then(name())
        .map(Stmt::Var)
        .then_ignore(just(Semic))
}

fn assign_stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    name()
        .spanned()
        .then_ignore(just(Eq))
        .then(expr())
        .map(|(name, value)| Stmt::Assign(name, value))
        .then_ignore(just(Semic))
}

fn stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    choice((var_stmt(), assign_stmt()))
}

fn block<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Block<'src>> {
    stmt()
        .repeated()
        .collect()
        .then(expr())
        .map(|(stmts, tail)| Block::new(stmts, tail))
}

pub fn parse<'src>(tokens: &[Spanned<Token<'src>>], src: &str) -> Result<Block<'src>, Error> {
    let expr = block()
        .parse(tokens.split_spanned((src.len()..src.len()).into()))
        .into_result()?;
    Ok(expr)
}
