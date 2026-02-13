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
}
