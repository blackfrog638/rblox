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

    let result = match &args[1..] {
        [] => repl(&mut vm),
        [flag] if flag == "--help" => {
            println!(
                "Usage: rblox-vm [path]\n       rblox-vm --profile path (requires --features profiling)"
            );
            return ExitCode::SUCCESS;
        }
        [path] if path != "--profile" => run_file(&mut vm, path),
        [flag, path] if flag == "--profile" => run_profiled_file(&mut vm, path),
        _ => {
            eprintln!(
                "Usage: rblox-vm [path]\n       rblox-vm --profile path (requires --features profiling)"
            );
            return ExitCode::from(EX_USAGE);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

#[cfg(feature = "profiling")]
fn run_profiled_file(vm: &mut rblox_vm::VM, path: &str) -> Result<(), u8> {
    vm.set_profiling(true);
    let result = run_file(vm, path);
    // Partial profiles are useful when a script fails, too.
    if let Some(profile) = vm.profile() {
        eprint!("{profile}");
    }
    result
}

#[cfg(not(feature = "profiling"))]
fn run_profiled_file(_vm: &mut rblox_vm::VM, _path: &str) -> Result<(), u8> {
    eprintln!(
        "Profiling requires: cargo run -p rblox-vm --bin rblox-vm --release --features profiling -- --profile path"
    );
    Err(EX_USAGE)
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

    vm.interpret(trimmed).map_err(|error| {
        eprintln!("{}", error);
        if error.starts_with("Compile error:") {
            EX_DATAERR
        } else {
            EX_SOFTWARE
        }
    })
}

#[cfg(test)]
mod tests {
    use rblox_vm::Heap;
    #[test]
    fn compile_accepts_number_literal() {
        let mut heap = Heap::new();
        let chunk = rblox_vm::compile("3.14;", &mut heap)
            .expect("number literal should compile")
            .chunk;
        assert_eq!(chunk.code.len(), 5);
    }

    #[test]
    fn compile_accepts_arithmetic_expression() {
        let mut heap = Heap::new();
        let chunk = rblox_vm::compile("1 + 2;", &mut heap)
            .expect("arithmetic expression should compile")
            .chunk;
        assert_eq!(chunk.code.len(), 8);
    }
}
