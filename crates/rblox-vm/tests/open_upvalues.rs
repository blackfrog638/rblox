use rblox_vm::{Heap, VM};

#[test]
fn closure_disassembly_consumes_local_and_forwarded_capture_operands() {
    let mut heap = Heap::new();
    use rblox_vm::chunk::OP_CLOSURE;
    use rblox_vm::{compile, disassemble_instruction};
    let script = compile(
        "fun outer() { var x = 1; fun middle() { fun inner() { return x; } } }",
        &mut heap,
    )
    .unwrap();
    let outer = script
        .chunk
        .constants
        .iter()
        .find_map(|v| v.as_function(&heap))
        .unwrap();
    let middle = outer
        .chunk
        .constants
        .iter()
        .find_map(|v| v.as_function(&heap))
        .unwrap();
    for (function, expected) in [(outer, "local 1"), (middle, "upvalue 0")] {
        let mut offset = 0;
        let mut found = false;
        while offset < function.chunk.code.len() {
            let (text, next) = disassemble_instruction(&function.chunk, offset, &heap);
            if function.chunk.code[offset] == OP_CLOSURE {
                assert!(text.contains("OP_CLOSURE"));
                assert!(text.contains(expected), "{text}");
                assert_eq!(next, offset + 4);
                found = true;
            }
            assert!(next > offset);
            offset = next;
        }
        assert!(found);
    }
}

#[test]
fn compiler_emits_and_disassembler_recognizes_close_upvalue() {
    let mut heap = Heap::new();
    use rblox_vm::chunk::OP_CLOSE_UPVALUE;
    use rblox_vm::{compile, disassemble_chunk};

    let script = compile("{ var x = 1; fun inner() { print x; } }", &mut heap).unwrap();
    assert!(script.chunk.code.contains(&OP_CLOSE_UPVALUE));
    assert!(disassemble_chunk(&script.chunk, "close upvalue", &heap).contains("OP_CLOSE_UPVALUE"));
}

#[test]
fn open_captures_share_mutations_and_forward_through_nested_closures() {
    VM::new()
        .interpret(
            r#"
        fun outer() {
            var x = 1;
            fun set() { x = x + 1; }
            fun middle() {
                fun inner() { set(); return x; }
                return inner();
            }
            if (middle() != 2) nil();
            if (x != 2) nil();
            x = 10;
            if (middle() != 11) nil();
        }
        outer(); outer();
    "#,
        )
        .unwrap();
}

#[test]
fn each_declaration_execution_creates_a_fresh_closure() {
    VM::new()
        .interpret(
            r#"
        fun make() { fun f() { return 42; } return f; }
        var a = make(); var b = make();
        if (a == b) nil();
        if (a != a) nil();
        if (a() != 42) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn returned_closures_keep_captured_variables_alive() {
    VM::new()
        .interpret(
            r#"
        fun outer() {
            var x = "outside";
            fun inner() { print x; return x; }
            return inner;
        }
        var closure = outer();
        closure();
        if (closure() != "outside") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn sibling_closures_share_a_closed_mutable_variable() {
    VM::new()
        .interpret(
            r#"
        var increment;
        var read;
        fun makeClosures() {
            var value = 0;
            fun inc() { value = value + 1; }
            fun get() { return value; }
            increment = inc;
            read = get;
        }
        makeClosures();
        increment();
        if (read() != 1) nil();
        increment();
        if (read() != 2) nil();
    "#,
        )
        .unwrap();
}

#[test]
fn block_scope_closes_captured_local_before_its_slot_is_reused() {
    VM::new()
        .interpret(
            r#"
        var saved;
        {
            var value = "kept";
            fun get() { return value; }
            saved = get;
        }
        { var other = "replacement"; }
        if (saved() != "kept") nil();
    "#,
        )
        .unwrap();
}

#[test]
fn loop_continue_and_break_close_captured_body_locals() {
    VM::new()
        .interpret(
            r#"
        var first;
        var second;
        for (var i = 0; i < 3; i = i + 1) {
            var captured = i;
            fun get() { return captured; }
            if (i == 0) first = get;
            if (i == 1) {
                second = get;
                continue;
            }
            if (i == 2) break;
        }
        if (first() != 0) nil();
        if (second() != 1) nil();
    "#,
        )
        .unwrap();
}
