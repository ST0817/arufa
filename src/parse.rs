use chumsky::{
    IterParser, Parser as ChumskyParser,
    error::Rich,
    extra::Err,
    input::{Input, ValueInput},
    pratt::{infix, left},
    primitive::{choice, just},
    recursive::Recursive,
    select,
    span::{SimpleSpan, SpanWrap, Spanned},
    text::TextExpected,
};

use crate::{
    ast::{BinOp, Block, Expr, Stmt, Type},
    error::Error,
    token::Token::{self, *},
};

fn box_spanned<T>(value: Spanned<T>) -> Spanned<Box<T>> {
    Box::new(value.inner).with_span(value.span)
}

trait InputBound<'tokens, 'src> = ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>;

trait Parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyParser<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

fn name<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, &'src str> {
    select! { Name(ident) => ident }.labelled(TextExpected::<()>::AnyIdentifier)
}

fn i32_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    just(I32).to(Type::I32)
}

fn bool_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    just(Bool).to(Type::Bool)
}

fn ty<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>() -> impl Parser<'tokens, 'src, I, Type>
{
    choice((i32_type(), bool_type())).labelled("type")
}

fn int_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Int(value) => Expr::Int(value) }.labelled(TextExpected::<()>::Int)
}

fn bool_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    choice((just(True).to(true), just(False).to(false))).map(Expr::Bool)
}

fn var_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    name().spanned().map(Expr::Var)
}

fn bin_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Expr<'src>> {
    let infix_left = |prec, op_token, op| {
        infix(
            left(prec),
            just(op_token).to(op).labelled("binary operator"),
            |lhs, op, rhs, extra| {
                Expr::Bin(box_spanned(lhs), op, box_spanned(rhs)).with_span(extra.span())
            },
        )
    };
    expr.spanned()
        .pratt((
            infix_left(1, Star, BinOp::Mul),
            infix_left(1, Slash, BinOp::Div),
            infix_left(1, Perc, BinOp::Mod),
            infix_left(0, Plus, BinOp::Add),
            infix_left(0, Minus, BinOp::Sub),
        ))
        .map(|spanned| spanned.inner)
}

fn expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    let mut expr = Recursive::declare();
    expr.define({
        let atom = choice((int_expr(), bool_expr(), var_expr()));
        bin_expr(atom)
    });
    expr.labelled("expression")
}

fn var_stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    just(Var)
        .ignore_then(name())
        .then_ignore(just(Colon))
        .then(ty())
        .map(|(name, ty)| Stmt::Var(name, ty))
        .then_ignore(just(Semic))
}

fn assign_stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    name()
        .spanned()
        .then_ignore(just(Eq))
        .then(expr().spanned())
        .map(|(name, expr)| Stmt::Assign(name, expr))
        .then_ignore(just(Semic))
}

fn stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    choice((var_stmt(), assign_stmt())).labelled("statement")
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
