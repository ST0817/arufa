use std::{fs::File, io::Read, process::ExitCode};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use chumsky::{Parser, error::Rich};
use inkwell::context::Context;

use crate::{cli::Cli, compile::Compiler, emit::emit, error::Error, parse::parser};

mod cli;
mod compile;
mod emit;
mod error;
mod parse;

fn run(cli: &Cli, src: &mut String) -> Result<(), Error> {
    File::open(cli.src_path())?.read_to_string(src)?;
    let value = parser().parse(src).into_result()?;
    let context = Context::create();
    let compiler = Compiler::new(&context);
    let module = compiler.compile(value)?;
    emit(cli, &module)?;
    Ok(())
}

fn report_parse_errors(errors: &[Rich<char>], src: &str) {
    for error in errors {
        Report::build(ReportKind::Error, error.span().into_range())
            .with_config(Config::new().with_index_type(IndexType::Byte))
            .with_message(error.to_string())
            .with_label(
                Label::new(error.span().into_range())
                    .with_message(error.reason().to_string())
                    .with_color(Color::Red),
            )
            .finish()
            .print(Source::from(src))
            .unwrap();
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut src = String::new();
    match run(&cli, &mut src) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Parse(errors)) => {
            report_parse_errors(&errors, &src);
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
