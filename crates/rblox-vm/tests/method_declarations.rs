use rblox_vm::chunk::{OP_CLOSURE, OP_METHOD};
use rblox_vm::{Heap, VM, compile, disassemble_instruction};

#[test]
fn declarations_emit_named_methods_and_do_not_execute_their_bodies() {
    let mut heap = Heap::new();
    let source = "class Example { first(a) { nil(); } second() { return 42; } }";
    let script = compile(source, &mut heap).unwrap();
    let mut offset = 0;
    let mut names = Vec::new();
    let mut previous = None;
    while offset < script.chunk.code.len() {
        let opcode = script.chunk.code[offset];
        let (text, next) = disassemble_instruction(&script.chunk, offset, &heap);
        if opcode == OP_METHOD {
            assert_eq!(previous, Some(OP_CLOSURE));
            assert_eq!(next, offset + 2);
            let name = script.chunk.constants[script.chunk.code[offset + 1] as usize]
                .as_string(&heap)
                .unwrap();
            assert!(text.contains("OP_METHOD") && text.contains(name));
            names.push(name.as_str());
        }
        previous = Some(opcode);
        offset = next;
    }
    assert_eq!(names, ["first", "second"]);
    VM::new().interpret(source).unwrap();
}

#[test]
fn malformed_methods_are_compile_errors_and_compiler_recovers() {
    let mut vm = VM::new();
    for source in [
        "class A { fun method() {} }",
        "class A { method {} }",
        "class A { method(a, a) {} }",
        "class A { method() { return 1; }",
        "class A { method() { break; } }",
    ] {
        assert!(vm.interpret(source).is_err(), "{source}");
        vm.interpret("class Valid { method() { return 1; } }")
            .unwrap();
    }
}
