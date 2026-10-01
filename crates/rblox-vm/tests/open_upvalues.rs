use rblox_vm::VM;

#[test]
fn closure_disassembly_consumes_local_and_forwarded_capture_operands() {
    use rblox_vm::chunk::OP_CLOSURE;
    use rblox_vm::{compile, disassemble_instruction};
    let script =
        compile("fun outer() { var x = 1; fun middle() { fun inner() { return x; } } }").unwrap();
    let outer = script
        .chunk
        .constants
        .iter()
        .find_map(|v| v.as_function())
        .unwrap();
    let middle = outer
        .chunk
        .constants
        .iter()
        .find_map(|v| v.as_function())
        .unwrap();
    for (function, expected) in [(outer, "local 1"), (middle, "upvalue 0")] {
        let mut offset = 0;
        let mut found = false;
        while offset < function.chunk.code.len() {
            let (text, next) = disassemble_instruction(&function.chunk, offset);
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
    use rblox_vm::chunk::OP_CLOSE_UPVALUE;
    use rblox_vm::{compile, disassemble_chunk};

    let script = compile("{ var x = 1; fun inner() { print x; } }").unwrap();
    assert!(script.chunk.code.contains(&OP_CLOSE_UPVALUE));
    assert!(disassemble_chunk(&script.chunk, "close upvalue").contains("OP_CLOSE_UPVALUE"));
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
fn returned_closures_still_need_runtime_upvalue_closing() {
    let error = VM::new()
        .interpret(
            r#"
        fun outer() {
            var x = "outside";
            fun inner() { print x; }
            return inner;
        }
        var closure = outer();
        closure();
    "#,
        )
        .expect_err("the runtime close-upvalue instruction is not implemented yet");

    assert!(error.contains("Invalid upvalue stack slot."), "{error}");
}
