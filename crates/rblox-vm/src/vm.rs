use crate::chunk::{
    Chunk, OP_ADD, OP_AND, OP_CONSTANT, OP_DEFINE_GLOBAL, OP_DIVIDE, OP_EQUAL, OP_FALSE,
    OP_GET_GLOBAL, OP_GET_LOCAL, OP_GREATER, OP_JUMP, OP_JUMP_IF_FALSE, OP_LESS, OP_MULTIPLY,
    OP_NEGATE, OP_NIL, OP_NOT, OP_OR, OP_POP, OP_PRINT, OP_RETURN, OP_SET_GLOBAL, OP_SET_LOCAL,
    OP_SUBTRACT, OP_TRUE, Object, Value, allocate_string, disassemble_instruction,
};
use crate::compiler::compile;
use crate::table::Table;

pub struct VM {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
    globals: Table,
    trace_execution: bool,
}

impl VM {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Vec::new(),
            globals: Table::new(),
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
                OP_DEFINE_GLOBAL => {
                    let name = self.read_constant_string()?;
                    let value = self.pop()?;
                    self.globals.set(name, value);
                }
                OP_GET_GLOBAL => {
                    let name = self.read_constant_string()?;
                    let value = self
                        .globals
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| self.runtime_error("Undefined variable."))?;
                    self.push(value);
                }
                OP_SET_GLOBAL => {
                    let name = self.read_constant_string()?;
                    let value = self
                        .stack
                        .last()
                        .cloned()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    if self.globals.get(&name).is_none() {
                        return Err(self.runtime_error("Undefined variable."));
                    }
                    self.globals.set(name, value);
                }
                OP_GET_LOCAL => {
                    let slot = self.read_byte()? as usize;
                    let value = self
                        .stack
                        .get(slot)
                        .cloned()
                        .ok_or_else(|| self.runtime_error("Invalid local variable slot."))?;
                    self.push(value);
                }
                OP_SET_LOCAL => {
                    let slot = self.read_byte()? as usize;
                    let value = self
                        .stack
                        .last()
                        .cloned()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    if slot >= self.stack.len() {
                        return Err(self.runtime_error("Invalid local variable slot."));
                    }
                    self.stack[slot] = value;
                }
                OP_EQUAL => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(left == right));
                }
                OP_GREATER => {
                    self.binary_bool_op("Operands must be numbers.", |a, b| a > b)?;
                }
                OP_LESS => {
                    self.binary_bool_op("Operands must be numbers.", |a, b| a < b)?;
                }
                OP_AND => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(
                        self.is_truthy(left.clone()) && self.is_truthy(right.clone()),
                    ));
                }
                OP_OR => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(
                        self.is_truthy(left.clone()) || self.is_truthy(right.clone()),
                    ));
                }
                OP_ADD => {
                    let right = self.pop()?;
                    let left = self.pop()?;

                    match (left, right) {
                        (Value::Number(a), Value::Number(b)) => {
                            self.push(Value::Number(a + b));
                        }
                        (Value::Obj(left_obj), Value::Obj(right_obj)) => {
                            let (Some(left_value), Some(right_value)) =
                                (left_obj.string_value(), right_obj.string_value())
                            else {
                                return Err(
                                    self.runtime_error("Operands must be numbers or strings.")
                                );
                            };
                            self.push(allocate_string(format!("{}{}", left_value, right_value)));
                        }
                        _ => return Err(self.runtime_error("Operands must be numbers or strings.")),
                    }
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
                    let value = self.pop().unwrap_or(Value::Nil);
                    println!("{}", value);
                    return Ok(());
                }
                OP_POP => {
                    self.pop()?;
                }
                OP_JUMP => {
                    let offset = self.read_short()? as usize;
                    self.ip += offset;
                }
                OP_JUMP_IF_FALSE => {
                    let offset = self.read_short()? as usize;
                    let value = self
                        .stack
                        .last()
                        .cloned()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    if !self.is_truthy(value) {
                        self.ip += offset;
                    }
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

    fn read_short(&mut self) -> Result<u16, String> {
        let high = self.read_byte()?;
        let low = self.read_byte()?;
        Ok(u16::from_be_bytes([high, low]))
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

    fn read_constant_string(&mut self) -> Result<std::rc::Rc<Object>, String> {
        let value = self.read_constant()?;
        let Value::Obj(object) = value else {
            return Err(self.runtime_error("Global name must be a string."));
        };
        if object.string_value().is_none() {
            return Err(self.runtime_error("Global name must be a string."));
        }
        Ok(object)
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

    fn binary_bool_op<F>(&mut self, error_message: &str, operation: F) -> Result<(), String>
    where
        F: FnOnce(f64, f64) -> bool,
    {
        let right = self.pop()?;
        let left = self.pop()?;

        let (Value::Number(a), Value::Number(b)) = (left, right) else {
            return Err(self.runtime_error(error_message));
        };

        self.push(Value::Bool(operation(a, b)));
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
    fn binary_bool_op_returns_boolean_for_comparison() {
        let mut vm = VM::new();
        vm.push(Value::Number(2.0));
        vm.push(Value::Number(1.0));

        vm.binary_bool_op("Operands must be numbers.", |a, b| a > b)
            .unwrap();

        assert_eq!(vm.pop().unwrap(), Value::Bool(true));
    }

    #[test]
    fn run_executes_string_addition_program() {
        let mut chunk = Chunk::new();
        let left = chunk.add_constant(allocate_string("hello".to_string()));
        let right = chunk.add_constant(allocate_string(" world".to_string()));

        chunk.write(OP_CONSTANT, 1);
        chunk.write(left, 1);
        chunk.write(OP_CONSTANT, 1);
        chunk.write(right, 1);
        chunk.write(OP_ADD, 1);
        chunk.write(OP_RETURN, 1);

        let mut vm = VM::new();
        let result = vm.interpret_chunk(chunk);

        assert!(result.is_ok());
        assert_eq!(vm.stack.len(), 0);
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

    #[test]
    fn interpret_runs_then_branch_for_truthy_if() {
        let mut vm = VM::new();

        let result = vm.interpret("if (true) { print 42; }");

        assert!(result.is_ok());
    }

    #[test]
    fn interpret_runs_else_branch_for_falsy_if() {
        let mut vm = VM::new();

        let result = vm.interpret("if (false) { print 1; } else { print 2; }");

        assert!(result.is_ok());
    }

    #[test]
    fn interpret_reads_and_assigns_global_variable() {
        let mut vm = VM::new();

        let result = vm.interpret("var a = 1; a = 2;");

        assert!(result.is_ok());
        let name = match allocate_string("a".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&name), Some(&Value::Number(2.0)));
    }
}
