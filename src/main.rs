use clap::Parser;
use cutver::cli::{run, Cli};
use std::process;

fn main() {
    let code = run(Cli::parse());
    if code != 0 {
        process::exit(code);
    }
}
