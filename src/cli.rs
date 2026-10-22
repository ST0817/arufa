use std::path::{Path, PathBuf};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, arg_required_else_help = true)]
pub struct Cli {
    #[arg(value_name = "SOURCE FILE")]
    src_path: PathBuf,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn src_path(&self) -> &Path {
        &self.src_path
    }
}
