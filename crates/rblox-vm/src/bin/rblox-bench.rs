use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use rblox_vm::{VM, compile};

const WORKLOADS: &[(&str, &str)] = &[
    (
        "arithmetic",
        include_str!("../../benchmarks/arithmetic.lox"),
    ),
    ("globals", include_str!("../../benchmarks/globals.lox")),
    ("zoo", include_str!("../../benchmarks/zoo.lox")),
    ("closures", include_str!("../../benchmarks/closures.lox")),
    ("strings", include_str!("../../benchmarks/strings.lox")),
    (
        "inheritance",
        include_str!("../../benchmarks/inheritance.lox"),
    ),
];
const USAGE: &str = "Usage: rblox-bench [--filter NAME] [--iterations N] [--warmup N] [--samples N]
                   [--csv PATH] [--emit PATH] [--list]
Defaults: all workloads, 100000 iterations, 2 warmups, 7 samples.
--emit requires --filter and exports a runnable Lox script without measuring.";

struct Options {
    filter: Option<String>,
    iterations: u32,
    warmup: u32,
    samples: u32,
    csv: Option<PathBuf>,
    emit: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            filter: None,
            iterations: 100_000,
            warmup: 2,
            samples: 7,
            csv: None,
            emit: None,
        }
    }
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        if !matches!(
            flag.as_str(),
            "--filter" | "--iterations" | "--warmup" | "--samples" | "--csv" | "--emit"
        ) {
            return Err(format!("Unknown option: {flag}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        match flag.as_str() {
            "--filter" => options.filter = Some(value.clone()),
            "--csv" => options.csv = Some(PathBuf::from(value)),
            "--emit" => options.emit = Some(PathBuf::from(value)),
            _ => {
                let number: u32 = value
                    .parse()
                    .map_err(|_| format!("Invalid number for {flag}: {value}"))?;
                if number == 0 && flag != "--warmup" {
                    return Err(format!("{flag} must be greater than zero"));
                }
                match flag.as_str() {
                    // Keep integer checksum arithmetic exactly representable as f64.
                    "--iterations" if number <= 10_000_000 => options.iterations = number,
                    "--iterations" => return Err("--iterations must be <= 10000000".into()),
                    "--warmup" => options.warmup = number,
                    "--samples" => options.samples = number,
                    _ => unreachable!(),
                }
            }
        }
    }
    if let Some(filter) = &options.filter
        && !WORKLOADS.iter().any(|(name, _)| name == filter)
    {
        return Err(format!("Unknown workload: {filter}; use --list"));
    }
    if options.emit.is_some() && (options.filter.is_none() || options.csv.is_some()) {
        return Err("--emit requires --filter and cannot be combined with --csv".into());
    }
    Ok(options)
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

// Each sample gets a fresh VM. Compilation happens in that VM's heap, and no
// collection is requested before interpret_chunk roots the compiled objects.
fn sample(source: &str) -> Result<(f64, f64), String> {
    let mut vm = VM::new();
    let start = Instant::now();
    let script = compile(std::hint::black_box(source), vm.heap_mut())?;
    let compile_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    vm.interpret_chunk(script.chunk)?;
    let execute_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok((compile_ms, execute_ms))
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn run(options: Options) -> Result<(), String> {
    let workloads: Vec<_> = WORKLOADS
        .iter()
        .filter(|(name, _)| options.filter.as_ref().is_none_or(|filter| name == filter))
        .collect();
    if let Some(path) = &options.emit {
        let source = format!("{}\nbench({});\n", workloads[0].1, options.iterations);
        write_file(path, &source)?;
        println!("Wrote {}", path.display());
        return Ok(());
    }
    if cfg!(any(
        feature = "profiling",
        feature = "debug-gc",
        feature = "stress-gc"
    )) {
        return Err(
            "Benchmark timings require a build without profiling/debug-gc/stress-gc features."
                .into(),
        );
    }
    if cfg!(debug_assertions) {
        return Err("Benchmark timings require --release (or --profile profiling).".into());
    }
    println!(
        "rblox-vm {} / {} / {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "{} iterations; {} warmups; {} samples per workload",
        options.iterations, options.warmup, options.samples
    );
    println!(
        "Execution includes script setup and final GC; excludes compilation, VM construction and destruction."
    );
    println!(
        "{:<14} {:>12} {:>12} {:>12} {:>12}",
        "workload", "compile ms", "exec min", "exec median", "exec max"
    );
    let mut csv = String::from("workload,iterations,sample,compile_ms,execute_ms\n");
    for (name, template) in workloads {
        let source = format!("{template}\nbench({});\n", options.iterations);
        for _ in 0..options.warmup {
            sample(&source).map_err(|error| format!("{name}: {error}"))?;
        }
        let mut compile_times = Vec::new();
        let mut execute_times = Vec::new();
        for index in 1..=options.samples {
            let (compile_ms, execute_ms) =
                sample(&source).map_err(|error| format!("{name}: {error}"))?;
            writeln!(
                csv,
                "{name},{},{index},{compile_ms:.6},{execute_ms:.6}",
                options.iterations
            )
            .unwrap();
            compile_times.push(compile_ms);
            execute_times.push(execute_ms);
        }
        let compile_median = median(&mut compile_times);
        let execute_median = median(&mut execute_times);
        println!(
            "{name:<14} {compile_median:>12.3} {:>12.3} {execute_median:>12.3} {:>12.3}",
            execute_times[0],
            execute_times.last().unwrap()
        );
    }
    if let Some(path) = options.csv {
        write_file(&path, &csv)?;
        println!("Raw samples: {}", path.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args == ["--list"] {
        for (name, _) in WORKLOADS {
            println!("{name}");
        }
        return ExitCode::SUCCESS;
    }
    match parse(&args).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_workload_checks_its_result_at_small_and_boundary_sizes() {
        for (name, source) in WORKLOADS {
            for n in [1, 32, 33] {
                sample(&format!("{source}\nbench({n});"))
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
            }
        }
    }

    #[test]
    fn median_handles_odd_and_even_samples() {
        assert_eq!(median(&mut [9.0, 1.0, 3.0]), 3.0);
        assert_eq!(median(&mut [9.0, 1.0, 5.0, 3.0]), 4.0);
    }

    #[test]
    fn rejects_empty_samples_unknown_workloads_and_ambiguous_exports() {
        for args in [
            vec!["--samples", "0"],
            vec!["--iterations", "0"],
            vec!["--iterations", "10000001"],
            vec!["--filter", "typo"],
            vec!["--emit", "out.lox"],
            vec!["--filter", "zoo", "--emit", "out.lox", "--csv", "out.csv"],
            vec!["--warmup"],
            vec!["--unknown", "1"],
        ] {
            assert!(parse(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>()).is_err());
        }
        assert_eq!(parse(&["--warmup".into(), "0".into()]).unwrap().warmup, 0);
    }
}
