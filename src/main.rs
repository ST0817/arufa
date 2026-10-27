#![feature(trait_alias)]

use std::{fs::File, io::Read, path::Path, process::ExitCode};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use chumsky::span::Spanned;
use inkwell::context::Context;

use crate::{
    check::check,
    cli::Cli,
    compile::compile,
    emit::emit,
    error::{CheckError, Error},
    parse::parse,
    resolve::resolve,
    tokenize::tokenize,
};

mod ast;
mod check;
mod checked_ast;
mod cli;
mod compile;
mod emit;
mod error;
mod parse;
mod resolve;
mod scope;
mod symbols;
mod token;
mod tokenize;

fn run(cli: &Cli, src: &mut String, src_path: &Path) -> Result<(), Error> {
    File::open(cli.src_path())?.read_to_string(src)?;
    let tokens = tokenize(src)?;
    let items = parse(&tokens, src)?;
    let symbols = resolve(&items)?;
    let checked_funcs = check(&items, &symbols)?;
    let context = Context::create();
    let module = compile(&context, &symbols, src_path, &checked_funcs)?;
    emit(cli, &module)?;
    Ok(())
}

fn report_parse_errors(errors: &[Spanned<String>], id: &str, src: &str) {
    for error in errors {
        Report::build(ReportKind::Error, (id, error.span.into_range()))
            .with_config(Config::new().with_index_type(IndexType::Byte))
            .with_message(error.inner.clone())
            .with_label(
                Label::new((id, error.span.into_range()))
                    .with_message(error.inner.clone())
                    .with_color(Color::Red),
            )
            .finish()
            .print((id, Source::from(src)))
            .unwrap();
    }
}

fn report_check_error(error: &CheckError, id: &str, src: &str) {
    Report::build(ReportKind::Error, (id, error.span().into_range()))
        .with_config(Config::new().with_index_type(IndexType::Byte))
        .with_message(error.kind().to_string())
        .with_label(
            Label::new((id, error.span().into_range()))
                .with_message(error.kind().to_string())
                .with_color(Color::Red),
        )
        .finish()
        .print((id, Source::from(src)))
        .unwrap();
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut src = String::new();
    match run(&cli, &mut src, cli.src_path()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Parse(errors)) => {
            report_parse_errors(&errors, &cli.src_path().to_string_lossy(), &src);
            ExitCode::FAILURE
        }
        Err(Error::Check(error)) => {
            report_check_error(&error, &cli.src_path().to_string_lossy(), &src);
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("{error}");
            if error.is_internal() {
                eprintln!("This error is an internal error. Please report it.");
            }
            ExitCode::FAILURE
        }
    }
}
