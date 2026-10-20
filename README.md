# rblox

Yet another Rust implementation of the Lox interpreter.

## Features

- Lexer, parser, and AST
- Basic expression evaluation
- REPL / script entry point

## Run

```bash
cargo run
```

To run a script file:

```bash
cargo run -- path/to/script.lox
```

## Development

- Rust 2021
- Entry point: `src/main.rs`

## Bytecode VM performance

The bytecode VM lives in `crates/rblox-vm` (Rust 2024).
See [benchmark and profiling instructions](docs/performance.md) for repeatable
measurements, Lox function/bytecode statistics, and native Rust CPU sampling.

```powershell
cargo run -p rblox-vm --bin rblox-bench --release -- --csv target/benchmarks/baseline.csv
```

## License

MIT

