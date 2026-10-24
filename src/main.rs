#![feature(trait_alias)]

use std::{fs::File, io::Read, process::ExitCode};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use chumsky::span::Spanned;
use inkwell::context::Context;

use crate::{
    cli::Cli,
    compile::Compiler,
    emit::emit,
    error::{CheckError, Error},
    parse::parse,
    tokenize::tokenize,
};

mod ast;
mod cli;
mod compile;
mod emit;
mod error;
mod parse;
mod scope;
mod token;
mod tokenize;

fn run(cli: &Cli, src: &mut String) -> Result<(), Error> {
    File::open(cli.src_path())?.read_to_string(src)?;
    let tokens = tokenize(src)?;
    let block = parse(&tokens, src)?;
    let context = Context::create();
    let compiler = Compiler::new(&context);
    let module = compiler.compile(&block)?;
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
    match run(&cli, &mut src) {
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
