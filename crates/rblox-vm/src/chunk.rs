use std::{fmt, rc::Rc};

pub const OP_CONSTANT: u8 = 0;
pub const OP_NIL: u8 = 1;
pub const OP_TRUE: u8 = 2;
pub const OP_FALSE: u8 = 3;
pub const OP_EQUAL: u8 = 4;
pub const OP_GREATER: u8 = 5;
pub const OP_LESS: u8 = 6;
pub const OP_AND: u8 = 7;
pub const OP_OR: u8 = 8;
pub const OP_ADD: u8 = 9;
pub const OP_SUBTRACT: u8 = 10;
pub const OP_MULTIPLY: u8 = 11;
pub const OP_DIVIDE: u8 = 12;
pub const OP_NOT: u8 = 13;
pub const OP_NEGATE: u8 = 14;
pub const OP_PRINT: u8 = 15;
pub const OP_RETURN: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjType {
    String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Nil,
    Obj(Rc<Object>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Object {
    String(String),
}

impl Object {
    pub fn obj_type(&self) -> ObjType {
        match self {
            Object::String(_) => ObjType::String,
        }
    }
}

impl Value {
    pub fn is_obj_type(&self, obj_type: ObjType) -> bool {
        match self {
            Value::Obj(obj) => obj.obj_type() == obj_type,
            _ => false,
        }
    }

    pub fn as_obj(&self) -> Option<&Object> {
        match self {
            Value::Obj(obj) => Some(obj.as_ref()),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            Value::Obj(obj) => match obj.as_ref() {
                Object::String(s) => Some(s),
            },
            _ => None,
        }
    }
}

impl std::fmt::Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Object::String(s) => write!(f, "{}", s),
        }
    }
}

pub fn is_obj_type(value: &Value, obj_type: ObjType) -> bool {
    value.is_obj_type(obj_type)
}

pub fn as_string(value: &Value) -> Option<&String> {
    value.as_string()
}

pub fn as_str(value: &Value) -> Option<&str> {
    value.as_string().map(String::as_str)
}

pub fn allocate_string(value: String) -> Value {
    Value::Obj(Rc::new(Object::String(value)))
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(value) => write!(f, "{}", value),
            Value::Bool(value) => write!(f, "{}", value),
            Value::Nil => write!(f, "nil"),
            Value::Obj(value) => write!(f, "{}", value),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineRun {
    pub line: usize,
    pub run_length: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub lines: Vec<LineRun>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn write(&mut self, byte: u8, line: usize) {
        self.code.push(byte);

        if let Some(last) = self.lines.last_mut() {
            if last.line == line {
                last.run_length += 1;
                return;
            }
        }

        self.lines.push(LineRun {
            line,
            run_length: 1,
        });
    }

    pub fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);

        let index = self.constants.len() - 1;
        u8::try_from(index).expect("Too many constants in chunk.")
    }

    pub fn line_at(&self, offset: usize) -> Option<usize> {
        if offset >= self.code.len() {
            return None;
        }

        let mut consumed = 0;
        for run in &self.lines {
            consumed += run.run_length;
            if offset < consumed {
                return Some(run.line);
            }
        }

        None
    }
}

pub fn disassemble_chunk(chunk: &Chunk, name: &str) -> String {
    let mut output = String::new();
    output.push_str(&format!("== {} ==\n", name));

    let mut offset = 0;
    while offset < chunk.code.len() {
        let (line, next_offset) = disassemble_instruction(chunk, offset);
        output.push_str(&line);
        output.push('\n');
        offset = next_offset;
    }

    output
}

