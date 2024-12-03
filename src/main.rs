#![allow(clippy::upper_case_acronyms)]
#![warn(clippy::complexity)]

mod args;
mod ast;
mod debug;
mod files;
mod report;
mod types;
mod scope;
mod passes;

use crate::args::ARGS;
use crate::ast::parser::Parser;
use crate::report::{ReportChannel, UnwrapReport};

fn main() {
    let mut report_channel = ReportChannel::new();
    if let Some(filename) = ARGS.input() {
        let sender = report_channel.get_sender();
        let mut parser = Parser::new(filename, sender).unwrap_report();
        let ast = parser.parse();
        dprintln!("{:#?}", ast);
        report_channel.check_reports();
    } else if ARGS.input().is_none() {
        // Repl::new(&reporter).start_loop()
        unimplemented!("No repl yet. Please provide a file with --input");
    }
}
