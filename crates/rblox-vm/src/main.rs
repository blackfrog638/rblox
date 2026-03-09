use rblox_vm::{Chunk, OpCode, Value, Vm};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut chunk = Chunk::new();

    let one = chunk.add_constant(Value::Number(1.0));
    let two = chunk.add_constant(Value::Number(2.0));
    let three = chunk.add_constant(Value::Number(3.0));

    // Computes (1 + 2) * 3.
    chunk.write_op(OpCode::Constant, 1);
    chunk.write_byte(one as u8, 1);
    chunk.write_op(OpCode::Constant, 1);
    chunk.write_byte(two as u8, 1);
    chunk.write_op(OpCode::Add, 1);
    chunk.write_op(OpCode::Constant, 1);
    chunk.write_byte(three as u8, 1);
    chunk.write_op(OpCode::Multiply, 1);
    chunk.write_op(OpCode::Return, 1);

    let mut vm = Vm::new(chunk);
    let result = vm.run()?;
    println!("{}", result);

    Ok(())
}
