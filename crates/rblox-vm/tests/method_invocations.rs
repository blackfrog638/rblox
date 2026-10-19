use rblox_vm::chunk::OP_INVOKE;
use rblox_vm::{Heap, VM, compile, disassemble_instruction};

#[test]
fn invoke_encodes_the_name_and_argument_count_in_three_bytes() {
    let arguments = vec!["nil"; 255].join(",");
    let mut heap = Heap::new();
    let script = compile(
        &format!("obj.zero(); obj.two(1, 2); obj.many({arguments});"),
        &mut heap,
    )
    .unwrap();
    let mut offset = 0;
    let mut invocations = Vec::new();
    while offset < script.chunk.code.len() {
        let (text, next) = disassemble_instruction(&script.chunk, offset, &heap);
        if script.chunk.code[offset] == OP_INVOKE {
            let name = script.chunk.constants[script.chunk.code[offset + 1] as usize]
                .as_string(&heap)
                .unwrap();
            let count = script.chunk.code[offset + 2];
            assert_eq!(next, offset + 3);
            assert!(text.contains(name), "{text}");
            assert!(text.contains(&format!("({count} args)")), "{text}");
            invocations.push((name.to_string(), count));
        }
        offset = next;
    }
    assert_eq!(
        invocations,
        [("zero".into(), 0), ("two".into(), 2), ("many".into(), 255)]
    );
    let error = compile(&format!("obj.many({arguments}, nil);"), &mut heap).unwrap_err();
    assert!(error.contains("Can't have more than 255 arguments."));
}

#[test]
fn direct_calls_support_recursion_chaining_and_returned_functions() {
    VM::new()
        .interpret(
            r#"
        class Counter {
            init(value) { this.value = value; }
            add(value) { this.value = this.value + value; return this; }
            recurse(depth) {
                if (depth == 0) return this.value;
                return this.recurse(depth - 1);
            }
            reader() {
                fun read() { return this.value; }
                return read;
            }
        }
        var counter = Counter(10);
        if (counter.add(20).add(12).recurse(3) != 42) nil();
        if (counter.reader()() != 42) nil();
        var add = counter.add;
        if (add(1) != counter) nil();
        if ((counter.add)(2) != counter) nil();
        if (counter.value != 45) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn callable_fields_shadow_methods_and_keep_their_own_calling_conventions() {
    VM::new()
        .interpret(
            r#"
        class Box {
            init(value) { this.value = value; }
            read(extra) { return this.value + extra; }
        }
        var box = Box(1);
        fun closure(value) { return value + 10; }
        box.read = closure;
        if (box.read(2) != 12) nil();
        box.read = clock;
        if (box.read() < 0) nil();
        box.read = Box;
        var created = box.read(42);
        if (created.value != 42) nil();
        box.read = created.read;
        if (box.read(3) != 45) nil();
        if (box.value != 1) nil();
        class Empty {}
        box.factory = Empty;
        var empty = box.factory();
        empty.value = 7;
        if (empty.value != 7) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn receiver_is_evaluated_once_and_kept_alive_while_arguments_run_left_to_right() {
    VM::new()
        .interpret(
            r#"
        var order = "";
        class Box {
            init(value) { this.value = value; }
            sum(a, b) { return this.value + a + b; }
        }
        var box = Box(1);
        fun receiver() { order = order + "r"; return box; }
        fun argument(label, value) {
            order = order + label;
            box = nil;
            return value;
        }
        if (receiver().sum(argument("a", 20), argument("b", 21)) != 42) nil();
        if (order != "rab") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn invocation_looks_up_the_field_after_evaluating_arguments_like_clox() {
    VM::new()
        .interpret(
            r#"
        class Box { method(value) { return "method"; } }
        var box = Box();
        fun replacement(value) { return "field"; }
        fun replace() { box.method = replacement; return nil; }
        if (box.method(replace()) != "field") nil();
        box = Box();
        // Parentheses force property access before evaluating the arguments.
        if ((box.method)(replace()) != "method") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn invocation_errors_leave_the_vm_reusable() {
    let mut vm = VM::new();
    for (source, expected) in [
        ("nil.method();", "Only instances have methods."),
        ("123.method();", "Only instances have methods."),
        ("class C {} C.method();", "Only instances have methods."),
        (
            "class C { method() { this.method(); } } C().method();",
            "Stack overflow.",
        ),
        (
            "class C { method() {} } var c = C(); c.method = nil; c.method();",
            "Can only call functions.",
        ),
    ] {
        let error = vm.interpret(source).unwrap_err();
        assert!(error.contains(expected), "{source}: {error}");
        vm.interpret("class Valid { method() { return 42; } } if (Valid().method() != 42) nil();")
            .unwrap();
    }
}
