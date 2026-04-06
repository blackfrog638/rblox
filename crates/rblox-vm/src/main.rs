fn main() {
    let mut chunk = rblox_vm::Chunk::new();
    let a = chunk.add_constant(rblox_vm::Value::Number(1.2));
    let b = chunk.add_constant(rblox_vm::Value::Number(3.4));
    let c = chunk.add_constant(rblox_vm::Value::Number(5.6));

    chunk.write(rblox_vm::OP_CONSTANT, 123);
    chunk.write(a, 123);
    chunk.write(rblox_vm::OP_CONSTANT, 123);
    chunk.write(b, 123);
    chunk.write(rblox_vm::OP_ADD, 123);
    chunk.write(rblox_vm::OP_CONSTANT, 123);
    chunk.write(c, 123);
    chunk.write(rblox_vm::OP_DIVIDE, 123);
    chunk.write(rblox_vm::OP_NEGATE, 123);
    chunk.write(rblox_vm::OP_RETURN, 123);

    let output = rblox_vm::disassemble_chunk(&chunk, "test chunk");
    print!("{}", output);

    let mut vm = rblox_vm::VM::new();
    if let Err(error) = vm.interpret(chunk) {
        eprintln!("{}", error);
        std::process::exit(70);
    }
}
