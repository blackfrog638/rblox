use std::time::Instant;

use crate::memory::{Heap, ObjId};

use crate::chunk::{
    Chunk, OP_ADD, OP_AND, OP_CALL, OP_CLASS, OP_CLOSE_UPVALUE, OP_CLOSURE, OP_CONSTANT,
    OP_DEFINE_GLOBAL, OP_DIVIDE, OP_EQUAL, OP_FALSE, OP_GET_GLOBAL, OP_GET_LOCAL, OP_GET_PROPERTY,
    OP_GET_SUPER, OP_GET_UPVALUE, OP_GREATER, OP_INHERIT, OP_INVOKE, OP_JUMP, OP_JUMP_IF_FALSE,
    OP_LESS, OP_LOOP, OP_METHOD, OP_MULTIPLY, OP_NEGATE, OP_NIL, OP_NOT, OP_OR, OP_POP, OP_PRINT,
    OP_RETURN, OP_SET_GLOBAL, OP_SET_LOCAL, OP_SET_PROPERTY, OP_SET_UPVALUE, OP_SUBTRACT,
    OP_SUPER_INVOKE, OP_TRUE, disassemble_instruction,
};
use crate::compiler::compile;
use crate::object::{Closure, NativeFn, ObjFunction, Object, Upvalue};
use crate::table::Table;
use crate::value::Value;

const FRAMES_MAX: usize = 64;

struct CallFrame {
    function: ObjId,
    ip: usize,
    stack_start: usize,
}

impl CallFrame {
    fn function<'a>(&self, heap: &'a Heap) -> Result<&'a ObjFunction, String> {
        let Object::Closure(closure) = heap.get(self.function) else {
            return Err("Call frame does not contain a closure.".to_string());
        };
        match heap.get(closure.function) {
            Object::Function(function) => Ok(function),
            _ => Err("Closure does not contain a function.".to_string()),
        }
    }
}

