use std::env;
use std::process;

use rblox::app::App;

fn main() {
    let mut app = App::new();
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        eprintln!("Usage: lox [script]");
        process::exit(64);
    } else if args.len() == 2 {
        if let Err(err) = app.run_file(&args[1]) {
            eprintln!("{err}");
            process::exit(65);
        }
    } else {
        if let Err(err) = app.run_prompt() {
            eprintln!("{err}");
            process::exit(66);
        }
    }
}
