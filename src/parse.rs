use chumsky::{
    IterParser, Parser as ChumskyParser,
    error::Rich,
    extra::Err,
    input::{Input, ValueInput},
    pratt::{Associativity, Operator as ChumskyOperator, infix, left, prefix},
    primitive::{choice, just},
    recursive::Recursive,
    select,
    span::{SimpleSpan, SpanWrap, Spanned},
    text::TextExpected,
};

use crate::{
    ast::{ArithBinOp, Block, CmpBinOp, Expr, Stmt, Type, UnaryOp},
    error::Error,
    token::Token::{self, *},
};

fn box_spanned<T>(value: Spanned<T>) -> Spanned<Box<T>> {
    Box::new(value.inner).with_span(value.span)
}

trait InputBound<'tokens, 'src> = ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>;

trait Parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyParser<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

trait Operator<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyOperator<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

fn braces<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T>(
    parser: impl Parser<'tokens, 'src, I, T>,
) -> impl Parser<'tokens, 'src, I, T> {
    parser.delimited_by(just(LBrace), just(RBrace))
}

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

fn if_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Expr<'src>> {
    just(If)
        .ignore_then(expr.clone().map(Box::new).spanned())
        .then(braces(block(expr.clone())))
        .then_ignore(just(Else))
        .then(braces(block(expr).spanned()))
        .map(|((cond, then), els)| Expr::If(cond, then, els))
}

fn var_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    name().spanned().map(Expr::Var)
}

fn prefix_op<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    prec: u16,
    op_token: Token<'src>,
    op: UnaryOp,
) -> impl Operator<'tokens, 'src, I, Spanned<Expr<'src>>> {
    prefix(
        prec,
        just(op_token).to(op).labelled("prefix operator"),
        |op, expr, extra| Expr::Unary(box_spanned(expr), op).with_span(extra.span()),
    )
}

fn infix_op<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, Op: Clone + 'tokens>(
    assoc: Associativity,
    op_token: Token<'src>,
    op: Op,
    map: impl Fn(Spanned<Box<Expr<'src>>>, Op, Spanned<Box<Expr<'src>>>) -> Expr<'src> + Clone,
) -> impl Operator<'tokens, 'src, I, Spanned<Expr<'src>>> {
    infix(
        assoc,
        just(op_token).to(op).labelled("binary operator"),
        move |lhs, op, rhs, extra| {
            map(box_spanned(lhs), op, box_spanned(rhs)).with_span(extra.span())
        },
    )
}

fn op_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Expr<'src>> {
    use Expr::{ArithBin, CmpBin};
    expr.spanned()
        .pratt((
            prefix_op(3, Minus, UnaryOp::Neg),
            prefix_op(3, Not, UnaryOp::Not),
            infix_op(left(2), Star, ArithBinOp::Mul, ArithBin),
            infix_op(left(2), Slash, ArithBinOp::Div, ArithBin),
            infix_op(left(2), Perc, ArithBinOp::Mod, ArithBin),
            infix_op(left(1), Plus, ArithBinOp::Add, ArithBin),
            infix_op(left(1), Minus, ArithBinOp::Sub, ArithBin),
            infix_op(left(0), Eq, CmpBinOp::Eq, CmpBin),
            infix_op(left(0), NotEq, CmpBinOp::NotEq, CmpBin),
            infix_op(left(0), LT, CmpBinOp::LT, CmpBin),
            infix_op(left(0), LE, CmpBinOp::LE, CmpBin),
            infix_op(left(0), GT, CmpBinOp::GT, CmpBin),
            infix_op(left(0), GE, CmpBinOp::GE, CmpBin),
        ))
        .map(|spanned| spanned.inner)
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

fn assign_stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    name()
        .spanned()
        .then_ignore(just(Assign))
        .then(expr.spanned())
        .map(|(name, expr)| Stmt::Assign(name, expr))
        .then_ignore(just(Semic))
}

fn stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    choice((var_stmt(), assign_stmt(expr))).labelled("statement")
}

fn block<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Block<'src>> {
    stmt(expr.clone())
        .repeated()
        .collect()
        .then(expr.map(Box::new))
        .map(|(stmts, tail)| Block::new(stmts, tail))
}

fn parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Block<'src>> {
    let mut expr = Recursive::declare();
    expr.define({
        let atom = choice((int_expr(), bool_expr(), if_expr(expr.clone()), var_expr()));
        op_expr(atom).labelled("expression")
    });
    block(expr)
}

pub fn parse<'src>(tokens: &[Spanned<Token<'src>>], src: &str) -> Result<Block<'src>, Error> {
    let block = parser()
        .parse(tokens.split_spanned((src.len()..src.len()).into()))
        .into_result()?;
    Ok(block)
}
