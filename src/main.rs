use clap::Parser;
use cutver::cli::{Cli, run};
use std::process;

fn main() {
    let code = run(Cli::parse());
    if code != 0 {
        process::exit(code);
    }
}