pub struct VM {
    heap: Heap,
    call_stack: Vec<CallFrame>,
    stack: Vec<Value>,
    open_upvalues: Option<ObjId>,
    globals: Table,
    init_string: ObjId,
    trace_execution: bool,
    start_time: Instant,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        let mut heap = Heap::new();
        let Value::Obj(init_string) = heap.alloc_string("init".to_string()) else {
            unreachable!("alloc_string always returns an object")
        };
        let mut vm = Self {
            heap,
            call_stack: Vec::with_capacity(FRAMES_MAX),
            stack: Vec::new(),
            open_upvalues: None,
            globals: Table::new(),
            init_string,
            trace_execution: false,
            start_time: Instant::now(),
        };
        vm.define_native_function("clock", Self::clock_native);
        vm
    }

    /// Object handles used with this VM must be allocated in this heap.
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }

    pub fn set_trace_execution(&mut self, enabled: bool) {
        self.trace_execution = enabled;
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        let function = match compile(source, &mut self.heap) {
            Ok(function) => function,
            Err(error) => {
                // No builder can still reference the failed compilation's objects.
                self.collect_garbage();
                return Err(error);
            }
        };
        self.interpret_function(function)
    }

    pub fn interpret_chunk(&mut self, chunk: Chunk) -> Result<(), String> {
        let function = ObjFunction {
            chunk,
            ..ObjFunction::new()
        };
        self.interpret_function(function)
    }

    fn interpret_function(&mut self, function: ObjFunction) -> Result<(), String> {
        self.reset_stack();

        let upvalue_count = function.upvalue_count;
        let prototype = self.heap.alloc(Object::Function(function));
        let function = self
            .heap
            .alloc(Object::Closure(Closure::new(prototype, upvalue_count)));
        self.push(Value::Obj(function));
        self.push_call_frame(function, 0)?;

        let result = self.run();
        if result.is_err() {
            self.reset_stack();
        }
        // Also reclaim completed scripts in REPL sessions and on runtime errors.
        self.collect_garbage();
        result
    }

    pub fn run(&mut self) -> Result<(), String> {
        loop {
            // A safe point: the previous instruction has published its results
            // to the VM stack/objects. Rust-local builders are not GC roots.
            if self.heap.should_collect() {
                self.collect_garbage();
            }
            if self.trace_execution && self.current_ip()? < self.current_chunk()?.code.len() {
                self.trace_current_state();
            }

            let instruction = self.read_byte()?;
            match instruction {
                OP_CONSTANT => {
                    let constant = self.read_constant()?;
                    self.push(constant);
                }
                OP_CLOSURE => {
                    let constant = self.read_constant()?;
                    let Value::Obj(function) = constant else {
                        return Err(self.runtime_error("Closure constant must be a function."));
                    };
                    let Object::Function(prototype) = self.heap.get(function) else {
                        return Err(self.runtime_error("Closure constant must be a function."));
                    };
                    let upvalue_count = prototype.upvalue_count;
                    let mut closure = Closure::new(function, upvalue_count);
                    for _ in 0..upvalue_count {
                        let is_local = self.read_byte()?;
                        let index = self.read_byte()? as usize;
                        let upvalue = match is_local {
                            1 => {
                                let location = self.current_frame()?.stack_start + index;
                                self.capture_upvalue(location)
                            }
                            0 => self.current_upvalue(index)?,
                            _ => return Err(self.runtime_error("Invalid upvalue capture flag.")),
                        };
                        closure.upvalues.push(upvalue);
                    }
                    let closure = self.heap.alloc(Object::Closure(closure));
                    self.push(Value::Obj(closure));
                }
                OP_CLASS => {
                    let name = self.read_constant_string()?;
                    let class = self.heap.alloc(Object::Class {
                        name: self.heap.get(name).string_value().unwrap().to_string(),
                        methods: Table::new(),
                    });
                    self.push(Value::Obj(class));
                }
                OP_METHOD => {
                    let name = self.read_constant_string()?;
                    let method = self.pop()?;
                    let Some(Value::Obj(class)) = self.stack.last().copied() else {
                        return Err(self.runtime_error("Method declaration requires a class."));
                    };
                    self.heap
                        .set_class_method(class, name, method)
                        .map_err(|error| self.runtime_error(error))?;
                }
                OP_GET_UPVALUE => {
                    let index = self.read_byte()? as usize;
                    let id = self.current_upvalue(index)?;
                    let upvalue = self.upvalue(id);
                    let value = match upvalue.closed {
                        Some(value) => value,
                        None => self
                            .stack
                            .get(upvalue.location)
                            .copied()
                            .ok_or_else(|| self.runtime_error("Invalid upvalue stack slot."))?,
                    };
                    self.push(value);
                }
                OP_SET_UPVALUE => {
                    let index = self.read_byte()? as usize;
                    let id = self.current_upvalue(index)?;
                    let value = self
                        .stack
                        .last()
                        .copied()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    let upvalue = self.upvalue(id);
                    if upvalue.closed.is_some() {
                        self.upvalue_mut(id).closed = Some(value);
                    } else {
                        let location = upvalue.location;
                        if location >= self.stack.len() {
                            return Err(self.runtime_error("Invalid upvalue stack slot."));
                        }
                        self.stack[location] = value;
                    }
                }
                OP_INHERIT => {
                    let subclass = self.pop()?;
                    let superclass = self
                        .stack
                        .last()
                        .copied()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    let Value::Obj(superclass) = superclass else {
                        return Err(self.runtime_error("Superclass must be a class."));
                    };
                    let Value::Obj(subclass) = subclass else {
                        return Err(self.runtime_error("Subclass must be a class."));
                    };
                    self.heap
                        .inherit_methods(superclass, subclass)
                        .map_err(|error| self.runtime_error(error))?;
                }
                OP_NIL => self.push(Value::Nil),
                OP_TRUE => self.push(Value::Bool(true)),
                OP_FALSE => self.push(Value::Bool(false)),
                OP_DEFINE_GLOBAL => {
                    let name = self.read_constant_string()?;
                    let value = self.pop()?;
                    self.globals.set(name, value, &self.heap);
                }
                OP_GET_GLOBAL => {
                    let name = self.read_constant_string()?;
                    let value = self
                        .globals
                        .get(&name, &self.heap)
                        .copied()
                        .ok_or_else(|| self.runtime_error("Undefined variable."))?;
                    self.push(value);
                }
                OP_GET_PROPERTY => {
                    let name = self.read_constant_string()?;
                    let receiver = self.pop()?;
                    let Some(Object::Instance { class, fields }) = receiver.as_obj(&self.heap)
                    else {
                        return Err(self.runtime_error("Only instances have properties."));
                    };
                    let value = match fields.get(&name, &self.heap).copied() {
                        Some(value) => value,
                        None => self.bind_method(*class, name, receiver)?,
                    };
                    self.push(value);
                }
                OP_GET_SUPER => {
                    let name = self.read_constant_string()?;
                    let Value::Obj(superclass) = self.pop()? else {
                        return Err(self.runtime_error("Superclass must be a class."));
                    };
                    let receiver = self.pop()?;
                    let method = self.bind_method(superclass, name, receiver)?;
                    self.push(method);
                }
                OP_SET_PROPERTY => {
                    let name = self.read_constant_string()?;
                    let value = self.pop()?;
                    let receiver = self.pop()?;
                    let Value::Obj(instance) = receiver else {
                        return Err(self.runtime_error("Only instances have fields."));
                    };
                    self.heap
                        .set_instance_field(instance, name, value)
                        .map_err(|error| self.runtime_error(error))?;
                    // Assignment is an expression: leave its value, not the receiver.
                    self.push(value);
                }
                OP_SET_GLOBAL => {
                    let name = self.read_constant_string()?;
                    let value = self
                        .stack
                        .last()
                        .copied()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    if self.globals.get(&name, &self.heap).is_none() {
                        return Err(self.runtime_error("Undefined variable."));
                    }
                    self.globals.set(name, value, &self.heap);
                }
                OP_GET_LOCAL => {
                    let slot = self.read_byte()? as usize;
                    let slot = self.current_frame()?.stack_start + slot;
                    let value = self
                        .stack
                        .get(slot)
                        .copied()
                        .ok_or_else(|| self.runtime_error("Invalid local variable slot."))?;
                    self.push(value);
                }
                OP_SET_LOCAL => {
                    let slot = self.read_byte()? as usize;
                    let slot = self.current_frame()?.stack_start + slot;
                    let value = self
                        .stack
                        .last()
                        .copied()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    if slot >= self.stack.len() {
                        return Err(self.runtime_error("Invalid local variable slot."));
                    }
                    self.stack[slot] = value;
                }
                OP_EQUAL => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(left.equals(&right, &self.heap)));
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
                    self.push(Value::Bool(self.is_truthy(left) && self.is_truthy(right)));
                }
                OP_OR => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(Value::Bool(self.is_truthy(left) || self.is_truthy(right)));
                }
                OP_ADD => {
                    let right = self.pop()?;
                    let left = self.pop()?;

                    match (left, right) {
                        (Value::Number(a), Value::Number(b)) => {
                            self.push(Value::Number(a + b));
                        }
                        (Value::Obj(left_obj), Value::Obj(right_obj)) => {
                            let (Some(left_value), Some(right_value)) = (
                                self.heap.get(left_obj).string_value(),
                                self.heap.get(right_obj).string_value(),
                            ) else {
                                return Err(
                                    self.runtime_error("Operands must be numbers or strings.")
                                );
                            };
                            let text = format!("{}{}", left_value, right_value);
                            let value = self.heap.alloc_string(text);
                            self.push(value);
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
                    println!("{}", value.display(&self.heap));
                }
                OP_RETURN => {
                    let result = self.pop()?;
                    let stack_start = self.current_frame()?.stack_start;
                    self.close_upvalues(stack_start)?;
                    let frame = self
                        .call_stack
                        .pop()
                        .ok_or_else(|| self.runtime_error("No active call frame."))?;
                    self.stack.truncate(frame.stack_start);

                    if self.call_stack.is_empty() {
                        return Ok(());
                    }
                    self.push(result);
                }
                OP_POP => {
                    self.pop()?;
                }
                OP_JUMP => {
                    let offset = self.read_short()? as usize;
                    self.current_frame_mut()?.ip += offset;
                }
                OP_JUMP_IF_FALSE => {
                    let offset = self.read_short()? as usize;
                    let value = self
                        .stack
                        .last()
                        .copied()
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    if !self.is_truthy(value) {
                        self.current_frame_mut()?.ip += offset;
                    }
                }
                OP_LOOP => {
                    let offset = self.read_short()? as usize;
                    if offset > self.current_ip()? {
                        return Err(self.runtime_error("Invalid loop offset."));
                    }
                    self.current_frame_mut()?.ip -= offset;
                }
                OP_CALL => {
                    let arg_count = self.read_byte()? as usize;
                    self.call_value(arg_count)?;
                }
                OP_INVOKE => {
                    let name = self.read_constant_string()?;
                    let arg_count = self.read_byte()? as usize;
                    self.invoke(name, arg_count)?;
                }
                OP_SUPER_INVOKE => {
                    let name = self.read_constant_string()?;
                    let arg_count = self.read_byte()? as usize;
                    let Value::Obj(superclass) = self.pop()? else {
                        return Err(self.runtime_error("Superclass must be a class."));
                    };
                    // Popping the lookup class leaves [receiver, args...] ready for the frame.
                    self.invoke_from_class(superclass, name, arg_count)?;
                }
                OP_CLOSE_UPVALUE => {
                    let last = self
                        .stack
                        .len()
                        .checked_sub(1)
                        .ok_or_else(|| self.runtime_error("Stack underflow."))?;
                    self.close_upvalues(last)?;
                    self.pop()?;
                }
                _ => {
                    return Err(format!(
                        "Unknown opcode {} at offset {}",
                        instruction,
                        self.current_ip()?.saturating_sub(1)
                    ));
                }
            }
        }
    }

    fn invoke(&mut self, name: ObjId, arg_count: usize) -> Result<(), String> {
        let receiver_index = self
            .stack
            .len()
            .checked_sub(arg_count + 1)
            .ok_or_else(|| self.runtime_error("Stack underflow."))?;
        let receiver = self.stack[receiver_index];
        let Some(Object::Instance { class, fields }) = receiver.as_obj(&self.heap) else {
            return Err(self.runtime_error("Only instances have methods."));
        };
        let class = *class;
        if let Some(value) = fields.get(&name, &self.heap).copied() {
            self.stack[receiver_index] = value;
            return self.call_value(arg_count);
        }
        self.invoke_from_class(class, name, arg_count)
    }

    fn invoke_from_class(
        &mut self,
        class: ObjId,
        name: ObjId,
        arg_count: usize,
    ) -> Result<(), String> {
        let method = self.find_method(class, name)?;
        self.call_function(method, arg_count)
    }

    fn bind_method(&mut self, class: ObjId, name: ObjId, receiver: Value) -> Result<Value, String> {
        let method = self.find_method(class, name)?;
        // Allocation does not collect; the caller roots the result on the VM stack
        // before the next instruction's GC safe point.
        Ok(Value::Obj(
            self.heap.alloc(Object::BoundMethod { receiver, method }),
        ))
    }

    fn find_method(&self, class: ObjId, name: ObjId) -> Result<ObjId, String> {
        let Object::Class { methods, .. } = self.heap.get(class) else {
            return Err(self.runtime_error("Instance does not refer to a class."));
        };
        let Some(Value::Obj(method)) = methods.get(&name, &self.heap).copied() else {
            return Err(self.runtime_error(&format!(
                "Undefined property '{}'.",
                self.heap
                    .get(name)
                    .string_value()
                    .expect("property name is a string")
            )));
        };
        Ok(method)
    }

    fn upvalue(&self, id: ObjId) -> &Upvalue {
        let Object::Upvalue(upvalue) = self.heap.get(id) else {
            unreachable!("upvalue handles must point to upvalues")
        };
        upvalue
    }

    fn upvalue_mut(&mut self, id: ObjId) -> &mut Upvalue {
        let Object::Upvalue(upvalue) = self.heap.get_mut(id) else {
            unreachable!("upvalue handles must point to upvalues")
        };
        upvalue
    }

    fn close_upvalues(&mut self, last: usize) -> Result<(), String> {
        while let Some(id) = self.open_upvalues {
            let upvalue = self.upvalue(id);
            if upvalue.location < last {
                break;
            }
            let value = self
                .stack
                .get(upvalue.location)
                .copied()
                .ok_or_else(|| self.runtime_error("Invalid upvalue stack slot."))?;
            let upvalue = self.upvalue_mut(id);
            upvalue.closed = Some(value);
            // Closed upvalues no longer belong to the open list.
            self.open_upvalues = upvalue.next.take();
        }
        Ok(())
    }

    fn capture_upvalue(&mut self, location: usize) -> ObjId {
        let mut previous = None;
        let mut current = self.open_upvalues;
        while let Some(id) = current {
            let upvalue = self.upvalue(id);
            if upvalue.location <= location {
                break;
            }
            previous = Some(id);
            current = upvalue.next;
        }
        if let Some(id) = current
            && self.upvalue(id).location == location
        {
            return id;
        }
        let created = self.heap.alloc(Object::Upvalue(Upvalue {
            location,
            closed: None,
            next: current,
        }));
        if let Some(previous) = previous {
            self.upvalue_mut(previous).next = Some(created);
        } else {
            self.open_upvalues = Some(created);
        }
        created
    }

    fn current_upvalue(&self, index: usize) -> Result<ObjId, String> {
        let Object::Closure(closure) = self.heap.get(self.current_frame()?.function) else {
            return Err(self.runtime_error("Call frame does not contain a closure."));
        };
        closure
            .upvalues
            .get(index)
            .copied()
            .ok_or_else(|| self.runtime_error("Invalid upvalue index."))
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        let ip = self.current_ip()?;
        let byte = self
            .current_chunk()?
            .code
            .get(ip)
            .copied()
            .ok_or_else(|| format!("Instruction pointer out of bounds at offset {}", ip))?;
        self.current_frame_mut()?.ip += 1;
        Ok(byte)
    }

    fn read_short(&mut self) -> Result<u16, String> {
        let high = self.read_byte()?;
        let low = self.read_byte()?;
        Ok(u16::from_be_bytes([high, low]))
    }

    fn read_constant(&mut self) -> Result<Value, String> {
        let constant_index = self.read_byte()?;
        self.current_chunk()?
            .constants
            .get(constant_index as usize)
            .copied()
            .ok_or_else(|| {
                format!(
                    "Invalid constant index {} at offset {}",
                    constant_index,
                    self.current_ip().unwrap_or(0).saturating_sub(1)
                )
            })
    }

    fn read_constant_string(&mut self) -> Result<ObjId, String> {
        let value = self.read_constant()?;
        let Value::Obj(object) = value else {
            return Err(self.runtime_error("Global name must be a string."));
        };
        if self.heap.get(object).string_value().is_none() {
            return Err(self.runtime_error("Global name must be a string."));
        }
        Ok(object)
    }

    fn reset_stack(&mut self) {
        // Globals may retain closures even after a runtime error.
        // Preserve their captured variables before discarding the stack.
        let _ = self.close_upvalues(0);
        self.stack.clear();
        self.call_stack.clear();
        self.open_upvalues = None;
    }

    fn current_frame(&self) -> Result<&CallFrame, String> {
        self.call_stack
            .last()
            .ok_or_else(|| "No active call frame.".to_string())
    }

    fn current_frame_mut(&mut self) -> Result<&mut CallFrame, String> {
        self.call_stack
            .last_mut()
            .ok_or_else(|| "No active call frame.".to_string())
    }

    fn current_ip(&self) -> Result<usize, String> {
        Ok(self.current_frame()?.ip)
    }

    fn current_chunk(&self) -> Result<&Chunk, String> {
        Ok(&self.current_frame()?.function(&self.heap)?.chunk)
    }

    fn push_call_frame(&mut self, function: ObjId, stack_start: usize) -> Result<(), String> {
        if self.call_stack.len() >= FRAMES_MAX {
            return Err(self.runtime_error("Stack overflow."));
        }
        self.call_stack.push(CallFrame {
            function,
            ip: 0,
            stack_start,
        });
        Ok(())
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, String> {
        self.stack
            .pop()
            .ok_or_else(|| self.runtime_error("Stack underflow."))
    }

    fn clock_native(vm: &mut Self, args: &[Value]) -> Result<Value, String> {
        if !args.is_empty() {
            return Err(format!("Expected 0 arguments but got {}.", args.len()));
        }

        Ok(Value::Number(vm.start_time.elapsed().as_secs_f64()))
    }

    fn call_value(&mut self, arg_count: usize) -> Result<(), String> {
        let callee_index = self
            .stack
            .len()
            .checked_sub(arg_count + 1)
            .ok_or_else(|| self.runtime_error("Stack underflow."))?;
        let Value::Obj(callee) = self.stack[callee_index] else {
            return Err(self.runtime_error("Can only call functions."));
        };
        match self.heap.get(callee) {
            Object::Closure(_) => self.call_function(callee, arg_count),
            Object::BoundMethod { receiver, method } => {
                let (receiver, method) = (*receiver, *method);
                self.stack[callee_index] = receiver;
                self.call_function(method, arg_count)
            }
            Object::NativeFunction(function) => {
                self.call_native(*function, callee_index, arg_count)
            }
            Object::Function(_) | Object::String { .. } | Object::Upvalue(_) => {
                Err(self.runtime_error("Can only call functions."))
            }
            Object::Class { methods, .. } => {
                let initializer = methods.get(&self.init_string, &self.heap).copied();
                if initializer.is_none() && arg_count != 0 {
                    return Err(
                        self.runtime_error(&format!("Expected 0 arguments but got {}.", arg_count))
                    );
                }
                let instance = self.heap.alloc(Object::Instance {
                    class: callee,
                    fields: Table::new(),
                });
                // Reuse the callee slot as `this`, leaving arguments in place.
                self.stack[callee_index] = Value::Obj(instance);
                if let Some(Value::Obj(initializer)) = initializer {
                    self.call_function(initializer, arg_count)?;
                }
                Ok(())
            }
            Object::Instance { .. } => {
                Err(self.runtime_error("Can only call functions and classes."))
            }
        }
    }

    fn call_native(
        &mut self,
        function: NativeFn,
        callee_index: usize,
        arg_count: usize,
    ) -> Result<(), String> {
        let args = self
            .stack
            .get(callee_index + 1..)
            .ok_or_else(|| self.runtime_error("Stack underflow."))?
            .to_vec();
        if args.len() != arg_count {
            return Err(self.runtime_error("Stack underflow."));
        }

        let result = function(self, &args).map_err(|error| self.runtime_error(&error))?;
        self.stack.truncate(callee_index);
        self.push(result);
        Ok(())
    }

    fn call_function(&mut self, function: ObjId, arg_count: usize) -> Result<(), String> {
        let arity = match self.heap.get(function) {
            Object::Closure(closure) => match self.heap.get(closure.function) {
                Object::Function(prototype) => prototype.arity,
                _ => panic!("closure prototype"),
            },
            Object::Function(_) | Object::String { .. } => {
                return Err(self.runtime_error("Can only call functions."));
            }
            Object::NativeFunction(_) => {
                return Err(self.runtime_error("Can only call functions."));
            }
            Object::Upvalue(_) => {
                return Err(self.runtime_error("Can only call functions."));
            }
            Object::Class { name, .. } => {
                return Err(self.runtime_error(&format!("Cannot call class {}.", name)));
            }
            Object::Instance { .. } | Object::BoundMethod { .. } => {
                return Err(self.runtime_error("Can only call functions and classes."));
            }
        };

        if arg_count != arity {
            return Err(self.runtime_error(&format!(
                "Expected {} arguments but got {}.",
                arity, arg_count
            )));
        }

        let stack_start = self
            .stack
            .len()
            .checked_sub(arg_count + 1)
            .ok_or_else(|| self.runtime_error("Stack underflow."))?;
        self.push_call_frame(function, stack_start)
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
        let line = self
            .current_ip()
            .ok()
            .and_then(|ip| self.current_chunk().ok()?.line_at(ip.saturating_sub(1)))
            .unwrap_or(0);
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
            .map(|value| format!("[ {} ]", value.display(&self.heap)))
            .collect::<Vec<String>>()
            .join("");
        println!("          {}", stack_dump);

        if let (Ok(chunk), Ok(ip)) = (self.current_chunk(), self.current_ip()) {
            let (line, _) = disassemble_instruction(chunk, ip, &self.heap);
            println!("{}", line);
        }
    }

    fn define_native_function(&mut self, name: &str, function: NativeFn) {
        let Value::Obj(name) = self.heap.alloc_string(name.to_string()) else {
            unreachable!("alloc_string always returns an object")
        };
        let function = self.heap.alloc(Object::NativeFunction(function));
        self.globals.set(name, Value::Obj(function), &self.heap);
    }

    pub fn collect_garbage(&mut self) {
        crate::gc_log!(
            "begin: {} objects, {} managed bytes",
            self.heap.len(),
            self.heap.bytes_allocated()
        );
        self.mark_roots();
        self.heap.trace_references();
        self.heap.finish_collection();
        crate::gc_log!(
            "end: {} objects, {} managed bytes",
            self.heap.len(),
            self.heap.bytes_allocated()
        );
    }

    fn mark_roots(&mut self) {
        let mut roots = self.stack.clone();
        roots.extend(
            self.call_stack
                .iter()
                .map(|frame| Value::Obj(frame.function)),
        );

        let mut open_upvalue = self.open_upvalues;
        while let Some(id) = open_upvalue {
            roots.push(Value::Obj(id));
            open_upvalue = self.upvalue(id).next;
        }

        roots.extend(self.globals.gc_roots());
        // The intern table is weak; this cached method name must remain live.
        roots.push(Value::Obj(self.init_string));
        self.heap.mark_roots(roots);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_bound_method_keeps_receiver_and_closed_captures_alive() {
        let mut vm = VM::new();
        let baseline = vm.heap.len();
        vm.interpret(
            r#"
            var saved;
            {
                var text = "cap" + "tured";
                class Local { read() { return text; } }
                var instance = Local();
                instance.payload = "re" + "ceiver";
                saved = instance.read;
            }
        "#,
        )
        .unwrap();
        vm.collect_garbage();
        vm.interpret("if (saved() != \"captured\") nil();").unwrap();

        let Value::Obj(key) = vm.heap.alloc_string("saved".into()) else {
            unreachable!()
        };
        let bound = *vm.globals.get(&key, &vm.heap).unwrap();
        let Some(Object::BoundMethod { receiver, method }) = bound.as_obj(&vm.heap) else {
            unreachable!()
        };
        let receiver = *receiver;
        assert!(matches!(vm.heap.get(*method), Object::Closure(_)));
        let Value::Obj(payload) = vm.heap.alloc_string("payload".into()) else {
            unreachable!()
        };
        let Some(Object::Instance { fields, .. }) = receiver.as_obj(&vm.heap) else {
            unreachable!()
        };
        assert_eq!(
            fields
                .get(&payload, &vm.heap)
                .unwrap()
                .as_string(&vm.heap)
                .unwrap(),
            "receiver"
        );

        vm.interpret("saved = nil;").unwrap();
        vm.collect_garbage();
        assert_eq!(vm.heap.len(), baseline + 1);
    }

    #[test]
    fn method_tables_preserve_closures_captures_and_local_class_cycles() {
        let mut vm = VM::new();
        let baseline = vm.heap.len();
        vm.interpret(
            r#"
            var saved;
            var extracted;
            {
                var text = "ke" + "pt";
                class Local {
                    read(a, b) { nil(); }
                    read() { return text; }
                    own() { return Local; }
                }
                saved = Local;
                var after = 123;
                if (after != 123) nil();
            }
        "#,
        )
        .unwrap();
        vm.collect_garbage();
        let Value::Obj(saved_key) = vm.heap.alloc_string("saved".into()) else {
            unreachable!()
        };
        let Value::Obj(class) = *vm.globals.get(&saved_key, &vm.heap).unwrap() else {
            unreachable!()
        };
        let Object::Class { methods, .. } = vm.heap.get(class) else {
            unreachable!()
        };
        assert_eq!(methods.len(), 2);

        // Inspect the stored closures directly to isolate declaration behavior
        // from method lookup and receiver binding.
        for (name, assertion) in [
            ("read", "if (extracted() != \"kept\") nil();"),
            ("own", "if (extracted() != saved) nil();"),
        ] {
            let Value::Obj(key) = vm.heap.alloc_string(name.into()) else {
                unreachable!()
            };
            let Object::Class { methods, .. } = vm.heap.get(class) else {
                unreachable!()
            };
            let method = *methods.get(&key, &vm.heap).unwrap();
            assert_eq!(method.as_function(&vm.heap).unwrap().arity, 0);
            let Value::Obj(extracted) = vm.heap.alloc_string("extracted".into()) else {
                unreachable!()
            };
            vm.globals.set(extracted, method, &vm.heap);
            vm.interpret(assertion).unwrap();
        }
        vm.interpret("saved = nil; extracted = nil;").unwrap();
        vm.collect_garbage();
        assert_eq!(vm.heap.len(), baseline + 2);
    }

    #[test]
    fn automatic_collection_runs_inside_a_long_running_script() {
        fn heap_size(vm: &mut VM, _: &[Value]) -> Result<Value, String> {
            Ok(Value::Number(vm.heap.len() as f64))
        }
        let mut vm = VM::new();
        vm.define_native_function("heapSize", heap_size);
        vm.interpret(
            r#"
            var text = "";
            for (var i = 0; i < 2500; i = i + 1) text = text + "x";
            if (heapSize() > 1500) nil();
        "#,
        )
        .unwrap();
    }

    #[test]
    fn marking_roots_covers_vm_state_and_live_global_entries() {
        let mut vm = VM::new();
        let Value::Obj(stack_root) = vm.heap.alloc_string("stack".to_string()) else {
            unreachable!()
        };
        let frame_root = vm.heap.alloc(Object::NativeFunction(VM::clock_native));
        let upvalue_tail = vm.heap.alloc(Object::Upvalue(Upvalue {
            location: 0,
            closed: None,
            next: None,
        }));
        let upvalue_head = vm.heap.alloc(Object::Upvalue(Upvalue {
            location: 1,
            closed: None,
            next: Some(upvalue_tail),
        }));
        let Value::Obj(global_key) = vm.heap.alloc_string("global".to_string()) else {
            unreachable!()
        };
        let Value::Obj(global_value) = vm.heap.alloc_string("value".to_string()) else {
            unreachable!()
        };
        let Value::Obj(orphan) = vm.heap.alloc_string("orphan".to_string()) else {
            unreachable!()
        };

        vm.stack.push(Value::Obj(stack_root));
        vm.call_stack.push(CallFrame {
            function: frame_root,
            ip: 0,
            stack_start: 0,
        });
        vm.open_upvalues = Some(upvalue_head);
        vm.globals
            .set(global_key, Value::Obj(global_value), &vm.heap);

        vm.mark_roots();

        for root in [
            stack_root,
            frame_root,
            upvalue_head,
            upvalue_tail,
            global_key,
            global_value,
        ] {
            assert!(vm.heap.is_marked(root), "expected {root:?} to be marked");
        }
        assert!(!vm.heap.is_marked(orphan));
    }

    #[test]
    fn run_executes_arithmetic_program() {
        let mut vm = VM::new();
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

        let result = vm.interpret_chunk(chunk);

        assert!(result.is_ok());
        assert!(vm.stack.is_empty());
        assert!(vm.call_stack.is_empty());
    }

    #[test]
    fn run_reports_type_error_for_negate() {
        let mut vm = VM::new();
        let mut chunk = Chunk::new();
        let idx = chunk.add_constant(Value::Bool(true));

        chunk.write(OP_CONSTANT, 1);
        chunk.write(idx, 1);
        chunk.write(OP_NEGATE, 1);
        chunk.write(OP_RETURN, 1);

        let result = vm.interpret_chunk(chunk);

        assert!(result.is_err());
        assert!(
            result
                .expect_err("expected runtime error")
                .contains("Operand must be a number.")
        );
        assert!(vm.call_stack.is_empty());
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
        let mut vm = VM::new();
        let mut chunk = Chunk::new();
        let left = chunk.add_constant(vm.heap.alloc_string("hello".to_string()));
        let right = chunk.add_constant(vm.heap.alloc_string(" world".to_string()));

        chunk.write(OP_CONSTANT, 1);
        chunk.write(left, 1);
        chunk.write(OP_CONSTANT, 1);
        chunk.write(right, 1);
        chunk.write(OP_ADD, 1);
        chunk.write(OP_RETURN, 1);

        let result = vm.interpret_chunk(chunk);

        assert!(result.is_ok());
        assert_eq!(vm.stack.len(), 0);
    }

    #[test]
    fn run_with_trace_execution_enabled() {
        let mut vm = VM::new();
        let mut chunk = Chunk::new();
        let idx = chunk.add_constant(Value::Number(7.0));
        chunk.write(OP_CONSTANT, 1);
        chunk.write(idx, 1);
        chunk.write(OP_RETURN, 1);

        vm.set_trace_execution(true);

        let result = vm.interpret_chunk(chunk);
        assert!(result.is_ok());
    }

    #[test]
    fn local_slots_are_relative_to_the_current_call_frame() {
        let mut vm = VM::new();
        let mut chunk = Chunk::new();
        let name = chunk.add_constant(vm.heap.alloc_string("captured".to_string()));
        chunk.write(OP_GET_LOCAL, 1);
        chunk.write(1, 1);
        chunk.write(OP_DEFINE_GLOBAL, 1);
        chunk.write(name, 1);
        chunk.write(OP_NIL, 1);
        chunk.write(OP_RETURN, 1);

        let prototype = vm.heap.alloc(Object::Function(ObjFunction {
            chunk,
            ..ObjFunction::new()
        }));
        let function = vm.heap.alloc(Object::Closure(Closure::new(prototype, 0)));
        let function_value = Value::Obj(function);
        vm.call_stack.push(CallFrame {
            function,
            ip: 0,
            stack_start: 1,
        });
        vm.stack = vec![Value::Bool(false), function_value, Value::Number(42.0)];

        vm.run().expect("frame bytecode should run");

        let name = match vm.heap.alloc_string("captured".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&name, &vm.heap), Some(&Value::Number(42.0)));
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
    fn interpret_executes_while_loop_until_condition_is_false() {
        let mut vm = VM::new();

        let result = vm.interpret("var i = 0; while (i < 3) { i = i + 1; }");

        assert!(result.is_ok());
        let name = match vm.heap.alloc_string("i".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&name, &vm.heap), Some(&Value::Number(3.0)));
    }

    #[test]
    fn interpret_executes_continue_in_while_loop() {
        let mut vm = VM::new();

        let result = vm.interpret(
            "var total = 0; var i = 0; while (i < 3) { i = i + 1; continue; total = total + 10; }",
        );

        assert!(result.is_ok());
        let total = match vm.heap.alloc_string("total".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        let i = match vm.heap.alloc_string("i".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&total, &vm.heap), Some(&Value::Number(0.0)));
        assert_eq!(vm.globals.get(&i, &vm.heap), Some(&Value::Number(3.0)));
    }

    #[test]
    fn interpret_executes_break_in_while_loop() {
        let mut vm = VM::new();

        let result = vm.interpret(
            "var total = 0; var i = 0; while (i < 3) { i = i + 1; if (i == 2) break; total = total + 1; }",
        );

        assert!(result.is_ok());
        let total = match vm.heap.alloc_string("total".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        let i = match vm.heap.alloc_string("i".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&total, &vm.heap), Some(&Value::Number(1.0)));
        assert_eq!(vm.globals.get(&i, &vm.heap), Some(&Value::Number(2.0)));
    }

    #[test]
    fn interpret_break_cleans_up_block_locals() {
        let mut vm = VM::new();

        let result = vm.interpret("while (true) { var local = 1; break; } print 2;");

        assert!(result.is_ok());
    }

    #[test]
    fn interpret_executes_for_loop_until_condition_is_false() {
        let mut vm = VM::new();

        let result =
            vm.interpret("var total = 0; for (var i = 0; i < 3; i = i + 1) { total = total + 1; }");

        assert!(result.is_ok());

        let total = match vm.heap.alloc_string("total".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&total, &vm.heap), Some(&Value::Number(3.0)));
    }

    #[test]
    fn interpret_evaluates_and_and_or_short_circuit() {
        let mut vm = VM::new();

        let result = vm.interpret(
            "var a = false and (1 / 0); var b = true or (1 / 0); var c = true and false; var d = false or true;",
        );

        assert!(result.is_ok());

        let a = vm.heap.alloc_string("a".to_string());
        let b = vm.heap.alloc_string("b".to_string());
        let c = vm.heap.alloc_string("c".to_string());
        let d = vm.heap.alloc_string("d".to_string());

        let a_value = match a {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        let b_value = match b {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        let c_value = match c {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        let d_value = match d {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };

        assert_eq!(
            vm.globals.get(&a_value, &vm.heap),
            Some(&Value::Bool(false))
        );
        assert_eq!(vm.globals.get(&b_value, &vm.heap), Some(&Value::Bool(true)));
        assert_eq!(
            vm.globals.get(&c_value, &vm.heap),
            Some(&Value::Bool(false))
        );
        assert_eq!(vm.globals.get(&d_value, &vm.heap), Some(&Value::Bool(true)));
    }

    #[test]
    fn interpret_reads_and_assigns_global_variable() {
        let mut vm = VM::new();

        let result = vm.interpret("var a = 1; a = 2;");

        assert!(result.is_ok());
        let name = match vm.heap.alloc_string("a".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        assert_eq!(vm.globals.get(&name, &vm.heap), Some(&Value::Number(2.0)));
    }

    #[test]
    fn interpret_defines_function_global() {
        let mut vm = VM::new();

        vm.interpret("fun breakfast() { print \"beignets\"; }")
            .expect("function declaration should run");

        let name = match vm.heap.alloc_string("breakfast".to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        };
        let function = vm
            .globals
            .get(&name, &vm.heap)
            .and_then(|value| value.as_function(&vm.heap))
            .expect("global should contain the declared function");
        assert_eq!(function.name.as_deref(), Some("breakfast"));
        assert_eq!(function.arity, 0);
    }
}