pub fn disassemble_instruction(chunk: &Chunk, offset: usize) -> (String, usize) {
    let prefix = format_prefix(chunk, offset);
    let instruction = chunk.code[offset];

    match instruction {
        OP_CONSTANT => constant_instruction(chunk, offset, &prefix),
        OP_NIL => {
            let line = format!("{}{}", prefix, simple_instruction("OP_NIL"));
            (line, offset + 1)
        }
        OP_TRUE => {
            let line = format!("{}{}", prefix, simple_instruction("OP_TRUE"));
            (line, offset + 1)
        }
        OP_FALSE => {
            let line = format!("{}{}", prefix, simple_instruction("OP_FALSE"));
            (line, offset + 1)
        }
        OP_EQUAL => {
            let line = format!("{}{}", prefix, simple_instruction("OP_EQUAL"));
            (line, offset + 1)
        }
        OP_GREATER => {
            let line = format!("{}{}", prefix, simple_instruction("OP_GREATER"));
            (line, offset + 1)
        }
        OP_LESS => {
            let line = format!("{}{}", prefix, simple_instruction("OP_LESS"));
            (line, offset + 1)
        }
        OP_AND => {
            let line = format!("{}{}", prefix, simple_instruction("OP_AND"));
            (line, offset + 1)
        }
        OP_OR => {
            let line = format!("{}{}", prefix, simple_instruction("OP_OR"));
            (line, offset + 1)
        }
        OP_ADD => {
            let line = format!("{}{}", prefix, simple_instruction("OP_ADD"));
            (line, offset + 1)
        }
        OP_SUBTRACT => {
            let line = format!("{}{}", prefix, simple_instruction("OP_SUBTRACT"));
            (line, offset + 1)
        }
        OP_MULTIPLY => {
            let line = format!("{}{}", prefix, simple_instruction("OP_MULTIPLY"));
            (line, offset + 1)
        }
        OP_DIVIDE => {
            let line = format!("{}{}", prefix, simple_instruction("OP_DIVIDE"));
            (line, offset + 1)
        }
        OP_NOT => {
            let line = format!("{}{}", prefix, simple_instruction("OP_NOT"));
            (line, offset + 1)
        }
        OP_NEGATE => {
            let line = format!("{}{}", prefix, simple_instruction("OP_NEGATE"));
            (line, offset + 1)
        }
        OP_PRINT => {
            let line = format!("{}{}", prefix, simple_instruction("OP_PRINT"));
            (line, offset + 1)
        }
        OP_RETURN => {
            let line = format!("{}{}", prefix, simple_instruction("OP_RETURN"));
            (line, offset + 1)
        }
        _ => {
            let line = format!("{}Unknown opcode {}", prefix, instruction);
            (line, offset + 1)
        }
    }
}

fn format_prefix(chunk: &Chunk, offset: usize) -> String {
    let line = chunk.line_at(offset).unwrap_or(0);

    if offset > 0 && chunk.line_at(offset - 1) == Some(line) {
        format!("{:04}    | ", offset)
    } else {
        format!("{:04} {:4} ", offset, line)
    }
}

fn simple_instruction(name: &str) -> String {
    name.to_string()
}

fn constant_instruction(chunk: &Chunk, offset: usize, prefix: &str) -> (String, usize) {
    let Some(index) = chunk.code.get(offset + 1).copied() else {
        return (
            format!("{}OP_CONSTANT <missing constant index>", prefix),
            offset + 1,
        );
    };

    let value_text = chunk
        .constants
        .get(index as usize)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<invalid constant index>".to_string());

    (
        format!("{}OP_CONSTANT {:4} {}", prefix, index, value_text),
        offset + 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_merges_line_runs_for_same_line() {
        let mut chunk = Chunk::new();
        chunk.write(OP_RETURN, 1);
        chunk.write(OP_RETURN, 1);
        chunk.write(OP_RETURN, 2);

        assert_eq!(chunk.code.len(), 3);
        assert_eq!(
            chunk.lines,
            vec![
                LineRun {
                    line: 1,
                    run_length: 2
                },
                LineRun {
                    line: 2,
                    run_length: 1
                }
            ]
        );
    }

    #[test]
    fn add_constant_returns_index() {
        let mut chunk = Chunk::new();
        let index = chunk.add_constant(Value::Number(1.2));

        assert_eq!(index, 0);
        assert_eq!(chunk.constants, vec![Value::Number(1.2)]);
    }

    #[test]
    fn disassemble_shows_constant_and_return() {
        let mut chunk = Chunk::new();
        let constant_index = chunk.add_constant(Value::Number(3.14));

        chunk.write(OP_CONSTANT, 123);
        chunk.write(constant_index, 123);
        chunk.write(OP_RETURN, 123);

        let output = disassemble_chunk(&chunk, "test chunk");

        assert!(output.contains("== test chunk =="));
        assert!(output.contains("OP_CONSTANT"));
        assert!(output.contains("3.14"));
        assert!(output.contains("OP_RETURN"));
    }

    #[test]
    fn disassemble_shows_arithmetic_opcodes() {
        let mut chunk = Chunk::new();
        chunk.write(OP_ADD, 1);
        chunk.write(OP_SUBTRACT, 1);
        chunk.write(OP_MULTIPLY, 1);
        chunk.write(OP_DIVIDE, 1);
        chunk.write(OP_NEGATE, 1);
        chunk.write(OP_RETURN, 1);

        let output = disassemble_chunk(&chunk, "arith");

        assert!(output.contains("OP_ADD"));
        assert!(output.contains("OP_SUBTRACT"));
        assert!(output.contains("OP_MULTIPLY"));
        assert!(output.contains("OP_DIVIDE"));
        assert!(output.contains("OP_NEGATE"));
    }
}
