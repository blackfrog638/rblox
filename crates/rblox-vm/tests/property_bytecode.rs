use rblox_vm::chunk::{OP_GET_PROPERTY, OP_SET_PROPERTY};
use rblox_vm::{Heap, compile, disassemble_instruction};

#[test]
fn property_instructions_encode_names_and_disassemble_with_correct_width() {
    let mut heap = Heap::new();
    let script = compile(
        "class Box {} var a = Box(); a.item = 42; print a.item;",
        &mut heap,
    )
    .unwrap();
    let mut offset = 0;
    let mut properties = Vec::new();
    while offset < script.chunk.code.len() {
        let opcode = script.chunk.code[offset];
        let (text, next) = disassemble_instruction(&script.chunk, offset, &heap);
        if matches!(opcode, OP_GET_PROPERTY | OP_SET_PROPERTY) {
            let name = script.chunk.constants[script.chunk.code[offset + 1] as usize];
            assert_eq!(name.as_string(&heap).unwrap(), "item");
            assert_eq!(next, offset + 2);
            assert!(text.contains(if opcode == OP_GET_PROPERTY {
                "OP_GET_PROPERTY"
            } else {
                "OP_SET_PROPERTY"
            }));
            assert!(text.contains("item"));
            properties.push(opcode);
        }
        offset = next;
    }
    assert_eq!(properties, [OP_SET_PROPERTY, OP_GET_PROPERTY]);
}
