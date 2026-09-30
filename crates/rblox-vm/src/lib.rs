pub mod chunk;
pub mod compiler;
pub mod object;
pub mod scanner;
pub mod table;
pub mod value;
pub mod vm;

pub use chunk::{
    Chunk, LineRun, OP_ADD, OP_CONSTANT, OP_DIVIDE, OP_MULTIPLY, OP_NEGATE, OP_RETURN, OP_SUBTRACT,
    disassemble_chunk, disassemble_instruction,
};
pub use compiler::compile;
pub use scanner::{Scanner, Token, TokenKind};
pub use table::{Entry, Table};
pub use vm::VM;

pub use object::{Closure, NativeFn, ObjFunction, ObjType, Object, allocate_string, hash_string};
pub use value::Value;
