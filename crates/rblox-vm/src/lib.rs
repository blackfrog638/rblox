#[cfg(feature = "debug-gc")]
macro_rules! gc_log {
    ($($argument:tt)*) => {
        eprintln!("[GC] {}", format_args!($($argument)*));
    };
}

#[cfg(not(feature = "debug-gc"))]
macro_rules! gc_log {
    ($($argument:tt)*) => {};
}

pub(crate) use gc_log;

pub mod chunk;
pub mod compiler;
pub mod memory;
pub mod object;
pub mod scanner;
pub mod table;
pub mod value;
pub mod vm;

pub use chunk::{
    Chunk, LineRun, OP_ADD, OP_CLOSE_UPVALUE, OP_CONSTANT, OP_DIVIDE, OP_MULTIPLY, OP_NEGATE,
    OP_RETURN, OP_SUBTRACT, disassemble_chunk, disassemble_instruction,
};
pub use compiler::compile;
pub use memory::{Heap, ObjId};
pub use scanner::{Scanner, Token, TokenKind};
pub use table::{Entry, Table};
pub use vm::VM;

pub use object::{Closure, NativeFn, ObjFunction, Object, Upvalue, UpvalueDesc, hash_string};
pub use value::Value;
