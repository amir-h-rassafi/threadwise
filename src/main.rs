mod adapters;
mod advice;
mod app;
mod hooks;
mod monitor;
mod paths;
mod registry;
mod session_index;
mod transcripts;
mod vector;

use std::env;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let code = match app::run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    };
    std::process::exit(code);
}
