use rblox_vm::chunk::OP_INHERIT;
use rblox_vm::{Heap, VM, compile, disassemble_instruction};

#[test]
fn inheritance_copies_methods_and_overrides_without_changing_the_parent() {
    VM::new()
        .interpret(
            r#"
        class Base {
            answer() { return 1; }
            dispatch() { return this.answer(); }
            read() { return this.value; }
        }
        class Child < Base { answer() { return 2; } }
        class Grandchild < Child {}
        class Sibling < Base {}
        if (Base().answer() != 1) nil();
        if (Sibling().answer() != 1) nil();
        var instance = Grandchild();
        instance.value = 42;
        if (instance.dispatch() != 2) nil();
        if (instance.read() != 42) nil();
        var detached = instance.read;
        if (detached() != 42) nil();
        fun field() { return 3; }
        instance.answer = field;
        if (instance.dispatch() != 3) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn initializers_are_inherited_and_can_be_overridden() {
    VM::new()
        .interpret(
            r#"
        var calls = 0;
        class Base {
            init(value) { calls = calls + 1; this.value = value; }
        }
        class Child < Base {}
        class Grandchild < Child {}
        var instance = Grandchild(42);
        if (instance.value != 42 or calls != 1) nil();
        if (instance.init(43) != instance) nil();
        if (instance.value != 43 or calls != 2) nil();
        class Override < Base { init() { this.value = 7; } }
        if (Override().value != 7 or calls != 2) nil();
        class Empty {}
        class EmptyChild < Empty {}
        EmptyChild();
    "#,
        )
        .unwrap();
}

#[test]
fn local_and_captured_superclasses_do_not_displace_later_local_variables() {
    VM::new()
        .interpret(
            r#"
        class Base { inherited() { return 42; } }
        fun factory(parent) {
            fun create() {
                var before = "before";
                class Child < parent { own() { return before; } }
                var after = "after";
                fun readAfter() { return after; }
                if (after != "after" or readAfter() != "after") nil();
                return Child;
            }
            return create;
        }
        var create = factory(Base);
        var Child = create();
        if (Child().inherited() != 42 or Child().own() != "before") nil();
        for (var i = 0; i < 100; i = i + 1) {
            class Local < Child {}
            var marker = i;
            if (marker != i or Local().inherited() != 42) nil();
        }
    "#,
        )
        .unwrap();
}

#[test]
fn invalid_superclasses_and_constructor_arities_report_errors_and_vm_recovers() {
    let mut vm = VM::new();
    for value in [
        "nil", "true", "123", "\"text\"", "clock", "Box()", "function",
    ] {
        let source = format!(
            "class Box {{}} fun function() {{}} var Parent = {value}; class Child < Parent {{}}"
        );
        let error = vm.interpret(&source).unwrap_err();
        assert!(
            error.contains("Superclass must be a class."),
            "{source}: {error}"
        );
        vm.interpret("class Base {} class Valid < Base {} Valid();")
            .unwrap();
    }
    for (source, message) in [
        ("class Child < Missing {}", "Undefined variable."),
        (
            "class Base { init(a) {} } class Child < Base {} Child();",
            "Expected 1 arguments but got 0.",
        ),
        (
            "class Base { init(a) {} } class Child < Base {} Child(1, 2);",
            "Expected 1 arguments but got 2.",
        ),
        (
            "class Base {} class Child < Base {} Child(1);",
            "Expected 0 arguments but got 1.",
        ),
        (
            "class Base {} class Child < Base {} Child().missing();",
            "Undefined property 'missing'.",
        ),
    ] {
        let error = vm.interpret(source).unwrap_err();
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn self_inheritance_and_malformed_superclass_clauses_are_compile_errors() {
    for (source, message) in [
        ("class Self < Self {}", "A class can't inherit from itself."),
        (
            "class A {} { class A < A {} }",
            "A class can't inherit from itself.",
        ),
        ("class Child < 123 {}", "Expect superclass name."),
        ("class Child < {}", "Expect superclass name."),
    ] {
        let error = compile(source, &mut Heap::new()).unwrap_err();
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn inherited_closures_survive_parent_collection_and_unreachable_cycles_are_freed() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    vm.interpret(
        r#"
        var saved;
        {
            var text = "still " + "alive";
            class Parent { read() { return text + this.suffix; } }
            class Child < Parent {}
            var instance = Child();
            instance.suffix = "!";
            instance.self = instance;
            saved = instance.read;
        }
    "#,
    )
    .unwrap();
    vm.collect_garbage();
    vm.interpret("if (saved() != \"still alive!\") nil();")
        .unwrap();
    vm.interpret("saved = nil;").unwrap();
    vm.collect_garbage();
    assert_eq!(vm.heap().len(), baseline + 1);
}

#[test]
fn inherit_disassembles_as_a_single_byte_instruction() {
    let mut heap = Heap::new();
    let script = compile("class Base {} class Child < Base {}", &mut heap).unwrap();
    let mut offset = 0;
    let mut inherit_count = 0;
    while offset < script.chunk.code.len() {
        let (text, next) = disassemble_instruction(&script.chunk, offset, &heap);
        if script.chunk.code[offset] == OP_INHERIT {
            assert!(text.contains("OP_INHERIT"), "{text}");
            assert_eq!(next, offset + 1);
            inherit_count += 1;
        }
        offset = next;
    }
    assert_eq!(inherit_count, 1);
}
