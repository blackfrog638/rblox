use rblox_vm::chunk::{OP_CALL, OP_GET_PROPERTY};
use rblox_vm::{Heap, VM, compile, disassemble_instruction};

#[test]
fn direct_and_detached_method_calls_reuse_property_and_call_instructions() {
    let mut heap = Heap::new();
    let script = compile(
        "obj.method(1); var method = obj.method; method(2);",
        &mut heap,
    )
    .unwrap();
    let mut offset = 0;
    let mut operations = Vec::new();
    while offset < script.chunk.code.len() {
        let opcode = script.chunk.code[offset];
        let (_, next) = disassemble_instruction(&script.chunk, offset, &heap);
        if matches!(opcode, OP_GET_PROPERTY | OP_CALL) {
            operations.push(opcode);
        }
        offset = next;
    }
    assert_eq!(
        operations,
        [OP_GET_PROPERTY, OP_CALL, OP_GET_PROPERTY, OP_CALL]
    );
}

#[test]
fn escaped_method_retains_captures_after_runtime_error_and_releases_cycles() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    vm.interpret(
        r#"
        var saved;
        {
            var text = "sur" + "vives";
            class Local { read() { return text; } }
            var instance = Local();
            saved = instance.read;
            instance.self = instance;
            instance.method = saved;
            nil();
        }
    "#,
    )
    .unwrap_err();
    vm.collect_garbage();
    vm.interpret("if (saved() != \"survives\") nil();").unwrap();
    vm.interpret("saved = nil;").unwrap();
    vm.collect_garbage();
    assert_eq!(vm.heap().len(), baseline + 1);
}
