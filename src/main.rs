use std::{fs::File, io::Read, process::ExitCode};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use chumsky::{Parser, error::Rich};

use crate::{cli::Cli, error::Error, parse::parser};

mod cli;
mod error;
mod parse;

fn run(cli: &Cli, src: &mut String) -> Result<(), Error> {
    File::open(cli.src_path())?.read_to_string(src)?;
    let value = parser().parse(src).into_result()?;
    println!("parsed: {value}");
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
        Err(error) => {
            use Error::*;
            match error {
                IO(error) => eprintln!("{error}"),
                Parse(errors) => report_parse_errors(&errors, &src),
            }
            ExitCode::FAILURE
        }
    }
}
