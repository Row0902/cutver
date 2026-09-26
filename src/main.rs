use clap::Parser;
use cutver::cli::{Cli, run};
use std::process;

fn main() {
    let code = run(Cli::parse_from(cutver::cli::args::normalize_args(std::env::args())));
    if code != 0 {
        process::exit(code);
    }
}
