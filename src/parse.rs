use chumsky::{
    IterParser, Parser as ChumskyParser,
    error::Rich,
    extra::Err,
    input::{Input, ValueInput},
    pratt::{Associativity, Operator as ChumskyOperator, infix, left, postfix, prefix},
    primitive::{choice, empty, just},
    recursive::Recursive,
    select,
    span::{SimpleSpan, SpanWrap, Spanned},
    text::TextExpected,
};

use crate::{
    ast::{ArithBinOp, Block, CmpBinOp, Expr, Item, LogicBinOp, Param, ShiftBinOp, Stmt, Type},
    error::Error,
    token::{
        Lit,
        Token::{self, LParen, RParen},
    },
};

fn box_spanned<T>(value: Spanned<T>) -> Spanned<Box<T>> {
    Box::new(value.inner).with_span(value.span)
}

trait InputBound<'tokens, 'src> = ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>;

trait Parser<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyParser<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

trait Operator<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T> =
    ChumskyOperator<'tokens, I, T, Err<Rich<'tokens, Token<'src>>>> + Clone;

fn parens<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T>(
    parser: impl Parser<'tokens, 'src, I, T>,
) -> impl Parser<'tokens, 'src, I, T> {
    parser.delimited_by(just(LParen), just(RParen))
}

fn parens_comma<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>, T>(
    parser: impl Parser<'tokens, 'src, I, T>,
) -> impl Parser<'tokens, 'src, I, Vec<T>> {
    use Token::*;
    parens(parser.separated_by(just(Comma)).collect())
}

fn name<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, &'src str> {
    select! { Token::Name(ident) => ident }.labelled(TextExpected::<()>::AnyIdentifier)
}

fn atom_type<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Type> {
    choice((
        just(Token::U8).to(Type::U8),
        just(Token::U16).to(Type::U16),
        just(Token::U32).to(Type::U32),
        just(Token::U64).to(Type::U64),
        just(Token::I8).to(Type::I8),
        just(Token::I16).to(Type::I16),
        just(Token::I32).to(Type::I32),
        just(Token::I64).to(Type::I64),
        just(Token::Bool).to(Type::Bool),
        just(Token::Char).to(Type::Char),
        just(Token::Str).to(Type::Str),
        parens(empty()).to(Type::Unit),
    ))
}

fn ty<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>() -> impl Parser<'tokens, 'src, I, Type>
{
    let mut ty = Recursive::declare();
    ty.define({
        let atom = atom_type();
        choice((
            atom.clone()
                .map(Box::new)
                .then_ignore(just(Token::Star))
                .map(Type::Ptr),
            atom,
        ))
    });
    ty
}

fn int_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Token::Lit(Lit::Int(value)) => Expr::Int(value) }.labelled(TextExpected::<()>::Int)
}

fn bool_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    use Token::*;
    choice((just(True).to(true), just(False).to(false))).map(Expr::Bool)
}

fn char_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Token::Lit(Lit::Char(value)) => Expr::Char(value) }
}

fn str_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    select! { Token::Lit(Lit::Str(value)) => Expr::Str(value) }
}

fn unit_expr<'tokens, 'src: 'tokens, I: InputBound<'tokens, 'src>>()
-> impl Parser<'tokens, 'src, I, Expr<'src>> {
    parens(empty()).map(|_| Expr::Unit)
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
    op_token: Token<'src>,
    map: impl Fn(Spanned<Box<Expr<'src>>>) -> Expr<'src> + Clone,
) -> impl Operator<'tokens, 'src, I, Spanned<Expr<'src>>> {
    prefix(
        10,
        just(op_token).labelled("prefix operator"),
        move |_, expr, extra| map(box_spanned(expr)).with_span(extra.span()),
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
        Expr::{ArithBin, CmpBin, LogicBin, ShiftBin},
        Token::*,
    };
    let as_op = |prec| {
        postfix(
            prec,
            just(As).ignore_then(ty()),
            |expr: Spanned<_>, ty, extra| {
                Expr::As(Box::new(expr.inner), ty, extra.span()).with_span(extra.span())
            },
        )
    };
    expr.spanned()
        .pratt((
            prefix_op(Minus, Expr::Neg),
            prefix_op(Not, Expr::Not),
            prefix_op(Amp, Expr::Ref),
            prefix_op(Star, Expr::Deref),
            as_op(9),
            infix_op(left(8), Star, ArithBinOp::Mul, ArithBin),
            infix_op(left(8), Slash, ArithBinOp::Div, ArithBin),
            infix_op(left(8), Perc, ArithBinOp::Mod, ArithBin),
            infix_op(left(7), Plus, ArithBinOp::Add, ArithBin),
            infix_op(left(7), Minus, ArithBinOp::Sub, ArithBin),
            infix_op(left(6), LShift, ShiftBinOp::LShift, ShiftBin),
            infix_op(left(6), RShift, ShiftBinOp::RShift, ShiftBin),
            infix_op(left(5), Amp, ArithBinOp::And, ArithBin),
            infix_op(left(4), Hat, ArithBinOp::Xor, ArithBin),
            infix_op(left(3), Pipe, ArithBinOp::Or, ArithBin),
            infix_op(left(2), Eq, CmpBinOp::Eq, CmpBin),
            infix_op(left(2), NotEq, CmpBinOp::NotEq, CmpBin),
            infix_op(left(2), LT, CmpBinOp::LT, CmpBin),
            infix_op(left(2), LE, CmpBinOp::LE, CmpBin),
            infix_op(left(2), GT, CmpBinOp::GT, CmpBin),
            infix_op(left(2), GE, CmpBinOp::GE, CmpBin),
            infix_op(left(1), And, LogicBinOp::And, LogicBin),
            infix_op(left(0), Or, LogicBinOp::Or, LogicBin),
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
    expr.clone()
        .spanned()
        .then_ignore(just(Assign))
        .then(expr.spanned())
        .map(|(target, value)| Stmt::Assign(target, value))
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
            char_expr(),
            str_expr(),
            unit_expr(),
            if_expr(expr.clone()),
            call_expr(expr.clone()),
            var_expr(),
            parens(expr.clone()),
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
