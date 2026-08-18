use crate::chunk::{
    Chunk, OP_ADD, OP_AND, OP_CONSTANT, OP_DIVIDE, OP_EQUAL, OP_FALSE, OP_GREATER, OP_LESS,
    OP_MULTIPLY, OP_NEGATE, OP_NIL, OP_NOT, OP_OR, OP_PRINT, OP_RETURN, OP_SUBTRACT, OP_TRUE,
    Value, disassemble_instruction,
};
use crate::compiler::compile;

pub struct VM {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
    trace_execution: bool,
}

impl VM {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Vec::new(),
            trace_execution: false,
        }
    }

    pub fn set_trace_execution(&mut self, enabled: bool) {
        self.trace_execution = enabled;
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        self.chunk = compile(source)?;
        self.ip = 0;
        self.reset_stack();

        self.run()
    }

    pub fn interpret_chunk(&mut self, chunk: Chunk) -> Result<(), String> {
        self.chunk = chunk;
        self.ip = 0;
        self.reset_stack();

        self.run()
    }

    pub fn run(&mut self) -> Result<(), String> {
        loop {
            if self.trace_execution && self.ip < self.chunk.code.len() {
                self.trace_current_state();
            }

            let instruction = self.read_byte()?;
            match instruction {
                OP_CONSTANT => {
                    let constant = self.read_constant()?;
                    self.push(constant);
                }
                OP_NIL => self.push(Value::Nil),
                OP_TRUE => self.push(Value::Bool(true)),
                OP_FALSE => self.push(Value::Bool(false)),
                OP_EQUAL => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(left == right));
                }
                OP_GREATER => {
                    self.binary_number_op("Operands must be numbers.", |a, b| {
                        if a > b { 1.0 } else { 0.0 }
                    })?;
                }
                OP_LESS => {
                    self.binary_number_op("Operands must be numbers.", |a, b| {
                        if a < b { 1.0 } else { 0.0 }
                    })?;
                }
                OP_AND => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(self.is_truthy(left.clone()) && self.is_truthy(right.clone())));
                }
                OP_OR => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(self.is_truthy(left.clone()) || self.is_truthy(right.clone())));
                }
                OP_ADD => {
                    self.binary_number_op("Operands must be numbers.", |a, b| a + b)?;
                }
                OP_SUBTRACT => {
                    self.binary_number_op("Operands must be numbers.", |a, b| a - b)?;
                }
                OP_MULTIPLY => {
                    self.binary_number_op("Operands must be numbers.", |a, b| a * b)?;
                }
                OP_DIVIDE => {
                    self.binary_number_op("Operands must be numbers.", |a, b| a / b)?;
                }
                OP_NOT => {
                    let value = self.pop()?;
                    self.push(Value::Bool(!self.is_truthy(value)));
                }
                OP_NEGATE => {
                    let value = self.pop()?;
                    match value {
                        Value::Number(number) => self.push(Value::Number(-number)),
                        _ => return Err(self.runtime_error("Operand must be a number.")),
                    }
                }
                OP_PRINT => {
                    let value = self.pop()?;
                    println!("{}", value);
                }
                OP_RETURN => {
                    let value = self.pop()?;
                    println!("{}", value);
                    return Ok(());
                }
                _ => {
                    return Err(format!(
                        "Unknown opcode {} at offset {}",
                        instruction,
                        self.ip.saturating_sub(1)
                    ));
                }
            }
        }
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        let byte =
            self.chunk.code.get(self.ip).copied().ok_or_else(|| {
                format!("Instruction pointer out of bounds at offset {}", self.ip)
            })?;
        self.ip += 1;
        Ok(byte)
    }

    fn read_constant(&mut self) -> Result<Value, String> {
        let constant_index = self.read_byte()?;
        self.chunk
            .constants
            .get(constant_index as usize)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Invalid constant index {} at offset {}",
                    constant_index,
                    self.ip.saturating_sub(1)
                )
            })
    }

    fn reset_stack(&mut self) {
        self.stack.clear();
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, String> {
        self.stack
            .pop()
            .ok_or_else(|| self.runtime_error("Stack underflow."))
    }

    fn binary_number_op<F>(&mut self, error_message: &str, operation: F) -> Result<(), String>
    where
        F: FnOnce(f64, f64) -> f64,
    {
        let right = self.pop()?;
        let left = self.pop()?;

        let (Value::Number(a), Value::Number(b)) = (left, right) else {
            return Err(self.runtime_error(error_message));
        };

        self.push(Value::Number(operation(a, b)));
        Ok(())
    }

    fn runtime_error(&self, message: &str) -> String {
        let line = self.chunk.line_at(self.ip.saturating_sub(1)).unwrap_or(0);
        format!("{}\n[line {}] in script", message, line)
    }

    fn is_truthy(&self, value: Value) -> bool {
        match value {
            Value::Nil => false,
            Value::Bool(value) => value,
            _ => true,
        }
    }

    fn trace_current_state(&self) {
        let stack_dump = self
            .stack
            .iter()
            .map(|value| format!("[ {} ]", value))
            .collect::<Vec<String>>()
            .join("");
        println!("          {}", stack_dump);

        let (line, _) = disassemble_instruction(&self.chunk, self.ip);
        println!("{}", line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_executes_arithmetic_program() {
        let mut chunk = Chunk::new();
        let a = chunk.add_constant(Value::Number(1.2));
        let b = chunk.add_constant(Value::Number(3.4));
        let c = chunk.add_constant(Value::Number(5.6));

        chunk.write(OP_CONSTANT, 1);
        chunk.write(a, 1);
        chunk.write(OP_CONSTANT, 1);
        chunk.write(b, 1);
        chunk.write(OP_ADD, 1);
        chunk.write(OP_CONSTANT, 1);
        chunk.write(c, 1);
        chunk.write(OP_DIVIDE, 1);
        chunk.write(OP_NEGATE, 1);
        chunk.write(OP_RETURN, 1);

        let mut vm = VM::new();
        let result = vm.interpret_chunk(chunk);

        assert!(result.is_ok());
        assert!(vm.stack.is_empty());
    }

    #[test]
    fn run_reports_type_error_for_negate() {
        let mut chunk = Chunk::new();
        let idx = chunk.add_constant(Value::Bool(true));

        chunk.write(OP_CONSTANT, 1);
        chunk.write(idx, 1);
        chunk.write(OP_NEGATE, 1);
        chunk.write(OP_RETURN, 1);

        let mut vm = VM::new();
        let result = vm.interpret_chunk(chunk);

        assert!(result.is_err());
        assert!(
            result
                .expect_err("expected runtime error")
                .contains("Operand must be a number.")
        );
    }

    #[test]
    fn run_with_trace_execution_enabled() {
        let mut chunk = Chunk::new();
        let idx = chunk.add_constant(Value::Number(7.0));
        chunk.write(OP_CONSTANT, 1);
        chunk.write(idx, 1);
        chunk.write(OP_RETURN, 1);

        let mut vm = VM::new();
        vm.set_trace_execution(true);

        let result = vm.interpret_chunk(chunk);
        assert!(result.is_ok());
    }
}
