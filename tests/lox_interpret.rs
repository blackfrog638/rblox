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

#[test]
fn subclass_inherits_instance_methods() {
    let source = r#"
class Doughnut {
    cook() {
        return "Fry until golden brown.";
    }
}

class BostonCream < Doughnut {}

var cooked = BostonCream().cook();
"#;

    let mut app = run_source(source);
    let interpreter = app.interpreter_mut();

    let cooked = interpreter
        .evaluate(&Expr::Variable {
            name: ident("cooked"),
        })
        .expect("cooked should exist");
    assert_eq!(cooked, Value::Str("Fry until golden brown.".to_string()));
}

#[test]
fn superclass_must_be_a_class() {
    let source = r#"
var NotClass = "I am not a class";
class Sub < NotClass {}
"#;

    let mut app = App::new();
    let err = app.run_source(source).expect_err("run should fail");
    assert!(
        err.contains("Superclass must be a class."),
        "unexpected error: {err}"
    );
}

#[test]
fn class_cannot_inherit_from_itself() {
    let source = r#"
class Oops < Oops {}
"#;

    let mut app = App::new();
    let err = app.run_source(source).expect_err("run should fail");
    assert!(
        err.contains("A class can't inherit from itself."),
        "unexpected error: {err}"
    );
}

#[test]
fn subclass_can_call_super_method() {
    let source = r#"
class A {
    cook() {
        return "A";
    }
}

class B < A {
    cook() {
        return "B";
    }

    test() {
        return super.cook();
    }
}

var result = B().test();
"#;

    let mut app = run_source(source);
    let interpreter = app.interpreter_mut();

    let result = interpreter
        .evaluate(&Expr::Variable {
            name: ident("result"),
        })
        .expect("result should exist");
    assert_eq!(result, Value::Str("A".to_string()));
}

#[test]
fn super_cannot_be_used_outside_class() {
    let source = r#"
super.nope();
"#;

    let mut app = App::new();
    let err = app.run_source(source).expect_err("run should fail");
    assert!(
        err.contains("Can't use 'super' outside of a class."),
        "unexpected error: {err}"
    );
}

#[test]
fn super_cannot_be_used_without_superclass() {
    let source = r#"
class Solo {
    test() {
        return super.test();
    }
}
"#;

    let mut app = App::new();
    let err = app.run_source(source).expect_err("run should fail");
    assert!(
        err.contains("Can't use 'super' in a class with no superclass."),
        "unexpected error: {err}"
    );
}

#[test]
fn subclass_inherits_superclass_initializer() {
    let source = r#"
class A {
    init(v) {
        this.v = v;
    }
}

class B < A {}

var got = B(42).v;
"#;

    let mut app = run_source(source);
    let interpreter = app.interpreter_mut();

    let got = interpreter
        .evaluate(&Expr::Variable { name: ident("got") })
        .expect("got should exist");
    assert_eq!(got, Value::Number(42.0));
}

#[test]
fn subclass_initializer_can_delegate_to_super_init() {
    let source = r#"
class A {
    init(v) {
        this.v = v;
    }
}

class B < A {
    init(v) {
        super.init(v + 1);
    }
}

var got = B(41).v;
"#;

    let mut app = run_source(source);
    let interpreter = app.interpreter_mut();

    let got = interpreter
        .evaluate(&Expr::Variable { name: ident("got") })
        .expect("got should exist");
    assert_eq!(got, Value::Number(42.0));
}

#[test]
fn subclass_inherits_static_methods() {
    let source = r#"
class A {
    class ping() {
        return "pong";
    }
}

class B < A {}

var got = B.ping();
"#;

    let mut app = run_source(source);
    let interpreter = app.interpreter_mut();

    let got = interpreter
        .evaluate(&Expr::Variable { name: ident("got") })
        .expect("got should exist");
    assert_eq!(got, Value::Str("pong".to_string()));
}
