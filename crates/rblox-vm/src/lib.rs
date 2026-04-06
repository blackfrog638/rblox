pub mod chunk;
pub mod vm;

pub use chunk::{
    Chunk, LineRun, OP_CONSTANT, OP_RETURN, Value, disassemble_chunk, disassemble_instruction,
};
pub use vm::VM;
