use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_PROGRAM_ID: AtomicUsize = AtomicUsize::new(0);

struct TempProgram(std::path::PathBuf);

impl TempProgram {
    fn new(source: &str) -> Self {
        let id = NEXT_PROGRAM_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("rblox-program-{}-{id}.lox", std::process::id()));
        std::fs::write(&path, source).expect("write temporary Lox program");
        Self(path)
    }
}

impl Drop for TempProgram {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn run_program(source: &str) -> Output {
    let program = TempProgram::new(source);
    Command::new(env!("CARGO_BIN_EXE_rblox-vm"))
        .arg(&program.0)
        .output()
        .expect("run rblox-vm")
}

#[test]
fn native_function_can_be_stored_called_and_return_a_value_to_lox() {
    let output = run_program(
        r#"
        var timer = clock;
        fun readTime() {
            return timer();
        }

        print timer == clock;
        print readTime() >= 0;
        print timer;
        "#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "true\ntrue\n<native function>\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn native_function_reports_arity_errors_as_runtime_errors() {
    let output = run_program("clock(1);");

    assert_eq!(output.status.code(), Some(70));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Expected 0 arguments but got 1."));
    assert!(stderr.contains("[line 1] in script"));
}

#[test]
fn calling_a_non_function_still_reports_a_runtime_error() {
    let output = run_program("var value = \"text\"; value();");

    assert_eq!(output.status.code(), Some(70));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Can only call functions."));
}

#[test]
fn user_function_binds_arguments_in_order_and_preserves_nested_call_results() {
    let output = run_program(
        r#"
        var order = 0;
        fun mark(digit) {
            order = order * 10 + digit;
            return digit;
        }
        fun encode(a, b, c) {
            return order * 1000 + a * 100 + b * 10 + c;
        }
        print encode(mark(1), mark(2), mark(3));
        "#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "123123\n");
}

#[test]
fn recursive_functions_and_early_returns_work_in_full_programs() {
    let output = run_program(
        r#"
        fun noReturnValue() {}
        fun factorial(n) {
            if (n <= 1) return 1;
            return n * factorial(n - 1);
        }
        fun maybe(flag) {
            if (flag) return factorial(6);
            return;
            print "unreachable";
        }
        print noReturnValue() == nil;
        print maybe(true);
        print maybe(false);
        "#,
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "true\n720\nnil\n"
    );
}

#[test]
fn local_function_declarations_are_callable_within_their_scope() {
    let output = run_program(
        r#"
        {
            fun increment(value) {
                return value + 1;
            }
            print increment(41);
        }
        "#,
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "42\n");
}

#[test]
fn user_function_arity_mismatch_reports_expected_and_actual_counts() {
    for (source, actual) in [
        ("fun sum(a, b) { return a + b; } sum(1);", 1),
        ("fun sum(a, b) { return a + b; } sum(1, 2, 3);", 3),
    ] {
        let output = run_program(source);

        assert_eq!(output.status.code(), Some(70));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(&format!("Expected 2 arguments but got {actual}.")));
    }
}

#[test]
fn function_argument_and_parameter_byte_limits_are_enforced() {
    let parameters = (0..=255)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let too_many_parameters = run_program(&format!("fun tooMany({parameters}) {{}}"));
    assert_eq!(too_many_parameters.status.code(), Some(65));
    assert!(
        String::from_utf8(too_many_parameters.stderr)
            .unwrap()
            .contains("Can't have more than 255 parameters.")
    );

    let arguments = vec!["nil"; 256].join(", ");
    let too_many_arguments = run_program(&format!("clock({arguments});"));
    assert_eq!(too_many_arguments.status.code(), Some(65));
    assert!(
        String::from_utf8(too_many_arguments.stderr)
            .unwrap()
            .contains("Can't have more than 255 arguments.")
    );
}

#[test]
fn exactly_255_parameters_and_arguments_fit_in_bytecode_slots() {
    let parameters = (0..255)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut arguments = vec!["false"; 254];
    arguments.push("true");
    let arguments = arguments.join(", ");
    let source = format!("fun last({parameters}) {{ return p254; }} print last({arguments});");

    let output = run_program(&source);

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "true\n");
}

#[test]
fn returning_from_top_level_is_a_compile_error() {
    let output = run_program("return 42;");

    assert_eq!(output.status.code(), Some(65));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Can't return from top-level code.")
    );
}

#[test]
fn recursive_calls_stop_at_the_call_frame_limit() {
    let output = run_program("fun recurse() { return recurse(); } recurse();");

    assert_eq!(output.status.code(), Some(70));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Stack overflow.")
    );
}
