mod cursor;
mod environment;
mod expr;
mod interpreter;
mod parser;
mod scanner;
mod stmt;
mod token;
mod token_type;
mod value;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

struct App {
    interpreter: interpreter::Interpreter,
}

impl App {
    fn new() -> Self {
        App {
            interpreter: interpreter::Interpreter::new(),
        }
    }

    fn run_file(&mut self, path: &str) -> io::Result<()> {
        let bytes = fs::read(path)?;
        let source = String::from_utf8_lossy(&bytes);

        if let Err(err) = self.run(&source) {
            eprintln!("{err}");
            process::exit(65);
        }
        Ok(())
    }

    fn run_prompt(&mut self) -> io::Result<()> {
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
            if let Err(err) = self.run(&line) {
                eprintln!("{err}");
            }
        }

        Ok(())
    }

    fn run(&mut self, source: &str) -> Result<(), String> {
        let mut scanner = scanner::Scanner::new(source);
        let tokens = scanner
            .scan_tokens()
            .map_err(|err| format!("Scan error: {err}"))?;
        let mut parser = parser::Parser::new(tokens);

        let statements = parser
            .parse()
            .map_err(|err| format!("Parse error: {err:#?}"))?;

        self.interpreter
            .interpret(&statements)
            .map_err(|err| format!("Runtime error: {:?}", err))?;
        Ok(())
    }
}

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
