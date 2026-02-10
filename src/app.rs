use crate::interpreter::Interpreter;
use crate::parser::Parser;
use crate::scanner::Scanner;
use std::fs;
use std::io::{self, Write};

pub struct App {
    interpreter: Interpreter,
}

impl App {
    pub fn new() -> Self {
        App {
            interpreter: Interpreter::new(),
        }
    }

    pub fn run_file(&mut self, path: &str) -> io::Result<()> {
        let bytes = fs::read(path)?;
        let source = String::from_utf8_lossy(&bytes);

        if let Err(err) = self.run_source(&source) {
            eprintln!("{err}");
            std::process::exit(65);
        }
        Ok(())
    }

    pub fn run_prompt(&mut self) -> io::Result<()> {
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
            if let Err(err) = self.run_source(&line) {
                eprintln!("{err}");
            }
        }

        Ok(())
    }

    pub fn run_source(&mut self, source: &str) -> Result<(), String> {
        let mut scanner = Scanner::new(source);
        let tokens = scanner
            .scan_tokens()
            .map_err(|err| format!("Scan error: {err}"))?;
        let mut parser = Parser::new(tokens);

        let statements = parser
            .parse()
            .map_err(|err| format!("Parse error: {err:#?}"))?;

        self.interpreter
            .interpret(&statements)
            .map_err(|err| format!("Runtime error: {:?}", err))?;
        Ok(())
    }

    pub fn interpreter_mut(&mut self) -> &mut Interpreter {
        &mut self.interpreter
    }
}
