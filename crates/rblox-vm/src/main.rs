use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

const EX_USAGE: u8 = 64;
const EX_DATAERR: u8 = 65;
const EX_SOFTWARE: u8 = 70;
const EX_IOERR: u8 = 74;

fn main() -> ExitCode {
    let mut vm = rblox_vm::VM::new();
    let args: Vec<String> = std::env::args().collect();

    let result = match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&mut vm, &args[1]),
        _ => {
            eprintln!("Usage: rblox-vm [path]");
            return ExitCode::from(EX_USAGE);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn repl(vm: &mut rblox_vm::VM) -> Result<(), u8> {
    let stdin = io::stdin();

    loop {
        print!("> ");
        io::stdout().flush().map_err(|error| {
            eprintln!("Failed to flush prompt: {}", error);
            EX_IOERR
        })?;

        let mut line = String::new();
        let read = stdin.read_line(&mut line).map_err(|error| {
            eprintln!("Failed to read input: {}", error);
            EX_IOERR
        })?;

        if read == 0 {
            return Ok(());
        }

        if let Err(code) = run_source(vm, &line) {
            if code == EX_DATAERR {
                eprintln!("Compile error.");
            } else if code == EX_SOFTWARE {
                eprintln!("Runtime error.");
            }
        }
    }
}

fn run_file(vm: &mut rblox_vm::VM, path: &str) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("Could not read file '{}': {}", path, error);
        EX_IOERR
    })?;

    run_source(vm, &source)
}

fn run_source(vm: &mut rblox_vm::VM, source: &str) -> Result<(), u8> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    // Chapter 16 wiring: source -> compile -> VM.
    // Replace this stub with the real compiler as you continue chapters 17+.
    let chunk = compile_source_stub(trimmed).map_err(|error| {
        eprintln!("{}", error);
        EX_DATAERR
    })?;

    vm.interpret(chunk).map_err(|error| {
        eprintln!("{}", error);
        EX_SOFTWARE
    })
}

fn compile_source_stub(source: &str) -> Result<rblox_vm::Chunk, String> {
    let number = source.parse::<f64>().map_err(|_| {
        format!(
            "Compile error: only number literals are supported for now (got '{}').",
            source
        )
    })?;

    let mut chunk = rblox_vm::Chunk::new();
    let index = chunk.add_constant(rblox_vm::Value::Number(number));
    chunk.write(rblox_vm::OP_CONSTANT, 1);
    chunk.write(index, 1);
    chunk.write(rblox_vm::OP_RETURN, 1);
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_source_stub_accepts_number_literal() {
        let chunk = compile_source_stub("3.14").expect("number literal should compile");
        assert_eq!(chunk.code.len(), 3);
    }

    #[test]
    fn compile_source_stub_rejects_non_number() {
        let result = compile_source_stub("1 + 2");
        assert!(result.is_err());
    }
}
