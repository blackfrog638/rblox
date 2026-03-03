use rblox::app::App;
use rblox::expr::Expr;
use rblox::token::Token;
use rblox::token_type::TokenType;
use rblox::value::Value;

fn ident(name: &str) -> Token {
    Token::new(TokenType::Identifier, name.to_string(), None, 1)
}

fn run_source(source: &str) -> App {
    let mut app = App::new();
    app.run_source(source).expect("run should succeed");
    app
}

fn load_fixture(name: &str) -> String {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push(name);
    std::fs::read_to_string(path).expect("fixture should be readable")
}

#[test]
fn interprets_statements_fixture() {
    let source = load_fixture("statements.lox");
    let mut app = run_source(&source);
    let interpreter = app.interpreter_mut();

    let a = interpreter
        .evaluate(&Expr::Variable { name: ident("a") })
        .expect("a should exist");
    assert_eq!(a, Value::Number(3.0));

    let s = interpreter
        .evaluate(&Expr::Variable { name: ident("s") })
        .expect("s should exist");
    assert_eq!(s, Value::Str("hi!".to_string()));

    let gt = interpreter
        .evaluate(&Expr::Variable { name: ident("gt") })
        .expect("gt should exist");
    assert_eq!(gt, Value::Boolean(true));

    let lt = interpreter
        .evaluate(&Expr::Variable { name: ident("lt") })
        .expect("lt should exist");
    assert_eq!(lt, Value::Boolean(false));

    let eq = interpreter
        .evaluate(&Expr::Variable { name: ident("eq") })
        .expect("eq should exist");
    assert_eq!(eq, Value::Boolean(true));

    let neq = interpreter
        .evaluate(&Expr::Variable { name: ident("neq") })
        .expect("neq should exist");
    assert_eq!(neq, Value::Boolean(false));

    let n = interpreter
        .evaluate(&Expr::Variable { name: ident("n") })
        .expect("n should exist");
    assert_eq!(n, Value::Nil);

    let bang = interpreter
        .evaluate(&Expr::Variable {
            name: ident("bang"),
        })
        .expect("bang should exist");
    assert_eq!(bang, Value::Boolean(true));

    let scoped = interpreter
        .evaluate(&Expr::Variable {
            name: ident("scoped"),
        })
        .expect("scoped should exist");
    assert_eq!(scoped, Value::Str("outer".to_string()));

    let or_value = interpreter
        .evaluate(&Expr::Variable {
            name: ident("or_value"),
        })
        .expect("or_value should exist");
    assert_eq!(or_value, Value::Boolean(true));

    let and_value = interpreter
        .evaluate(&Expr::Variable {
            name: ident("and_value"),
        })
        .expect("and_value should exist");
    assert_eq!(and_value, Value::Boolean(false));

    let side = interpreter
        .evaluate(&Expr::Variable {
            name: ident("side"),
        })
        .expect("side should exist");
    assert_eq!(side, Value::Number(0.0));

    let or_short = interpreter
        .evaluate(&Expr::Variable {
            name: ident("or_short"),
        })
        .expect("or_short should exist");
    assert_eq!(or_short, Value::Boolean(true));

    let and_short = interpreter
        .evaluate(&Expr::Variable {
            name: ident("and_short"),
        })
        .expect("and_short should exist");
    assert_eq!(and_short, Value::Boolean(false));

    let sum = interpreter
        .evaluate(&Expr::Variable { name: ident("sum") })
        .expect("sum should exist");
    assert_eq!(sum, Value::Number(3.0));

    let t0 = interpreter
        .evaluate(&Expr::Variable { name: ident("t0") })
        .expect("t0 should exist");
    match t0 {
        Value::Number(_) => {}
        other => panic!("expected number, got {:?}", other),
    }

    let add_result = interpreter
        .evaluate(&Expr::Variable {
            name: ident("add_result"),
        })
        .expect("add_result should exist");
    assert_eq!(add_result, Value::Number(3.0));

    let nil_result = interpreter
        .evaluate(&Expr::Variable {
            name: ident("nil_result"),
        })
        .expect("nil_result should exist");
    assert_eq!(nil_result, Value::Nil);

    let fib_result = interpreter
        .evaluate(&Expr::Variable {
            name: ident("fib_result"),
        })
        .expect("fib_result should exist");
    assert_eq!(fib_result, Value::Number(8.0));

    let c1 = interpreter
        .evaluate(&Expr::Variable { name: ident("c1") })
        .expect("c1 should exist");
    assert_eq!(c1, Value::Number(1.0));

    let c2 = interpreter
        .evaluate(&Expr::Variable { name: ident("c2") })
        .expect("c2 should exist");
    assert_eq!(c2, Value::Number(2.0));

    let c3 = interpreter
        .evaluate(&Expr::Variable { name: ident("c3") })
        .expect("c3 should exist");
    assert_eq!(c3, Value::Number(3.0));

    // Independent closure starts from 0 again.
    let b1 = interpreter
        .evaluate(&Expr::Variable { name: ident("b1") })
        .expect("b1 should exist");
    assert_eq!(b1, Value::Number(1.0));

    // Closure over parameter.
    let adder_result = interpreter
        .evaluate(&Expr::Variable {
            name: ident("adder_result"),
        })
        .expect("adder_result should exist");
    assert_eq!(adder_result, Value::Number(8.0));

    // Closure reads updated variable (late binding).
    let late_result = interpreter
        .evaluate(&Expr::Variable {
            name: ident("late_result"),
        })
        .expect("late_result should exist");
    assert_eq!(late_result, Value::Str("after".to_string()));
}

#[test]
fn supports_static_methods_on_class_object() {
    let source = r#"
class Math {
    class add(a, b) {
        return a + b;
    }

    twice(n) {
        return n * 2;
    }
}

var static_sum = Math.add(3, 4);
var instance_twice = Math().twice(5);
"#;

    let mut app = run_source(source);
    let interpreter = app.interpreter_mut();

    let static_sum = interpreter
        .evaluate(&Expr::Variable {
            name: ident("static_sum"),
        })
        .expect("static_sum should exist");
    assert_eq!(static_sum, Value::Number(7.0));

    let instance_twice = interpreter
        .evaluate(&Expr::Variable {
            name: ident("instance_twice"),
        })
        .expect("instance_twice should exist");
    assert_eq!(instance_twice, Value::Number(10.0));
}

#[test]
fn static_method_is_not_available_on_instance() {
    let source = r#"
class Tools {
    class ping() {
        return 1;
    }
}

var inst = Tools();
inst.ping();
"#;

    let mut app = App::new();
    let err = app.run_source(source).expect_err("run should fail");
    assert!(
        err.contains("UndefinedProperty(\"ping\")"),
        "unexpected error: {err}"
    );
}
