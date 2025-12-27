mod repl;
mod shell;
mod builtins;
mod exec;
mod error;
mod completion;
mod config;
mod dirfreq;
mod formatter;
mod diagnostics;
mod parser;
mod jobs;
mod aliases;
mod shell_config;

use crate::repl::run_repl;

fn main() {
    if let Err(err) = run_repl() {
        eprintln!("squish: {}", err);
        std::process::exit(1);
    }
}
