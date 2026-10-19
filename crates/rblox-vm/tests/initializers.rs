use rblox_vm::{Heap, VM, compile};

#[test]
fn class_calls_forward_arguments_and_return_distinct_initialized_instances() {
    VM::new()
        .interpret(
            r#"
        var order = "";
        var calls = 0;
        fun argument(label, value) { order = order + label; return value; }
        class Pair {
            init(first, second) {
                calls = calls + 1;
                this.first = first;
                this.second = second;
            }
        }
        var first = Pair(argument("a", 10), argument("b", 20));
        var second = Pair(30, 40);
        if (order != "ab") nil();
        if (calls != 2) nil();
        if (first == second) nil();
        if (first.first != 10 or first.second != 20) nil();
        if (second.first != 30 or second.second != 40) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn implicit_and_bare_returns_preserve_the_receiver_for_all_call_forms() {
    VM::new()
        .interpret(
            r#"
        class Box {
            init(value) {
                this.value = value;
                if (value < 0) return;
                this.value = value + 1;
            }
        }
        var box = Box(-1);
        if (box.value != -1) nil();
        if (box.init(10) != box) nil();
        if (box.value != 11) nil();
        if (box.init(-2) != box) nil();
        if (box.value != -2) nil();
        var detached = box.init;
        if (detached(20) != box) nil();
        if (box.value != 21) nil();
        if (detached(-3) != box) nil();
        if (box.value != -3) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn initializer_rules_are_local_to_the_current_function() {
    let mut vm = VM::new();
    vm.interpret(
        r#"
        fun init() { return 42; }
        if (init() != 42) nil();
        fun makeClass(prefix) {
            class Box {
                init(value) {
                    fun init() { return this.value; }
                    this.value = prefix + value;
                    this.read = init;
                    class Inner {
                        init(value) { this.value = value; return; }
                    }
                    this.inner = Inner(7);
                }
                valueMethod() { return this.value; }
                emptyMethod() { return; }
            }
            return Box;
        }
        var Box = makeClass("prefix:");
        var box = Box("value");
    "#,
    )
    .unwrap();
    vm.collect_garbage();
    vm.interpret(
        r#"
        if (box.read() != "prefix:value") nil();
        if (box.valueMethod() != "prefix:value") nil();
        if (box.emptyMethod() != nil) nil();
        if (box.inner.value != 7) nil();
    "#,
    )
    .unwrap();
}

#[test]
fn initializers_reject_all_explicit_return_values() {
    for value in ["42", "nil", "this"] {
        let source = format!("class Box {{ init() {{ return {value}; }} }}");
        let error = compile(&source, &mut Heap::new()).unwrap_err();
        assert!(
            error.contains("Can't return a value from an initializer."),
            "{source}: {error}"
        );
    }
}

#[test]
fn constructor_arity_matches_the_initializer_or_defaults_to_zero() {
    VM::new()
        .interpret("class Empty {} Empty(); class Zero { init() {} } Zero();")
        .unwrap();

    for (source, expected) in [
        (
            "class Empty {} Empty(1);",
            "Expected 0 arguments but got 1.",
        ),
        (
            "class Box { init(value) {} } Box();",
            "Expected 1 arguments but got 0.",
        ),
        (
            "class Box { init(value) {} } Box(1, 2);",
            "Expected 1 arguments but got 2.",
        ),
        (
            "class Zero { init() {} } Zero(1);",
            "Expected 0 arguments but got 1.",
        ),
    ] {
        let error = VM::new().interpret(source).unwrap_err();
        assert!(error.contains(expected), "{source}: {error}");
    }
}

#[test]
fn cached_init_name_survives_collection_without_any_classes() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    for _ in 0..3 {
        vm.collect_garbage();
        vm.interpret(
            r#"
            {
                class Local { init(value) { this.value = value; } }
                var instance = Local(42);
                if (instance.value != 42) nil();
            }
        "#,
        )
        .unwrap();
        vm.collect_garbage();
        assert_eq!(vm.heap().len(), baseline);
    }
}

#[test]
fn failed_initializers_preserve_escaped_captures_and_allow_vm_reuse() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    let error = vm
        .interpret(
            r#"
        var saved;
        {
            class Local {
                init(text) {
                    this.text = text;
                    this.self = this;
                    fun readLater() { return this.text; }
                    saved = readLater;
                    nil();
                }
            }
            Local("still " + "alive");
        }
    "#,
        )
        .unwrap_err();
    assert!(error.contains("Can only call functions."), "{error}");
    vm.collect_garbage();
    vm.interpret("if (saved() != \"still alive\") nil();")
        .unwrap();
    vm.interpret("saved = nil;").unwrap();
    vm.collect_garbage();
    assert_eq!(vm.heap().len(), baseline + 1);
}
