use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 1 {
        eprintln!("Usage: generate_AST <output_directory>");
        std::process::exit(64);
    }
    let output_dir = &args[0];
}
