mod scanner;
mod token;
mod token_type;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

use token::Token;

static HAD_ERROR: AtomicBool = AtomicBool::new(false);

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        eprintln!("Usage: lox [script]");
        process::exit(64);
    } else if args.len() == 2 {
        if let Err(err) = run_file(&args[1]) {
            eprintln!("{err}");
            process::exit(65);
        }
    } else {
        if let Err(err) = run_prompt() {
            eprintln!("{err}");
            process::exit(66);
        }
    }
}

fn run_file(path: &str) -> io::Result<()> {
    let bytes = fs::read(path)?;
    let _source = String::from_utf8_lossy(&bytes);

    run(&_source);
    if had_error() {
        process::exit(65);
    }
    Ok(())
}

fn run_prompt() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;

        let mut line = String::new();
        let bytes = stdin.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        run(&line);
        reset_had_error();
    }

    Ok(())
}

fn run(source: &str) {
    //Scanner scanner = new Scanner(source);
    //List<Token> tokens = scanner.scanTokens();

    // For now, just print the tokens.
    //for (Token token : tokens) {
    //   System.out.println(token);
    // }
}

fn error(line: usize, message: &str) {
    report(line, "", message);
}

fn report(line: usize, where_: &str, message: &str) {
    eprintln!("[line {line}] Error{where_}: {message}");
    HAD_ERROR.store(true, Ordering::Relaxed);
}

fn had_error() -> bool {
    HAD_ERROR.load(Ordering::Relaxed)
}

fn reset_had_error() {
    HAD_ERROR.store(false, Ordering::Relaxed);
}
