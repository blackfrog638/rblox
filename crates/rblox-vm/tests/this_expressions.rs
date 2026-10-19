use rblox_vm::{Heap, VM, compile};

#[test]
fn direct_and_detached_methods_use_their_original_receiver() {
    VM::new()
        .interpret(
            r#"
        class Box {
            set(a, b) { this.value = a + b; return this; }
            read() { return this.value; }
        }
        var first = Box();
        var second = Box();
        if (first.set(10, 2) != first) nil();
        second.set(20, 3);
        var set = first.set;
        if (set(30, 4) != first) nil();
        if (first.read() != 34) nil();
        if (second.read() != 23) nil();
        second.callback = first.read;
        if (second.callback() != 34) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn ordinary_functions_nested_in_methods_capture_this_through_upvalues() {
    let mut vm = VM::new();
    vm.interpret(
        r#"
        class Box {
            capture() {
                fun middle() {
                    fun inner() { return this.value; }
                    return inner;
                }
                return middle();
            }
        }
        var box = Box();
        box.value = "before";
        var captured = box.capture();
        box.value = "after";
        box = nil;
    "#,
    )
    .unwrap();
    vm.collect_garbage();
    vm.interpret("if (captured() != \"after\") nil();").unwrap();
}

#[test]
fn nested_classes_have_their_own_this_and_restore_the_outer_context() {
    VM::new()
        .interpret(
            r#"
        class Outer {
            make() {
                var outer = this;
                class Inner {
                    total() { return outer.value + this.value; }
                }
                var inner = Inner();
                inner.value = 2;
                this.value = 40;
                return inner;
            }
        }
        var outer = Outer();
        var inner = outer.make();
        if (inner.total() != 42) nil();
        if (outer.value != 40) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn this_cannot_be_used_outside_classes_or_assigned_to() {
    for (source, message) in [
        ("print this;", "Can't use 'this' outside of a class."),
        (
            "fun f() { return this; }",
            "Can't use 'this' outside of a class.",
        ),
        (
            "class C { f() { return this; } } print this;",
            "Can't use 'this' outside of a class.",
        ),
        (
            "class C { f() { this = nil; } }",
            "Invalid assignment target.",
        ),
        (
            "class C { f() { fun g() { this = nil; } } }",
            "Invalid assignment target.",
        ),
    ] {
        let error = compile(source, &mut Heap::new()).unwrap_err();
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn runtime_errors_close_this_captures_and_unreachable_cycles_are_collected() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    let error = vm
        .interpret(
            r#"
        var saved;
        {
            class Local {
                escape() {
                    fun readLater() { return this.text; }
                    saved = readLater;
                    nil();
                }
            }
            var instance = Local();
            instance.text = "still " + "alive";
            instance.self = instance;
            instance.escape();
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
