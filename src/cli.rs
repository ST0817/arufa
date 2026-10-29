use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Emit {
    Ir,
    Asm,
    Obj,
}

#[derive(Debug, Parser)]
#[command(version, arg_required_else_help = true)]
pub struct Cli {
    #[arg(value_name = "SOURCE FILE")]
    src_path: PathBuf,
    #[arg(value_name = "OUTPUT FILE")]
    out_path: PathBuf,
    #[arg(long)]
    emit: Emit,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn src_path(&self) -> &Path {
        &self.src_path
    }

    pub fn out_path(&self) -> &Path {
        &self.out_path
    }

    pub fn emit(&self) -> Emit {
        self.emit
    }
}
