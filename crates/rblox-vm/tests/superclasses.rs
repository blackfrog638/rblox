use rblox_vm::chunk::{OP_CLOSE_UPVALUE, OP_GET_SUPER, OP_SUPER_INVOKE};
use rblox_vm::{Heap, VM, compile, disassemble_instruction};

#[test]
fn super_uses_the_defining_classes_parent_and_binds_the_current_receiver() {
    VM::new()
        .interpret(
            r#"
        class A { method() { return "A:" + this.name; } }
        class B < A {
            method() { return "B"; }
            direct() { return super.method(); }
            reference() { return super.method; }
        }
        class C < B { method() { return "C"; } }
        var instance = C();
        instance.name = "first";
        if (instance.direct() != "A:first") nil();
        var reference = instance.reference();
        instance.name = "second";
        instance.method = nil;
        if (instance.direct() != "A:second") nil();
        if (reference() != "A:second") nil();
        if (instance.reference()() != "A:second") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn super_initializers_initialize_and_return_the_same_subclass_instance() {
    VM::new()
        .interpret(
            r#"
        class A {
            init(a, b) { this.value = a + b; }
            who() { return this.kind; }
        }
        class B < A {
            init(a, b) {
                if (super.init(a, b) != this) nil();
                this.kind = "child";
            }
            reset(value) {
                var initialize = super.init;
                return initialize(value, 1);
            }
        }
        class C < B {}
        var instance = C(20, 22);
        if (instance.value != 42 or instance.who() != "child") nil();
        if (instance.reset(9) != instance or instance.value != 10) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn escaped_nested_functions_capture_this_and_the_original_superclass() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    vm.interpret(
        r#"
        var saved;
        {
            class Parent { read() { return "parent:" + this.name; } }
            fun extend(parent) {
                class Child < parent {
                    capture() {
                        fun middle() {
                            fun later() { return super.read(); }
                            return later;
                        }
                        return middle();
                    }
                }
                parent = nil;
                return Child;
            }
            var Child = extend(Parent);
            var instance = Child();
            instance.name = "alive";
            instance.self = instance;
            saved = instance.capture();
            Parent = nil;
        }
    "#,
    )
    .unwrap();
    vm.collect_garbage();
    vm.interpret("if (saved() != \"parent:alive\") nil();")
        .unwrap();
    vm.interpret("saved = nil;").unwrap();
    vm.collect_garbage();
    assert_eq!(vm.heap().len(), baseline + 1);
}

#[test]
fn nested_classes_and_repeated_declarations_keep_their_own_superclass() {
    VM::new()
        .interpret(
            r#"
        class A { read() { return "A"; } }
        class B { read() { return "B"; } }
        fun extend(parent) {
            class Child < parent {
                read() { return super.read(); }
                nested() {
                    class Inner < B { read() { return super.read(); } }
                    var after = "after";
                    if (after != "after" or Inner().read() != "B") nil();
                    return super.read();
                }
            }
            class Sibling < B { read() { return super.read(); } }
            var after = "after";
            if (after != "after" or Sibling().read() != "B") nil();
            return Child;
        }
        var First = extend(A);
        var Second = extend(B);
        if (First().read() != "A" or Second().read() != "B") nil();
        if (First().nested() != "A" or Second().nested() != "B") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn super_calls_evaluate_arguments_left_to_right_and_preserve_dynamic_this() {
    VM::new()
        .interpret(
            r#"
        var order = "";
        fun argument(value) { order = order + value; return value; }
        class A {
            combine(a, b) { return this.label() + a + b; }
            label() { return "A"; }
        }
        class B < A {}
        class C < B {
            label() { return "C"; }
            run() { return super.combine(argument("1"), argument("2")); }
        }
        if (C().run() != "C12" or order != "12") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn super_runtime_errors_report_the_cause_and_the_vm_recovers() {
    let mut vm = VM::new();
    for (expression, expected) in [
        ("super.missing", "Undefined property 'missing'."),
        ("super.missing()", "Undefined property 'missing'."),
        ("super.method()", "Expected 1 arguments but got 0."),
        ("super.method(1, 2)", "Expected 1 arguments but got 2."),
        ("(super.method)()", "Expected 1 arguments but got 0."),
        ("(super.method)(1, 2)", "Expected 1 arguments but got 2."),
    ] {
        let source = format!(
            "class A {{ method(value) {{ return value; }} }} \
             class B < A {{ run() {{ return {expression}; }} }} B().run();"
        );
        let error = vm.interpret(&source).unwrap_err();
        assert!(error.contains(expected), "{source}: {error}");
        vm.interpret(
            "class A { method() { return 42; } } \
             class B < A { run() { return super.method(); } } \
             if (B().run() != 42) nil();",
        )
        .unwrap();
    }
}

#[test]
fn runtime_error_closes_escaped_this_and_super_captures() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    let error = vm
        .interpret(
            r#"
        var saved;
        {
            class A { read() { return this.value; } }
            class B < A {
                fail() {
                    fun later() { return super.read(); }
                    saved = later;
                    nil();
                }
            }
            var instance = B();
            instance.value = "alive";
            instance.self = instance;
            instance.fail();
        }
    "#,
        )
        .unwrap_err();
    assert!(error.contains("Can only call functions."), "{error}");
    vm.collect_garbage();
    vm.interpret("if (saved() != \"alive\") nil();").unwrap();
    vm.interpret("saved = nil;").unwrap();
    vm.collect_garbage();
    assert_eq!(vm.heap().len(), baseline + 1);
}

#[test]
fn super_bytecode_encodes_method_names_argument_counts_and_closes_captures() {
    let mut heap = Heap::new();
    let script = compile(
        "class A {} class B < A { \
         reference() { return super.method; } \
         direct() { return super.method(1, 2); } }",
        &mut heap,
    )
    .unwrap();
    for (name, opcode, width) in [
        ("reference", OP_GET_SUPER, 2),
        ("direct", OP_SUPER_INVOKE, 3),
    ] {
        let method = script
            .chunk
            .constants
            .iter()
            .filter_map(|value| value.as_function(&heap))
            .find(|function| function.name.as_deref() == Some(name))
            .unwrap();
        assert_eq!(method.upvalue_count, 1);
        let mut offset = 0;
        let mut count = 0;
        while offset < method.chunk.code.len() {
            let (text, next) = disassemble_instruction(&method.chunk, offset, &heap);
            if method.chunk.code[offset] == opcode {
                assert!(text.contains("method"), "{text}");
                assert_eq!(next, offset + width);
                if opcode == OP_SUPER_INVOKE {
                    assert!(text.contains("OP_SUPER_INVOKE"), "{text}");
                    assert!(text.contains("(2 args)"), "{text}");
                    assert_eq!(method.chunk.code[offset + 2], 2);
                } else {
                    assert!(text.contains("OP_GET_SUPER"), "{text}");
                }
                count += 1;
            }
            offset = next;
        }
        assert_eq!(count, 1);
    }
    let mut offset = 0;
    let mut close_count = 0;
    while offset < script.chunk.code.len() {
        if script.chunk.code[offset] == OP_CLOSE_UPVALUE {
            close_count += 1;
        }
        offset = disassemble_instruction(&script.chunk, offset, &heap).1;
    }
    assert_eq!(close_count, 1);
}

#[test]
fn invalid_super_expressions_are_compile_errors() {
    for (source, expected) in [
        ("super.method();", "Can't use 'super' outside of a class."),
        (
            "fun f() { super.method(); }",
            "Can't use 'super' outside of a class.",
        ),
        (
            "class A { method() { super.method(); } }",
            "Can't use 'super' in a class with no superclass.",
        ),
        (
            "class A {} class B < A { method() { class C { method() { super.method(); } } } }",
            "Can't use 'super' in a class with no superclass.",
        ),
        (
            "class A {} class B < A { method() { return super; } }",
            "Expect '.' after 'super'.",
        ),
        (
            "class A {} class B < A { method() { return super.; } }",
            "Expect superclass method name.",
        ),
        (
            "class A {} class B < A { method() { super.method = nil; } }",
            "Invalid assignment target.",
        ),
    ] {
        let error = compile(source, &mut Heap::new()).unwrap_err();
        assert!(error.contains(expected), "{source}: {error}");
    }
}
