use chumsky::{
    IterParser, Parser as ChumskyParser,
    error::Rich,
    extra::Err,
    input::{Input, ValueInput},
    pratt::{Associativity, Operator as ChumskyOperator, infix, left, prefix},
    primitive::{choice, empty, just},
    recursive::Recursive,
    select,
    span::{SimpleSpan, SpanWrap, Spanned},
    text::TextExpected,
};

use crate::{
    ast::{ArithBinOp, Block, CmpBinOp, Expr, Item, Param, Stmt, Type, UnaryOp},
    error::Error,
    token::Token::{self, LParen, RParen},
};

fn box_spanned<T>(value: Spanned<T>) -> Spanned<Box<T>> {
    Box::new(value.inner).with_span(value.span)
}

trait InputBound<'tokens, 'src> = ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>;

trait Parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyParser<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

trait Operator<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyOperator<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

fn parens_comma<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T>(
    parser: impl Parser<'tokens, 'src, I, T>,
) -> impl Parser<'tokens, 'src, I, Vec<T>> {
    use Token::*;
    parser
        .separated_by(just(Comma))
        .collect()
        .delimited_by(just(LParen), just(RParen))
}

fn name<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, &'src str> {
    select! { Token::Name(ident) => ident }.labelled(TextExpected::<()>::AnyIdentifier)
}

fn i32_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    just(Token::I32).to(Type::I32)
}

fn bool_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    just(Token::Bool).to(Type::Bool)
}

fn str_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    just(Token::Str).to(Type::Str)
}

fn unit_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    just(LParen).ignore_then(just(RParen)).to(Type::Unit)
}

fn ty<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>() -> impl Parser<'tokens, 'src, I, Type>
{
    choice((i32_type(), bool_type(), str_type(), unit_type())).labelled("type")
}

fn int_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Token::Int(value) => Expr::Int(value) }.labelled(TextExpected::<()>::Int)
}

fn bool_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    use Token::*;
    choice((just(True).to(true), just(False).to(false))).map(Expr::Bool)
}

fn str_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Token::String(value) => Expr::Str(value) }
}

fn unit_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    just(LParen).ignore_then(just(RParen)).map(|_| Expr::Unit)
}

fn if_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Expr<'src>> {
    use Token::*;
    let else_clause = just(Else).ignore_then(block(expr.clone()).spanned());
    just(If)
        .ignore_then(expr.clone().map(Box::new).spanned())
        .then(block(expr).spanned())
        .then(else_clause.or_not())
        .map(|((cond, then), els)| Expr::If(cond, then, els))
}

fn call_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Expr<'src>> {
    name()
        .spanned()
        .then(parens_comma(expr.spanned()).spanned())
        .map(|(name, args)| Expr::Call(name, args))
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
    use {
        Expr::{ArithBin, CmpBin},
        Token::*,
    };
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
    use Token::*;
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
    use Token::*;
    name()
        .spanned()
        .then_ignore(just(Assign))
        .then(expr.spanned())
        .map(|(name, expr)| Stmt::Assign(name, expr))
        .then_ignore(just(Semic))
}

fn expr_stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    expr.map(Stmt::Expr).then_ignore(just(Token::Semic))
}

fn stmt<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Stmt<'src>> {
    choice((var_stmt(), assign_stmt(expr.clone()), expr_stmt(expr))).labelled("statement")
}

fn block<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Block<'src>> {
    use Token::*;
    stmt(expr.clone())
        .repeated()
        .collect()
        .then(expr.map(Box::new).or_not())
        .delimited_by(just(LBrace), just(RBrace))
        .map(|(stmts, tail)| Block::new(stmts, tail))
}

fn param<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Param<'src>> {
    use Token::*;
    name()
        .then_ignore(just(Colon))
        .then(ty())
        .map(|(name, ty)| Param::new(name, ty))
}

fn func_item<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Item<'src>> {
    use Token::*;
    just(Fn)
        .ignore_then(name().spanned())
        .then(parens_comma(param()))
        .then(just(Arrow).ignore_then(ty()).or(empty().to(Type::Unit)))
        .then(block(expr))
        .map(|(((name, params), ret_type), body)| Item::Func(name, params, ret_type, body))
}

fn extern_item<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Item<'src>> {
    use Token::*;
    just(Extern)
        .ignore_then(just(Fn))
        .ignore_then(name().spanned())
        .then(parens_comma(ty()))
        .then(just(Arrow).ignore_then(ty()).or(empty().to(Type::Unit)))
        .then_ignore(just(Semic))
        .map(|((name, param_types), ret_type)| Item::Extern(name, param_types, ret_type))
}

fn item<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>(
    expr: impl Parser<'tokens, 'src, I, Expr<'src>>,
) -> impl Parser<'tokens, 'src, I, Item<'src>> {
    choice((func_item(expr), extern_item())).labelled("item")
}

fn parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Vec<Item<'src>>> {
    let mut expr = Recursive::declare();
    expr.define({
        let atom = choice((
            int_expr(),
            bool_expr(),
            str_expr(),
            unit_expr(),
            if_expr(expr.clone()),
            call_expr(expr.clone()),
            var_expr(),
        ));
        op_expr(atom).labelled("expression")
    });
    item(expr).repeated().collect()
}

pub fn parse<'src>(tokens: &[Spanned<Token<'src>>], src: &str) -> Result<Vec<Item<'src>>, Error> {
    let items = parser()
        .parse(tokens.split_spanned((src.len()..src.len()).into()))
        .into_result()?;
    Ok(items)
}
