fn main() {
    let mut chunk = rblox_vm::Chunk::new();
    let constant_index = chunk.add_constant(rblox_vm::Value::Number(1.2));

    chunk.write(rblox_vm::OP_CONSTANT, 123);
    chunk.write(constant_index, 123);
    chunk.write(rblox_vm::OP_RETURN, 123);

    let output = rblox_vm::disassemble_chunk(&chunk, "test chunk");
    print!("{}", output);
}
