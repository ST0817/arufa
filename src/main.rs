use std::fs::read_to_string;

use crate::cli::Cli;

mod cli;

fn main() {
    let cli = Cli::parse();
    let src = read_to_string(cli.src_path()).expect("Failed to read file");
    println!("{src}");
}
